use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, QueryBuilder};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::activity::types::{NewActivity, Activity, ActivityRow, UpdateActivity, ActivityDocumentReference, ActivityFilter, ActivityStatistics, ActivityStatusBreakdown, ActivityMetadataCounts};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use std::sync::Arc;
use std::collections::HashMap;
use serde_json;
use chrono::DateTime;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::domains::user::repository::MergeableEntityRepository;

/// Trait defining activity repository operations
#[async_trait]
pub trait ActivityRepository: DeleteServiceRepository<Activity> + MergeableEntityRepository<Activity> + Send + Sync {
    async fn create(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity>;
    async fn create_with_tx<'t>(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    /// Find activities within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    /// Find activities by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    /// Get document references for an activity
    async fn get_activity_document_references(
        &self,
        activity_id: Uuid,
    ) -> DomainResult<Vec<ActivityDocumentReference>>;

    /// Find activities by status ID
    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    /// Search activities by description, KPI, or other text fields
    async fn search_activities(
        &self,
        query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;

    /// Find activity IDs that match complex filter criteria
    /// This is the key to enabling efficient UI-driven bulk selections and exports
    async fn find_ids_by_filter(
        &self,
        filter: ActivityFilter,
    ) -> DomainResult<Vec<Uuid>>;

    /// Bulk update activity status for multiple activities
    async fn bulk_update_status(
        &self,
        ids: &[Uuid],
        status_id: i64,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    /// Count activities by status
    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>>;
    
    /// Count activities by project
    async fn count_by_project(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>>;
    
    /// Get comprehensive activity statistics
    async fn get_activity_statistics(&self) -> DomainResult<ActivityStatistics>;
    
    /// Get activity status breakdown
    async fn get_activity_status_breakdown(&self) -> DomainResult<Vec<ActivityStatusBreakdown>>;
    
    /// Get activity metadata counts
    async fn get_activity_metadata_counts(&self) -> DomainResult<ActivityMetadataCounts>;
}

/// SQLite implementation for ActivityRepository
#[derive(Clone)]
pub struct SqliteActivityRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteActivityRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: ActivityRow) -> DomainResult<Activity> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "activities"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        let row = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Activity".to_string(), id))?;

        Self::map_row_to_entity(row)
    }

    async fn log_change_entry<'t>(
        &self,
        entry: ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await
    }
}

#[async_trait]
impl FindById<Activity> for SqliteActivityRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Activity> {
        let row = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Activity".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteActivityRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
     
        
        let result = query(
            "UPDATE activities SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now_str)
        .bind(user_id_str)
        .bind(device_id_str)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Activity".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.soft_delete_with_tx(id, auth, &mut tx).await {
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
impl HardDeletable for SqliteActivityRepository {
    fn entity_name(&self) -> &'static str {
        "activities"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM activities WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Activity".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
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

// Blanket implementation in core::delete_service handles DeleteServiceRepository

#[async_trait]
impl ActivityRepository for SqliteActivityRepository {
    async fn create(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity> {
        println!("🗃️ [ACTIVITY_REPO] create called");
        println!("🗃️ [ACTIVITY_REPO] new_activity: {:?}", new_activity);
        println!("🗃️ [ACTIVITY_REPO] auth.user_id: {}", auth.user_id);
        
        println!("🗃️ [ACTIVITY_REPO] Starting database transaction...");
        let mut tx = self.pool.begin().await.map_err(|e| {
            println!("❌ [ACTIVITY_REPO] Failed to begin transaction: {:?}", e);
            DbError::from(e)
        })?;
        
        match self.create_with_tx(new_activity, auth, &mut tx).await {
            Ok(activity) => {
                println!("✅ [ACTIVITY_REPO] create_with_tx successful, committing transaction...");
                tx.commit().await.map_err(|e| {
                    println!("❌ [ACTIVITY_REPO] Failed to commit transaction: {:?}", e);
                    DbError::from(e)
                })?;
                println!("✅ [ACTIVITY_REPO] Transaction committed successfully! Activity ID: {}", activity.id);
                Ok(activity)
            }
            Err(e) => {
                println!("❌ [ACTIVITY_REPO] create_with_tx failed: {:?}", e);
                println!("🗃️ [ACTIVITY_REPO] Rolling back transaction...");
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        println!("🗃️ [ACTIVITY_REPO] create_with_tx called");
        
        let id = Uuid::new_v4();
        println!("🗃️ [ACTIVITY_REPO] Generated new activity ID: {}", id);
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id; // Get Uuid directly
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let project_id_str = new_activity.project_id.map(|id| id.to_string());
        
        println!("🗃️ [ACTIVITY_REPO] Prepared data: project_id={:?}, user_id={}, device_id={:?}", 
                 project_id_str, user_id_str, device_id_str);
        println!("🗃️ [ACTIVITY_REPO] Activity fields: description={:?}, kpi={:?}, target_value={:?}, actual_value={:?}, status_id={:?}",
                 new_activity.description, new_activity.kpi, new_activity.target_value, new_activity.actual_value, new_activity.status_id);

        query(
            r#"
            INSERT INTO activities (
                id, project_id,
                description, description_updated_at, description_updated_by, description_updated_by_device_id,
                kpi, kpi_updated_at, kpi_updated_by, kpi_updated_by_device_id,
                target_value, target_value_updated_at, target_value_updated_by, target_value_updated_by_device_id,
                actual_value, actual_value_updated_at, actual_value_updated_by, actual_value_updated_by_device_id,
                status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?, ?,
                ?, 
                ?, ?, ?, ?, 
                ?, ?,
                NULL, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(project_id_str)
        .bind(&new_activity.description).bind(new_activity.description.as_ref().map(|_| &now_str)).bind(new_activity.description.as_ref().map(|_| &user_id_str)).bind(new_activity.description.as_ref().map(|_| &device_id_str))
        .bind(&new_activity.kpi).bind(new_activity.kpi.as_ref().map(|_| &now_str)).bind(new_activity.kpi.as_ref().map(|_| &user_id_str)).bind(new_activity.kpi.as_ref().map(|_| &device_id_str))
        .bind(new_activity.target_value).bind(new_activity.target_value.map(|_| &now_str)).bind(new_activity.target_value.map(|_| &user_id_str)).bind(new_activity.target_value.map(|_| &device_id_str))
        .bind(new_activity.actual_value.unwrap_or(0.0)).bind(new_activity.actual_value.map(|_| &now_str)).bind(new_activity.actual_value.map(|_| &user_id_str)).bind(new_activity.actual_value.map(|_| &device_id_str))
        .bind(new_activity.status_id)
        .bind(new_activity.status_id.map(|_| &now_str))
        .bind(new_activity.status_id.map(|_| &user_id_str))
        .bind(new_activity.status_id.map(|_| &device_id_str))
        .bind(new_activity.sync_priority.as_str())
        .bind(&now_str).bind(&now_str)
        .bind(&user_id_str).bind(&user_id_str)
        .bind(&device_id_str).bind(&device_id_str) // created_by_device_id, updated_by_device_id
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            println!("❌ [ACTIVITY_REPO] INSERT query failed: {:?}", e);
            DbError::from(e)
        })?;
        
        println!("✅ [ACTIVITY_REPO] INSERT query successful!");

        // Log Create Operation
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None,
            old_value: None,
            new_value: None,
            timestamp: now, // Use DateTime<Utc>
            user_id: user_id,
            device_id: device_uuid,
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        println!("🗃️ [ACTIVITY_REPO] Logging change entry...");
        self.log_change_entry(entry, tx).await.map_err(|e| {
            println!("❌ [ACTIVITY_REPO] Failed to log change entry: {:?}", e);
            e
        })?; // Add log call here

        println!("🗃️ [ACTIVITY_REPO] Finding created activity by ID...");
        let result = self.find_by_id_with_tx(id, tx).await;
        match &result {
            Ok(activity) => {
                println!("✅ [ACTIVITY_REPO] create_with_tx completed successfully! Activity: {:?}", activity);
            }
            Err(e) => {
                println!("❌ [ACTIVITY_REPO] Failed to find created activity: {:?}", e);
            }
        }
        result
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.update_with_tx(id, update_data, auth, &mut tx).await {
            Ok(activity) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(activity)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        // Fetch old state first
        let old_entity = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());

        let mut builder = QueryBuilder::new("UPDATE activities SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value { // Check if update DTO contains field
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone()); // Bind value
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone()); // Bind timestamp
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone()); // Bind user
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_id_str.clone()); // Bind device_id
                    fields_updated = true; // Mark SQL update needed
                }
            };
            // Variant for Option<Uuid> like project_id
            ($field_name:ident, $field_sql:literal, $value:expr, uuid_opt) => {
                if let Some(opt_val) = $value { // Check if update DTO contains field (even if inner value is None)
                    let val_str = opt_val.map(|id| id.to_string());
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val_str); // Bind Option<String>
                    // FK changes don't have specific LWW columns in schema, rely on main updated_at
                    // However, if we want to track device_id for FK changes, it needs separate handling
                    // For now, assuming FK changes don't have _updated_by_device_id specific columns
                    fields_updated = true; // Mark SQL update needed
                }
            };
        }

        // Apply updates using macros
        add_lww!(project_id, "project_id", &update_data.project_id, uuid_opt);
        add_lww!(description, "description", &update_data.description.as_ref());
        add_lww!(kpi, "kpi", &update_data.kpi.as_ref());
        add_lww!(target_value, "target_value", &update_data.target_value.as_ref());
        add_lww!(actual_value, "actual_value", &update_data.actual_value.as_ref());
        add_lww!(status_id, "status_id", &update_data.status_id.as_ref());
        // Update sync_priority if provided
        if let Some(sp) = &update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(sp.as_str());
            fields_updated = true;
        }

        if !fields_updated {
            return Ok(old_entity); // No fields present in DTO
        }

        // Always update main timestamps
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str.clone());
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());

        // Finalize and Execute SQL
        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");
        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;
        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        // Fetch New State & Log Field Changes
        let new_entity = self.find_by_id_with_tx(id, tx).await?;

        macro_rules! log_if_changed {
            ($field_name:ident, $field_sql:literal) => {
                if old_entity.$field_name != new_entity.$field_name {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_sql.to_string()),
                        old_value: serde_json::to_string(&old_entity.$field_name).ok(),
                        new_value: serde_json::to_string(&new_entity.$field_name).ok(),
                        timestamp: now,
                        user_id: user_id,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.log_change_entry(entry, tx).await?;
                }
            };
        }

        log_if_changed!(project_id, "project_id");
        log_if_changed!(description, "description");
        log_if_changed!(kpi, "kpi");
        log_if_changed!(target_value, "target_value");
        log_if_changed!(actual_value, "actual_value");
        log_if_changed!(status_id, "status_id");
        // Log sync_priority change
        if old_entity.sync_priority != new_entity.sync_priority {
             let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("sync_priority".to_string()),
                old_value: serde_json::to_string(old_entity.sync_priority.as_str()).ok(),
                new_value: serde_json::to_string(new_entity.sync_priority.as_str()).ok(),
                timestamp: now,
                user_id: user_id,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, tx).await?;
        }
        
        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM activities WHERE deleted_at IS NULL"
         )
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE deleted_at IS NULL ORDER BY created_at ASC LIMIT ? OFFSET ?",
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find activities within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM activities WHERE deleted_at IS NULL AND (created_at BETWEEN ? AND ? OR updated_at BETWEEN ? AND ?)"
         )
         .bind(start_date.to_rfc3339())
         .bind(end_date.to_rfc3339())
         .bind(start_date.to_rfc3339())
         .bind(end_date.to_rfc3339())
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE deleted_at IS NULL AND (created_at BETWEEN ? AND ? OR updated_at BETWEEN ? AND ?) ORDER BY created_at ASC LIMIT ? OFFSET ?",
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find activities by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        let offset = (params.page - 1) * params.per_page;

        // Build COUNT query with dynamic placeholders
        let count_placeholders = vec!["?"; ids.len()].join(", ");
        let count_query = format!(
            "SELECT COUNT(*) FROM activities WHERE id IN ({}) AND deleted_at IS NULL",
            count_placeholders
        );

        let mut count_builder = QueryBuilder::new(&count_query);
        for id in ids {
            count_builder.push_bind(id.to_string());
        }

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Build SELECT query with dynamic placeholders
        let select_placeholders = vec!["?"; ids.len()].join(", ");
        let select_query = format!(
            "SELECT * FROM activities WHERE id IN ({}) AND deleted_at IS NULL ORDER BY created_at ASC LIMIT ? OFFSET ?",
            select_placeholders
        );

        let mut select_builder = QueryBuilder::new(&select_query);
        for id in ids {
            select_builder.push_bind(id.to_string());
        }
        select_builder.push_bind(params.per_page as i64);
        select_builder.push_bind(offset as i64);

        let rows = select_builder
            .build_query_as::<ActivityRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Activity>>>()?;

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
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;
        let project_id_str = project_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM activities WHERE project_id = ? AND deleted_at IS NULL"
         )
         .bind(&project_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at ASC LIMIT ? OFFSET ?",
        )
        .bind(project_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn get_activity_document_references(
        &self,
        activity_id: Uuid,
    ) -> DomainResult<Vec<ActivityDocumentReference>> {
        let activity_id_str = activity_id.to_string();
        
        let mut references = Vec::new();
        let doc_ref_fields: Vec<_> = Activity::field_metadata()
            .into_iter()
            .filter(|field| field.is_document_reference_only)
            .collect();
            
        for field in doc_ref_fields {
            let column_name = format!("{}_ref", field.field_name);
            
            // Query to fetch document details directly
            let query_str = format!(
                "SELECT a.{} as doc_id, m.original_filename, m.created_at, m.size_bytes as file_size 
                 FROM activities a 
                 LEFT JOIN media_documents m ON a.{} = m.id AND m.deleted_at IS NULL
                 WHERE a.id = ? AND a.deleted_at IS NULL", 
                column_name, column_name
            );
            
            let row = query(&query_str)
                .bind(&activity_id_str)
                .fetch_optional(&self.pool)
                .await
                .map_err(DbError::from)?;
                
            if let Some(row) = row {
                let doc_id_str: Option<String> = row.get("doc_id");
                let doc_id = doc_id_str.map(|id_str| 
                    Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::Internal(format!("Invalid UUID: {}", id_str)))
                ).transpose()?;
                
                let (filename, upload_date, file_size) = if doc_id.is_some() {
                    (
                        row.get("original_filename"),
                        row.get::<Option<String>, _>("created_at").map(|dt_str| 
                            DateTime::parse_from_rfc3339(&dt_str)
                                .map_err(|_| DomainError::Internal(format!("Invalid datetime: {}", dt_str)))
                                .map(|dt| dt.with_timezone(&Utc))
                        ).transpose()?,
                        row.get::<Option<i64>, _>("file_size").map(|fs| fs as u64),
                    )
                } else {
                    (None, None, None)
                };
                
                references.push(ActivityDocumentReference {
                    field_name: field.field_name.to_string(),
                    display_name: field.display_name.to_string(),
                    document_id: doc_id,
                    filename,
                    upload_date,
                    file_size,
                });
            }
        }
        
        Ok(references)
    }

    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities WHERE status_id = ? AND deleted_at IS NULL"
        )
        .bind(status_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities 
             WHERE status_id = ? AND deleted_at IS NULL 
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
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
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn search_activities(
        &self,
        query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;
        let search_term = format!("%{}%", query);

        // Get total count - search in description and kpi fields
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities 
             WHERE (description LIKE ? OR kpi LIKE ?) 
             AND deleted_at IS NULL"
        )
        .bind(&search_term)
        .bind(&search_term)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities 
             WHERE (description LIKE ? OR kpi LIKE ?) 
             AND deleted_at IS NULL 
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(&search_term)
        .bind(&search_term)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_ids_by_filter(
        &self,
        filter: ActivityFilter,
    ) -> DomainResult<Vec<Uuid>> {
        use sqlx::QueryBuilder;
        
        let mut query_builder = QueryBuilder::new("SELECT a.id FROM activities a WHERE 1=1");
        
        if filter.exclude_deleted.unwrap_or(true) {
            query_builder.push(" AND a.deleted_at IS NULL");
        }
        
        if let Some(status_ids) = &filter.status_ids {
            if !status_ids.is_empty() {
                query_builder.push(" AND a.status_id IN (");
                let mut separated = query_builder.separated(", ");
                for status_id in status_ids {
                    separated.push_bind(status_id);
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(project_ids) = &filter.project_ids {
            if !project_ids.is_empty() {
                query_builder.push(" AND a.project_id IN (");
                let mut separated = query_builder.separated(", ");
                for project_id in project_ids {
                    separated.push_bind(project_id.to_string());
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(search_text) = &filter.search_text {
            if !search_text.trim().is_empty() {
                let search_pattern = format!("%{}%", search_text.trim());
                query_builder.push(" AND (a.description LIKE ")
                    .push_bind(search_pattern.clone())
                    .push(" OR a.kpi LIKE ")
                    .push_bind(search_pattern)
                    .push(")");
            }
        }
        
        if let Some((start_date, end_date)) = &filter.date_range {
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(start_date),
                DateTime::parse_from_rfc3339(end_date)
            ) {
                let start_utc = start.with_timezone(&Utc);
                let end_utc = end.with_timezone(&Utc);
                
                query_builder.push(" AND a.updated_at BETWEEN ")
                    .push_bind(start_utc.to_rfc3339())
                    .push(" AND ")
                    .push_bind(end_utc.to_rfc3339());
            }
        }
        
        if let Some((min_target, max_target)) = &filter.target_value_range {
            query_builder.push(" AND a.target_value BETWEEN ")
                .push_bind(min_target)
                .push(" AND ")
                .push_bind(max_target);
        }
        
        if let Some((min_actual, max_actual)) = &filter.actual_value_range {
            query_builder.push(" AND a.actual_value BETWEEN ")
                .push_bind(min_actual)
                .push(" AND ")
                .push_bind(max_actual);
        }
        
        let query = query_builder.build_query_as::<(String,)>();
        let rows = query.fetch_all(&self.pool).await.map_err(DbError::from)?;
        
        rows.into_iter()
            .map(|(id_str,)| Uuid::parse_str(&id_str).map_err(|e| DomainError::InvalidUuid(e.to_string())))
            .collect()
    }

    async fn bulk_update_status(
        &self,
        ids: &[Uuid],
        status_id: i64,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = if auth.device_id.is_empty() {
            None
        } else {
            Some(auth.device_id.clone())
        };

        // Build query using QueryBuilder pattern from project repository
        let mut update_builder = QueryBuilder::new("UPDATE activities SET ");
        update_builder.push("status_id = ").push_bind(status_id);
        update_builder.push(", status_id_updated_at = ").push_bind(&now_str);
        update_builder.push(", status_id_updated_by = ").push_bind(&user_id_str);
        update_builder.push(", status_id_updated_by_device_id = ").push_bind(&device_id_str);
        update_builder.push(", updated_at = ").push_bind(&now_str);
        update_builder.push(", updated_by_user_id = ").push_bind(&user_id_str);
        update_builder.push(", updated_by_device_id = ").push_bind(&device_id_str);
        update_builder.push(" WHERE id IN (");
        let mut id_separated = update_builder.separated(",");
        for id in ids {
            id_separated.push_bind(id.to_string());
        }
        update_builder.push(") AND deleted_at IS NULL");

        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;

        // Log changes for each updated activity
        for id in ids {
            let change = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: "activities".to_string(),
                entity_id: *id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("status_id".to_string()),
                old_value: None, // We'd need to fetch this beforehand if required
                new_value: Some(status_id.to_string()),
                document_metadata: None,
                timestamp: now,
                user_id: auth.user_id,
                device_id: if auth.device_id.is_empty() { 
                    None 
                } else { 
                    auth.device_id.parse().ok() 
                },
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };

            self.change_log_repo
                .create_change_log_with_tx(&change, &mut tx)
                .await
                .map_err(DomainError::from)?;
        }

        tx.commit().await.map_err(DbError::from)?;
        Ok(result.rows_affected())
    }

    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>> {
        let counts = query_as::<_, (Option<i64>, i64)>(
            "SELECT status_id, COUNT(*) 
             FROM activities 
             WHERE deleted_at IS NULL 
             GROUP BY status_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_project(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>> {
        let rows = query(
            "SELECT project_id, COUNT(*) as count
             FROM activities 
             WHERE deleted_at IS NULL 
             GROUP BY project_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Manual mapping to handle Option<Uuid>
        let mut results = Vec::new();
        for row in rows {
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
    
    async fn get_activity_statistics(&self) -> DomainResult<ActivityStatistics> {
        // Get total activity count
        let total_activities: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Get document count
        let document_count: i64 = query_scalar(
            "SELECT COUNT(*) 
             FROM media_documents 
             WHERE related_table = 'activities'
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Get status distribution
        let status_counts = self.count_by_status().await?;
        let mut by_status = HashMap::new();
        for (status_id_opt, count) in status_counts {
            let status_name = match status_id_opt {
                Some(1) => "Not Started".to_string(),
                Some(2) => "In Progress".to_string(),
                Some(3) => "Completed".to_string(),
                Some(4) => "On Hold".to_string(),
                Some(id) => format!("Status {}", id),
                None => "Unspecified".to_string(),
            };
            by_status.insert(status_name, count);
        }
        
        // Get project distribution
        let project_counts = self.count_by_project().await?;
        let mut by_project = HashMap::new();
        for (project_id_opt, count) in project_counts {
            let project_name = match project_id_opt {
                Some(id) => {
                    match query_scalar::<_, String>(
                        "SELECT name FROM projects WHERE id = ? AND deleted_at IS NULL"
                    )
                    .bind(id.to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(DbError::from)? {
                        Some(name) => name,
                        None => format!("Project {}", id),
                    }
                },
                None => "No Project".to_string(),
            };
            by_project.insert(project_name, count);
        }
        
        // Calculate completion rate (activities with status_id = 3)
        let completed_count = by_status.get("Completed").unwrap_or(&0);
        let completion_rate = if total_activities > 0 {
            (*completed_count as f64 / total_activities as f64) * 100.0
        } else {
            0.0
        };
        
        // Calculate average progress (based on actual vs target values)
        let avg_progress_opt: Option<f64> = query_scalar(
            "SELECT AVG(CASE 
                WHEN target_value > 0 THEN (actual_value / target_value) * 100.0 
                ELSE NULL 
             END) 
             FROM activities 
             WHERE deleted_at IS NULL 
             AND target_value IS NOT NULL 
             AND actual_value IS NOT NULL"
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .flatten();
        
        let average_progress = avg_progress_opt.unwrap_or(0.0);
        
        Ok(ActivityStatistics {
            total_activities,
            by_status,
            by_project,
            completion_rate,
            average_progress,
            document_count,
        })
    }
    
    async fn get_activity_status_breakdown(&self) -> DomainResult<Vec<ActivityStatusBreakdown>> {
        // Get status counts
        let status_counts = self.count_by_status().await?;
        
        // Get total count for percentage calculation
        let total: i64 = status_counts.iter().map(|(_, count)| count).sum();
        
        // Create breakdown objects
        let mut breakdown = Vec::new();
        for (status_id_opt, count) in status_counts {
            let status_id = status_id_opt.unwrap_or(0);
            let status_name = match status_id {
                1 => "Not Started".to_string(),
                2 => "In Progress".to_string(),
                3 => "Completed".to_string(),
                4 => "On Hold".to_string(),
                _ => "Unknown".to_string(),
            };
            
            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            
            breakdown.push(ActivityStatusBreakdown {
                status_id,
                status_name,
                count,
                percentage,
            });
        }
        
        // Sort by status ID for consistent order
        breakdown.sort_by_key(|b| b.status_id);
        
        Ok(breakdown)
    }
    
    async fn get_activity_metadata_counts(&self) -> DomainResult<ActivityMetadataCounts> {
        // Get project counts
        let project_counts = self.count_by_project().await?;
        let mut activities_by_project = HashMap::new();
        for (project_id_opt, count) in project_counts {
            let project_name = match project_id_opt {
                Some(id) => {
                    match query_scalar::<_, String>(
                        "SELECT name FROM projects WHERE id = ? AND deleted_at IS NULL"
                    )
                    .bind(id.to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(DbError::from)? {
                        Some(name) => name,
                        None => format!("Project {}", id),
                    }
                },
                None => "No Project".to_string(),
            };
            activities_by_project.insert(project_name, count);
        }
        
        // Get status counts
        let status_counts = self.count_by_status().await?;
        let mut activities_by_status = HashMap::new();
        for (status_id_opt, count) in status_counts {
            let status_name = match status_id_opt {
                Some(1) => "Not Started".to_string(),
                Some(2) => "In Progress".to_string(),
                Some(3) => "Completed".to_string(),
                Some(4) => "On Hold".to_string(),
                Some(id) => format!("Status {}", id),
                None => "Unspecified".to_string(),
            };
            activities_by_status.insert(status_name, count);
        }
        
        // Count activities with targets
        let activities_with_targets: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities WHERE target_value IS NOT NULL AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Count activities with actuals
        let activities_with_actuals: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities WHERE actual_value IS NOT NULL AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Count activities with documents (activities that have at least one document reference)
        let activities_with_documents: i64 = query_scalar(
            "SELECT COUNT(*) FROM activities 
             WHERE (photo_evidence_ref IS NOT NULL 
                    OR receipts_ref IS NOT NULL 
                    OR signed_report_ref IS NOT NULL 
                    OR monitoring_data_ref IS NOT NULL 
                    OR output_verification_ref IS NOT NULL)
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(ActivityMetadataCounts {
            activities_by_project,
            activities_by_status,
            activities_with_targets,
            activities_with_actuals,
            activities_with_documents,
        })
    }
}

// === Sync Merge Implementation for Activity ===
#[async_trait]
impl MergeableEntityRepository<Activity> for SqliteActivityRepository {
    fn entity_name(&self) -> &'static str { "activities" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for activity change")))?;
                let remote_state: Activity = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_activity", &format!("Invalid JSON: {}", e))))?;
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

impl SqliteActivityRepository {
    async fn upsert_remote_state_with_tx<'t>(&self, tx: &mut Transaction<'t, Sqlite>, remote: &Activity) -> DomainResult<()> {
        sqlx::query(
            r#"
INSERT OR REPLACE INTO activities (
    id, project_id,
    description, description_updated_at, description_updated_by, description_updated_by_device_id,
    kpi, kpi_updated_at, kpi_updated_by, kpi_updated_by_device_id,
    target_value, target_value_updated_at, target_value_updated_by, target_value_updated_by_device_id,
    actual_value, actual_value_updated_at, actual_value_updated_by, actual_value_updated_by_device_id,
    status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
    sync_priority, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id,
    deleted_at, deleted_by_user_id, deleted_by_device_id,
    photo_evidence_ref, receipts_ref, signed_report_ref, monitoring_data_ref, output_verification_ref
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(remote.id.to_string())
        .bind(remote.project_id.map(|u| u.to_string()))
        .bind(&remote.description)
        .bind(remote.description_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.description_updated_by.map(|u| u.to_string()))
        .bind(remote.description_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.kpi)
        .bind(remote.kpi_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.kpi_updated_by.map(|u| u.to_string()))
        .bind(remote.kpi_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.target_value)
        .bind(remote.target_value_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.target_value_updated_by.map(|u| u.to_string()))
        .bind(remote.target_value_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.actual_value)
        .bind(remote.actual_value_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.actual_value_updated_by.map(|u| u.to_string()))
        .bind(remote.actual_value_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.status_id)
        .bind(remote.status_id_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.status_id_updated_by.map(|u| u.to_string()))
        .bind(remote.status_id_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.sync_priority.as_str())
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.to_string())
        .bind(remote.created_by_device_id.map(|u| u.to_string()))
        .bind(remote.updated_by_user_id.to_string())
        .bind(remote.updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|u| u.to_string()))
        .bind(remote.deleted_by_device_id.map(|u| u.to_string()))
        .bind(remote.photo_evidence_ref.map(|u| u.to_string()))
        .bind(remote.receipts_ref.map(|u| u.to_string()))
        .bind(remote.signed_report_ref.map(|u| u.to_string()))
        .bind(remote.monitoring_data_ref.map(|u| u.to_string()))
        .bind(remote.output_verification_ref.map(|u| u.to_string()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}
