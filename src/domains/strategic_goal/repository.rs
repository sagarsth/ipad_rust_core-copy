use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite, QueryBuilder};
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalRow, UpdateStrategicGoal, UserGoalRole, GoalValueSummary,
};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;

/// Trait defining strategic goal repository operations
#[async_trait]
pub trait StrategicGoalRepository:
    DeleteServiceRepository<StrategicGoal> + Send + Sync
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

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    async fn set_document_reference(
        &self,
        goal_id: Uuid,
        field_name: &str,
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;

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
    async fn count_stale(&self, cutoff_date: &str) -> DomainResult<i64>;

    /// Find goals that haven't been updated since a specific date (paginated)
    async fn find_stale(
        &self,
        cutoff_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;
}

/// SQLite implementation for StrategicGoalRepository
#[derive(Debug, Clone)]
pub struct SqliteStrategicGoalRepository {
    pool: SqlitePool,
}

impl SqliteStrategicGoalRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE strategic_goals SET 
             deleted_at = ?, 
             deleted_by_user_id = ?
             WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
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

    async fn create_with_tx<'t>(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        let id = new_goal.id.unwrap_or_else(Uuid::new_v4);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        // Insert the new strategic goal
        query(
            r#"
            INSERT INTO strategic_goals (
                id, objective_code, objective_code_updated_at, objective_code_updated_by,
                outcome, outcome_updated_at, outcome_updated_by,
                kpi, kpi_updated_at, kpi_updated_by,
                target_value, target_value_updated_at, target_value_updated_by,
                actual_value, actual_value_updated_at, actual_value_updated_by,
                status_id, status_id_updated_at, status_id_updated_by,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 
                ?, -- for sync_priority
                ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(&new_goal.objective_code)
        .bind(&now_str).bind(&user_id_str) // objective_code LWW
        .bind(&new_goal.outcome)
        .bind(new_goal.outcome.as_ref().map(|_| &now_str)).bind(new_goal.outcome.as_ref().map(|_| &user_id_str)) // outcome LWW
        .bind(&new_goal.kpi)
        .bind(new_goal.kpi.as_ref().map(|_| &now_str)).bind(new_goal.kpi.as_ref().map(|_| &user_id_str)) // kpi LWW
        .bind(new_goal.target_value)
        .bind(new_goal.target_value.map(|_| &now_str)).bind(new_goal.target_value.map(|_| &user_id_str)) // target_value LWW
        .bind(new_goal.actual_value.unwrap_or(0.0)) // Use default if None
        .bind(new_goal.actual_value.map(|_| &now_str)).bind(new_goal.actual_value.map(|_| &user_id_str)) // actual_value LWW
        .bind(new_goal.status_id)
        .bind(new_goal.status_id.map(|_| &now_str)).bind(new_goal.status_id.map(|_| &user_id_str)) // status_id LWW
        .bind(&new_goal.responsible_team)
        .bind(new_goal.responsible_team.as_ref().map(|_| &now_str)).bind(new_goal.responsible_team.as_ref().map(|_| &user_id_str)) // responsible_team LWW
        .bind(new_goal.sync_priority as i64)
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        // Fetch the created goal to return it
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
        // Check existence first to ensure we don't try updating a non-existent/deleted record
        let _current_goal = self.find_by_id_with_tx(id, tx).await?;
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let id_str = id.to_string();

        // Use QueryBuilder instead of manual string concatenation
        let mut builder = QueryBuilder::new("UPDATE strategic_goals SET ");
        let mut separated = builder.separated(", "); // Use separated for SET clauses

        let mut fields_updated = false; // Track if any LWW fields actually changed

        // Macro to simplify adding LWW fields
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone()); // Bind the value

                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());

                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    fields_updated = true;
                }
            };
        }

        // Apply updates using the macro
        add_lww!(objective_code, "objective_code", &update_data.objective_code);
        add_lww!(outcome, "outcome", &update_data.outcome);
        add_lww!(kpi, "kpi", &update_data.kpi);
        add_lww!(target_value, "target_value", &update_data.target_value);
        add_lww!(actual_value, "actual_value", &update_data.actual_value);
        add_lww!(status_id, "status_id", &update_data.status_id);
        add_lww!(responsible_team, "responsible_team", &update_data.responsible_team);

        // Add sync_priority if provided
        if let Some(priority) = update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority as i64);
            fields_updated = true;
        }

        // Only execute if actual fields were updated
        if !fields_updated {
            // If no fields would change, just return current
            return Ok(_current_goal);
        }

        // Always add updated_at and updated_by_user_id
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str);
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str);

        // Finalize the query with WHERE clause
        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL"); // Ensure we only update non-deleted records

        let query = builder.build();

        // Execute the built query
        let result = query
            .execute(&mut **tx) // Use **tx
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Should ideally not happen if find_by_id_with_tx succeeded,
            // but could occur in a race condition if deleted between find and update
            return Err(DomainError::EntityNotFound(Self::entity_name(self).to_string(), id));
        }
        
        // Fetch and return the updated entity
        self.find_by_id_with_tx(id, tx).await
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

    // UPDATED: Using QueryBuilder pattern for update_sync_priority
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let priority_val = priority as i64;
        
        let mut builder = QueryBuilder::new("UPDATE strategic_goals SET ");
        builder.push("sync_priority = ");
        builder.push_bind(priority_val);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        
        // Build the WHERE clause with IN condition
        builder.push(" WHERE id IN (");
        let mut id_separated = builder.separated(",");
        for id in ids {
            id_separated.push_bind(id.to_string());
        }
        builder.push(") AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;
        
        Ok(result.rows_affected())
    }

    async fn set_document_reference(
        &self,
        goal_id: Uuid,
        field_name: &str,
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let column_name = format!("{}_ref", field_name); 
        
        if !StrategicGoal::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for StrategicGoal: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let document_id_str = document_id.to_string();
        
        let mut builder = sqlx::QueryBuilder::new("UPDATE strategic_goals SET ");
        builder.push(&column_name);
        builder.push(" = ");
        builder.push_bind(document_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(goal_id.to_string());
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("StrategicGoal".to_string(), goal_id))
        } else {
            Ok(())
        }
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
    async fn count_stale(&self, cutoff_date: &str) -> DomainResult<i64> {
        // Validate cutoff_date format? (Optional, depends on desired robustness)
        // Example: if chrono::DateTime::parse_from_rfc3339(cutoff_date).is_err() { ... }

        let count = query_scalar::<_, i64>(
            "SELECT COUNT(*) 
             FROM strategic_goals 
             WHERE updated_at < ? 
                AND deleted_at IS NULL"
        )
        .bind(cutoff_date)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(count)
    }

    /// Find goals that haven't been updated since a specific date (paginated)
    async fn find_stale(
        &self,
        cutoff_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count of stale goals
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM strategic_goals WHERE updated_at < ? AND deleted_at IS NULL"
        )
        .bind(cutoff_date)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated stale rows
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals 
             WHERE updated_at < ? AND deleted_at IS NULL 
             ORDER BY updated_at ASC -- Or objective_code ASC, depending on desired order
             LIMIT ? OFFSET ?"
        )
        .bind(cutoff_date)
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
}