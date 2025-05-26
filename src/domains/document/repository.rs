use crate::auth::context::AuthContext;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome, SyncPriority as SyncPriorityFromSyncTypes};
use crate::domains::sync::repository::{ChangeLogRepository, MergeableEntityRepository};
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginationParams, PaginatedResult};
use crate::validation::Validate;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use log::{error, info, warn};
use serde_json;
use sqlx::query::Query;
use sqlx::{query, query_as, query_scalar, Execute, Pool, QueryBuilder, Sqlite, Transaction, Executor};
use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use uuid::Uuid;

use super::types::{
    BlobSyncStatus, CompressionStatus, DocumentAccessLog, DocumentAccessLogRow,
    DocumentType, DocumentTypeRow, DocumentVersion, DocumentVersionRow, MediaDocument,
    MediaDocumentFullState, MediaDocumentRow, NewDocumentAccessLog, NewDocumentType, NewMediaDocument, UpdateDocumentType
};

pub const TEMP_RELATED_TABLE: &str = "TEMP";

// --- Document Type Repository ---

#[async_trait]
pub trait DocumentTypeRepository: DeleteServiceRepository<DocumentType> + Send + Sync {
    async fn create(
        &self,
        new_type: &NewDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType>;

    async fn find_all(&self, params: PaginationParams) -> DomainResult<PaginatedResult<DocumentType>>;
    async fn find_by_name(&self, name: &str) -> DomainResult<Option<DocumentType>>;
}

pub struct SqliteDocumentTypeRepository {
    pool: Pool<Sqlite>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteDocumentTypeRepository {
    pub fn new(pool: Pool<Sqlite>, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    fn entity_name() -> &'static str { "document_types" }
    fn map_row(row: DocumentTypeRow) -> DomainResult<DocumentType> { row.into_entity() }
    
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<DocumentType> {
        query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE id = ? AND deleted_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?
            .ok_or_else(|| DomainError::EntityNotFound(Self::entity_name().to_string(), id))
            .and_then(Self::map_row)
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
impl FindById<DocumentType> for SqliteDocumentTypeRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<DocumentType> {
        query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE id = ? AND deleted_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?
            .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))
            .and_then(Self::map_row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteDocumentTypeRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let _device_uuid: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse::<Uuid>().ok() };

        let result = query(
            "UPDATE document_types SET deleted_at = ?, deleted_by_user_id = ?, updated_at = ?, deleted_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&now)
        .bind(auth.user_id.to_string())
        .bind(&now)
        .bind(_device_uuid.map(|u| u.to_string()))
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else { Ok(()) }
    }
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(()) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteDocumentTypeRepository {
    fn entity_name(&self) -> &'static str { "document_types" }
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, 
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let _device_uuid: Option<Uuid> = if _auth.device_id.is_empty() { None } else { _auth.device_id.as_str().parse::<Uuid>().ok() };

        let result = query("DELETE FROM document_types WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(<Self as HardDeletable>::entity_name(self).to_string(), id))
        } else { Ok(()) }
    }
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(()) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl DocumentTypeRepository for SqliteDocumentTypeRepository {
    async fn create(
        &self,
        new_type: &NewDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType> {
        new_type.validate()?; // Validate DTO

        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let user_id_str_val = auth.user_id.to_string();
        let device_id_uuid_val: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse().ok() };
        let device_id_for_bind = device_id_uuid_val.map(|u| u.to_string());


        // Bind variables for direct fields from DTO
        let id_bind = id.to_string();
        let name_bind = &new_type.name; // String
        let allowed_extensions_bind = &new_type.allowed_extensions; // String
        let max_size_bind = new_type.max_size; // i64
        let compression_level_bind = new_type.compression_level; // i32
        let default_priority_bind = new_type.default_priority.as_str(); // &str

        // Optional fields from DTO
        let desc_bind = new_type.description.as_ref(); // Option<&String>
        let icon_bind = new_type.icon.as_ref(); // Option<&String>
        let compression_method_bind = new_type.compression_method.as_ref(); // Option<&String>
        let min_size_for_compression_bind = new_type.min_size_for_compression; // Option<i64>
        let related_tables_bind = new_type.related_tables.as_ref(); // Option<&String>

        // Audit fields for the record itself
        let created_at_bind = &now_str;
        let updated_at_bind = &now_str;
        let created_by_user_id_bind = &user_id_str_val;
        let updated_by_user_id_bind = &user_id_str_val; // On create, updated_by is same as created_by
        let created_by_device_id_bind = device_id_for_bind.as_deref();
        let updated_by_device_id_bind = device_id_for_bind.as_deref(); // On create, same as created_by

        // LWW meta fields - for NOT NULL fields or those guaranteed to be Some() from DTO on create
        let name_updated_at_bind = &now_str;
        let name_updated_by_bind = &user_id_str_val;
        let name_updated_by_device_id_bind = device_id_for_bind.as_deref();

        let allowed_extensions_updated_at_bind = &now_str;
        let allowed_extensions_updated_by_bind = &user_id_str_val;
        let allowed_extensions_updated_by_device_id_bind = device_id_for_bind.as_deref();
        
        let max_size_updated_at_bind = &now_str;
        let max_size_updated_by_bind = &user_id_str_val;
        let max_size_updated_by_device_id_bind = device_id_for_bind.as_deref();

        let compression_level_updated_at_bind = &now_str;
        let compression_level_updated_by_bind = &user_id_str_val;
        let compression_level_updated_by_device_id_bind = device_id_for_bind.as_deref();
        
        let default_priority_updated_at_bind = &now_str;
        let default_priority_updated_by_bind = &user_id_str_val;
        let default_priority_updated_by_device_id_bind = device_id_for_bind.as_deref();

        // LWW meta fields - for Option<T> fields
        let desc_updated_at_bind = desc_bind.map(|_| created_at_bind);
        let desc_updated_by_bind = desc_bind.map(|_| created_by_user_id_bind);
        let desc_updated_by_device_id_bind = if desc_bind.is_some() { device_id_for_bind.as_deref() } else { None };

        let icon_updated_at_bind = icon_bind.map(|_| created_at_bind);
        let icon_updated_by_bind = icon_bind.map(|_| created_by_user_id_bind);
        let icon_updated_by_device_id_bind = if icon_bind.is_some() { device_id_for_bind.as_deref() } else { None };

        let compression_method_updated_at_bind = compression_method_bind.map(|_| created_at_bind);
        let compression_method_updated_by_bind = compression_method_bind.map(|_| created_by_user_id_bind);
        let compression_method_updated_by_device_id_bind = if compression_method_bind.is_some() { device_id_for_bind.as_deref() } else { None };

        let min_size_for_compression_updated_at_bind = min_size_for_compression_bind.map(|_| created_at_bind);
        let min_size_for_compression_updated_by_bind = min_size_for_compression_bind.map(|_| created_by_user_id_bind);
        let min_size_for_compression_updated_by_device_id_bind = if min_size_for_compression_bind.is_some() { device_id_for_bind.as_deref() } else { None };
        
        let related_tables_updated_at_bind = related_tables_bind.map(|_| created_at_bind);
        let related_tables_updated_by_bind = related_tables_bind.map(|_| created_by_user_id_bind);
        let related_tables_updated_by_device_id_bind = if related_tables_bind.is_some() { device_id_for_bind.as_deref() } else { None };

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        sqlx::query!(
            r#"
            INSERT INTO document_types (
                id, name, description, icon, default_priority, 
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                name_updated_at, name_updated_by, name_updated_by_device_id,
                allowed_extensions, allowed_extensions_updated_at, allowed_extensions_updated_by, allowed_extensions_updated_by_device_id,
                max_size, max_size_updated_at, max_size_updated_by, max_size_updated_by_device_id,
                compression_level, compression_level_updated_at, compression_level_updated_by, compression_level_updated_by_device_id,
                compression_method, compression_method_updated_at, compression_method_updated_by, compression_method_updated_by_device_id,
                min_size_for_compression, min_size_for_compression_updated_at, min_size_for_compression_updated_by, min_size_for_compression_updated_by_device_id,
                description_updated_at, description_updated_by, description_updated_by_device_id,
                default_priority_updated_at, default_priority_updated_by, default_priority_updated_by_device_id,
                icon_updated_at, icon_updated_by, icon_updated_by_device_id,
                related_tables, related_tables_updated_at, related_tables_updated_by, related_tables_updated_by_device_id
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, 
                $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38, $39, $40, 
                $41, $42, $43, $44, $45, $46, $47
            )
            "#,
            id_bind, name_bind, desc_bind, icon_bind, default_priority_bind,
            created_at_bind, updated_at_bind, created_by_user_id_bind, updated_by_user_id_bind,
            created_by_device_id_bind, updated_by_device_id_bind,
            name_updated_at_bind, name_updated_by_bind, name_updated_by_device_id_bind,
            allowed_extensions_bind, allowed_extensions_updated_at_bind, allowed_extensions_updated_by_bind, allowed_extensions_updated_by_device_id_bind,
            max_size_bind, max_size_updated_at_bind, max_size_updated_by_bind, max_size_updated_by_device_id_bind,
            compression_level_bind, compression_level_updated_at_bind, compression_level_updated_by_bind, compression_level_updated_by_device_id_bind,
            compression_method_bind, compression_method_updated_at_bind, compression_method_updated_by_bind, compression_method_updated_by_device_id_bind,
            min_size_for_compression_bind, min_size_for_compression_updated_at_bind, min_size_for_compression_updated_by_bind, min_size_for_compression_updated_by_device_id_bind,
            desc_updated_at_bind, desc_updated_by_bind, desc_updated_by_device_id_bind,
            default_priority_updated_at_bind, default_priority_updated_by_bind, default_priority_updated_by_device_id_bind,
            icon_updated_at_bind, icon_updated_by_bind, icon_updated_by_device_id_bind,
            related_tables_bind, related_tables_updated_at_bind, related_tables_updated_by_bind, related_tables_updated_by_device_id_bind
        )
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;

        // Corrected ChangeLogEntry construction
        let change_log_entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(), 
            entity_table: Self::entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create, 
            field_name: None,
            old_value: None,
            new_value: Some(serde_json::to_string(&new_type).map_err(|e| DomainError::Internal(e.to_string()))?), // Use DomainError::Internal and to_string for new_value
            document_metadata: None, 
            timestamp: now, 
            user_id: auth.user_id,
            device_id: device_id_uuid_val, 
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.log_change_entry(change_log_entry, &mut tx).await?;
        
        tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;

        self.find_by_id(id).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        let result = async {
            let old_entity = self.find_by_id_with_tx(id, &mut tx).await?;
            
            let now_val = Utc::now();
            let now_str_val = now_val.to_rfc3339();
            let user_id_str_val = auth.user_id.to_string();
            let user_uuid_val = auth.user_id;
            let device_uuid_val: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse::<Uuid>().ok() };
            let device_id_str_val = device_uuid_val.map(|id| id.to_string());

            let mut sets_changed_for_log: Vec<String> = Vec::new(); // To track which fields were actually changed for logging
            let mut query_builder = QueryBuilder::new("UPDATE document_types SET ");
            let mut first_set = true;

            // Simplified Last-Write-Wins helper. Mirrors the approach used in the Project repository.
            macro_rules! add_lww {
                ($field_ident:ident, $sql_field:literal, $dto_opt:expr) => {
                    if let Some(val) = $dto_opt {
                        if !first_set { query_builder.push(", "); } else { first_set = false; }
                        query_builder.push(format!("{} = ", $sql_field));
                        query_builder.push_bind(val.clone());
                        query_builder.push(format!(", {}_updated_at = ", $sql_field));
                        query_builder.push_bind(now_str_val.clone());
                        query_builder.push(format!(", {}_updated_by = ", $sql_field));
                        query_builder.push_bind(user_id_str_val.clone());
                        query_builder.push(format!(", {}_updated_by_device_id = ", $sql_field));
                        query_builder.push_bind(device_id_str_val.clone());
                        sets_changed_for_log.push($sql_field.to_string());
                    }
                };
            }

            // Use the simplified macro for each updatable field
            add_lww!(name, "name", update_data.name.as_ref());
            add_lww!(description, "description", update_data.description.as_ref());
            add_lww!(icon, "icon", update_data.icon.as_ref());
            add_lww!(default_priority, "default_priority", update_data.default_priority.as_ref());
            add_lww!(allowed_extensions, "allowed_extensions", update_data.allowed_extensions.as_ref());
            add_lww!(max_size, "max_size", update_data.max_size.as_ref());
            add_lww!(compression_level, "compression_level", update_data.compression_level.as_ref());
            add_lww!(compression_method, "compression_method", update_data.compression_method.as_ref());
            add_lww!(min_size_for_compression, "min_size_for_compression", update_data.min_size_for_compression.as_ref());
            add_lww!(related_tables, "related_tables", update_data.related_tables.as_ref());

            if first_set { return Ok(old_entity); }

            query_builder.push(", updated_at = "); query_builder.push_bind(now_str_val.clone());
            query_builder.push(", updated_by_user_id = "); query_builder.push_bind(user_id_str_val.clone());
            query_builder.push(", updated_by_device_id = "); query_builder.push_bind(device_id_str_val.clone());
            
            query_builder.push(" WHERE id = "); query_builder.push_bind(id.to_string());
            let q = query_builder.build();
            q.execute(&mut *tx).await.map_err(|e| DomainError::Database(DbError::from(e)))?;

            let new_entity = self.find_by_id_with_tx(id, &mut tx).await?;

            macro_rules! log_if_changed_field {
                ($field_sql:expr, $old_val_expr:expr, $new_val_expr:expr) => {
                    if sets_changed_for_log.contains(&$field_sql.to_string()) { // Only log if it was in the SET clause
                        let entry = ChangeLogEntry {
                            operation_id: Uuid::new_v4(),
                            entity_table: Self::entity_name().to_string(),
                            entity_id: id,
                            operation_type: ChangeOperationType::Update,
                            field_name: Some($field_sql.to_string()),
                            old_value: Some(serde_json::to_string(&$old_val_expr).unwrap_or_default()),
                            new_value: Some(serde_json::to_string(&$new_val_expr).unwrap_or_default()),
                            timestamp: now_val,
                            user_id: user_uuid_val,
                            device_id: device_uuid_val,
                            document_metadata: None,
                            sync_batch_id: None,
                            processed_at: None,
                            sync_error: None,
                        };
                        self.log_change_entry(entry, &mut tx).await?;
                    }
                };
            }
            log_if_changed_field!("name", old_entity.name, new_entity.name);
            log_if_changed_field!("description", old_entity.description, new_entity.description);
            log_if_changed_field!("icon", old_entity.icon, new_entity.icon);
            log_if_changed_field!("default_priority", old_entity.default_priority, new_entity.default_priority);
            log_if_changed_field!("allowed_extensions", old_entity.allowed_extensions, new_entity.allowed_extensions);
            log_if_changed_field!("max_size", old_entity.max_size, new_entity.max_size);
            log_if_changed_field!("compression_level", old_entity.compression_level, new_entity.compression_level);
            log_if_changed_field!("compression_method", old_entity.compression_method, new_entity.compression_method);
            log_if_changed_field!("min_size_for_compression", old_entity.min_size_for_compression, new_entity.min_size_for_compression);
            log_if_changed_field!("related_tables", old_entity.related_tables, new_entity.related_tables);
            
            Ok(new_entity)
        }.await;

        match result {
            Ok(doc_type) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(doc_type) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }

    async fn find_all(&self, params: PaginationParams) -> DomainResult<PaginatedResult<DocumentType>> {
        let offset = (params.page - 1) * params.per_page;
        let total: i64 = query_scalar("SELECT COUNT(*) FROM document_types WHERE deleted_at IS NULL")
            .fetch_one(&self.pool).await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        let rows = query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?")
            .bind(params.per_page as i64).bind(offset as i64)
            .fetch_all(&self.pool).await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        let items = rows.into_iter().map(Self::map_row).collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }

    async fn find_by_name(&self, name: &str) -> DomainResult<Option<DocumentType>> {
        query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE name = ? AND deleted_at IS NULL")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::Database(DbError::from(e)))?
            .map(Self::map_row)
            .transpose()
    }
}

// --- Media Document Repository ---

#[async_trait]
pub trait MediaDocumentRepository:
    DeleteServiceRepository<MediaDocument> +
    MergeableEntityRepository<MediaDocument> +
    Send + Sync 
{
    async fn create(
        &self,
        new_doc: &NewMediaDocument,
        // file_path provided by service after saving file
    ) -> DomainResult<MediaDocument>;

    // UPDATE methods REMOVED - Documents are immutable via public API

    async fn find_by_related_entity(
        &self,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<MediaDocument>>;

    async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument>;

    async fn find_by_id_with_tx<'t>(&self, id: Uuid, tx: &mut Transaction<'t, Sqlite>) -> DomainResult<MediaDocument>;

    /// Update compression status and optionally the compressed file path and size.
    /// Called internally by compression service.
    async fn update_compression_status(
        &self,
        id: Uuid,
        status: CompressionStatus,
        compressed_file_path: Option<&str>,
        compressed_size_bytes: Option<i64>, // ADDED size
    ) -> DomainResult<()>;

    /// Update blob sync status and key. Called internally by sync service.
    async fn update_blob_sync_status(
        &self,
        id: Uuid,
        status: BlobSyncStatus,
        blob_key: Option<&str>,
    ) -> DomainResult<()>;

    /// Update sync priority for one or more documents. Called internally? Or by specific API?
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncTypes, // CORRECTED
        auth: &AuthContext, // To track who initiated the change
    ) -> DomainResult<u64>;

    /// Update file paths and potentially status after sync download. Called internally by sync service.
    async fn update_paths_and_status(
        &self,
        document_id: Uuid,
        file_path: Option<&str>,
        compressed_file_path: Option<&str>,
        compressed_size_bytes: Option<i64>,
        compression_status: Option<CompressionStatus>,
    ) -> DomainResult<()>;


    /// Links documents created with a temporary ID to the actual entity ID after creation.
    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>, // Requires a transaction
    ) -> DomainResult<u64>; // Returns the number of documents linked
    
    /// Links documents with a temporary ID to their final entity
    /// This method handles the actual implementation and is called by link_temp_documents
    async fn link_temp_documents_with_tx(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<u64>;

    /// Get document counts by related entity
    async fn get_document_counts_by_related_entity(
        &self,
        related_entity_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>>;
    
    /// Find media documents within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<MediaDocument>>;
}

pub struct SqliteMediaDocumentRepository {
    pool: Pool<Sqlite>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteMediaDocumentRepository {
    pub fn new(pool: Pool<Sqlite>, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    fn entity_name() -> &'static str {
        "media_documents" // Table name
    }
     fn map_row(row: MediaDocumentRow) -> DomainResult<MediaDocument> {
        row.into_entity() // Use the conversion method
    }
    // Helper to log changes consistently
    async fn log_change_entry<'t>(
        &self,
        entry: ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await
    }

    async fn find_by_id_option_with_tx<'t>(&self, id: Uuid, tx: &mut Transaction<'t, Sqlite>) -> DomainResult<Option<MediaDocument>> {
        let row_opt = query_as::<_, MediaDocumentRow>(
            "SELECT * FROM media_documents WHERE id = ?"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?;

        match row_opt {
            Some(row) => Ok(Some(Self::map_row(row)?)),
            None => Ok(None),
        }
    }

    async fn insert_from_full_state<'t>(&self, tx: &mut Transaction<'t, Sqlite>, state: &MediaDocumentFullState) -> DomainResult<()> {
        // Values from MediaDocumentFullState (LWW-style)
        let id_str = state.id.to_string();
        let original_filename_str = state.original_filename.as_deref().unwrap_or("unknown.bin");
        let mime_type_str = state.mime_type.as_deref().unwrap_or("application/octet-stream");
        let file_path_str = state.file_path.as_deref().unwrap_or("pending/path"); // This path should ideally be set meaningfully
        let size_bytes_val = state.size_bytes.unwrap_or(0);
        let blob_status_str = state.blob_status.as_deref().unwrap_or(BlobSyncStatus::Pending.as_str());
        let related_table_str = state.related_table.as_deref().unwrap_or("unknown_table");
        let related_id_str = state.related_id.map(|id| id.to_string());
        let description_str = state.description.as_deref();
        let created_at_str = state.created_at.to_rfc3339();
        let created_by_user_id_str = state.created_by_user_id.to_string(); // In FullState, this is Uuid, not Option<Uuid>
        let updated_at_str = state.updated_at.to_rfc3339();
        let updated_by_user_id_str = state.updated_by_user_id.to_string(); // In FullState, this is Uuid, not Option<Uuid>
        let deleted_at_str = state.deleted_at.map(|dt| dt.to_rfc3339());
        let deleted_by_user_id_str = state.deleted_by_user_id.map(|id| id.to_string());

        // Defaults for fields not in MediaDocumentFullState or needing schema defaults
        let temp_related_id_str: Option<String> = None;
        
        // Resolve document type name to its UUID. If not found, skip insertion with error.
        let type_id_result: Option<String> = sqlx::query_scalar(
            "SELECT id FROM document_types WHERE name = ? AND deleted_at IS NULL"
        )
        .bind(&state.document_type)
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?;

        let type_id_str = match type_id_result {
            Some(id) => id,
            None => {
                return Err(DomainError::Validation(ValidationError::custom(&format!(
                    "Unknown document type name: {:?}", state.document_type
                ))));
            }
        };

        let compressed_file_path_str: Option<String> = None;
        let compressed_size_bytes_val: Option<i64> = None;
        let field_identifier_str: Option<String> = None;
        let title_str: Option<String> = None;
        let has_error_val: i32 = 0; // Default 0 (false)
        let error_message_str: Option<String> = None;
        let error_type_str: Option<String> = None;
        let compression_status_str = CompressionStatus::Pending.as_str(); // Default 'pending'
        let blob_key_str: Option<String> = None;
        // Assuming SyncPriorityFromSyncDomain is available via `use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;`
        let sync_priority_str = SyncPriorityFromSyncTypes::Normal.as_str(); // Default 'normal'
        let last_sync_attempt_at_str: Option<String> = None;
        let sync_attempt_count_val: i32 = 0; // Default 0
        // New field: source_of_change
        let source_of_change_str = "sync"; // Because insert_from_full_state is called for remote changes

        // Ensure all NOT NULL columns in media_documents are covered
        sqlx::query!(
            r#"
            INSERT INTO media_documents (
                id, related_table, related_id, temp_related_id, type_id,
                original_filename, file_path, compressed_file_path, compressed_size_bytes,
                field_identifier, title, description, mime_type, size_bytes,
                has_error, error_message, error_type, compression_status,
                blob_status, blob_key, sync_priority, source_of_change, last_sync_attempt_at,
                sync_attempt_count, created_at, updated_at, created_by_user_id,
                updated_by_user_id, deleted_at, deleted_by_user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id_str, related_table_str, related_id_str, temp_related_id_str, type_id_str,
            original_filename_str, file_path_str, compressed_file_path_str, compressed_size_bytes_val,
            field_identifier_str, title_str, description_str, mime_type_str, size_bytes_val,
            has_error_val, error_message_str, error_type_str, compression_status_str,
            blob_status_str, blob_key_str, sync_priority_str, source_of_change_str, last_sync_attempt_at_str,
            sync_attempt_count_val, created_at_str, updated_at_str, created_by_user_id_str,
            updated_by_user_id_str, deleted_at_str, deleted_by_user_id_str
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            eprintln!("Insert from full state failed. State: {:?}, Error: {:?}", state, e);
            DbError::from(e)
        })?;
        Ok(())
    }
}

// --- Basic trait implementations remain the same ---

#[async_trait]
impl FindById<MediaDocument> for SqliteMediaDocumentRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument> {
        query_as::<_, MediaDocumentRow>(
            "SELECT * FROM media_documents WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        .and_then(Self::map_row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteMediaDocumentRepository {
     async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let device_uuid: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse::<Uuid>().ok() };

        let result = query(
            "UPDATE media_documents SET deleted_at = ?, deleted_by_user_id = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&now)
        .bind(auth.user_id.to_string())
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else {
            Ok(())
        }
    }
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?; // Changed map_err(DbError::from)
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(()) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteMediaDocumentRepository {
     fn entity_name(&self) -> &'static str {
        "media_documents"
    }
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
         let result = query("DELETE FROM media_documents WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(<Self as HardDeletable>::entity_name(self).to_string(), id))
        } else {
            Ok(())
        }
    }
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?; // Changed map_err(DbError::from)
        // Consider adding file system cleanup logic here or in the service calling this
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(()) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

// --- MediaDocumentRepository implementation ---

#[async_trait]
impl MediaDocumentRepository for SqliteMediaDocumentRepository {
    async fn create(
        &self,
        new_doc: &NewMediaDocument,
        // file_path is NOT part of NewMediaDocument DTO. Assumed to be handled by service.
        // The file_path column in DB will be set by the service calling create or later update.
    ) -> DomainResult<MediaDocument> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            let now = Utc::now().to_rfc3339();
            let now_dt = Utc::now(); // For logging
            let user_uuid = new_doc.created_by_user_id;
            // NOTE: NewMediaDocument does not have device_id. Logging will use None.
            // If device_id logging is needed here, add it to NewMediaDocument DTO.
            let device_uuid: Option<Uuid> = None;

            // Use the user_uuid captured outside the async block for consistency
            let user_id_str_bind = user_uuid.map(|id| id.to_string());

            // Determine related_table and related_id based on temp_related_id
            let (actual_related_table, actual_related_id_str) = if new_doc.temp_related_id.is_some() {
                (TEMP_RELATED_TABLE.to_string(), None) // Store temp ID separately
            } else {
                // If temp_id is None, related_id MUST be Some (validated in DTO)
                (new_doc.related_table.clone(), new_doc.related_id.map(|id| id.to_string()))
            };
            let temp_related_id_str = new_doc.temp_related_id.map(|id| id.to_string());

            // REMOVED file_path and description from INSERT list and bind list
            // Assumes file_path will be set later, description column might not exist or is optional
            query(
                r#"INSERT INTO media_documents (
                    id, related_table, related_id, type_id,
                    original_filename, compressed_file_path, compressed_size_bytes,
                    field_identifier, title, mime_type, size_bytes,
                    compression_status, blob_key, blob_status, sync_priority,
                    temp_related_id,
                    created_at, updated_at, created_by_user_id, updated_by_user_id,
                    deleted_at, deleted_by_user_id,
                    file_path -- explicitly setting file_path to NULL initially
                ) VALUES (
                    ?, ?, ?, ?, -- id, related_table, related_id, type_id
                    ?, NULL, NULL, -- original_filename, compressed_file_path, compressed_size_bytes
                    ?, ?, ?, ?, -- field_identifier, title, mime_type, size_bytes
                    ?, NULL, ?, ?, -- compression_status, blob_key, blob_status, sync_priority
                    ?, -- temp_related_id
                    ?, ?, ?, ?, -- created_at, updated_at, created_by_user_id, updated_by_user_id
                    NULL, NULL, -- deleted_at, deleted_by_user_id
                    NULL -- file_path
                )"#
            )
            .bind(new_doc.id.to_string())
            .bind(actual_related_table) // Store actual or TEMP_RELATED_TABLE
            .bind(actual_related_id_str) // Store actual ID or NULL if temp
            .bind(new_doc.type_id.to_string())
            .bind(&new_doc.original_filename)
             // compressed fields initialized as NULL
            .bind(&new_doc.field_identifier)
            .bind(&new_doc.title)
            .bind(&new_doc.mime_type)
            .bind(new_doc.size_bytes)
            .bind(CompressionStatus::Pending.as_str()) // Default status
            .bind(BlobSyncStatus::Pending.as_str()) // Default status
            .bind(
                SyncPriorityFromSyncTypes::from_str(&new_doc.sync_priority) // CORRECTED
                    .map_err(|_| DomainError::Validation(ValidationError::custom(&format!("Invalid sync priority string: {}", new_doc.sync_priority))))?
                    .as_str() // Bind the string representation
            )
            .bind(temp_related_id_str) // Store temp ID if provided
            .bind(&now).bind(&now)
            .bind(user_id_str_bind.as_deref()).bind(user_id_str_bind.as_deref()) // Use Option<String> binding for both
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                 if let sqlx::Error::Database(db_err) = &e {
                     if db_err.is_unique_violation() {
                         return DomainError::Database(DbError::Conflict(format!(
                             "MediaDocument with ID {} already exists.", new_doc.id
                         )));
                     }
                      // Check for foreign key violation on type_id
                     if db_err.message().contains("FOREIGN KEY constraint failed") {
                         // FIX: Use ValidationError::Custom instead of non-existent foreign_key variant
                         return DomainError::Validation(ValidationError::Custom(format!(
                             "Invalid document type ID ({}): Does not exist.", new_doc.type_id
                         )));
                     }
                 }
                 DomainError::Database(DbError::from(e))
             })?;

            // Log Create Operation
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::entity_name().to_string(),
                entity_id: new_doc.id,
                operation_type: ChangeOperationType::Create,
                field_name: None,
                old_value: None,
                new_value: None, // Optionally serialize new_doc
                timestamp: now_dt,
                user_id: user_uuid.unwrap_or_else(Uuid::nil), // Provide default if None
                device_id: device_uuid,
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;

            self.find_by_id_with_tx(new_doc.id, &mut tx).await
        }.await;

        match result {
            Ok(doc) => { 
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?; 
                Ok(doc) 
            },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }

    async fn find_by_related_entity(
        &self,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<MediaDocument>> {
        let offset = (params.page - 1) * params.per_page;
        let related_id_str = related_id.to_string();

        let count_query = query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM media_documents WHERE related_table = ? AND related_id = ? AND deleted_at IS NULL"
        ).bind(related_table).bind(&related_id_str);

        let total: i64 = count_query.fetch_one(&self.pool).await.map_err(DbError::from)?;

        // Order by creation date, newest first
        let select_query = query_as::<_, MediaDocumentRow>(
            "SELECT * FROM media_documents WHERE related_table = ? AND related_id = ? AND deleted_at IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?"
        ).bind(related_table).bind(related_id_str).bind(params.per_page as i64).bind(offset as i64);

        let rows = select_query.fetch_all(&self.pool).await.map_err(DbError::from)?;
        let items = rows.into_iter().map(Self::map_row).collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }

    async fn update_compression_status(
        &self,
        id: Uuid,
        status: CompressionStatus,
        compressed_file_path: Option<&str>,
        compressed_size_bytes: Option<i64>, // ADDED size
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let old_entity = self.find_by_id_with_tx(id, &mut tx).await.ok(); // Ignore error if not found for logging
        let now = Utc::now();
        let system_user_id = Uuid::nil(); // System operation
        let device_uuid: Option<Uuid> = None;

        let result = query(
            "UPDATE media_documents SET compression_status = ?, compressed_file_path = ?, compressed_size_bytes = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status.as_str())
        .bind(compressed_file_path)
        .bind(compressed_size_bytes) // Bind the size
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .execute(&mut *tx) // Use transaction
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else {
            // Log changes if old entity was found
            if let Some(old) = old_entity {
                // Fetch new state within the transaction
                let new_entity = self.find_by_id_with_tx(id, &mut tx).await?;

                macro_rules! log_if_changed {
                    ($field_name:ident, $field_sql:literal) => {
                        if old.$field_name != new_entity.$field_name {
                            let entry = ChangeLogEntry {
                                operation_id: Uuid::new_v4(),
                                entity_table: Self::entity_name().to_string(),
                                entity_id: id,
                                operation_type: ChangeOperationType::Update,
                                field_name: Some($field_sql.to_string()),
                                old_value: serde_json::to_string(&old.$field_name).ok(),
                                new_value: serde_json::to_string(&new_entity.$field_name).ok(),
                                timestamp: now,
                                user_id: system_user_id,
                                device_id: device_uuid.clone(),
                                document_metadata: None,
                                sync_batch_id: None,
                                processed_at: None,
                                sync_error: None,
                            };
                            self.log_change_entry(entry, &mut tx).await?;
                        }
                    };
                }
                log_if_changed!(compression_status, "compression_status");
                log_if_changed!(compressed_file_path, "compressed_file_path");
                log_if_changed!(compressed_size_bytes, "compressed_size_bytes");
            }
            tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
            Ok(())
        }
    }

    async fn update_blob_sync_status(
        &self,
        id: Uuid,
        status: BlobSyncStatus,
        blob_key: Option<&str>,
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let now = Utc::now();
        let now_str = now.to_rfc3339(); // Define now_str
        let status_str = status.as_str(); // Create a binding for status string
        let id_string = id.to_string(); // Create a binding for id string

        let result = sqlx::query!(
            "UPDATE media_documents SET blob_status = ?, blob_key = ?, updated_at = ? WHERE id = ?",
            status_str, // Use bound variable
            blob_key,
            now_str,
            id_string // Use bound variable
        )
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await; // Rollback on failure to find
            return Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id));
        }

        // Log change if needed (e.g., if blob_status is considered a syncable field)
        // For now, assuming this is an internal status update, not requiring a full change log entry.
        // If it *does* require a change log for LWW, this would be more complex.
        // Example light-weight log:
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: Self::entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Update, // Internal status updates are still 'Update'
            field_name: Some("blob_status_update".to_string()), // Specific action/field
            old_value: None, // Could fetch and serialize old status if needed for full LWW
            new_value: Some(format!("status: {}, key: {:?}", status.as_str(), blob_key)), // Details of the change
            timestamp: now, // Use the 'now' from earlier in the function
            user_id: Uuid::nil(), // System operation
            device_id: None, // System operation
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.log_change_entry(entry, &mut tx).await?;

        match tx.commit().await {
            Ok(_) => Ok(()),
            Err(e) => Err(DomainError::Database(DbError::from(e))),
        }
    }

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncTypes, // CORRECTED
        auth: &AuthContext, // To track who initiated the change
    ) -> DomainResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        // --- Fetch Old Priorities ---
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query = format!(
            "SELECT id, sync_priority FROM media_documents WHERE id IN ({})",
            vec!["?"; ids.len()].join(", ")
        );
        let mut select_builder = query_as::<_, (String, Option<String>)>(&select_query); // Expect Option<TEXT>
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: std::collections::HashMap<Uuid, SyncPriorityFromSyncTypes> = select_builder
            .fetch_all(&mut *tx) // Changed from &mut **tx to &mut *tx
            .await.map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str_val, prio_text_option)| { // Renamed id_str to id_str_val to avoid conflict
                match Uuid::parse_str(&id_str_val) {
                     Ok(id_uuid) => { // Renamed id to id_uuid
                        let priority_str_slice = prio_text_option.as_deref().unwrap_or_else(|| SyncPriorityFromSyncTypes::Normal.as_str());
                        Some((id_uuid, SyncPriorityFromSyncTypes::from_str(priority_str_slice).unwrap_or(SyncPriorityFromSyncTypes::Normal)))
                     },
                     Err(_) => None,
                }
            }).collect();

        let now = Utc::now().to_rfc3339();
        let now_dt = Utc::now(); // For logging
        let user_id_str = auth.user_id.to_string();
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse::<Uuid>().ok() };
        let priority_str = priority.as_str(); // Store as string

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE media_documents
             SET sync_priority = ?, updated_at = ?, updated_by_user_id = ?
             WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut query_builder = sqlx::query(&query_str)
            .bind(priority_str) // Bind string
            .bind(now)
            .bind(user_id_str);

        for id in ids {
            query_builder = query_builder.bind(id.to_string());
        }

        let result = query_builder.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();

        // --- Log Changes ---
        for id in ids {
            if let Some(old_priority) = old_priorities.get(id) {
                if *old_priority != priority { // Compare SyncPriority enums
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: Self::entity_name().to_string(),
                        entity_id: *id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: serde_json::to_string(old_priority.as_str()).ok(), // Log old priority as string
                        new_value: serde_json::to_string(priority_str).ok(), // Log new priority as string
                        timestamp: now_dt,
                        user_id: user_uuid,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.log_change_entry(entry, &mut tx).await?;
                }
            }
        }

        tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        Ok(rows_affected)
    }

    /// Internal method to find by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<MediaDocument> {
        query_as::<_, MediaDocumentRow>("SELECT * FROM media_documents WHERE id = ? AND deleted_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(DbError::from)?
            .ok_or_else(|| DomainError::EntityNotFound(Self::entity_name().to_string(), id))
            .and_then(Self::map_row)
    }

    // Re-implement find_by_id to satisfy the trait bound, delegating to the public FindById impl
    async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument> {
        <Self as FindById<MediaDocument>>::find_by_id(self, id).await
    }

     /// Links documents created with a temporary ID to the actual entity ID.
     /// Implementation that delegates to link_temp_documents_with_tx
    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<u64> {
        self.link_temp_documents_with_tx(
            temp_related_id,
            final_related_table,
            final_related_id,
            tx
        ).await
    }
    
    /// Implementation of the actual linking logic
    async fn link_temp_documents_with_tx(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<u64> {
        let now_dt = Utc::now(); // For logging
        // Assume this is called by a system user or within a context without specific user/device
        let system_user_id = Uuid::nil(); // Placeholder system user ID
        let device_uuid: Option<Uuid> = None;

        // --- Fetch IDs and old paths/state BEFORE updating ---
        let temp_id_str_fetch = temp_related_id.to_string();
        struct OldDocState {
            id: Uuid,
            file_path: Option<String>,
            compressed_file_path: Option<String>,
        }
        let old_docs = query_as::<_, (String, Option<String>, Option<String>)>(r#"
            SELECT id, file_path, compressed_file_path
            FROM media_documents
            WHERE temp_related_id = ? AND related_table = ?
            "#)
            .bind(&temp_id_str_fetch)
            .bind(TEMP_RELATED_TABLE)
            .fetch_all(&mut **tx)
            .await
            .map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, fp, cfp)| {
                Uuid::parse_str(&id_str).ok().map(|id| OldDocState {
                    id,
                    file_path: fp,
                    compressed_file_path: cfp,
                })
            })
            .collect::<Vec<_>>();

        if old_docs.is_empty() {
            return Ok(0); // No documents to link or log
        }

        // First query to count how many documents will be updated
        let temp_id_str_count = temp_related_id.to_string(); // Store in variable
        let count = sqlx::query!(
            r#"
            SELECT COUNT(*) as count
            FROM media_documents
            WHERE temp_related_id = ? AND related_table = ?
            "#,
            temp_id_str_count, // Use variable
            TEMP_RELATED_TABLE
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| {
            DomainError::Database(DbError::from(e)) // Simplified error mapping
        })?
        .count as u64;

        if count == 0 {
            return Ok(0); // No documents to update
        }

        // Update the documents to point to the final entity
        let final_id_str_update = final_related_id.to_string(); // Store in variable
        let now_str_update = Utc::now().to_rfc3339();        // Store in variable
        let temp_id_str_update = temp_related_id.to_string();   // Store in variable
        let rows_affected = sqlx::query!(
            r#"
            UPDATE media_documents
            SET related_id = ?, 
                related_table = ?, 
                temp_related_id = NULL,
                updated_at = ? -- Also update timestamp
            WHERE temp_related_id = ? AND related_table = ?
            "#,
            final_id_str_update,  // Use variable
            final_related_table,
            now_str_update,       // Use variable
            temp_id_str_update,   // Use variable
            TEMP_RELATED_TABLE
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            DomainError::Database(DbError::from(e)) // Simplified error mapping
        })?
        .rows_affected() as u64;

        // --- Log Field Changes ---
        // Fetch new paths after update
        let ids_to_fetch: Vec<String> = old_docs.iter().map(|d| d.id.to_string()).collect();
        let query_fetch_new_paths = format!(
            "SELECT id, file_path, compressed_file_path FROM media_documents WHERE id IN ({})",
            vec!["?"; ids_to_fetch.len()].join(", ")
        );
        let mut new_paths_builder = query_as::<_, (String, Option<String>, Option<String>)>(&query_fetch_new_paths);
        for id_str in &ids_to_fetch {
            new_paths_builder = new_paths_builder.bind(id_str);
        }
        let new_paths_map: HashMap<Uuid, (Option<String>, Option<String>)> = new_paths_builder
            .fetch_all(&mut **tx)
            .await
            .map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, fp, cfp)| Uuid::parse_str(&id_str).ok().map(|id| (id, (fp, cfp))))
            .collect();

        for old_doc in old_docs {
            let new_paths = new_paths_map.get(&old_doc.id);
            let new_file_path = new_paths.and_then(|(fp, _)| fp.clone());
            let new_compressed_path = new_paths.and_then(|(_, cfp)| cfp.clone());

            // Log related_id change
            self.log_change_entry(ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::entity_name().to_string(),
                entity_id: old_doc.id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("related_id".to_string()),
                old_value: serde_json::to_string(&Option::<Uuid>::None).ok(), // Was implicitly null when temp_related_id was set
                new_value: serde_json::to_string(&Some(final_related_id)).ok(),
                timestamp: now_dt,
                user_id: system_user_id,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            }, tx).await?;

            // Log related_table change
            self.log_change_entry(ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::entity_name().to_string(),
                entity_id: old_doc.id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("related_table".to_string()),
                old_value: Some(TEMP_RELATED_TABLE.to_string()),
                new_value: Some(final_related_table.to_string()),
                timestamp: now_dt,
                user_id: system_user_id,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            }, tx).await?;

            // Log temp_related_id change (becoming NULL)
            self.log_change_entry(ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::entity_name().to_string(),
                entity_id: old_doc.id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("temp_related_id".to_string()),
                old_value: serde_json::to_string(&Some(temp_related_id)).ok(),
                new_value: serde_json::to_string(&Option::<Uuid>::None).ok(),
                timestamp: now_dt,
                user_id: system_user_id,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            }, tx).await?;

            // Log file_path change if it changed
            if old_doc.file_path != new_file_path {
                self.log_change_entry(ChangeLogEntry {
                    operation_id: Uuid::new_v4(),
                    entity_table: Self::entity_name().to_string(),
                    entity_id: old_doc.id,
                    operation_type: ChangeOperationType::Update,
                    field_name: Some("file_path".to_string()),
                    old_value: serde_json::to_string(&old_doc.file_path).ok(),
                    new_value: serde_json::to_string(&new_file_path).ok(),
                    timestamp: now_dt,
                    user_id: system_user_id,
                    device_id: device_uuid.clone(),
                    document_metadata: None,
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                }, tx).await?;
            }

            // Log compressed_file_path change if it changed
            if old_doc.compressed_file_path != new_compressed_path {
                self.log_change_entry(ChangeLogEntry {
                    operation_id: Uuid::new_v4(),
                    entity_table: Self::entity_name().to_string(),
                    entity_id: old_doc.id,
                    operation_type: ChangeOperationType::Update,
                    field_name: Some("compressed_file_path".to_string()),
                    old_value: serde_json::to_string(&old_doc.compressed_file_path).ok(),
                    new_value: serde_json::to_string(&new_compressed_path).ok(),
                    timestamp: now_dt,
                    user_id: system_user_id,
                    device_id: device_uuid.clone(),
                    document_metadata: None,
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                }, tx).await?;
            }
        }

        Ok(rows_affected) // Return rows affected by the main linking update
    }

     /// Updates paths and status, typically after a sync download.
     async fn update_paths_and_status(
         &self,
         document_id: Uuid,
         file_path: Option<&str>,
         compressed_file_path: Option<&str>,
         compressed_size_bytes: Option<i64>,
         compression_status: Option<CompressionStatus>,
     ) -> DomainResult<()> {
          let mut tx = self.pool.begin().await.map_err(DbError::from)?;

          let old_entity = match self.find_by_id_with_tx(document_id, &mut tx).await {
             Ok(entity) => entity,
             Err(e) => {
                 let _ = tx.rollback().await;
                 return Err(e);
             }
         };

        let mut qb = QueryBuilder::new("UPDATE media_documents SET ");
        let mut separated = qb.separated(", ");
        let mut updates_made = false;

        // Temporary owned string for compression_status if it's not 'static str
        let compression_status_val_str: Option<String> = 
            compression_status.map(|s| s.as_str().to_string());

        if let Some(val) = file_path {
            separated.push("file_path = ");
            separated.push_bind_unseparated(val);
            updates_made = true;
        }
        if let Some(val) = compressed_file_path {
            separated.push("compressed_file_path = ");
            separated.push_bind_unseparated(val);
            updates_made = true;
        }
        if let Some(val) = compressed_size_bytes {
            separated.push("compressed_size_bytes = ");
            separated.push_bind_unseparated(val);
            updates_made = true;
        }
        if let Some(ref val_str) = compression_status_val_str { // Borrow the Option<String>
            separated.push("compression_status = ");
            separated.push_bind_unseparated(val_str); // Bind &String
            updates_made = true;
        }

        if !updates_made {
            // If no specific fields were updated, still update `updated_at` if we proceed,
            // or return early if no update at all is desired.
            // For now, let's assume if called, an update to `updated_at` is intended.
            // If no fields were provided, we might want to return Ok(()) to avoid an empty SET.
            // The current logic will proceed to update `updated_at`.
             // If STRICTLY no update if no optional fields are Some, then:
            // return Ok(()); 
            // However, the old logic proceeded to update `updated_at` anyway.
            // We'll keep that behavior: if called, at least `updated_at` will be set.
            // If `updates_made` is false and we want to ensure `updated_at` is the only thing set:
            if !updates_made { // If this is the first item
                 //qb.push("updated_at = "); // No comma needed if it's the only set
            } else { // if other items were set, add comma
                 //qb.push(", updated_at = ");
            }
            // This logic with separated handles it better.
        }
        
        // Always update updated_at
        let now = Utc::now();
        let now_str = now.to_rfc3339(); // Corrected: ensure rfc3339
        
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.as_str()); // Bind as &str

        // Optionally update updated_by_user_id if tracking sync agent ID
        // let system_user_id = Uuid::nil(); 
        // separated.push("updated_by_user_id = ");
        // separated.push_bind_unseparated(system_user_id.to_string());


        qb.push(" WHERE id = ");
        qb.push_bind(document_id.to_string());

        let final_query = qb.build();
        let result = final_query.execute(&mut *tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), document_id))
        } else {
            let new_entity = self.find_by_id_with_tx(document_id, &mut tx).await?;
            let system_user_id = Uuid::nil(); // Assuming system operation for logging these internal updates
            let device_uuid: Option<Uuid> = None; // Assuming system operation

            macro_rules! log_if_changed {
                ($field_name:ident, $field_sql:literal) => {
                    if old_entity.$field_name != new_entity.$field_name {
                        let entry = ChangeLogEntry {
                            operation_id: Uuid::new_v4(),
                            entity_table: Self::entity_name().to_string(),
                            entity_id: document_id,
                            operation_type: ChangeOperationType::Update,
                            field_name: Some($field_sql.to_string()),
                            old_value: serde_json::to_string(&old_entity.$field_name).ok(),
                            new_value: serde_json::to_string(&new_entity.$field_name).ok(),
                            timestamp: now, // Use the same 'now' for all logs in this op
                            user_id: system_user_id,
                            device_id: device_uuid.clone(),
                            document_metadata: None,
                            sync_batch_id: None,
                            processed_at: None,
                            sync_error: None,
                        };
                        self.log_change_entry(entry, &mut tx).await?;
                    }
                };
            }

            log_if_changed!(file_path, "file_path");
            log_if_changed!(compressed_file_path, "compressed_file_path");
            log_if_changed!(compressed_size_bytes, "compressed_size_bytes");
            log_if_changed!(compression_status, "compression_status");

            tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
            Ok(())
        }
    }

    // --- Added: required trait methods ---------------------------------------

    async fn get_document_counts_by_related_entity(
        &self,
        related_entity_ids: &[Uuid],
    ) -> DomainResult<HashMap<Uuid, i64>> {
        let mut counts = HashMap::new();
        for &entity_id in related_entity_ids {
            let count: i64 = query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM media_documents WHERE related_entity_id = ? AND deleted_at IS NULL",
            )
            .bind(entity_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
            counts.insert(entity_id, count);
        }
        Ok(counts)
    }

    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<MediaDocument>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM media_documents\n             WHERE deleted_at IS NULL\n             AND ((created_at >= ? AND created_at <= ?)\n                  OR (updated_at >= ? AND updated_at <= ?))",
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        let rows = query_as::<_, MediaDocumentRow>(
            "SELECT * FROM media_documents\n             WHERE deleted_at IS NULL\n             AND ((created_at >= ? AND created_at <= ?)\n                  OR (updated_at >= ? AND updated_at <= ?))\n             ORDER BY created_at DESC LIMIT ? OFFSET ?",
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

        let items = rows
            .into_iter()
            .map(Self::map_row)
            .collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }
}

#[async_trait]
impl MergeableEntityRepository<MediaDocument> for SqliteMediaDocumentRepository {
    fn entity_name(&self) -> &'static str { "media_documents" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        if remote_change.entity_table != <Self as MergeableEntityRepository<MediaDocument>>::entity_name(self) {
            return Err(DomainError::Internal(format!(
                "MediaDocumentRepository received change for wrong table: {}",
                remote_change.entity_table
            )));
        }
        let document_id = remote_change.entity_id;
        use crate::domains::sync::types::ChangeOperationType as Op;
        match remote_change.operation_type {
            Op::Create => {
                let state_json = remote_change.new_value.as_ref()
                    .ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for document create")))?;
                let payload: MediaDocumentFullState = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_document_create", &format!("Invalid JSON: {}", e))))?;
                if self.find_by_id_option_with_tx(document_id, tx).await?.is_some() {
                    return Ok(MergeOutcome::NoOp("Document already exists".to_string()));
                }
                self.insert_from_full_state(tx, &payload).await?;
                Ok(MergeOutcome::Created(document_id))
            }
            Op::Update => {
                // Simplified LWW: overwrite whole row if exists else NoOp
                let state_json = remote_change.new_value.as_ref()
                    .ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for document update")))?;
                let payload: MediaDocumentFullState = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_document_update", &format!("Invalid JSON: {}", e))))?;
                if self.find_by_id_option_with_tx(document_id, tx).await?.is_none() {
                    // treat as create
                    self.insert_from_full_state(tx, &payload).await?;
                    return Ok(MergeOutcome::Created(document_id));
                }
                // Overwrite some columns (simplified)
                let original_filename_bind = payload.original_filename.as_deref();
                let mime_type_bind = payload.mime_type.as_deref();
                let size_bytes_bind = payload.size_bytes.unwrap_or(0);
                let updated_at_bind = payload.updated_at.to_rfc3339();
                let document_id_bind = document_id.to_string();

                query!(r#"UPDATE media_documents SET
                    original_filename = ?, mime_type = ?, size_bytes = ?, updated_at = ?
                    WHERE id = ?"#,
                    original_filename_bind,
                    mime_type_bind,
                    size_bytes_bind,
                    updated_at_bind,
                    document_id_bind)
                    .execute(&mut **tx).await.map_err(DbError::from)?;
                Ok(MergeOutcome::Updated(document_id))
            }
            Op::HardDelete => {
                if self.find_by_id_option_with_tx(document_id, tx).await?.is_none() {
                    return Ok(MergeOutcome::NoOp("Already deleted".to_string()));
                }
                let temp_auth = AuthContext::internal_system_context();
                self.hard_delete_with_tx(document_id, &temp_auth, tx).await?;
                Ok(MergeOutcome::HardDeleted(document_id))
            }
            Op::Delete => {
                Ok(MergeOutcome::NoOp("Soft delete ignored".to_string()))
            }
        }
    }
}

// --- Document Version Repository --- (Assumed schema matches types)

#[async_trait]
pub trait DocumentVersionRepository: Send + Sync {
    async fn create(
        &self,
        doc_id: Uuid,
        version_number: i64,
        file_path: &str,
        file_size: i64,
        mime_type: &str,
        blob_key: Option<&str>,
        auth: &AuthContext, // Assuming versions are created by user actions
    ) -> DomainResult<DocumentVersion>;

    async fn find_by_document_id(&self, doc_id: Uuid) -> DomainResult<Vec<DocumentVersion>>;
}

pub struct SqliteDocumentVersionRepository {
    pool: Pool<Sqlite>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteDocumentVersionRepository {
    pub fn new(pool: Pool<Sqlite>, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }
    fn map_row(row: DocumentVersionRow) -> DomainResult<DocumentVersion> {
        row.into_entity()
    }
}

#[async_trait]
impl DocumentVersionRepository for SqliteDocumentVersionRepository {
    async fn create(
        &self,
        doc_id: Uuid,
        version_number: i64,
        file_path: &str,
        file_size: i64,
        mime_type: &str,
        blob_key: Option<&str>,
        auth: &AuthContext,
    ) -> DomainResult<DocumentVersion> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let now_dt = Utc::now(); // For logging
        let user_id_str = auth.user_id.to_string();
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        let result = async {
            query(
                r#"INSERT INTO document_versions (
                    id, document_id, version_number, file_path, file_size, mime_type, blob_storage_key,
                    created_at, created_by_user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
            )
            .bind(id.to_string())
            .bind(doc_id.to_string())
            .bind(version_number)
            .bind(file_path)
            .bind(file_size)
            .bind(mime_type)
            .bind(blob_key)
            .bind(&now)
            .bind(user_id_str) // Use user ID from AuthContext
            .execute(&mut *tx)
            .await
            .map_err(DbError::from)?;

            // Log Create Operation within the transaction
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: "document_versions".to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Create,
                field_name: None,
                old_value: None,
                new_value: None,
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid,
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;

            Ok(id) // Return ID to fetch after commit
        }.await;

        match result {
            Ok(created_id) => {
                tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
                // Fetch the newly created record outside the transaction
                let row = query_as::<_, DocumentVersionRow>("SELECT * FROM document_versions WHERE id = ?")
                   .bind(created_id.to_string()).fetch_one(&self.pool).await.map_err(DbError::from)?;
                Self::map_row(row)
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn find_by_document_id(&self, doc_id: Uuid) -> DomainResult<Vec<DocumentVersion>> {
        let rows = query_as::<_, DocumentVersionRow>(
            "SELECT * FROM document_versions WHERE document_id = ? ORDER BY version_number DESC"
        )
        .bind(doc_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        rows.into_iter().map(Self::map_row).collect()
    }
}

// --- Document Access Log Repository --- (Assumed schema matches types)

#[async_trait]
pub trait DocumentAccessLogRepository: Send + Sync {
     async fn create(&self, new_log: &NewDocumentAccessLog) -> DomainResult<DocumentAccessLog>;
     async fn find_by_document_id(&self, doc_id: Uuid, params: PaginationParams) -> DomainResult<PaginatedResult<DocumentAccessLog>>;
}

pub struct SqliteDocumentAccessLogRepository {
    pool: Pool<Sqlite>
}

impl SqliteDocumentAccessLogRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    fn map_row(row: DocumentAccessLogRow) -> DomainResult<DocumentAccessLog> {
        row.into_entity()
    }
}

#[async_trait]
impl DocumentAccessLogRepository for SqliteDocumentAccessLogRepository {
    async fn create(&self, new_log: &NewDocumentAccessLog) -> DomainResult<DocumentAccessLog> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();

        query(
            "INSERT INTO document_access_logs (id, document_id, user_id, access_type, access_date, details) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(new_log.document_id.to_string())
        .bind(new_log.user_id.to_string()) // Use provided user_id (could be system user)
        .bind(&new_log.access_type) // Assumes access_type is already validated string
        .bind(&now)
        .bind(&new_log.details)
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch the created record
        let row = query_as::<_, DocumentAccessLogRow>("SELECT * FROM document_access_logs WHERE id = ?")
            .bind(id.to_string()).fetch_one(&self.pool).await.map_err(DbError::from)?;
        Self::map_row(row)
    }

     async fn find_by_document_id(&self, doc_id: Uuid, params: PaginationParams) -> DomainResult<PaginatedResult<DocumentAccessLog>> {
        let offset = (params.page - 1) * params.per_page;
        let doc_id_str = doc_id.to_string();

        let count_query = query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM document_access_logs WHERE document_id = ?"
        ).bind(&doc_id_str);
        let total: i64 = count_query.fetch_one(&self.pool).await.map_err(DbError::from)?;

        let select_query = query_as::<_, DocumentAccessLogRow>(
            "SELECT * FROM document_access_logs WHERE document_id = ? ORDER BY access_date DESC LIMIT ? OFFSET ?"
        ).bind(doc_id_str).bind(params.per_page as i64).bind(offset as i64);

        let rows = select_query.fetch_all(&self.pool).await.map_err(DbError::from)?;
        let items = rows.into_iter().map(Self::map_row).collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }
}