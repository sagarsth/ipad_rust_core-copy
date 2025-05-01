use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, SoftDeletable, HardDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::document::types::{
    DocumentType, DocumentTypeRow, NewDocumentType, UpdateDocumentType,
    MediaDocument, MediaDocumentRow, NewMediaDocument, CompressionStatus, BlobSyncStatus,
    // UpdateMediaDocument, // REMOVED
    DocumentVersion, DocumentVersionRow,
    DocumentAccessLog, DocumentAccessLogRow, NewDocumentAccessLog
};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::domains::sync::types::SyncPriority;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar, Pool, Row, Sqlite, Transaction};
use uuid::Uuid;
use std::collections::HashMap; // For update_paths_and_status
use std::str::FromStr; // Import FromStr if not already imported at the top of the file
use crate::types::{PaginatedResult, PaginationParams};
// --- Add imports for ChangeLog ---
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType};
use std::sync::Arc;
use serde_json;

/// Temporary table identifier for linking documents before the main entity exists
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
    fn entity_name() -> &'static str {
        "document_types" // Table name
    }
    fn map_row(row: DocumentTypeRow) -> DomainResult<DocumentType> {
        row.into_entity() // Use the conversion method
    }
    // Helper for internal use within transactions
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<DocumentType> {
        query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE id = ? AND deleted_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(&mut **tx)
            .await
            .map_err(DbError::from)?
            .ok_or_else(|| DomainError::EntityNotFound(Self::entity_name().to_string(), id))
            .and_then(Self::map_row)
    }
    // Helper to log changes consistently
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
            .map_err(DbError::from)?
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
        let now_dt = Utc::now(); // For logging
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();

        let result = query(
            "UPDATE document_types SET deleted_at = ?, deleted_by_user_id = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&now)
        .bind(auth.user_id.to_string())
        .bind(&now)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Could be already deleted or not found
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else {
            Ok(())
        }
    }
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteDocumentTypeRepository {
    fn entity_name(&self) -> &'static str {
        "document_types"
    }
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Auth context might be used for logging/checks later
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now_dt = Utc::now(); // For logging
        let user_uuid = _auth.user_id;
        let device_uuid: Option<Uuid> = _auth.device_id.parse::<Uuid>().ok();

        let result = query("DELETE FROM document_types WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(self.entity_name().to_string(), id))
        } else {
            Ok(())
        }
    }
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        // Consider adding file system cleanup logic here or in the service calling this
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
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
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        let result = async {
            let now = Utc::now().to_rfc3339();
            let now_dt = Utc::now(); // Use DateTime<Utc> for logging
            let user_id = auth.user_id.to_string();
            let user_uuid = auth.user_id;
            let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();

            query(
                r#"INSERT INTO document_types (
                    id, name, description, icon, color, default_priority, -- Added default_priority
                    created_at, updated_at, created_by_user_id, updated_by_user_id,
                    name_updated_at, name_updated_by, description_updated_at, description_updated_by,
                    icon_updated_at, icon_updated_by, color_updated_at, color_updated_by
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"# // Added placeholder for default_priority
            )
            .bind(id.to_string())
            .bind(&new_type.name)
            .bind(&new_type.description)
            .bind(&new_type.icon)
            .bind(&new_type.color)
            .bind(&new_type.default_priority) // Bind default_priority
            .bind(&now).bind(&now)
            .bind(&user_id).bind(&user_id)
            // LWW fields initialization
            .bind(&now).bind(&user_id) // name
            .bind(new_type.description.as_ref().map(|_| &now))
            .bind(new_type.description.as_ref().map(|_| &user_id)) // description
            .bind(new_type.icon.as_ref().map(|_| &now))
            .bind(new_type.icon.as_ref().map(|_| &user_id)) // icon
            .bind(new_type.color.as_ref().map(|_| &now))
            .bind(new_type.color.as_ref().map(|_| &user_id)) // color
            .execute(&mut *tx)
            .await
            .map_err(DbError::from)?;

            // Log Create Operation within the transaction
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Create,
                field_name: None,
                old_value: None,
                new_value: None, // Optionally serialize new_type
                timestamp: now_dt,
                user_id: user_uuid,
                device_id: device_uuid,
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, &mut tx).await?;

            // Fetch within the transaction before commit might be slightly faster
            // but requires careful handling if commit fails.
            // Fetching after commit is safer.
            Ok(id) // Return ID to fetch after commit
        }.await;

        match result {
            Ok(created_id) => {
                tx.commit().await.map_err(DbError::from)?;
                // Fetch the newly created record outside the transaction
                self.find_by_id(created_id).await
            },
            Err(e) => {
                let _ = tx.rollback().await; // Ensure rollback on error
                Err(e)
            }
        }
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            // Fetch existing to ensure it exists and for potential LWW checks (though not implemented here)
            let old_entity = self.find_by_id_with_tx(id, &mut tx).await?;
            let now = Utc::now().to_rfc3339();
            let now_dt = Utc::now(); // For logging
            let user_id = auth.user_id.to_string();
            let user_uuid = auth.user_id;
            let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();

            // Use dynamic query building for updates
            let mut sets: Vec<String> = Vec::new();
            let mut binds: Vec<String> = Vec::new(); // Using String for simplicity, sqlx handles types

             macro_rules! add_lww_update {
                 ($field:ident, $value:expr) => {
                     sets.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                     binds.push($value.to_string()); // Value
                     binds.push(now.clone());       // Updated At
                     binds.push(user_id.clone());     // Updated By
                 };
                 ($field:ident, $value:expr, $type:ty) => { // For Option<String>
                     if let Some(val) = $value {
                        add_lww_update!($field, val);
                     }
                 };
             }

             if let Some(name) = &update_data.name {
                 add_lww_update!(name, name);
             }
             // Use explicit check for Option<String> to handle None vs Some("") if needed
             add_lww_update!(description, &update_data.description, Option<String>);
             add_lww_update!(icon, &update_data.icon, Option<String>);
             add_lww_update!(color, &update_data.color, Option<String>);

             // Non-LWW field update (if needed)
              if let Some(prio) = &update_data.default_priority {
                 sets.push("default_priority = ?".to_string());
                 binds.push(prio.clone());
             }


            if sets.is_empty() {
                // No fields to update, just return existing
                return self.find_by_id_with_tx(id, &mut tx).await;
            }

            // Always update updated_at and updated_by_user_id
            sets.push("updated_at = ?".to_string());
            binds.push(now.clone());
            sets.push("updated_by_user_id = ?".to_string());
            binds.push(user_id.clone());

            let query_str = format!("UPDATE document_types SET {} WHERE id = ?", sets.join(", "));

            // Build and execute the query
            let mut q = query(&query_str);
            for bind_val in binds {
                q = q.bind(bind_val);
            }
            q = q.bind(id.to_string());

            q.execute(&mut *tx).await.map_err(DbError::from)?;

            // Fetch and return the updated record
            let new_entity = self.find_by_id_with_tx(id, &mut tx).await?;

            // Log field-level updates
            macro_rules! log_if_changed {
                ($field:ident) => {
                    if old_entity.$field != new_entity.$field {
                        let entry = ChangeLogEntry {
                            operation_id: Uuid::new_v4(),
                            entity_table: Self::entity_name().to_string(),
                            entity_id: id,
                            operation_type: ChangeOperationType::Update,
                            field_name: Some(stringify!($field).to_string()),
                            old_value: Some(serde_json::to_string(&old_entity.$field).unwrap_or_default()),
                            new_value: Some(serde_json::to_string(&new_entity.$field).unwrap_or_default()),
                            timestamp: now_dt,
                            user_id: user_uuid,
                            device_id: device_uuid,
                            document_metadata: None,
                            sync_batch_id: None,
                            processed_at: None,
                            sync_error: None,
                        };
                        self.log_change_entry(entry, &mut tx).await?;
                    }
                };
            }

            log_if_changed!(name);
            log_if_changed!(description);
            log_if_changed!(icon);
            log_if_changed!(color);
            log_if_changed!(default_priority);

            Ok(new_entity)

        }.await;

        match result {
            Ok(doc_type) => { tx.commit().await.map_err(DbError::from)?; Ok(doc_type) },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }

    async fn find_all(&self, params: PaginationParams) -> DomainResult<PaginatedResult<DocumentType>> {
        let offset = (params.page - 1) * params.per_page;
        let total: i64 = query_scalar("SELECT COUNT(*) FROM document_types WHERE deleted_at IS NULL")
            .fetch_one(&self.pool).await.map_err(DbError::from)?;

        let rows = query_as::<_, DocumentTypeRow>(
                "SELECT * FROM document_types WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?"
            )
            .bind(params.per_page as i64).bind(offset as i64)
            .fetch_all(&self.pool).await.map_err(DbError::from)?;

        let items = rows.into_iter().map(Self::map_row).collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }

     async fn find_by_name(&self, name: &str) -> DomainResult<Option<DocumentType>> {
         query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE name = ? AND deleted_at IS NULL")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(DbError::from)?
            .map(Self::map_row) // Use map_row for conversion
            .transpose() // Convert Option<Result<T, E>> to Result<Option<T>, E>
     }
}

// --- Media Document Repository ---

#[async_trait]
pub trait MediaDocumentRepository: DeleteServiceRepository<MediaDocument> + Send + Sync {
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
        priority: SyncPriority,
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
        let now_dt = Utc::now(); // For logging
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();

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
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteMediaDocumentRepository {
     fn entity_name(&self) -> &'static str {
        Self::entity_name()
    }
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
         // Assumes ON DELETE CASCADE is set up for related logs/versions in DB schema
         let result = query("DELETE FROM media_documents WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound(self.entity_name().to_string(), id))
        } else {
            Ok(())
        }
    }
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        // Consider adding file system cleanup logic here or in the service calling this
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
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
                SyncPriority::from_str(&new_doc.sync_priority)
                    .map_err(|_| DomainError::Validation(ValidationError::custom(&format!("Invalid sync priority string: {}", new_doc.sync_priority))))?
                as i64 // Cast the resulting enum variant to i64
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
            Ok(doc) => { tx.commit().await.map_err(DbError::from)?; Ok(doc) }
            Err(e) => { let _ = tx.rollback().await; Err(e) }
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
            tx.commit().await.map_err(DbError::from)?;
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
        let old_entity = self.find_by_id_with_tx(id, &mut tx).await.ok();
        let now = Utc::now();
        let system_user_id = Uuid::nil(); // System operation
        let device_uuid: Option<Uuid> = None;

        let result = query(
            "UPDATE media_documents SET blob_status = ?, blob_key = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status.as_str())
        .bind(blob_key)
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else {
            // Log changes if old entity was found
            if let Some(old) = old_entity {
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
                log_if_changed!(blob_status, "blob_status");
                log_if_changed!(blob_key, "blob_key");
            }
            tx.commit().await.map_err(DbError::from)?;
            Ok(())
        }
    }

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext, // Keep auth context for tracking who updated
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
        let mut select_builder = query_as::<_, (String, Option<i64>)>(&select_query); // Priority is nullable
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: std::collections::HashMap<Uuid, SyncPriority> = select_builder
            .fetch_all(&mut *tx) // Use the transaction
            .await.map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_int_opt)| {
                match Uuid::parse_str(&id_str) {
                    Ok(id) => Some((id, prio_int_opt.map(SyncPriority::from).unwrap_or(SyncPriority::Normal))),
                    Err(_) => None, // Skip rows with invalid UUIDs
                }
            }).collect();

        let now = Utc::now().to_rfc3339();
        let now_dt = Utc::now(); // For logging
        let user_id_str = auth.user_id.to_string();
        let user_uuid = auth.user_id;
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let priority_val = priority as i64; // Store as integer

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE media_documents
             SET sync_priority = ?, updated_at = ?, updated_by_user_id = ?
             WHERE id IN ({}) AND deleted_at IS NULL",
            placeholders
        );

        let mut query_builder = sqlx::query(&query_str)
            .bind(priority_val)
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
                        old_value: serde_json::to_string(&(*old_priority as i64)).ok(), // Log old priority as i64
                        new_value: serde_json::to_string(&priority_val).ok(), // Log new priority as i64
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

        tx.commit().await.map_err(DbError::from)?;
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
            .fetch_optional(&mut **tx) // Use &mut **tx
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
         file_path: Option<&str>, // New original path? Unlikely use case?
         compressed_file_path: Option<&str>,
         compressed_size_bytes: Option<i64>,
         compression_status: Option<CompressionStatus>,
     ) -> DomainResult<()> {
          let mut tx = self.pool.begin().await.map_err(DbError::from)?;

          // --- Fetch Old State ---
          let old_entity = match self.find_by_id_with_tx(document_id, &mut tx).await {
             Ok(entity) => entity,
             Err(e) => {
                 let _ = tx.rollback().await;
                 return Err(e);
             }
         };

         let mut sets: Vec<String> = Vec::new();
         let mut binds: Vec<String> = Vec::new(); // Using String for simplicity
         let mut binds_i64: HashMap<String, i64> = HashMap::new(); // For integer binds

         let now = Utc::now(); // For logging and timestamp
         // Assuming system operation - no specific user/device ID
         let system_user_id = Uuid::nil();
         let device_uuid: Option<Uuid> = None;

         // Use macro to simplify adding updates
         macro_rules! add_update {
            ($field:ident, $value:expr, $bind_vec:ident) => {
               if let Some(val) = $value {
                   sets.push(format!("{} = ?", stringify!($field)));
                   $bind_vec.push(val.to_string());
               }
            };
             ($field:ident, $value:expr, $bind_map:ident, $type:ty) => {
                if let Some(val) = $value {
                    sets.push(format!("{} = ?", stringify!($field)));
                    $bind_map.insert(stringify!($field).to_string(), val as $type);
                }
            };
         }

        // Add fields to update
        add_update!(file_path, file_path, binds);
        add_update!(compressed_file_path, compressed_file_path, binds);
        add_update!(compressed_size_bytes, compressed_size_bytes, binds_i64, i64);
        if let Some(status) = compression_status {
            sets.push("compression_status = ?".to_string());
            binds.push(status.as_str().to_string());
        }


        if sets.is_empty() {
            return Ok(()); // Nothing to update
        }

        // Always update updated_at
        sets.push("updated_at = ?".to_string());
        binds.push(now.to_rfc3339()); // Use captured 'now'
        // Optionally update updated_by_user_id if tracking sync agent ID
        // sets.push("updated_by_user_id = ?".to_string());
        // binds.push(SYSTEM_USER_ID.to_string());

        let query_str = format!("UPDATE media_documents SET {} WHERE id = ?", sets.join(", "));

        // Build and execute the query
        let mut q = query(&query_str);
        for bind_val in binds { // Bind strings first
            q = q.bind(bind_val);
        }
        // Bind integers (order matters if placeholders are mixed)
        // Assuming integer placeholders come after string ones based on SET order
        if let Some(val) = binds_i64.get("compressed_size_bytes") {
            q = q.bind(val);
        }
        // Add binds for other i64 fields if needed

        q = q.bind(document_id.to_string()); // Bind the WHERE clause ID

        let result = q.execute(&mut *tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            let _ = tx.rollback().await;
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), document_id))
        } else {
            // --- Log Field Changes ---
            let new_entity = self.find_by_id_with_tx(document_id, &mut tx).await?;

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

            log_if_changed!(file_path, "file_path");
            log_if_changed!(compressed_file_path, "compressed_file_path");
            log_if_changed!(compressed_size_bytes, "compressed_size_bytes");
            log_if_changed!(compression_status, "compression_status");

            tx.commit().await.map_err(DbError::from)?;
            Ok(())
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
                tx.commit().await.map_err(DbError::from)?;
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