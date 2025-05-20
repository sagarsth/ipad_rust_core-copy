use crate::auth::AuthContext;
use sqlx::SqlitePool;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::workshop::types::{
    NewWorkshop, Workshop, WorkshopRow, UpdateWorkshop,
    WorkshopStatistics, WorkshopBudgetSummary, ProjectWorkshopMetrics,
};
use crate::domains::sync::types::SyncPriority;
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::{Utc, NaiveDate, Local};
use sqlx::{query, query_as, query_scalar, Executor, Row, Sqlite, Transaction, sqlite::SqliteArguments, QueryBuilder};
use sqlx::Arguments;
use uuid::Uuid;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::domains::user::repository::MergeableEntityRepository;

/// Trait defining workshop repository operations
#[async_trait]
pub trait WorkshopRepository:
    DeleteServiceRepository<Workshop> + MergeableEntityRepository<Workshop> + Send + Sync
{
    async fn create(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop>;
    async fn create_with_tx<'t>(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop>;

    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>, // Optional filter by project
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    async fn increment_participant_count(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;
    async fn decrement_participant_count(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;

    /// Count workshops by location
    async fn count_by_location(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Count workshops by month
    async fn count_by_month(&self) -> DomainResult<Vec<(String, i64)>>;
    
    /// Count workshops by project
    async fn count_by_project(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>>;
    
    /// Get comprehensive workshop statistics
    async fn get_workshop_statistics(&self) -> DomainResult<WorkshopStatistics>;
    
    /// Find workshops by date range
    async fn find_by_date_range(
        &self,
        start_date: NaiveDate, 
        end_date: NaiveDate,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    /// Find past workshops
    async fn find_past_workshops(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    /// Find upcoming workshops
    async fn find_upcoming_workshops(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    /// Find workshops by location
    async fn find_by_location(
        &self,
        location: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    /// Get detailed budget statistics for workshops
    async fn get_budget_statistics(
        &self,
        project_id: Option<Uuid>,
    ) -> DomainResult<(Decimal, Decimal, Decimal, f64)>; // (total_budget, total_actuals, total_variance, avg_variance_pct)
    
    /// Get workshop budget summaries for a project
    async fn get_workshop_budget_summaries_for_project(
        &self,
        project_id: Uuid,
    ) -> DomainResult<Vec<WorkshopBudgetSummary>>;
    
    /// Get project workshop metrics
    async fn get_project_workshop_metrics(
        &self,
        project_id: Uuid,
    ) -> DomainResult<ProjectWorkshopMetrics>;
}

/// SQLite implementation for WorkshopRepository
#[derive(Clone)]
pub struct SqliteWorkshopRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl std::fmt::Debug for SqliteWorkshopRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteWorkshopRepository")
            .field("pool", &self.pool)
            .field("change_log_repo", &"<ChangeLogRepository>")
            .finish()
    }
}

impl SqliteWorkshopRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: WorkshopRow) -> DomainResult<Workshop> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "workshops"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        let row = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Workshop".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Workshop> for SqliteWorkshopRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Workshop> {
        let row = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Workshop".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteWorkshopRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        let deleted_by_device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        let result = query(
            "UPDATE workshops SET 
             deleted_at = ?, 
             deleted_by_user_id = ?,
             deleted_by_device_id = ?,
             updated_at = ?, -- Also update the main updated_at timestamp
             updated_by_user_id = ?, -- and who updated it (the deleter)
             updated_by_device_id = ? -- and which device did it
             WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&now) // deleted_at
        .bind(&deleted_by) // deleted_by_user_id
        .bind(&deleted_by_device_id_str) // deleted_by_device_id
        .bind(&now) // updated_at
        .bind(&deleted_by) // updated_by_user_id
        .bind(&deleted_by_device_id_str) // updated_by_device_id
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Either not found or already deleted
            Err(DomainError::EntityNotFound("Workshop".to_string(), id)) 
        } else {
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(()) => {
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
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
impl HardDeletable for SqliteWorkshopRepository {
    fn entity_name(&self) -> &'static str {
        SqliteWorkshopRepository::entity_name(self)
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Usually only admin role check is needed, done in service
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM workshops WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
             // This might happen if the record was deleted between check and execution, or never existed.
            Err(DomainError::EntityNotFound("Workshop".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(()) => {
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
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
impl WorkshopRepository for SqliteWorkshopRepository {
    async fn create(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.create_with_tx(new_workshop, auth, &mut tx).await;
        match result {
            Ok(workshop) => {
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
                Ok(workshop)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let device_uuid_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok(); // For direct Uuid use in log

        let user_id_str = user_id.to_string();
        let project_id_str = new_workshop.project_id.map(|id| id.to_string());

        // Convert Decimals to String for DB storage
        let budget_str = new_workshop.budget.map(|d| d.to_string());
        let actuals_str = new_workshop.actuals.map(|d| d.to_string());

        // Insert the new workshop
        query(
            r#"
            INSERT INTO workshops (
                id, project_id, 
                purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id,
                event_date, event_date_updated_at, event_date_updated_by, event_date_updated_by_device_id,
                location, location_updated_at, location_updated_by, location_updated_by_device_id,
                budget, budget_updated_at, budget_updated_by, budget_updated_by_device_id,
                actuals, actuals_updated_at, actuals_updated_by, actuals_updated_by_device_id,
                participant_count, participant_count_updated_at, participant_count_updated_by, participant_count_updated_by_device_id,
                local_partner, local_partner_updated_at, local_partner_updated_by, local_partner_updated_by_device_id,
                partner_responsibility, partner_responsibility_updated_at, partner_responsibility_updated_by, partner_responsibility_updated_by_device_id,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, ?, 
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 
                ?, ?, ?, ?, ?, ?, NULL, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(project_id_str) // Bind Option<String>
        .bind(&new_workshop.purpose)
        .bind(new_workshop.purpose.as_ref().map(|_| &now_str)).bind(new_workshop.purpose.as_ref().map(|_| &user_id_str)).bind(new_workshop.purpose.as_ref().map(|_| &device_uuid_str)) // purpose LWW
        .bind(&new_workshop.event_date)
        .bind(new_workshop.event_date.as_ref().map(|_| &now_str)).bind(new_workshop.event_date.as_ref().map(|_| &user_id_str)).bind(new_workshop.event_date.as_ref().map(|_| &device_uuid_str)) // event_date LWW
        .bind(&new_workshop.location)
        .bind(new_workshop.location.as_ref().map(|_| &now_str)).bind(new_workshop.location.as_ref().map(|_| &user_id_str)).bind(new_workshop.location.as_ref().map(|_| &device_uuid_str)) // location LWW
        .bind(&budget_str) // Bind Option<String>
        .bind(new_workshop.budget.map(|_| &now_str)).bind(new_workshop.budget.map(|_| &user_id_str)).bind(new_workshop.budget.map(|_| &device_uuid_str)) // budget LWW
        .bind(&actuals_str) // Bind Option<String>
        .bind(new_workshop.actuals.map(|_| &now_str)).bind(new_workshop.actuals.map(|_| &user_id_str)).bind(new_workshop.actuals.map(|_| &device_uuid_str)) // actuals LWW
        .bind(new_workshop.participant_count.unwrap_or(0)) // Default to 0 if None
        .bind(new_workshop.participant_count.map(|_| &now_str)).bind(new_workshop.participant_count.map(|_| &user_id_str)).bind(new_workshop.participant_count.map(|_| &device_uuid_str)) // participant_count LWW
        .bind(&new_workshop.local_partner)
        .bind(new_workshop.local_partner.as_ref().map(|_| &now_str)).bind(new_workshop.local_partner.as_ref().map(|_| &user_id_str)).bind(new_workshop.local_partner.as_ref().map(|_| &device_uuid_str)) // local_partner LWW
        .bind(&new_workshop.partner_responsibility)
        .bind(new_workshop.partner_responsibility.as_ref().map(|_| &now_str)).bind(new_workshop.partner_responsibility.as_ref().map(|_| &user_id_str)).bind(new_workshop.partner_responsibility.as_ref().map(|_| &device_uuid_str)) // partner_responsibility LWW
        .bind(&new_workshop.sync_priority.to_string()) // sync_priority
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .bind(&device_uuid_str).bind(&device_uuid_str) // created_by_device_id, updated_by_device_id
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        // Fetch the created workshop to return it
        let created_workshop = self.find_by_id_with_tx(id, tx).await?;

        // Log the create operation
        let log_entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None,
            old_value: None,
            new_value: Some(serde_json::to_string(&created_workshop).unwrap_or_default()),
            timestamp: now,
            user_id: user_id,
            device_id: device_uuid_for_log, // Use the Option<Uuid> directly
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.change_log_repo.create_change_log_with_tx(&log_entry, tx).await?;

        Ok(created_workshop)
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(workshop) => {
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
                Ok(workshop)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        let user_id = auth.user_id;
        let device_uuid_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok(); // For direct Uuid use in log
        let id_str = id.to_string();

        // Fetch current state before update for comparison
        let old_entity = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut args = SqliteArguments::default(); // Use SqliteArguments

        macro_rules! add_lww_update {
            ($field:ident, $value:expr) => {
                if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?, {0}_updated_by_device_id = ?", stringify!($field)));
                    let _ = args.add(val);
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                    let _ = args.add(&device_uuid_str);
                }
            };
            ($field:ident, $value:expr, string_convert) => {
                if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?, {0}_updated_by_device_id = ?", stringify!($field)));
                    let _ = args.add(val.to_string()); // Convert to string before adding
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                    let _ = args.add(&device_uuid_str);
                }
            };
            // Handle Option<String> fields
            ($field:ident, $value:expr, option_string) => {
                if let Some(opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?, {0}_updated_by_device_id = ?", stringify!($field)));
                    let _ = args.add(opt_val);
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                    let _ = args.add(&device_uuid_str);
                }
            };
            // Handle Option<Decimal> fields
            ($field:ident, $value:expr, option_decimal) => {
                if let Some(opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?, {0}_updated_by_device_id = ?", stringify!($field)));
                    let _ = args.add(opt_val.to_string()); // Convert Decimal to String
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                    let _ = args.add(&device_uuid_str);
                }
            };
            // Handle Option<Option<Uuid>> for project_id
            ($field:ident, $value:expr, option_option_uuid) => {
                if let Some(opt_opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?, {0}_updated_by_device_id = ?", stringify!($field)));
                    let _ = args.add(opt_opt_val.map(|u| u.to_string()));
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                    let _ = args.add(&device_uuid_str);
                }
            };
        }

        // Apply the macros for each updatable field
        add_lww_update!(project_id, &update_data.project_id, option_option_uuid);
        add_lww_update!(purpose, &update_data.purpose, option_string);
        add_lww_update!(event_date, &update_data.event_date, option_string);
        add_lww_update!(location, &update_data.location, option_string);
        add_lww_update!(budget, &update_data.budget, option_decimal);
        add_lww_update!(actuals, &update_data.actuals, option_decimal);
        add_lww_update!(participant_count, &update_data.participant_count, string_convert);
        add_lww_update!(local_partner, &update_data.local_partner, option_string);
        add_lww_update!(partner_responsibility, &update_data.partner_responsibility, option_string);
        // Post-event fields
        add_lww_update!(partnership_success, &update_data.partnership_success, option_string);
        add_lww_update!(capacity_challenges, &update_data.capacity_challenges, option_string);
        add_lww_update!(strengths, &update_data.strengths, option_string);
        add_lww_update!(outcomes, &update_data.outcomes, option_string);
        add_lww_update!(recommendations, &update_data.recommendations, option_string);
        add_lww_update!(challenge_resolution, &update_data.challenge_resolution, option_string);

        if set_clauses.is_empty() {
            // No fields to update other than the main timestamp/user
            // Still need to potentially update sync_priority if passed? Check update_data definition.
            // For now, just return the old entity if nothing else changed.
            return Ok(old_entity);
        }

        // Always update the main timestamp and user ID
        set_clauses.push("updated_at = ?".to_string());
        let _ = args.add(&now_str);
        set_clauses.push("updated_by_user_id = ?".to_string());
        let _ = args.add(&user_id_str);
        set_clauses.push("updated_by_device_id = ?".to_string());
        let _ = args.add(&device_uuid_str);

        let query_str = format!(
            "UPDATE workshops SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        // Add the ID parameter last
        let _ = args.add(id_str); // Use the id string

        // Build and execute the query with arguments
        let result = sqlx::query_with(&query_str, args)
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Could be already deleted or ID not found during update
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        // Fetch the updated entity
        let new_entity = self.find_by_id_with_tx(id, tx).await?;

        // --- Start Field Change Logging ---
        macro_rules! log_field_change {
            ($field_name:expr, $old_val:expr, $new_val:expr) => {
                if $old_val != $new_val {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_name.to_string()),
                        old_value: serde_json::to_string(&$old_val).ok(),
                        new_value: serde_json::to_string(&$new_val).ok(),
                        timestamp: now,
                        user_id: user_id,
                        device_id: device_uuid_for_log, // Use the Option<Uuid> directly
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
                }
            };
        }

        log_field_change!("project_id", old_entity.project_id, new_entity.project_id);
        log_field_change!("purpose", old_entity.purpose, new_entity.purpose);
        log_field_change!("event_date", old_entity.event_date, new_entity.event_date);
        log_field_change!("location", old_entity.location, new_entity.location);
        log_field_change!("budget", old_entity.budget, new_entity.budget);
        log_field_change!("actuals", old_entity.actuals, new_entity.actuals);
        log_field_change!("participant_count", old_entity.participant_count, new_entity.participant_count);
        log_field_change!("local_partner", old_entity.local_partner, new_entity.local_partner);
        log_field_change!("partner_responsibility", old_entity.partner_responsibility, new_entity.partner_responsibility);
        log_field_change!("partnership_success", old_entity.partnership_success, new_entity.partnership_success);
        log_field_change!("capacity_challenges", old_entity.capacity_challenges, new_entity.capacity_challenges);
        log_field_change!("strengths", old_entity.strengths, new_entity.strengths);
        log_field_change!("outcomes", old_entity.outcomes, new_entity.outcomes);
        log_field_change!("recommendations", old_entity.recommendations, new_entity.recommendations);
        log_field_change!("challenge_resolution", old_entity.challenge_resolution, new_entity.challenge_resolution);
        // Note: sync_priority change is logged in update_sync_priority

        // --- End Field Change Logging ---

        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;
        
        let mut conditions = vec!["deleted_at IS NULL"];
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(p_id) = project_id {
            conditions.push("project_id = ?");
            bind_values.push(p_id.to_string());
        }
        
        let where_clause = if conditions.is_empty() { "".to_string() } else { format!("WHERE {}", conditions.join(" AND ")) };

        // Get total count with filter
        let count_query_str = format!("SELECT COUNT(*) FROM workshops {}", where_clause);
        let mut count_query = query_scalar(&count_query_str);
        for val in &bind_values {
            count_query = count_query.bind(val);
        }
        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Fetch paginated rows with filter
        let select_query_str = format!(
            "SELECT * FROM workshops {} ORDER BY event_date DESC, created_at DESC LIMIT ? OFFSET ?", 
            where_clause
        );
        let mut select_query = query_as::<_, WorkshopRow>(&select_query_str);
        for val in &bind_values {
            select_query = select_query.bind(val);
        }
        // Bind limit and offset
        select_query = select_query.bind(params.per_page as i64);
        select_query = select_query.bind(offset as i64);
        
        let rows = select_query
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
     async fn find_by_project_id(
         &self,
         project_id: Uuid,
         params: PaginationParams,
     ) -> DomainResult<PaginatedResult<Workshop>> {
         self.find_all(params, Some(project_id)).await
     }

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let device_uuid_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok(); // For direct Uuid use in log
        let user_id_str = user_id.to_string();
        
        // Convert enum to its string representation for the database
        // Assuming SyncPriority enum from sync::types has a .to_string() or .as_str() that gives "high", "normal" etc.
        let priority_val_str = priority.to_string(); 
        
        let mut builder = QueryBuilder::new("UPDATE workshops SET ");
        builder.push("sync_priority = ");
        builder.push_bind(priority_val_str.clone()); // Bind the string value
        builder.push(", updated_at = ");
        builder.push_bind(now_str.clone());
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str.clone());
        builder.push(", updated_by_device_id = ");
        builder.push_bind(device_uuid_str.clone());
        
        builder.push(" WHERE id IN (");
        let mut id_separated = builder.separated(",");
        for id in ids {
            id_separated.push_bind(id.to_string());
        }
        id_separated.push_unseparated(")");
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();
        
        if rows_affected > 0 {
            for &id in ids {
                // Fetch old priority as String from the database
                let old_priority_db_str: Option<String> = query_scalar("SELECT sync_priority FROM workshops WHERE id = ?")
                    .bind(id.to_string())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(DbError::from)?;

                let entry = ChangeLogEntry {
                    operation_id: Uuid::new_v4(),
                    entity_table: self.entity_name().to_string(),
                    entity_id: id,
                    operation_type: ChangeOperationType::Update,
                    field_name: Some("sync_priority".to_string()),
                    // Log the string value directly if it existed
                    old_value: old_priority_db_str.as_ref().map(|s| serde_json::to_string(s).ok()).flatten(),
                    // Log the new string value that was set
                    new_value: serde_json::to_string(&priority_val_str).ok(),
                    timestamp: now,
                    user_id: user_id, 
                    device_id: device_uuid_for_log, // Use the Option<Uuid> directly
                    document_metadata: None,
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                };
                self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
            }
        }

        tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        Ok(rows_affected)
    }

    async fn increment_participant_count(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            let old_entity = self.find_by_id_with_tx(id, &mut tx).await?;
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            let user_id = auth.user_id;
            let user_id_str = user_id.to_string(); // Define user_id_str here
            let device_uuid_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
            let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok(); // For direct Uuid use in log
            
            let update_result = query("UPDATE workshops SET participant_count = participant_count + 1, updated_at = ?, updated_by_user_id = ?, updated_by_device_id = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&now_str)
                .bind(&user_id_str)
                .bind(&device_uuid_str)
                .bind(id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(DbError::from)?;
                
            if update_result.rows_affected() == 0 {
                return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
            }
            
            let new_count = old_entity.participant_count + 1;
            
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("participant_count".to_string()),
                old_value: serde_json::to_string(&old_entity.participant_count).ok(),
                new_value: serde_json::to_string(&new_count).ok(),
                timestamp: now,
                user_id: user_id,
                device_id: device_uuid_for_log, // Use the Option<Uuid> directly
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
            
            Ok(())
        }.await;

        match result {
            Ok(_) => tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e))),
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn decrement_participant_count(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            let old_entity = self.find_by_id_with_tx(id, &mut tx).await?;
            let now = Utc::now();
            let now_str = now.to_rfc3339();
            let user_id = auth.user_id;
            let user_id_str = user_id.to_string(); // Define user_id_str here
            let device_uuid_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
            let device_uuid_for_log = auth.device_id.parse::<Uuid>().ok(); // For direct Uuid use in log
            
            let update_result = query("UPDATE workshops SET participant_count = MAX(0, participant_count - 1), updated_at = ?, updated_by_user_id = ?, updated_by_device_id = ? WHERE id = ? AND deleted_at IS NULL")
                 .bind(&now_str)
                .bind(&user_id_str)
                .bind(&device_uuid_str)
                .bind(id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(DbError::from)?;
                
             if update_result.rows_affected() == 0 {
                // May occur if ID doesn't exist or is already deleted
                return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
             }
             
             // Determine the new count safely
             let new_count = old_entity.participant_count.saturating_sub(1);

             // Only log if the count actually changed
             if new_count != old_entity.participant_count {
                 let entry = ChangeLogEntry {
                    operation_id: Uuid::new_v4(),
                    entity_table: self.entity_name().to_string(),
                    entity_id: id,
                    operation_type: ChangeOperationType::Update,
                    field_name: Some("participant_count".to_string()),
                    old_value: serde_json::to_string(&old_entity.participant_count).ok(),
                    new_value: serde_json::to_string(&new_count).ok(),
                    timestamp: now,
                    user_id: user_id,
                    device_id: device_uuid_for_log, // Use the Option<Uuid> directly
                    document_metadata: None,
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                 };
                 self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
             }
             Ok(())
        }.await;

        match result {
            Ok(_) => tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e))),
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn count_by_location(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT location, COUNT(*) 
             FROM workshops 
             WHERE deleted_at IS NULL 
             GROUP BY location"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_month(&self) -> DomainResult<Vec<(String, i64)>> {
        let counts = query_as::<_, (String, i64)>(
            "SELECT 
                strftime('%Y-%m', event_date) as month, 
                COUNT(*) as count
             FROM workshops 
             WHERE deleted_at IS NULL AND event_date IS NOT NULL
             GROUP BY month
             ORDER BY month"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_project(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>> {
        let project_counts = query(
            "SELECT project_id, COUNT(*) as count
             FROM workshops 
             WHERE deleted_at IS NULL 
             GROUP BY project_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Manual mapping to handle Option<Uuid>
        let mut results = Vec::new();
        for row in project_counts {
            let project_id_str: Option<String> = row.get("project_id");
            let count: i64 = row.get("count");
            
            let project_id = match project_id_str {
                Some(id_str) => Some(Uuid::parse_str(&id_str).map_err(|_| 
                    DomainError::Internal(format!("Invalid UUID in project_id: {}", id_str))
                )?),
                None => None,
            };
            
            results.push((project_id, count));
        }

        Ok(results)
    }
    
    async fn get_workshop_statistics(&self) -> DomainResult<WorkshopStatistics> {
        let today = Local::now().naive_local().date();
        
        // Get basic counts
        let (total_workshops, past_workshops, upcoming_workshops, total_participants, total_budget_str, total_actuals_str) = 
            query_as::<_, (i64, i64, i64, i64, Option<String>, Option<String>)>(
                r#"
                SELECT 
                    COUNT(*) as total,
                    COUNT(CASE WHEN event_date < ? THEN 1 END) as past,
                    COUNT(CASE WHEN event_date >= ? THEN 1 END) as upcoming,
                    SUM(participant_count) as total_participants,
                    SUM(budget) as total_budget,
                    SUM(actuals) as total_actuals
                FROM workshops
                WHERE deleted_at IS NULL
                "#
            )
            .bind(today.format("%Y-%m-%d").to_string())
            .bind(today.format("%Y-%m-%d").to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
        
        // Calculate avg participants per workshop
        let avg_participants_per_workshop = if total_workshops > 0 {
            total_participants as f64 / total_workshops as f64
        } else {
            0.0
        };
        
        // Parse decimal values
        let total_budget = match total_budget_str {
            Some(s) => Decimal::from_str(&s).unwrap_or_else(|_| Decimal::ZERO),
            None => Decimal::ZERO,
        };
        
        let total_actuals = match total_actuals_str {
            Some(s) => Decimal::from_str(&s).unwrap_or_else(|_| Decimal::ZERO),
            None => Decimal::ZERO,
        };
        
        // Get average budget variance percentage
        // Using SUM(actuals - budget) / SUM(budget) might be better than AVG of individual variances
        let (sum_variance_str, sum_budget_for_avg_str): (Option<String>, Option<String>) = query_as(
            r#"
            SELECT 
                SUM(actuals - budget) as sum_variance,
                SUM(budget) as sum_budget_for_avg
            FROM workshops
            WHERE deleted_at IS NULL AND budget > 0 AND actuals IS NOT NULL
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let avg_budget_variance = match (sum_variance_str, sum_budget_for_avg_str) {
            (Some(variance_s), Some(budget_s)) => {
                let sum_variance = Decimal::from_str(&variance_s).unwrap_or(Decimal::ZERO);
                let sum_budget = Decimal::from_str(&budget_s).unwrap_or(Decimal::ONE); // Avoid div by zero
                if sum_budget.is_zero() { Decimal::ZERO } else { (sum_variance / sum_budget) * dec!(100.0) }
            },
            _ => Decimal::ZERO
        };

        // Get location distribution
        let location_counts = self.count_by_location().await?;
        let mut by_location = HashMap::new();
        for (location_opt, count) in location_counts {
            let location_name = location_opt.unwrap_or_else(|| "Unspecified".to_string());
            by_location.insert(location_name, count);
        }
        
        // Get month distribution
        let month_counts = self.count_by_month().await?;
        let mut by_month = HashMap::new();
        for (month, count) in month_counts {
            by_month.insert(month, count);
        }
        
        // Get project distribution
        let project_counts = self.count_by_project().await?;
        let mut by_project = HashMap::new();
        for (project_id_opt, count) in project_counts {
            if let Some(project_id) = project_id_opt {
                by_project.insert(project_id, count);
            }
        }
        
        Ok(WorkshopStatistics {
            total_workshops,
            past_workshops,
            upcoming_workshops,
            total_participants,
            avg_participants_per_workshop,
            total_budget,
            total_actuals,
            avg_budget_variance, // Average variance percentage
            by_location,
            by_month,
            by_project,
        })
    }
    
    async fn find_by_date_range(
        &self,
        start_date: NaiveDate, 
        end_date: NaiveDate,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;
        let start_date_str = start_date.format("%Y-%m-%d").to_string();
        let end_date_str = end_date.format("%Y-%m-%d").to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM workshops 
             WHERE event_date >= ? AND event_date <= ? AND deleted_at IS NULL"
        )
        .bind(&start_date_str)
        .bind(&end_date_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops 
             WHERE event_date >= ? AND event_date <= ? AND deleted_at IS NULL 
             ORDER BY event_date ASC LIMIT ? OFFSET ?"
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
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_past_workshops(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM workshops 
             WHERE event_date < ? AND deleted_at IS NULL"
        )
        .bind(&today)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops 
             WHERE event_date < ? AND deleted_at IS NULL 
             ORDER BY event_date DESC LIMIT ? OFFSET ?"
        )
        .bind(&today)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_upcoming_workshops(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM workshops 
             WHERE event_date >= ? AND deleted_at IS NULL"
        )
        .bind(&today)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops 
             WHERE event_date >= ? AND deleted_at IS NULL 
             ORDER BY event_date ASC LIMIT ? OFFSET ?"
        )
        .bind(&today)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_location(
        &self,
        location: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM workshops 
             WHERE location = ? AND deleted_at IS NULL"
        )
        .bind(location)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops 
             WHERE location = ? AND deleted_at IS NULL 
             ORDER BY event_date DESC LIMIT ? OFFSET ?"
        )
        .bind(location)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn get_budget_statistics(
        &self,
        project_id: Option<Uuid>,
    ) -> DomainResult<(Decimal, Decimal, Decimal, f64)> {
        // Build the query based on whether we have a project_id filter
        let (query_str, project_id_str) = if let Some(pid) = project_id {
            (
                r#"
                SELECT 
                    COALESCE(SUM(budget), 0) as total_budget,
                    COALESCE(SUM(actuals), 0) as total_actuals,
                    COALESCE(SUM(actuals - budget), 0) as total_variance,
                    AVG(CASE WHEN budget > 0 THEN ((actuals - budget) * 100.0 / budget) ELSE NULL END) as avg_variance_pct
                FROM workshops
                WHERE deleted_at IS NULL AND project_id = ?
                "#,
                Some(pid.to_string())
            )
        } else {
            (
                r#"
                SELECT 
                    COALESCE(SUM(budget), 0) as total_budget,
                    COALESCE(SUM(actuals), 0) as total_actuals,
                    COALESCE(SUM(actuals - budget), 0) as total_variance,
                    AVG(CASE WHEN budget > 0 THEN ((actuals - budget) * 100.0 / budget) ELSE NULL END) as avg_variance_pct
                FROM workshops
                WHERE deleted_at IS NULL
                "#,
                None
            )
        };
        
        // Execute the query
        let mut query = sqlx::query(query_str);
        if let Some(pid_str) = project_id_str {
            query = query.bind(pid_str);
        }
        
        let row = query.fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        // Extract and parse results
        let total_budget_str: String = row.get("total_budget");
        let total_actuals_str: String = row.get("total_actuals");
        let total_variance_str: String = row.get("total_variance");
        let avg_variance_pct: Option<f64> = row.get("avg_variance_pct");
        
        let total_budget = Decimal::from_str(&total_budget_str)
            .map_err(|_| DomainError::Internal(format!("Failed to parse budget value: {}", total_budget_str)))?;
            
        let total_actuals = Decimal::from_str(&total_actuals_str)
            .map_err(|_| DomainError::Internal(format!("Failed to parse actuals value: {}", total_actuals_str)))?;
            
        let total_variance = Decimal::from_str(&total_variance_str)
            .map_err(|_| DomainError::Internal(format!("Failed to parse variance value: {}", total_variance_str)))?;
            
        Ok((total_budget, total_actuals, total_variance, avg_variance_pct.unwrap_or(0.0)))
    }
    
    async fn get_workshop_budget_summaries_for_project(
        &self,
        project_id: Uuid,
    ) -> DomainResult<Vec<WorkshopBudgetSummary>> {
        let project_id_str = project_id.to_string();
        
        // Query summaries
        let rows = query(
            r#"
            SELECT 
                id, 
                purpose, 
                event_date, 
                budget, 
                actuals
            FROM 
                workshops
            WHERE 
                project_id = ? AND deleted_at IS NULL
            ORDER BY 
                event_date DESC
            "#
        )
        .bind(&project_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut summaries = Vec::new();
        for row in rows {
            let id = Uuid::parse_str(row.get::<String, _>("id").as_str()).map_err(|_| {
                DomainError::Internal("Invalid UUID format in workshop id".to_string())
            })?;
            
            let budget_str: Option<String> = row.get("budget");
            let actuals_str: Option<String> = row.get("actuals");
            
            let budget = budget_str.map(|s| Decimal::from_str(&s)
                .map_err(|_| DomainError::Internal(format!("Invalid decimal format: {}", s)))
            ).transpose()?;
            
            let actuals = actuals_str.map(|s| Decimal::from_str(&s)
                .map_err(|_| DomainError::Internal(format!("Invalid decimal format: {}", s)))
            ).transpose()?;
            
            // Calculate variance and percentage if possible
            let variance = match (budget, actuals) {
                (Some(b), Some(a)) => Some(a - b),
                _ => None,
            };
            
            let variance_percentage = match (budget, actuals) {
                (Some(b), Some(a)) if !b.is_zero() => {
                    Some(((a - b) * Decimal::from(100)) / b)
                },
                _ => None,
            };
            
            summaries.push(WorkshopBudgetSummary {
                workshop_id: id,
                purpose: row.get("purpose"),
                event_date: row.get("event_date"),
                budget,
                actuals,
                variance,
                variance_percentage,
            });
        }
        
        Ok(summaries)
    }
    
    async fn get_project_workshop_metrics(
        &self,
        project_id: Uuid,
    ) -> DomainResult<ProjectWorkshopMetrics> {
        let project_id_str = project_id.to_string();
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();
        
        // Get project name first
        let project_name = query_scalar::<_, String>(
            "SELECT name FROM projects WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&project_id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Project".to_string(), project_id))?;
        
        // Get workshop counts
        let (total_workshops, completed_workshops, upcoming_workshops, total_participants) = 
            query_as::<_, (i64, i64, i64, i64)>(
                r#"
                SELECT 
                    COUNT(*) as total,
                    COUNT(CASE WHEN event_date < ? THEN 1 END) as completed,
                    COUNT(CASE WHEN event_date >= ? THEN 1 END) as upcoming,
                    COALESCE(SUM(participant_count), 0) as total_participants
                FROM workshops
                WHERE project_id = ? AND deleted_at IS NULL
                "#
            )
            .bind(&today)
            .bind(&today)
            .bind(&project_id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
        
        // Get budget stats
        let (total_budget, total_actuals, budget_variance, _) = 
            self.get_budget_statistics(Some(project_id)).await?;
        
        // Get workshops by month
        let month_counts = query_as::<_, (String, i64)>(
            r#"
            SELECT 
                strftime('%Y-%m', event_date) as month, 
                COUNT(*) as count
            FROM workshops 
            WHERE project_id = ? AND deleted_at IS NULL AND event_date IS NOT NULL
            GROUP BY month
            ORDER BY month
            "#
        )
        .bind(&project_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut workshops_by_month = HashMap::new();
        for (month, count) in month_counts {
            workshops_by_month.insert(month, count);
        }
        
        Ok(ProjectWorkshopMetrics {
            project_id,
            project_name,
            total_workshops,
            completed_workshops,
            upcoming_workshops,
            total_participants,
            total_budget,
            total_actuals,
            budget_variance,
            workshops_by_month,
        })
    }
}

// === Sync Merge Implementation ===
#[async_trait]
impl MergeableEntityRepository<Workshop> for SqliteWorkshopRepository {
    fn entity_name(&self) -> &'static str { "workshops" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for workshop change")))?;
                let remote_state: Workshop = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_workshop", &format!("Invalid JSON: {}", e))))?;
                let local_opt = match self.find_by_id_with_tx(remote_state.id, tx).await {
                    Ok(ent) => Some(ent),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };
                if let Some(local) = local_opt.clone() {
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local copy newer or equal".into()));
                    }
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Updated(remote_state.id))
                } else {
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Created(remote_state.id))
                }
            }
            ChangeOperationType::Delete => Ok(MergeOutcome::NoOp("Remote soft delete ignored".into())),
            ChangeOperationType::HardDelete => Ok(MergeOutcome::HardDeleted(remote_change.entity_id)),
        }
    }
}

impl SqliteWorkshopRepository {
    async fn upsert_remote_state_with_tx<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote: &Workshop,
    ) -> DomainResult<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO workshops (
                id, project_id,
                purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id,
                event_date, event_date_updated_at, event_date_updated_by, event_date_updated_by_device_id,
                location, location_updated_at, location_updated_by, location_updated_by_device_id,
                budget, budget_updated_at, budget_updated_by, budget_updated_by_device_id,
                actuals, actuals_updated_at, actuals_updated_by, actuals_updated_by_device_id,
                participant_count, participant_count_updated_at, participant_count_updated_by, participant_count_updated_by_device_id,
                local_partner, local_partner_updated_at, local_partner_updated_by, local_partner_updated_by_device_id,
                partner_responsibility, partner_responsibility_updated_at, partner_responsibility_updated_by, partner_responsibility_updated_by_device_id,
                partnership_success, partnership_success_updated_at, partnership_success_updated_by, partnership_success_updated_by_device_id,
                capacity_challenges, capacity_challenges_updated_at, capacity_challenges_updated_by, capacity_challenges_updated_by_device_id,
                strengths, strengths_updated_at, strengths_updated_by, strengths_updated_by_device_id,
                outcomes, outcomes_updated_at, outcomes_updated_by, outcomes_updated_by_device_id,
                recommendations, recommendations_updated_at, recommendations_updated_by, recommendations_updated_by_device_id,
                challenge_resolution, challenge_resolution_updated_at, challenge_resolution_updated_by, challenge_resolution_updated_by_device_id,
                sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            "#,
        )
        .bind(remote.id.to_string())
        .bind(remote.project_id.map(|id| id.to_string()))
        .bind(&remote.purpose)
        .bind(remote.purpose_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.purpose_updated_by.map(|id| id.to_string()))
        .bind(remote.purpose_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.event_date)
        .bind(remote.event_date_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.event_date_updated_by.map(|id| id.to_string()))
        .bind(remote.event_date_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.location)
        .bind(remote.location_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.location_updated_by.map(|id| id.to_string()))
        .bind(remote.location_updated_by_device_id.map(|id| id.to_string()))
        .bind(remote.budget.map(|d| d.to_string()))
        .bind(remote.budget_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.budget_updated_by.map(|id| id.to_string()))
        .bind(remote.budget_updated_by_device_id.map(|id| id.to_string()))
        .bind(remote.actuals.map(|d| d.to_string()))
        .bind(remote.actuals_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.actuals_updated_by.map(|id| id.to_string()))
        .bind(remote.actuals_updated_by_device_id.map(|id| id.to_string()))
        .bind(remote.participant_count)
        .bind(remote.participant_count_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.participant_count_updated_by.map(|id| id.to_string()))
        .bind(remote.participant_count_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.local_partner)
        .bind(remote.local_partner_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.local_partner_updated_by.map(|id| id.to_string()))
        .bind(remote.local_partner_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.partner_responsibility)
        .bind(remote.partner_responsibility_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.partner_responsibility_updated_by.map(|id| id.to_string()))
        .bind(remote.partner_responsibility_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.partnership_success)
        .bind(remote.partnership_success_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.partnership_success_updated_by.map(|id| id.to_string()))
        .bind(remote.partnership_success_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.capacity_challenges)
        .bind(remote.capacity_challenges_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.capacity_challenges_updated_by.map(|id| id.to_string()))
        .bind(remote.capacity_challenges_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.strengths)
        .bind(remote.strengths_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.strengths_updated_by.map(|id| id.to_string()))
        .bind(remote.strengths_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.outcomes)
        .bind(remote.outcomes_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.outcomes_updated_by.map(|id| id.to_string()))
        .bind(remote.outcomes_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.recommendations)
        .bind(remote.recommendations_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.recommendations_updated_by.map(|id| id.to_string()))
        .bind(remote.recommendations_updated_by_device_id.map(|id| id.to_string()))
        .bind(&remote.challenge_resolution)
        .bind(remote.challenge_resolution_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.challenge_resolution_updated_by.map(|id| id.to_string()))
        .bind(remote.challenge_resolution_updated_by_device_id.map(|id| id.to_string()))
        .bind(remote.sync_priority.as_str())
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|id| id.to_string()))
        .bind(remote.updated_by_user_id.map(|id| id.to_string()))
        .bind(remote.created_by_device_id.map(|id| id.to_string()))
        .bind(remote.updated_by_device_id.map(|id| id.to_string()))
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|id| id.to_string()))
        .bind(remote.deleted_by_device_id.map(|id| id.to_string()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}
