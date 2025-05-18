use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, QueryBuilder};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::activity::types::{NewActivity, Activity, ActivityRow, UpdateActivity};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use std::sync::Arc;
use serde_json;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType};

/// Trait defining activity repository operations
#[async_trait]
pub trait ActivityRepository: DeleteServiceRepository<Activity> + Send + Sync {
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

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;
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
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_with_tx(new_activity, auth, &mut tx).await {
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

    async fn create_with_tx<'t>(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id; // Get Uuid directly
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let project_id_str = new_activity.project_id.map(|id| id.to_string());

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
        .map_err(DbError::from)?;

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
        self.log_change_entry(entry, tx).await?; // Add log call here

        self.find_by_id_with_tx(id, tx).await
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
}
