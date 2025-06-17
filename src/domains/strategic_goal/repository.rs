use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite, QueryBuilder};
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalRow, UpdateStrategicGoal, UserGoalRole, GoalValueSummary, StrategicGoalFilter,
};
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use crate::validation::Validate;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use chrono::DateTime;
use std::sync::Arc;
use serde_json;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;
use std::str::FromStr;
use crate::domains::user::repository::MergeableEntityRepository;

/// Trait defining strategic goal repository operations
#[async_trait]
pub trait StrategicGoalRepository:
    DeleteServiceRepository<StrategicGoal> + MergeableEntityRepository<StrategicGoal> + Send + Sync
{
    async fn create(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal>;

    /// Create strategic goal within an existing transaction
    /// Used for operations that need to manage document linking
    async fn create_with_tx<'t>(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal>;

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;
    
    async fn find_by_objective_code(&self, code: &str) -> DomainResult<Option<StrategicGoal>>;

    /// Find strategic goals by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    /// Find strategic goals by status ID
    async fn find_by_status_id(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find strategic goals by responsible team
    async fn find_by_responsible_team(
        &self,
        team_name: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find goal IDs where a user has a specific role
    async fn find_ids_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole, // Assuming UserGoalRole is in scope
    ) -> DomainResult<Vec<Uuid>>;

    /// Count goals by status_id, grouped by status
    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>>;

    /// Count goals grouped by responsible team
    async fn count_by_responsible_team(&self) -> DomainResult<Vec<(Option<String>, i64)>>;

    /// Get aggregate value statistics for strategic goals
    async fn get_value_summary(&self) -> DomainResult<GoalValueSummary>; // Assuming GoalValueSummary is in scope

    /// Count goals that haven't been updated since a specific date
    async fn count_stale(&self, cutoff_date: DateTime<Utc>) -> DomainResult<i64>;

    /// Find goals that haven't been updated since a specific date (paginated)
    async fn find_stale(
        &self,
        cutoff_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find strategic goals by status ID
    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find goals created or updated by a specific user
    async fn find_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find goals that haven't been updated since a specific date
    async fn find_stale_since(
        &self,
        cutoff_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find strategic goals within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;

    /// Find strategic goal IDs that match complex filter criteria
    /// Supports AND/OR logic: (status1 OR status2) AND (year1 OR year2) AND (month1 OR month2)
    async fn find_ids_by_filter(
        &self,
        filter: StrategicGoalFilter,
    ) -> DomainResult<Vec<Uuid>>;
}

/// SQLite implementation for StrategicGoalRepository
#[derive(Clone)]
pub struct SqliteStrategicGoalRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteStrategicGoalRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: StrategicGoalRow) -> DomainResult<StrategicGoal> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "strategic_goals"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        let row = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx) // Use &mut **tx for borrowing
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("StrategicGoal".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<StrategicGoal> for SqliteStrategicGoalRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<StrategicGoal> {
        let row = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("StrategicGoal".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteStrategicGoalRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok();

        let result = query(
            "UPDATE strategic_goals SET 
             deleted_at = ?,
             deleted_by_user_id = ?,
             deleted_by_device_id = ?,
             updated_at = ?, 
             updated_by_user_id = ?, 
             updated_by_device_id = ? 
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&now_str) // deleted_at
        .bind(&deleted_by) // deleted_by_user_id
        .bind(&device_id_str) // deleted_by_device_id
        .bind(&now_str) // updated_at
        .bind(&deleted_by) // updated_by_user_id (deleter is the last updater)
        .bind(&device_id_str) // updated_by_device_id
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        // Log the soft delete operation
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Delete,
            field_name: None,
            old_value: None, 
            new_value: None,
            timestamp: now, 
            user_id: auth.user_id,
            device_id: device_uuid_for_log, // Use Option<Uuid> for log
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;

        Ok(())
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(()) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteStrategicGoalRepository {
    fn entity_name(&self) -> &'static str {
        SqliteStrategicGoalRepository::entity_name(self)
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM strategic_goals WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx) // Use &mut **tx here
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("StrategicGoal".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(()) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

#[async_trait]
impl StrategicGoalRepository for SqliteStrategicGoalRepository {
    async fn create(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.create_with_tx(new_goal, auth, &mut tx).await;
        match result {
            Ok(goal) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(goal)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    /// Create strategic goal within an existing transaction
    /// Used for operations that need to manage document linking
    async fn create_with_tx<'t>(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        new_goal.validate()?;

        let id = new_goal.id.unwrap_or_else(Uuid::new_v4);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = new_goal.created_by_user_id.unwrap_or(auth.user_id);
        let user_id_str = user_id.to_string();
        let device_uuid_for_query = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok();

        query(
            r#"
            INSERT INTO strategic_goals (
                id, 
                objective_code, objective_code_updated_at, objective_code_updated_by, objective_code_updated_by_device_id,
                outcome, outcome_updated_at, outcome_updated_by, outcome_updated_by_device_id,
                kpi, kpi_updated_at, kpi_updated_by, kpi_updated_by_device_id,
                target_value, target_value_updated_at, target_value_updated_by, target_value_updated_by_device_id,
                actual_value, actual_value_updated_at, actual_value_updated_by, actual_value_updated_by_device_id,
                status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by, responsible_team_updated_by_device_id,
                sync_priority, 
                created_at, updated_at, 
                created_by_user_id, created_by_device_id, 
                updated_by_user_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, 
                ?, ?, ?, ?, /* objective_code fields */
                ?, ?, ?, ?, /* outcome fields */
                ?, ?, ?, ?, /* kpi fields */
                ?, ?, ?, ?, /* target_value fields */
                ?, ?, ?, ?, /* actual_value fields */
                ?, ?, ?, ?, /* status_id fields */
                ?, ?, ?, ?, /* responsible_team fields */
                ?, 
                ?, ?, 
                ?, ?, 
                ?, ?, 
                NULL, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        // Objective Code
        .bind(&new_goal.objective_code)
        .bind(&now_str).bind(&user_id_str).bind(&device_uuid_for_query)
        // Outcome
        .bind(&new_goal.outcome)
        .bind(new_goal.outcome.as_ref().map(|_| &now_str)).bind(new_goal.outcome.as_ref().map(|_| &user_id_str)).bind(new_goal.outcome.as_ref().map(|_| &device_uuid_for_query))
        // KPI
        .bind(&new_goal.kpi)
        .bind(new_goal.kpi.as_ref().map(|_| &now_str)).bind(new_goal.kpi.as_ref().map(|_| &user_id_str)).bind(new_goal.kpi.as_ref().map(|_| &device_uuid_for_query))
        // Target Value
        .bind(new_goal.target_value)
        .bind(new_goal.target_value.map(|_| &now_str)).bind(new_goal.target_value.map(|_| &user_id_str)).bind(new_goal.target_value.map(|_| &device_uuid_for_query))
        // Actual Value
        .bind(new_goal.actual_value.unwrap_or(0.0))
        .bind(new_goal.actual_value.map(|_| &now_str)).bind(new_goal.actual_value.map(|_| &user_id_str)).bind(new_goal.actual_value.map(|_| &device_uuid_for_query))
        // Status ID
        .bind(new_goal.status_id)
        .bind(new_goal.status_id.map(|_| &now_str)).bind(new_goal.status_id.map(|_| &user_id_str)).bind(new_goal.status_id.map(|_| &device_uuid_for_query))
        // Responsible Team
        .bind(&new_goal.responsible_team)
        .bind(new_goal.responsible_team.as_ref().map(|_| &now_str)).bind(new_goal.responsible_team.as_ref().map(|_| &user_id_str)).bind(new_goal.responsible_team.as_ref().map(|_| &device_uuid_for_query))
        // Sync Priority
        .bind(new_goal.sync_priority.as_str())
        // Timestamps & User IDs
        .bind(&now_str) // created_at
        .bind(&now_str) // updated_at
        .bind(&user_id_str) // created_by_user_id
        .bind(&device_uuid_for_query) // created_by_device_id
        .bind(&user_id_str) // updated_by_user_id
        .bind(&device_uuid_for_query) // updated_by_device_id
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None, old_value: None, new_value: None, // For create, new_value could be the serialized entity
            timestamp: now, user_id, device_id: device_uuid_for_log, // Use Option<Uuid> for log
            document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
        };
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;

        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(goal) => { tx.commit().await.map_err(DbError::from)?; Ok(goal) }
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    // UPDATED: Using QueryBuilder for safer dynamic SQL like ActivityRepository
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        update_data.validate()?;
        let old_entity = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = update_data.updated_by_user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid_for_query = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok();

        let mut builder = QueryBuilder::new("UPDATE strategic_goals SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        macro_rules! add_lww_field {
            ($field_ident:ident, $field_sql:literal, $value_expr:expr) => {
                if let Some(val) = $value_expr {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_uuid_for_query.clone());
                    fields_updated = true;
                }
            };
            ($field_ident:ident, $field_sql:literal, $value_expr:expr, opt_opt) => {
                 if let Some(opt_val) = $value_expr {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(opt_val.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_uuid_for_query.clone());
                    fields_updated = true;
                }
            };
        }

        add_lww_field!(objective_code, "objective_code", &update_data.objective_code);
        add_lww_field!(outcome, "outcome", &update_data.outcome, opt_opt);
        add_lww_field!(kpi, "kpi", &update_data.kpi, opt_opt);
        add_lww_field!(target_value, "target_value", &update_data.target_value, opt_opt);
        add_lww_field!(actual_value, "actual_value", &update_data.actual_value, opt_opt);
        add_lww_field!(status_id, "status_id", &update_data.status_id, opt_opt);
        add_lww_field!(responsible_team, "responsible_team", &update_data.responsible_team, opt_opt);

        if let Some(priority) = &update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str());
            fields_updated = true;
        }

        if !fields_updated {
            // If no specific LWW fields were updated, still update the main record's timestamp and user, if different from creation
            if old_entity.updated_by_user_id != Some(user_id) || old_entity.updated_by_device_id.map(|u| u.to_string()) != device_uuid_for_query {
                 separated.push("updated_at = ");
                 separated.push_bind_unseparated(now_str.clone());
                 separated.push("updated_by_user_id = ");
                 separated.push_bind_unseparated(user_id_str.clone());
                 separated.push("updated_by_device_id = ");
                 separated.push_bind_unseparated(device_uuid_for_query.clone());
            } else {
                return Ok(old_entity); // No changes at all
            }
        } else {
            // If LWW fields were updated, always update the main record's timestamp and user
            separated.push("updated_at = ");
            separated.push_bind_unseparated(now_str.clone());
            separated.push("updated_by_user_id = ");
            separated.push_bind_unseparated(user_id_str.clone());
            separated.push("updated_by_device_id = ");
            separated.push_bind_unseparated(device_uuid_for_query.clone());
        }

        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
             // This might happen if the record was deleted by another process after `find_by_id_with_tx`
             // or if the WHERE clause (id = ? AND deleted_at IS NULL) didn't match.
            return self.find_by_id_with_tx(id, tx).await
                .err()
                .map_or(Err(DomainError::EntityNotFound(self.entity_name().to_string(), id)), |e| Err(e));
        }
        
        let new_entity = self.find_by_id_with_tx(id, tx).await?;

        // Log changes for each field
        macro_rules! log_if_changed {
            ($field:ident, $field_name_str:expr) => {
                if old_entity.$field != new_entity.$field {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: id, 
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_name_str.to_string()),
                        old_value: serde_json::to_string(&old_entity.$field).ok(),
                        new_value: serde_json::to_string(&new_entity.$field).ok(),
                        timestamp: now, user_id: user_id, device_id: device_uuid_for_log.clone(),
                        document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
                }
            };
        }

        log_if_changed!(objective_code, "objective_code");
        log_if_changed!(outcome, "outcome");
        log_if_changed!(kpi, "kpi");
        log_if_changed!(target_value, "target_value");
        log_if_changed!(actual_value, "actual_value");
        log_if_changed!(status_id, "status_id");
        log_if_changed!(responsible_team, "responsible_team");
        // log_if_changed!(sync_priority, "sync_priority"); // Handled below

        if old_entity.sync_priority != new_entity.sync_priority {
             let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("sync_priority".to_string()),
                old_value: serde_json::to_string(old_entity.sync_priority.as_str()).ok(),
                new_value: serde_json::to_string(new_entity.sync_priority.as_str()).ok(),
                timestamp: now, user_id: user_id, device_id: device_uuid_for_log.clone(),
                document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
            };
            self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
        }

        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar("SELECT COUNT(*) FROM strategic_goals WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE deleted_at IS NULL ORDER BY objective_code ASC LIMIT ? OFFSET ?",
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_objective_code(&self, code: &str) -> DomainResult<Option<StrategicGoal>> {
        let row = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE objective_code = ? AND deleted_at IS NULL",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;
         
        match row {
            Some(r) => Self::map_row_to_entity(r).map(Some),
            None => Ok(None),
        }
    }

    /// Find strategic goals by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        log::info!("Repository find_by_ids called with {} IDs: {:?}", ids.len(), ids);

        let offset = (params.page - 1) * params.per_page;

        // Build COUNT query using QueryBuilder properly
        let mut count_builder = QueryBuilder::new("SELECT COUNT(*) FROM strategic_goals WHERE id IN (");
        let mut count_separated = count_builder.separated(", ");
        for id in ids {
            let id_str = id.to_string();
            log::info!("Adding ID to query: '{}'", id_str);
            count_separated.push_bind(id_str);
        }
        count_builder.push(") AND deleted_at IS NULL");
        
        log::info!("Executing count query for strategic goals");

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        log::info!("Count query returned: {} matching records", total);
        
        // Debug: Show some sample IDs from database
        if total == 0 {
            let sample_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM strategic_goals WHERE deleted_at IS NULL LIMIT 5")
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default();
            log::info!("Sample IDs in database: {:?}", sample_ids);
        }

        // Build SELECT query using QueryBuilder properly
        let mut select_builder = QueryBuilder::new("SELECT * FROM strategic_goals WHERE id IN (");
        let mut select_separated = select_builder.separated(", ");
        for id in ids {
            select_separated.push_bind(id.to_string());
        }
        select_builder.push(") AND deleted_at IS NULL ORDER BY objective_code ASC LIMIT ");
        select_builder.push_bind(params.per_page as i64);
        select_builder.push(" OFFSET ");
        select_builder.push_bind(offset as i64);

        let rows = select_builder
            .build_query_as::<StrategicGoalRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    // --- UPDATED: update_sync_priority with Change Logging ---
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() { return Ok(0); }
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        // 1. Fetch old priorities for logging
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query = format!(
            "SELECT id, sync_priority FROM strategic_goals WHERE id IN ({})",
            vec!["?"; ids.len()].join(", ")
        );
        let mut select_builder = query_as::<_, (String, String)>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: std::collections::HashMap<Uuid, SyncPriorityFromSyncDomain> = select_builder
            .fetch_all(&mut *tx)
            .await
            .map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_text)| {
                 Uuid::parse_str(&id_str).ok()
                     .and_then(|id| SyncPriorityFromSyncDomain::from_str(&prio_text).map(|prio| (id, prio)).ok())
            })
            .collect();

        // 2. Perform the Update (existing logic moved into tx)
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let priority_str = priority.as_str();
        
        let mut update_builder = QueryBuilder::new("UPDATE strategic_goals SET ");
        update_builder.push("sync_priority = "); update_builder.push_bind(priority_str);
        update_builder.push(", updated_at = "); update_builder.push_bind(now_str.clone()); // Use clone as now_str is used later if needed
        update_builder.push(", updated_by_user_id = "); update_builder.push_bind(user_id_str.clone()); // Use clone
        update_builder.push(" WHERE id IN (");
        let mut id_separated = update_builder.separated(",");
        for id in ids { id_separated.push_bind(id.to_string()); }
        update_builder.push(") AND deleted_at IS NULL");
        
        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();

        // 3. Log changes for affected rows where priority actually changed
        for id in ids {
            if let Some(old_priority) = old_priorities.get(id) {
                if *old_priority != priority { // Log only if changed
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: *id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: serde_json::to_string(old_priority.as_str()).ok(),
                        new_value: serde_json::to_string(priority_str).ok(),
                        timestamp: now, 
                        user_id: user_id,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
                }
            }
            // If ID wasn't in old_priorities, it means it was already deleted or didn't exist,
            // so no change log is needed.
        }

        tx.commit().await.map_err(DbError::from)?; // Commit transaction
        
        Ok(rows_affected)
    }

    /// Find strategic goals by status ID
    async fn find_by_status_id(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals WHERE status_id = ? AND deleted_at IS NULL"
        )
        .bind(status_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE status_id = ? AND deleted_at IS NULL 
             ORDER BY objective_code ASC LIMIT ? OFFSET ?"
        )
        .bind(status_id)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find strategic goals by responsible team
    async fn find_by_responsible_team(
        &self,
        team_name: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals WHERE responsible_team = ? AND deleted_at IS NULL"
        )
        .bind(team_name)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE responsible_team = ? AND deleted_at IS NULL 
             ORDER BY objective_code ASC LIMIT ? OFFSET ?"
        )
        .bind(team_name)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find goal IDs where a user has a specific role
    async fn find_ids_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole,
    ) -> DomainResult<Vec<Uuid>> {
        let user_id_str = user_id.to_string();
        
        // Build query based on role
        let query_str = match role {
            UserGoalRole::Created => {
                "SELECT id FROM strategic_goals WHERE created_by_user_id = ? AND deleted_at IS NULL"
            }
            UserGoalRole::Updated => {
                "SELECT id FROM strategic_goals WHERE updated_by_user_id = ? AND deleted_at IS NULL"
            }
        };

        let id_strings: Vec<String> = query_scalar(query_str)
            .bind(&user_id_str)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Convert string IDs to UUIDs
        let ids = id_strings
            .into_iter()
            .map(|id_str| Uuid::parse_str(&id_str).map_err(|_| DomainError::InvalidUuid(id_str)))
            .collect::<Result<Vec<Uuid>, _>>()?; // Collect into a Result

        Ok(ids)
    }

    /// Count goals by status_id, grouped by status
    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>> {
        let counts = query_as::<_, (Option<i64>, i64)>(
            "SELECT status_id, COUNT(*) 
             FROM strategic_goals 
             WHERE deleted_at IS NULL 
             GROUP BY status_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    /// Count goals grouped by responsible team
    async fn count_by_responsible_team(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT responsible_team, COUNT(*) 
             FROM strategic_goals 
             WHERE deleted_at IS NULL 
             GROUP BY responsible_team"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    /// Get aggregate value statistics for strategic goals
    async fn get_value_summary(&self) -> DomainResult<GoalValueSummary> {
        // The query_as needs the target struct (GoalValueSummary) to derive FromRow
        let summary = query_as::<_, GoalValueSummary>( // Use GoalValueSummary directly
            "SELECT 
                AVG(target_value) as avg_target, 
                AVG(actual_value) as avg_actual, 
                SUM(target_value) as total_target, 
                SUM(actual_value) as total_actual, 
                COUNT(*) as count
             FROM strategic_goals 
             WHERE deleted_at IS NULL"
             // Removed conditions on NOT NULL values as AVG/SUM handle NULLs appropriately in SQL
             // and GoalValueSummary uses Option<f64>
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(summary)
    }

    /// Count goals that haven't been updated since a specific date
    async fn count_stale(&self, cutoff_date: DateTime<Utc>) -> DomainResult<i64> {
        let cutoff_date_str = cutoff_date.to_rfc3339();
        let count = query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM strategic_goals WHERE updated_at < ? AND deleted_at IS NULL"
        )
        .bind(cutoff_date_str) // Bind the RFC3339 string
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        Ok(count)
    }

    /// Find goals that haven't been updated since a specific date (paginated)
    async fn find_stale(
        &self,
        cutoff_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;
        let cutoff_date_str = cutoff_date.to_rfc3339();

        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals WHERE updated_at < ? AND deleted_at IS NULL"
        )
        .bind(&cutoff_date_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals 
             WHERE updated_at < ? AND deleted_at IS NULL 
             ORDER BY updated_at ASC 
             LIMIT ? OFFSET ?"
        )
        .bind(&cutoff_date_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }

    /// Find strategic goals by status ID
    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count for this status
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals 
             WHERE status_id = ? AND deleted_at IS NULL"
        )
        .bind(status_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows for this status
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals 
             WHERE status_id = ? AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(status_id)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find goals created or updated by a specific user
    async fn find_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;
        let user_id_str = user_id.to_string();

        let (count_column, where_clause) = match role {
            UserGoalRole::Created => ("created_by_user_id", "created_by_user_id = ?"),
            UserGoalRole::Updated => ("updated_by_user_id", "updated_by_user_id = ?"),
        };

        let count_query_str = format!(
            "SELECT COUNT(*) FROM strategic_goals WHERE {} AND deleted_at IS NULL",
            where_clause
        );
        
        let total: i64 = query_scalar(&count_query_str)
            .bind(&user_id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        let main_query_str = format!(
            "SELECT * FROM strategic_goals 
             WHERE {} AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?",
             where_clause
        );
        
        let rows = query_as::<_, StrategicGoalRow>(&main_query_str)
            .bind(&user_id_str)
            .bind(params.per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find goals that haven't been updated since a specific date
    async fn find_stale_since(
        &self,
        cutoff_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;
        let cutoff_date_str = cutoff_date.to_rfc3339();

        // Get total count of stale goals
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals 
             WHERE updated_at < ? AND deleted_at IS NULL"
        )
        .bind(&cutoff_date_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated stale goals
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals 
             WHERE updated_at < ? AND deleted_at IS NULL 
             ORDER BY updated_at ASC LIMIT ? OFFSET ?"
        )
        .bind(&cutoff_date_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find strategic goals within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;
        let start_date_str = start_date.to_rfc3339();
        let end_date_str = end_date.to_rfc3339();

        // Get total count of goals within the date range
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals WHERE updated_at >= ? AND updated_at <= ? AND deleted_at IS NULL"
        )
        .bind(&start_date_str)
        .bind(&end_date_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated goals within the date range
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE updated_at >= ? AND updated_at <= ? AND deleted_at IS NULL ORDER BY updated_at ASC LIMIT ? OFFSET ?"
        )
        .bind(&start_date_str)
        .bind(&end_date_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }

    /// Find strategic goal IDs that match complex filter criteria
    /// Supports AND/OR logic: (status1 OR status2) AND (year1 OR year2) AND (month1 OR month2)
    async fn find_ids_by_filter(
        &self,
        filter: StrategicGoalFilter,
    ) -> DomainResult<Vec<Uuid>> {
        use sqlx::QueryBuilder;
        
        let mut query_builder = QueryBuilder::new("SELECT DISTINCT sg.id FROM strategic_goals sg WHERE 1=1");
        
        // Exclude deleted items by default
        if filter.exclude_deleted.unwrap_or(true) {
            query_builder.push(" AND sg.deleted_at IS NULL");
        }
        
        // Status filter (OR logic within)
        if let Some(status_ids) = &filter.status_ids {
            if !status_ids.is_empty() {
                query_builder.push(" AND sg.status_id IN (");
                let mut separated = query_builder.separated(", ");
                for status_id in status_ids {
                    separated.push_bind(status_id);
                }
                separated.push_unseparated(")");
            }
        }
        
        // Responsible teams filter (OR logic within)
        if let Some(teams) = &filter.responsible_teams {
            if !teams.is_empty() {
                query_builder.push(" AND sg.responsible_team IN (");
                let mut separated = query_builder.separated(", ");
                for team in teams {
                    separated.push_bind(team);
                }
                separated.push_unseparated(")");
            }
        }
        
        // Year and Month filters (complex AND/OR logic)
        if let (Some(years), Some(months)) = (&filter.years, &filter.months) {
            if !years.is_empty() && !months.is_empty() {
                // (year1 OR year2) AND (month1 OR month2)
                query_builder.push(" AND (");
                
                // Year condition
                query_builder.push("CAST(strftime('%Y', sg.created_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for year in years {
                    separated.push_bind(year);
                }
                separated.push_unseparated(")");
                
                query_builder.push(" OR CAST(strftime('%Y', sg.updated_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for year in years {
                    separated.push_bind(year);
                }
                separated.push_unseparated(")");
                
                query_builder.push(") AND (");
                
                // Month condition
                query_builder.push("CAST(strftime('%m', sg.created_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for month in months {
                    separated.push_bind(month);
                }
                separated.push_unseparated(")");
                
                query_builder.push(" OR CAST(strftime('%m', sg.updated_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for month in months {
                    separated.push_bind(month);
                }
                separated.push_unseparated(")");
                
                query_builder.push(")");
            }
        } else if let Some(years) = &filter.years {
            if !years.is_empty() {
                query_builder.push(" AND (");
                query_builder.push("CAST(strftime('%Y', sg.created_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for year in years {
                    separated.push_bind(year);
                }
                separated.push_unseparated(")");
                
                query_builder.push(" OR CAST(strftime('%Y', sg.updated_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for year in years {
                    separated.push_bind(year);
                }
                separated.push_unseparated(")");
                query_builder.push(")");
            }
        } else if let Some(months) = &filter.months {
            if !months.is_empty() {
                query_builder.push(" AND (");
                query_builder.push("CAST(strftime('%m', sg.created_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for month in months {
                    separated.push_bind(month);
                }
                separated.push_unseparated(")");
                
                query_builder.push(" OR CAST(strftime('%m', sg.updated_at) AS INTEGER) IN (");
                let mut separated = query_builder.separated(", ");
                for month in months {
                    separated.push_bind(month);
                }
                separated.push_unseparated(")");
                query_builder.push(")");
            }
        }
        
        // Search text filter
        if let Some(search_text) = &filter.search_text {
            if !search_text.trim().is_empty() {
                let search_pattern = format!("%{}%", search_text.trim());
                query_builder.push(" AND (");
                query_builder.push("sg.objective_code LIKE ")
                    .push_bind(search_pattern.clone());
                query_builder.push(" OR sg.outcome LIKE ")
                    .push_bind(search_pattern.clone());
                query_builder.push(" OR sg.kpi LIKE ")
                    .push_bind(search_pattern.clone());
                query_builder.push(" OR sg.responsible_team LIKE ")
                    .push_bind(search_pattern.clone());
                query_builder.push(")");
            }
        }
        
        // Sync priority filter
        if let Some(priorities) = &filter.sync_priorities {
            if !priorities.is_empty() {
                query_builder.push(" AND sg.sync_priority IN (");
                let mut separated = query_builder.separated(", ");
                for priority in priorities {
                    separated.push_bind(priority.to_string());
                }
                separated.push_unseparated(")");
            }
        }
        
        // User role filter
        if let Some((user_id, role)) = &filter.user_role {
            match role {
                UserGoalRole::Created => {
                    query_builder.push(" AND sg.created_by_user_id = ").push_bind(user_id.to_string());
                }
                UserGoalRole::Updated => {
                    query_builder.push(" AND sg.updated_by_user_id = ").push_bind(user_id.to_string());
                }
            }
        }
        
        // Progress range filter
        if let Some((min_progress, max_progress)) = &filter.progress_range {
            query_builder.push(" AND sg.target_value IS NOT NULL AND sg.target_value > 0");
            query_builder.push(" AND ((sg.actual_value / sg.target_value) * 100) BETWEEN ")
                .push_bind(min_progress)
                .push(" AND ")
                .push_bind(max_progress);
        }
        
        // Target value range filter
        if let Some((min_target, max_target)) = &filter.target_value_range {
            query_builder.push(" AND sg.target_value BETWEEN ")
                .push_bind(min_target)
                .push(" AND ")
                .push_bind(max_target);
        }
        
        // Actual value range filter
        if let Some((min_actual, max_actual)) = &filter.actual_value_range {
            query_builder.push(" AND sg.actual_value BETWEEN ")
                .push_bind(min_actual)
                .push(" AND ")
                .push_bind(max_actual);
        }
        
        // Date range filter
        if let Some((start_date, end_date)) = &filter.date_range {
            // Parse RFC3339 dates
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(start_date),
                DateTime::parse_from_rfc3339(end_date)
            ) {
                let start_utc = start.with_timezone(&Utc);
                let end_utc = end.with_timezone(&Utc);
                
                query_builder.push(" AND (");
                query_builder.push("sg.created_at BETWEEN ")
                    .push_bind(start_utc.to_rfc3339())
                    .push(" AND ")
                    .push_bind(end_utc.to_rfc3339());
                query_builder.push(" OR sg.updated_at BETWEEN ")
                    .push_bind(start_utc.to_rfc3339())
                    .push(" AND ")
                    .push_bind(end_utc.to_rfc3339());
                query_builder.push(")");
            }
        }
        
        // Days stale filter
        if let Some(days_stale) = &filter.days_stale {
            let cutoff_date = Utc::now() - chrono::Duration::days(*days_stale as i64);
            query_builder.push(" AND sg.updated_at < ")
                .push_bind(cutoff_date.to_rfc3339());
        }
        
        // Execute the query
        let query = query_builder.build_query_as::<(String,)>();
        let rows = query.fetch_all(&self.pool).await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        // Parse UUIDs
        let ids: Result<Vec<Uuid>, _> = rows
            .into_iter()
            .map(|(id_str,)| Uuid::parse_str(&id_str))
            .collect();
        
        ids.map_err(|e| DomainError::InvalidUuid(e.to_string()))
    }
}

// === Sync Merge Implementation ===
#[async_trait]
impl MergeableEntityRepository<StrategicGoal> for SqliteStrategicGoalRepository {
    fn entity_name(&self) -> &'static str { "strategic_goals" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for strategic_goal change")))?;
                let remote_state: StrategicGoal = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_strategic_goal", &format!("Invalid JSON: {}", e))))?;

                let local_opt = match self.find_by_id_with_tx(remote_state.id, tx).await {
                    Ok(ent) => Some(ent),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };

                if let Some(local) = local_opt.as_ref() {
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local copy newer or equal".into()));
                    }
                }

                self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                Ok(if local_opt.is_some() { MergeOutcome::Updated(remote_state.id) } else { MergeOutcome::Created(remote_state.id) })
            }
            ChangeOperationType::Delete => Ok(MergeOutcome::NoOp("Remote soft delete ignored".into())),
            ChangeOperationType::HardDelete => Ok(MergeOutcome::HardDeleted(remote_change.entity_id)),
        }
    }
}

impl SqliteStrategicGoalRepository {
    async fn upsert_remote_state_with_tx<'t>(&self, tx: &mut Transaction<'t, Sqlite>, remote: &StrategicGoal) -> DomainResult<()> {
        sqlx::query(
            r#"
INSERT OR REPLACE INTO strategic_goals (
    id,
    objective_code, objective_code_updated_at, objective_code_updated_by, objective_code_updated_by_device_id,
    outcome, outcome_updated_at, outcome_updated_by, outcome_updated_by_device_id,
    kpi, kpi_updated_at, kpi_updated_by, kpi_updated_by_device_id,
    target_value, target_value_updated_at, target_value_updated_by, target_value_updated_by_device_id,
    actual_value, actual_value_updated_at, actual_value_updated_by, actual_value_updated_by_device_id,
    status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
    responsible_team, responsible_team_updated_at, responsible_team_updated_by, responsible_team_updated_by_device_id,
    sync_priority,
    created_at, updated_at,
    created_by_user_id, created_by_device_id,
    updated_by_user_id, updated_by_device_id,
    deleted_at, deleted_by_user_id, deleted_by_device_id
) VALUES (
    ?,?,?,?,?, ?,?,?,?, ?,?,?,?, ?,?,?,?, ?,?,?,?, ?,?,?,?, ?,?,?,?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
)
            "#
        )
        .bind(remote.id.to_string())
        // Objective Code fields
        .bind(&remote.objective_code)
        .bind(remote.objective_code_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.objective_code_updated_by.map(|u| u.to_string()))
        .bind(remote.objective_code_updated_by_device_id.map(|u| u.to_string()))
        // Outcome
        .bind(&remote.outcome)
        .bind(remote.outcome_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.outcome_updated_by.map(|u| u.to_string()))
        .bind(remote.outcome_updated_by_device_id.map(|u| u.to_string()))
        // KPI
        .bind(&remote.kpi)
        .bind(remote.kpi_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.kpi_updated_by.map(|u| u.to_string()))
        .bind(remote.kpi_updated_by_device_id.map(|u| u.to_string()))
        // Target Value
        .bind(remote.target_value)
        .bind(remote.target_value_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.target_value_updated_by.map(|u| u.to_string()))
        .bind(remote.target_value_updated_by_device_id.map(|u| u.to_string()))
        // Actual Value
        .bind(remote.actual_value.unwrap_or(0.0))
        .bind(remote.actual_value_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.actual_value_updated_by.map(|u| u.to_string()))
        .bind(remote.actual_value_updated_by_device_id.map(|u| u.to_string()))
        // Status ID
        .bind(remote.status_id)
        .bind(remote.status_id_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.status_id_updated_by.map(|u| u.to_string()))
        .bind(remote.status_id_updated_by_device_id.map(|u| u.to_string()))
        // Responsible Team
        .bind(&remote.responsible_team)
        .bind(remote.responsible_team_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.responsible_team_updated_by.map(|u| u.to_string()))
        .bind(remote.responsible_team_updated_by_device_id.map(|u| u.to_string()))
        // Sync Priority
        .bind(remote.sync_priority.as_str())
        // Created/Updated
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|u| u.to_string()))
        .bind(remote.created_by_device_id.map(|u| u.to_string()))
        .bind(remote.updated_by_user_id.map(|u| u.to_string()))
        .bind(remote.updated_by_device_id.map(|u| u.to_string()))
        // Deleted fields
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|u| u.to_string()))
        .bind(remote.deleted_by_device_id.map(|u| u.to_string()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}