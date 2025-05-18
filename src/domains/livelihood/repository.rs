use crate::auth::AuthContext;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::livelihood::types::{Livelihood, LivelihoodRow, NewLivelihood, SubsequentGrant, SubsequentGrantRow, UpdateLivelihood, NewSubsequentGrant, UpdateSubsequentGrant, LivelioodStatsSummary};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::PaginatedResult;
use crate::types::PaginationParams;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, Transaction, query_as, query, Row, QueryBuilder, query_scalar};
use uuid::Uuid;
use crate::validation::Validate;
use async_trait::async_trait;
use std::collections::HashMap;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType};
use crate::types::SyncPriority;
use serde_json;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;
use std::str::FromStr;
use std::sync::Arc;

/// Repository for livelihood entities and their subsequent grants
#[async_trait]
pub trait LivehoodRepository:
    DeleteServiceRepository<Livelihood> + Send + Sync
{
    /// Create a new livelihood
    async fn create(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood>;
    
    /// Create a new livelihood with transaction
    async fn create_with_tx<'t>(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood>;

    /// Update an existing livelihood
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood>;
    
    /// Update an existing livelihood with transaction
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood>;

    /// Find all livelihoods, optionally filtered by project or participant
    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Livelihood>>;
    
    /// Find livelihoods for a specific project
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;
    
    /// Find livelihoods for a specific participant
    async fn find_by_participant_id(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    /// Count livelihoods with grants above a certain amount
    async fn count_by_min_grant_amount(
        &self,
        min_amount: f64,
    ) -> DomainResult<i64>;

    /// Get livelihood statistics summary
    async fn get_livelihood_stats(&self) -> DomainResult<LivelioodStatsSummary>;

    /// Find livelihoods with outcome data
    async fn find_with_outcome(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    /// Find livelihoods needing outcome tracking
    async fn find_without_outcome(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    /// Find livelihoods with multiple subsequent grants
    async fn find_with_multiple_grants(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    /// Get document counts for livelihoods
    async fn get_document_counts(
        &self,
        livelihood_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>>;
    
    /// Get total grant amount including subsequent grants
    async fn get_total_grant_amount(&self, id: Uuid) -> DomainResult<f64>;
    
    /// Get outcome status distribution
    async fn get_outcome_status_distribution(&self) -> DomainResult<HashMap<String, i64>>;
    
    /// Find livelihoods by creation date range
    async fn find_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
}

/// Repository for subsequent grants
#[async_trait]
pub trait SubsequentGrantRepository: Send + Sync {
    /// Create a new subsequent grant
    async fn create(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Create a new subsequent grant with transaction
    async fn create_with_tx<'t>(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Update an existing subsequent grant
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Find a subsequent grant by ID
    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Find all subsequent grants for a livelihood
    async fn find_by_livelihood_id(
        &self,
        livelihood_id: Uuid,
    ) -> DomainResult<Vec<SubsequentGrant>>;
    
    /// Soft delete a subsequent grant
    async fn soft_delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
    
    /// Hard delete a subsequent grant
    async fn hard_delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;

    /// Set a document reference for a subsequent grant
    async fn set_document_reference(
        &self,
        grant_id: Uuid,
        field_name: &str, // e.g., "grant_agreement"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
    
    /// Get total subsequent grant amount for a livelihood
    async fn get_total_grant_amount(
        &self,
        livelihood_id: Uuid,
    ) -> DomainResult<f64>;
    
    /// Get subsequent grant counts by livelihood
    async fn get_grant_counts_by_livelihood(
        &self,
        livelihood_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>>;
    
    /// Find by date range
    async fn find_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> DomainResult<Vec<SubsequentGrant>>;
    
    /// Get monthly grant statistics
    async fn get_monthly_grant_stats(
        &self,
        months_back: i32,
    ) -> DomainResult<Vec<(String, i64, f64)>>;
}

/// SQLite implementation of the livelihood repository
pub struct SqliteLivelihoodRepository {
    pool: Pool<Sqlite>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteLivelihoodRepository {
    /// Create a new SQLite livelihood repository
    pub fn new(pool: Pool<Sqlite>, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    
    /// Map a database row to a domain entity
    fn map_row_to_entity(row: LivelihoodRow) -> DomainResult<Livelihood> {
        row.into_entity()
    }
    
    /// Get the entity name for this repository
    fn entity_name(&self) -> &'static str {
        "livelihood"
    }
    
    /// Find a livelihood by ID using a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        let row = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Livelihood> for SqliteLivelihoodRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Livelihood> {
        let row = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteLivelihoodRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // First check if the record exists and is not already deleted
        let existing = self.find_by_id_with_tx(id, tx).await?;
        
        if existing.is_deleted() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        // Update the record with deleted_at and deleted_by
        let rows_affected = query(
            r#"
            UPDATE livelihoods 
            SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#
        )
        .bind(now.to_rfc3339())
        .bind(auth.user_id.to_string())
        .bind(device_id_str)
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
    
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
}

#[async_trait]
impl HardDeletable for SqliteLivelihoodRepository {
    fn entity_name(&self) -> &'static str {
        "livelihood"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // Check if the record exists first
        let _existing = self.find_by_id_with_tx(id, tx).await?;
        
        // Delete the record permanently
        let rows_affected = query(
            "DELETE FROM livelihoods WHERE id = ?"
        )
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
    
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
}

#[async_trait]
impl LivehoodRepository for SqliteLivelihoodRepository {
    async fn create(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.create_with_tx(new_livelihood, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn create_with_tx<'t>(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        let id = new_livelihood.id.unwrap_or_else(Uuid::new_v4);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let created_by_user_id_for_query = new_livelihood.created_by_user_id.unwrap_or(user_id);
        let created_by_user_id_str = created_by_user_id_for_query.to_string();

        query(
            r#"
            INSERT INTO livelihoods (
                id, participant_id, project_id, 
                type, type_updated_at, type_updated_by, type_updated_by_device_id,
                description, description_updated_at, description_updated_by, description_updated_by_device_id,
                status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
                initial_grant_date, initial_grant_date_updated_at, initial_grant_date_updated_by, initial_grant_date_updated_by_device_id,
                initial_grant_amount, initial_grant_amount_updated_at, initial_grant_amount_updated_by, initial_grant_amount_updated_by_device_id,
                sync_priority, 
                created_at, updated_at, created_by_user_id, updated_by_user_id, 
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, ?, ?, /* id, participant_id, project_id */
                ?, ?, ?, ?, /* type, type_updated_at, type_updated_by, type_updated_by_device_id */
                ?, ?, ?, ?, /* description, description_updated_at, description_updated_by, description_updated_by_device_id */
                ?, ?, ?, ?, /* status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id */
                ?, ?, ?, ?, /* initial_grant_date, initial_grant_date_updated_at, initial_grant_date_updated_by, initial_grant_date_updated_by_device_id */
                ?, ?, ?, ?, /* initial_grant_amount, initial_grant_amount_updated_at, initial_grant_amount_updated_by, initial_grant_amount_updated_by_device_id */
                ?,       /* sync_priority */
                ?, ?, ?, ?, /* created_at, updated_at, created_by_user_id, updated_by_user_id */
                ?, ?, /* created_by_device_id, updated_by_device_id */
                NULL, NULL, NULL  /* deleted_at, deleted_by_user_id, deleted_by_device_id */
            )
            "#,
        )
        .bind(id.to_string())
        .bind(new_livelihood.participant_id.map(|uid| uid.to_string()))
        .bind(new_livelihood.project_id.map(|uid| uid.to_string()))
        .bind(&new_livelihood.type_)
        .bind(&now_str).bind(&user_id_str).bind(&device_id_str) // type_ LWW
        .bind(&new_livelihood.description)
        .bind(new_livelihood.description.as_ref().map(|_| &now_str)).bind(new_livelihood.description.as_ref().map(|_| &user_id_str)).bind(new_livelihood.description.as_ref().map(|_| &device_id_str)) // description LWW
        .bind(new_livelihood.status_id)
        .bind(new_livelihood.status_id.map(|_| &now_str)).bind(new_livelihood.status_id.map(|_| &user_id_str)).bind(new_livelihood.status_id.map(|_| &device_id_str)) // status_id LWW
        .bind(&new_livelihood.initial_grant_date)
        .bind(new_livelihood.initial_grant_date.as_ref().map(|_| &now_str)).bind(new_livelihood.initial_grant_date.as_ref().map(|_| &user_id_str)).bind(new_livelihood.initial_grant_date.as_ref().map(|_| &device_id_str)) // initial_grant_date LWW
        .bind(new_livelihood.initial_grant_amount)
        .bind(new_livelihood.initial_grant_amount.map(|_| &now_str)).bind(new_livelihood.initial_grant_amount.map(|_| &user_id_str)).bind(new_livelihood.initial_grant_amount.map(|_| &device_id_str)) // initial_grant_amount LWW
        .bind(new_livelihood.sync_priority.as_str())
        .bind(&now_str) // created_at
        .bind(&now_str) // updated_at
        .bind(&created_by_user_id_str) // created_by_user_id
        .bind(&user_id_str) // updated_by_user_id
        .bind(&device_id_str).bind(&device_id_str) // created_by_device_id, updated_by_device_id
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None, old_value: None, new_value: None,
            timestamp: now, user_id: created_by_user_id_for_query, device_id: device_uuid,
            document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
        };
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;

        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        let old_entity = self.find_by_id_with_tx(id, tx).await?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = update_data.updated_by_user_id.unwrap_or(auth.user_id);
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());

        let mut builder = QueryBuilder::new("UPDATE livelihoods SET ");
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
                    separated.push_bind_unseparated(device_id_str.clone());
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
                    separated.push_bind_unseparated(device_id_str.clone());
                    fields_updated = true;
                }
            };
        }
        
        let participant_id_update = &update_data.participant_id.map(|opt_uuid| opt_uuid.map(|u| u.to_string()));
        add_lww_field!(participant_id, "participant_id", participant_id_update, opt_opt);
        
        let project_id_update = &update_data.project_id.map(|opt_uuid| opt_uuid.map(|u| u.to_string()));
        add_lww_field!(project_id, "project_id", project_id_update, opt_opt);

        add_lww_field!(type_, "type", &update_data.type_);
        add_lww_field!(description, "description", &update_data.description, opt_opt);
        add_lww_field!(status_id, "status_id", &update_data.status_id, opt_opt);
        add_lww_field!(initial_grant_date, "initial_grant_date", &update_data.initial_grant_date, opt_opt);
        add_lww_field!(initial_grant_amount, "initial_grant_amount", &update_data.initial_grant_amount, opt_opt);
        
        if let Some(priority) = &update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str());
            fields_updated = true;
        }

        if !fields_updated {
            return Ok(old_entity);
        }

        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str.clone());
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());

        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return self.find_by_id_with_tx(id, tx).await
                .err()
                .map_or(Err(DomainError::EntityNotFound(self.entity_name().to_string(), id)), |e| Err(e));
        }

        let new_entity = self.find_by_id_with_tx(id, tx).await?;

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
                        timestamp: now, user_id: user_id, device_id: device_uuid.clone(),
                        document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
                }
            };
        }
        
        log_if_changed!(participant_id, "participant_id");
        log_if_changed!(project_id, "project_id");
        log_if_changed!(type_, "type");
        log_if_changed!(description, "description");
        log_if_changed!(status_id, "status_id");
        log_if_changed!(initial_grant_date, "initial_grant_date");
        log_if_changed!(initial_grant_amount, "initial_grant_amount");

        if old_entity.sync_priority != new_entity.sync_priority {
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("sync_priority".to_string()),
                old_value: serde_json::to_string(old_entity.sync_priority.as_str()).ok(),
                new_value: serde_json::to_string(new_entity.sync_priority.as_str()).ok(),
                timestamp: now, user_id: user_id, device_id: device_uuid.clone(),
                document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
            };
            self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
        }

        Ok(new_entity)
    }
    
    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        // --- Helper closure for WHERE clause --- 
        let build_where_clause = |builder: &mut QueryBuilder<'_, Sqlite>, proj_id: Option<Uuid>, part_id: Option<Uuid>| {
            builder.push(" WHERE deleted_at IS NULL");
            if let Some(pid) = proj_id {
                builder.push(" AND project_id = ");
                builder.push_bind(pid.to_string());
            }
            if let Some(pid) = part_id {
                builder.push(" AND participant_id = ");
                builder.push_bind(pid.to_string());
            }
        };

        // --- Count Query --- 
        let mut count_builder = QueryBuilder::new("SELECT COUNT(*) as count FROM livelihoods");
        build_where_clause(&mut count_builder, project_id, participant_id); // Apply WHERE logic
        
        let count_query = count_builder.build(); // Build the final count query
        
        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?
            .try_get("count")
            .map_err(|e| DbError::Query(format!("Failed to get count: {}", e)))?;
            
        // --- Main Query --- 
        let mut query_builder = QueryBuilder::new("SELECT * FROM livelihoods");
        build_where_clause(&mut query_builder, project_id, participant_id); // Apply WHERE logic again
        
        query_builder.push(" ORDER BY created_at DESC LIMIT ");
        query_builder.push_bind(params.per_page as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind((params.page as i64 - 1) * params.per_page as i64);
        
        // Build and execute the query_as directly
        let rows = query_builder.build_query_as::<LivelihoodRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        // Convert rows to entities
        let mut items = Vec::new();
        for row in rows {
            items.push(Self::map_row_to_entity(row)?);
        }
        
        // Calculate total pages
        let total_pages = (total as f64 / params.per_page as f64).ceil() as u32;
        
        Ok(PaginatedResult {
            items,
            total: total as u64,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        })
    }
    
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        self.find_all(params, Some(project_id), None).await
    }
    
    async fn find_by_participant_id(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        self.find_all(params, None, Some(participant_id)).await
    }

    /// Count livelihoods with grants above a certain amount
    async fn count_by_min_grant_amount(
        &self,
        min_amount: f64,
    ) -> DomainResult<i64> {
        let count = query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE grant_amount >= ? 
             AND deleted_at IS NULL"
        )
        .bind(min_amount)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(count)
    }

    /// Get livelihood statistics summary
    async fn get_livelihood_stats(&self) -> DomainResult<LivelioodStatsSummary> {
        // 1. Get basic counts and initial amounts
        let basic_stats_query = r#"
            SELECT 
                COUNT(*) as total,
                COALESCE(SUM(initial_grant_amount), 0) as total_amount,
                CASE WHEN COUNT(initial_grant_amount) > 0 THEN COALESCE(AVG(initial_grant_amount), 0) ELSE 0 END as avg_amount
            FROM livelihoods
            WHERE deleted_at IS NULL
            "#;
        let (total_livelihoods, total_initial_amount, avg_initial_amount) = query_as::<_, (i64, f64, f64)>(basic_stats_query)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // 2. Get project distribution (counts and initial amounts)
        let project_distribution_query = r#"
            SELECT 
                project_id, -- Select project_id (UUID)
                COUNT(l.id) as livelihood_count,
                COALESCE(SUM(l.initial_grant_amount), 0) as total_initial_amount
            FROM livelihoods l
            WHERE l.deleted_at IS NULL AND l.project_id IS NOT NULL
            GROUP BY l.project_id
            "#;
        let project_distribution_rows = query(project_distribution_query)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        let mut livelihoods_by_project: HashMap<Uuid, i64> = HashMap::new();
        let mut initial_grant_amounts_by_project: HashMap<Uuid, f64> = HashMap::new();
        for row in project_distribution_rows {
            if let Some(id_str) = row.get::<Option<String>, _>("project_id") {
                 if let Ok(id) = Uuid::parse_str(&id_str) {
                     livelihoods_by_project.insert(id, row.get("livelihood_count"));
                     initial_grant_amounts_by_project.insert(id, row.get("total_initial_amount"));
                 }
            }
        }

        // 3. Get subsequent grants stats
        let subsequent_stats_query = r#"
            SELECT 
                COUNT(*) as total_grants,
                COALESCE(SUM(amount), 0) as total_amount
            FROM subsequent_grants
            WHERE deleted_at IS NULL
            "#;
        let (total_subsequent_grants, total_subsequent_amount) = query_as::<_, (i64, f64)>(subsequent_stats_query)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        // 4. Get Livelihoods by type
        let type_distribution_query = r#"
            SELECT type, COUNT(*) as count
            FROM livelihoods
            WHERE deleted_at IS NULL
            GROUP BY type
        "#;
        let type_rows = query(type_distribution_query)
             .fetch_all(&self.pool)
             .await
             .map_err(DbError::from)?;
        let livelihoods_by_type: HashMap<String, i64> = type_rows.into_iter()
            .map(|row| (row.get("type"), row.get("count")))
            .collect();

        // 5. Build the response
        Ok(LivelioodStatsSummary {
            total_livelihoods,
            active_livelihoods: total_livelihoods, // Assuming active means not deleted
            total_initial_grant_amount: total_initial_amount,
            average_initial_grant_amount: avg_initial_amount,
            total_subsequent_grants,
            total_subsequent_grant_amount: total_subsequent_amount,
            livelihoods_by_project,
            initial_grant_amounts_by_project,
            livelihoods_by_type,
        })
    }

    /// Find livelihoods with outcome data
    async fn find_with_outcome(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE outcome IS NOT NULL AND outcome != '' 
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods 
             WHERE outcome IS NOT NULL AND outcome != '' 
             AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Livelihood>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find livelihoods needing outcome tracking
    async fn find_without_outcome(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE (outcome IS NULL OR outcome = '') 
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods 
             WHERE (outcome IS NULL OR outcome = '')
             AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Livelihood>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Find livelihoods with multiple subsequent grants
    async fn find_with_multiple_grants(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            r#"
            SELECT COUNT(DISTINCT l.id) 
            FROM livelihoods l
            INNER JOIN (
                SELECT livelihood_id
                FROM subsequent_grants
                WHERE deleted_at IS NULL
                GROUP BY livelihood_id
                HAVING COUNT(*) > 0
            ) sg ON l.id = sg.livelihood_id
            WHERE l.deleted_at IS NULL
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, LivelihoodRow>(
            r#"
            SELECT l.* 
            FROM livelihoods l
            INNER JOIN (
                SELECT livelihood_id
                FROM subsequent_grants
                WHERE deleted_at IS NULL
                GROUP BY livelihood_id
                HAVING COUNT(*) > 0
            ) sg ON l.id = sg.livelihood_id
            WHERE l.deleted_at IS NULL
            ORDER BY l.updated_at DESC LIMIT ? OFFSET ?
            "#
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Livelihood>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// Get document counts for livelihoods
    async fn get_document_counts(
        &self,
        livelihood_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>> {
        let mut counts = HashMap::new();
        
        if livelihood_ids.is_empty() {
            return Ok(counts);
        }

        // Convert UUIDs to strings for SQL query
        let id_strings: Vec<String> = livelihood_ids.iter().map(|id| id.to_string()).collect();
        
        // Use IN clause with the collected strings
        let placeholders = id_strings.iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");

        let query_str = format!(
            r#"
            SELECT parent_entity_id, COUNT(*) as doc_count
            FROM media_documents
            WHERE parent_entity_type = 'livelihoods'
            AND parent_entity_id IN ({})
            AND deleted_at IS NULL
            GROUP BY parent_entity_id
            "#,
            placeholders
        );

        let mut query = sqlx::query(&query_str);
        
        // Bind all the ID strings
        for id in &id_strings {
            query = query.bind(id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        for row in rows {
            let id_str: String = row.get("parent_entity_id");
            let count: i64 = row.get("doc_count");
            
            if let Ok(id) = Uuid::parse_str(&id_str) {
                counts.insert(id, count);
            }
        }

        // Ensure all requested IDs have an entry, even if no documents
        for id in livelihood_ids {
            counts.entry(*id).or_insert(0);
        }

        Ok(counts)
    }
    
    async fn get_total_grant_amount(&self, id: Uuid) -> DomainResult<f64> {
        // Get initial grant amount
        let initial_amount: f64 = query_scalar(
            "SELECT COALESCE(grant_amount, 0) FROM livelihoods WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Get sum of subsequent grants
        let subsequent_amount: f64 = query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM subsequent_grants 
             WHERE livelihood_id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(initial_amount + subsequent_amount)
    }
    
    async fn get_outcome_status_distribution(&self) -> DomainResult<HashMap<String, i64>> {
        let mut distribution = HashMap::new();
        
        // No outcome or empty outcome = Not Started
        let not_started: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE (outcome IS NULL OR outcome = '') 
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Has progress1 but no outcome = In Progress
        let in_progress: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE (progress1 IS NOT NULL AND progress1 != '')
             AND (outcome IS NULL OR outcome = '')
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Has outcome = Completed
        let completed: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE outcome IS NOT NULL AND outcome != '' 
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        distribution.insert("Not Started".to_string(), not_started - in_progress);
        distribution.insert("In Progress".to_string(), in_progress);
        distribution.insert("Completed".to_string(), completed);
        
        Ok(distribution)
    }
    
    async fn find_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM livelihoods 
             WHERE DATE(created_at) BETWEEN DATE(?) AND DATE(?)
             AND deleted_at IS NULL"
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods 
             WHERE DATE(created_at) BETWEEN DATE(?) AND DATE(?)
             AND deleted_at IS NULL 
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(start_date)
        .bind(end_date)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Livelihood>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() { return Ok(0); }
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let priority_str = priority.as_str();

        // Fetch old priorities (Map Uuid -> SyncPriorityFromSyncDomain)
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query_placeholders = vec!["?"; ids.len()].join(", ");
        let select_query = format!(
            "SELECT id, sync_priority FROM livelihoods WHERE id IN ({}) AND deleted_at IS NULL",
            select_query_placeholders
        );
        let mut select_builder = query_as::<_, (String, String)>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: HashMap<Uuid, SyncPriorityFromSyncDomain> = select_builder
            .fetch_all(&mut *tx)
            .await
            .map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_text)| {
                 Uuid::parse_str(&id_str).ok()
                     .and_then(|id| SyncPriorityFromSyncDomain::from_str(&prio_text).ok().map(|prio| (id, prio)))
            })
            .collect();

        // Perform Update
        let mut update_builder = QueryBuilder::new("UPDATE livelihoods SET ");
        update_builder.push("sync_priority = "); update_builder.push_bind(priority_str.clone());
        update_builder.push(", updated_at = "); update_builder.push_bind(now_str.clone());
        update_builder.push(", updated_by_user_id = "); update_builder.push_bind(user_id_str.clone());
        update_builder.push(", updated_by_device_id = "); update_builder.push_bind(device_id_str.clone());
        update_builder.push(" WHERE id IN (");
        let mut id_separated = update_builder.separated(",");
        for id_str in &id_strings { id_separated.push_bind(id_str); }
        update_builder.push(") AND deleted_at IS NULL");
        
        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();

        // Log Changes
        for id_uuid in ids {
            if let Some(old_priority) = old_priorities.get(id_uuid) {
                if *old_priority != priority { // Log only if changed
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: *id_uuid,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: serde_json::to_string(old_priority.as_str()).ok(),
                        new_value: serde_json::to_string(priority_str).ok(),
                        timestamp: now, user_id: user_id, device_id: device_uuid.clone(),
                        document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
                }
            } else {
                 // If the ID was in the input `ids` but not in `old_priorities`,
                 // it implies either it didn't exist, was deleted, or had an unparseable priority before.
                 // If rows_affected > 0, it means *some* rows were updated. We can infer that
                 // this specific row *might* have been updated from an unknown state to the new priority.
                 // It's safer to log this potential change from 'unknown'.
                 if rows_affected > 0 {
                     let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: *id_uuid,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: None, // Indicate unknown/null previous state
                        new_value: serde_json::to_string(priority_str).ok(),
                        timestamp: now, user_id: user_id, device_id: device_uuid.clone(),
                        document_metadata: None, sync_batch_id: None, processed_at: None, sync_error: None,
                    };
                    // Avoid erroring if logging fails for a row that might not have existed
                    let _ = self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await;
                 }
            }
        }

        tx.commit().await.map_err(DbError::from)?;
        Ok(rows_affected)
    }
}

/// SQLite implementation of the subsequent grant repository
pub struct SqliteSubsequentGrantRepository {
    pool: Pool<Sqlite>,
}

impl SqliteSubsequentGrantRepository {
    /// Create a new SQLite subsequent grant repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    
    /// Map a database row to a domain entity
    fn map_row_to_entity(row: SubsequentGrantRow) -> DomainResult<SubsequentGrant> {
        row.into_entity()
    }
    
    /// Get the entity name for this repository
    fn entity_name(&self) -> &'static str {
        "subsequent_grant"
    }
    
    /// Find a subsequent grant by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant> {
        let row = query_as::<_, SubsequentGrantRow>(
            "SELECT id, livelihood_id, amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id, purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id, grant_date, grant_date_updated_at, grant_date_updated_by, grant_date_updated_by_device_id, sync_priority, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM subsequent_grants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SubsequentGrantRepository for SqliteSubsequentGrantRepository {
    async fn create(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.create_with_tx(new_grant, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn create_with_tx<'t>(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant> {
        new_grant.validate()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let created_by = new_grant.created_by_user_id.unwrap_or(user_id);
        let created_by_str = created_by.to_string();
        
        query(
             r#"
             INSERT INTO subsequent_grants (
                 id, 
                 livelihood_id, 
                 amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id,
                 purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id,
                 grant_date, grant_date_updated_at, grant_date_updated_by, grant_date_updated_by_device_id,
                 sync_priority,
                 created_at, 
                 updated_at,
                 created_by_user_id,
                 updated_by_user_id,
                 created_by_device_id,
                 updated_by_device_id,
                 deleted_at, deleted_by_user_id, deleted_by_device_id
             ) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
             "#
         )
         .bind(id.to_string())
         .bind(new_grant.livelihood_id.to_string())
         .bind(new_grant.amount)
         .bind(new_grant.amount.map(|_| &now_str)).bind(new_grant.amount.map(|_| &user_id_str)).bind(new_grant.amount.map(|_| &device_id_str))
         .bind(&new_grant.purpose)
         .bind(new_grant.purpose.as_ref().map(|_| &now_str)).bind(new_grant.purpose.as_ref().map(|_| &user_id_str)).bind(new_grant.purpose.as_ref().map(|_| &device_id_str))
         .bind(&new_grant.grant_date)
         .bind(new_grant.grant_date.as_ref().map(|_| &now_str)).bind(new_grant.grant_date.as_ref().map(|_| &user_id_str)).bind(new_grant.grant_date.as_ref().map(|_| &device_id_str))
         .bind(new_grant.sync_priority.as_str())
         .bind(&now_str) // created_at
         .bind(&now_str) // updated_at
         .bind(&created_by_str) // created_by_user_id
         .bind(&user_id_str) // updated_by_user_id
         .bind(&device_id_str).bind(&device_id_str) // created_by_device_id, updated_by_device_id
         .execute(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant> {
        update_data.validate()?;
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        let mut builder = QueryBuilder::new("UPDATE subsequent_grants SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;
        
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_id_str.clone());
                    fields_updated = true;
                }
            };
        }
        
        add_lww!(amount, "amount", &update_data.amount);
        add_lww!(purpose, "purpose", &update_data.purpose);
        add_lww!(grant_date, "grant_date", &update_data.grant_date);
        
        if let Some(priority) = &update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str());
            fields_updated = true;
        }

        if !fields_updated {
            return Ok(existing);
        }
        
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        
        // Correctly handle updated_by_user_id binding
        let final_updated_by_user_id_str = update_data.updated_by_user_id
            .map(|u| u.to_string())
            .unwrap_or_else(|| user_id_str.clone()); // Default to current auth user if not specified in update_data
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(final_updated_by_user_id_str);
        
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());
        
        builder.push(" WHERE id = ");
        builder.push_bind(id.to_string());
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        let updated_grant = self.find_by_id_with_tx(id, &mut tx).await?;
        tx.commit().await.map_err(DbError::from)?;
        Ok(updated_grant)
    }
    
    async fn find_by_id(
        &self, 
        id: Uuid
    ) -> DomainResult<SubsequentGrant> {
        let row = query_as::<_, SubsequentGrantRow>(
            "SELECT id, livelihood_id, amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id, purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id, grant_date, grant_date_updated_at, grant_date_updated_by, grant_date_updated_by_device_id, sync_priority, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM subsequent_grants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
    
    async fn find_by_livelihood_id(
        &self, 
        livelihood_id: Uuid
    ) -> DomainResult<Vec<SubsequentGrant>> {
        let rows = query_as::<_, SubsequentGrantRow>(
            "SELECT id, livelihood_id, amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id, purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id, grant_date, grant_date_updated_at, grant_date_updated_by, grant_date_updated_by_device_id, sync_priority, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM subsequent_grants WHERE livelihood_id = ? AND deleted_at IS NULL ORDER BY created_at ASC"
        )
        .bind(livelihood_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut grants = Vec::new();
        for row in rows {
            grants.push(Self::map_row_to_entity(row)?);
        }
        
        Ok(grants)
    }
    
    async fn soft_delete(
        &self, 
        id: Uuid, 
        auth: &AuthContext
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        let mut builder = QueryBuilder::new("UPDATE subsequent_grants SET ");
        builder.push("deleted_at = ");
        builder.push_bind(now.to_rfc3339());
        builder.push(", deleted_by_user_id = ");
        builder.push_bind(auth.user_id.to_string());
        builder.push(", deleted_by_device_id = ");
        builder.push_bind(device_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now.to_rfc3339());
        builder.push(", updated_by_device_id = "); // Also update this on soft delete
        builder.push_bind(auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string()));
        
        builder.push(" WHERE id = ");
        builder.push_bind(id.to_string());
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }
    
    async fn hard_delete(
        &self, 
        id: Uuid, 
        _auth: &AuthContext
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let mut builder = QueryBuilder::new("DELETE FROM subsequent_grants ");
        builder.push("WHERE id = ");
        builder.push_bind(id.to_string());
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }

    /// Set a document reference for a subsequent grant
    async fn set_document_reference(
        &self,
        grant_id: Uuid,
        field_name: &str, // e.g., "grant_agreement"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let column_name = format!("{}_ref", field_name);
        
        // Validate the field name
        if !SubsequentGrant::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for SubsequentGrant: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let document_id_str = document_id.to_string();
        let grant_id_str = grant_id.to_string();
        
        let mut builder = sqlx::QueryBuilder::new("UPDATE subsequent_grants SET ");
        builder.push(&column_name);
        builder.push(" = ");
        builder.push_bind(document_id_str);
        // Since this is an update to a specific field, we also update the LWW metadata for that field.
        // Assuming the schema has `xxx_ref_updated_at`, `xxx_ref_updated_by`, `xxx_ref_updated_by_device_id`
        // For simplicity, we'll just update the main record's updated_at, _by, _by_device_id
        // A more granular approach would be ideal if such LWW columns exist per reference.
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(", updated_by_device_id = ");
        builder.push_bind(device_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(grant_id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("SubsequentGrant".to_string(), grant_id))
        } else {
            Ok(())
        }
    }

    /// Get total subsequent grant amount for a livelihood
    async fn get_total_grant_amount(
        &self,
        livelihood_id: Uuid,
    ) -> DomainResult<f64> {
        let total: f64 = query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM subsequent_grants 
             WHERE livelihood_id = ? AND deleted_at IS NULL"
        )
        .bind(livelihood_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(total)
    }
    
    async fn get_grant_counts_by_livelihood(
        &self,
        livelihood_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>> {
        let mut counts = HashMap::new();
        
        if livelihood_ids.is_empty() {
            return Ok(counts);
        }

        // Convert UUIDs to strings for SQL query
        let id_strings: Vec<String> = livelihood_ids.iter().map(|id| id.to_string()).collect();
        
        // Use IN clause with the collected strings
        let placeholders = id_strings.iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");

        let query_str = format!(
            r#"
            SELECT livelihood_id, COUNT(*) as grant_count
            FROM subsequent_grants
            WHERE livelihood_id IN ({})
            AND deleted_at IS NULL
            GROUP BY livelihood_id
            "#,
            placeholders
        );

        let mut query = sqlx::query(&query_str);
        
        // Bind all the ID strings
        for id in &id_strings {
            query = query.bind(id);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        for row in rows {
            let id_str: String = row.get("livelihood_id");
            let count: i64 = row.get("grant_count");
            
            if let Ok(id) = Uuid::parse_str(&id_str) {
                counts.insert(id, count);
            }
        }

        // Ensure all requested IDs have an entry, even if no subsequent grants
        for id in livelihood_ids {
            counts.entry(*id).or_insert(0);
        }

        Ok(counts)
    }
    
    async fn find_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> DomainResult<Vec<SubsequentGrant>> {
        let rows = query_as::<_, SubsequentGrantRow>(
            "SELECT id, livelihood_id, amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id, purpose, purpose_updated_at, purpose_updated_by, purpose_updated_by_device_id, grant_date, grant_date_updated_at, grant_date_updated_by, grant_date_updated_by_device_id, sync_priority, created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id FROM subsequent_grants 
             WHERE 
                (grant_date BETWEEN ? AND ?) OR
                (DATE(created_at) BETWEEN DATE(?) AND DATE(?))
             AND deleted_at IS NULL 
             ORDER BY created_at DESC"
        )
        .bind(start_date)
        .bind(end_date)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<SubsequentGrant>>>()?;

        Ok(entities)
    }
    
    async fn get_monthly_grant_stats(
        &self,
        months_back: i32,
    ) -> DomainResult<Vec<(String, i64, f64)>> {
        // Generate date range that includes all months back from current month
        let query_str = format!(
            r#"
            WITH RECURSIVE months(date) AS (
                SELECT DATE(DATETIME('now', 'start of month', '{} months')) 
                UNION ALL
                SELECT DATE(DATETIME(date, '+1 month'))
                FROM months
                WHERE date < DATE('now', 'start of month')
            )
            SELECT 
                strftime('%Y-%m', months.date) as month,
                COUNT(sg.id) as grant_count,
                COALESCE(SUM(sg.amount), 0) as total_amount
            FROM months
            LEFT JOIN subsequent_grants sg ON 
                (sg.grant_date IS NOT NULL AND strftime('%Y-%m', sg.grant_date) = strftime('%Y-%m', months.date)) OR
                (sg.grant_date IS NULL AND strftime('%Y-%m', sg.created_at) = strftime('%Y-%m', months.date))
            WHERE sg.deleted_at IS NULL OR sg.deleted_at IS NULL
            GROUP BY month
            ORDER BY month
            "#,
            -months_back
        );

        let rows = sqlx::query(&query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let mut stats = Vec::new();
        for row in rows {
            let month: String = row.get("month");
            let count: i64 = row.get("grant_count");
            let amount: f64 = row.get("total_amount");
            
            stats.push((month, count, amount));
        }

        Ok(stats)
    }
}