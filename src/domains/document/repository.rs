use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, SoftDeletable, HardDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::document::types::{
    DocumentType, DocumentTypeRow, NewDocumentType, UpdateDocumentType,
    MediaDocument, MediaDocumentRow, NewMediaDocument, UpdateMediaDocument, CompressionStatus, BlobSyncStatus,
    DocumentVersion, DocumentVersionRow,
    DocumentAccessLog, DocumentAccessLogRow, NewDocumentAccessLog
};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar, Pool, Row, Sqlite, Transaction};
use uuid::Uuid;

// Temporary table identifier for linking documents before the main entity exists
pub const TEMP_RELATED_TABLE: &str = "__TEMP__";

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
}

impl SqliteDocumentTypeRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    fn entity_name() -> &'static str {
        "document_types"
    }
    fn map_row(row: DocumentTypeRow) -> DomainResult<DocumentType> {
        Ok(row.into_entity()?)
    }
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
}

#[async_trait]
impl FindById<DocumentType> for SqliteDocumentTypeRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<DocumentType> {
        query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE id = ? AND deleted_at IS NULL")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(DbError::from)?
            .ok_or_else(|| DomainError::EntityNotFound(Self::entity_name().to_string(), id))
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
            Err(DomainError::EntityNotFound(Self::entity_name().to_string(), id))
        } else {
            Ok(())
        }
    }
    // Default soft_delete uses transaction
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
        Self::entity_name()
    }
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
         let result = query("DELETE FROM document_types WHERE id = ?")
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
     // Default hard_delete uses transaction
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
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
        let now = Utc::now().to_rfc3339();
        let user_id = auth.user_id.to_string();

        query(
            r#"INSERT INTO document_types (
                id, name, description, icon, color, 
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                name_updated_at, name_updated_by, description_updated_at, description_updated_by,
                icon_updated_at, icon_updated_by, color_updated_at, color_updated_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(id.to_string())
        .bind(&new_type.name)
        .bind(&new_type.description)
        .bind(&new_type.icon)
        .bind(&new_type.color)
        .bind(&now).bind(&now) // created_at, updated_at
        .bind(&user_id).bind(&user_id) // created_by, updated_by
        .bind(&now).bind(&user_id) // name LWW
        .bind(new_type.description.as_ref().map(|_| &now))
        .bind(new_type.description.as_ref().map(|_| &user_id)) // description LWW
        .bind(new_type.icon.as_ref().map(|_| &now))
        .bind(new_type.icon.as_ref().map(|_| &user_id)) // icon LWW
        .bind(new_type.color.as_ref().map(|_| &now))
        .bind(new_type.color.as_ref().map(|_| &user_id)) // color LWW
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;

        self.find_by_id(id).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateDocumentType,
        auth: &AuthContext,
    ) -> DomainResult<DocumentType> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            let _existing = self.find_by_id_with_tx(id, &mut tx).await?;
            let now = Utc::now().to_rfc3339();
            let user_id = auth.user_id.to_string();
            
            let mut sets = Vec::new();
            let mut args = Vec::new();

            macro_rules! add_lww_update {($field:ident) => {
                if let Some(val) = &update_data.$field {
                    sets.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    args.push(val.clone());
                    args.push(now.clone());
                    args.push(user_id.clone());
                }
            };}

            add_lww_update!(name);
            add_lww_update!(description);
            add_lww_update!(icon);
            add_lww_update!(color);

            if sets.is_empty() { return self.find_by_id_with_tx(id, &mut tx).await; }

            sets.push("updated_at = ?".to_string());
            args.push(now.clone());
            sets.push("updated_by_user_id = ?".to_string());
            args.push(user_id.clone());

            let query_str = format!("UPDATE document_types SET {} WHERE id = ?", sets.join(", "));
            let mut q = query(&query_str);
            for arg in args { q = q.bind(arg); }
            q = q.bind(id.to_string());

            q.execute(&mut *tx).await.map_err(DbError::from)?;
            self.find_by_id_with_tx(id, &mut tx).await

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
        let rows = query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?")
            .bind(params.per_page as i64).bind(offset as i64).fetch_all(&self.pool).await.map_err(DbError::from)?;
        let items = rows.into_iter().map(Self::map_row).collect::<DomainResult<Vec<_>>>()?;
        Ok(PaginatedResult::new(items, total as u64, params))
    }

     async fn find_by_name(&self, name: &str) -> DomainResult<Option<DocumentType>> {
         query_as::<_, DocumentTypeRow>("SELECT * FROM document_types WHERE name = ? AND deleted_at IS NULL")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(DbError::from)?
            .map(Self::map_row)
            .transpose()
     }
}

// --- Media Document Repository ---

#[async_trait]
pub trait MediaDocumentRepository: DeleteServiceRepository<MediaDocument> + Send + Sync {
    async fn create(
        &self,
        new_doc: &NewMediaDocument,
        file_path: &str,
    ) -> DomainResult<MediaDocument>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateMediaDocument,
        auth: &AuthContext,
    ) -> DomainResult<MediaDocument>;
    
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateMediaDocument,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<MediaDocument>;

    async fn find_by_related_entity(
        &self,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<MediaDocument>>;

    async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument>;

    async fn find_by_id_with_tx<'t>(&self, id: Uuid, tx: &mut Transaction<'t, Sqlite>) -> DomainResult<MediaDocument>;

    async fn update_compression_status(
        &self,
        id: Uuid,
        status: CompressionStatus,
        compressed_file_path: Option<&str>,
    ) -> DomainResult<()>;

    async fn update_blob_sync_status(
        &self,
        id: Uuid,
        status: BlobSyncStatus,
        blob_key: Option<&str>,
    ) -> DomainResult<()>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    /// Links documents created with a temporary ID to the actual entity ID after creation.
    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>, // Requires a transaction
    ) -> DomainResult<u64>; // Returns the number of documents linked
}

pub struct SqliteMediaDocumentRepository {
    pool: Pool<Sqlite>,
}

impl SqliteMediaDocumentRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    fn entity_name() -> &'static str {
        "media_documents"
    }
     fn map_row(row: MediaDocumentRow) -> DomainResult<MediaDocument> {
        Ok(row.into_entity()?)
    }
}

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
         // Note: CASCADE should handle versions and logs, but file system cleanup is needed in service
         let result = query("DELETE FROM media_documents WHERE id = ?")
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
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
            Ok(_) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) },
        }
    }
}

#[async_trait]
impl MediaDocumentRepository for SqliteMediaDocumentRepository {
    async fn create(
        &self,
        new_doc: &NewMediaDocument,
        file_path: &str,
    ) -> DomainResult<MediaDocument> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = async {
            let now = Utc::now().to_rfc3339();
            let user_id_str = new_doc.created_by_user_id.map(|id| id.to_string());

            // Determine related_table and related_id based on temp_related_id
            let (related_table, related_id_str) = if let Some(temp_id) = new_doc.temp_related_id {
                (TEMP_RELATED_TABLE.to_string(), temp_id.to_string())
            } else {
                // If temp_id is None, related_id MUST be Some.
                let id = new_doc.related_id.ok_or_else(|| {
                    DomainError::Validation(ValidationError::custom(
                        "MediaDocument.related_id cannot be None if temp_related_id is also None"
                    ))
                })?;
                (new_doc.related_table.clone(), id.to_string()) // Use clone() which gives String
            };

            query(
                r#"INSERT INTO media_documents (
                    id, related_table, related_id, type_id, file_path, original_filename,
                    title, mime_type, size_bytes, field_identifier,
                    compression_status, compressed_file_path, blob_sync_status, blob_storage_key,
                    sync_priority,
                    created_at, updated_at, created_by_user_id, updated_by_user_id,
                    deleted_at, deleted_by_user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?, ?, ?, ?, ?, NULL, NULL)"# // Removed blob_storage_key from values, assumed NULL initially
            )
            .bind(new_doc.id.to_string())
            // Now both related_table and related_id_str are String
            .bind(related_table) 
            .bind(related_id_str)
            .bind(new_doc.type_id.to_string())
            .bind(file_path)
            .bind(&new_doc.original_filename)
            .bind(&new_doc.title)
            .bind(&new_doc.mime_type)
            .bind(new_doc.size_bytes)
            .bind(&new_doc.field_identifier)
            .bind(CompressionStatus::Pending.as_str())
            // compressed_file_path is NULL
            .bind(BlobSyncStatus::Pending.as_str())
            // blob_storage_key is NULL
            .bind(new_doc.sync_priority as i64)
            .bind(&now).bind(&now) // created_at, updated_at
            .bind(user_id_str.clone()).bind(user_id_str) // created_by, updated_by
            // deleted_at, deleted_by_user_id are NULL
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                 if let sqlx::Error::Database(db_err) = &e {
                     if db_err.is_unique_violation() {
                         return DomainError::Database(DbError::Conflict(format!(
                             "MediaDocument with ID {} already exists.",
                             new_doc.id
                         )));
                     }
                 }
                 DomainError::Database(DbError::from(e))
             })?;

            self.find_by_id_with_tx(new_doc.id, &mut tx).await
        }.await;

        match result {
            Ok(doc) => { tx.commit().await.map_err(DbError::from)?; Ok(doc) }
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateMediaDocument,
        auth: &AuthContext,
    ) -> DomainResult<MediaDocument> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(doc) => { tx.commit().await.map_err(DbError::from)?; Ok(doc) }
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateMediaDocument,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<MediaDocument> {
        self.find_by_id_with_tx(id, tx).await?; // Ensure exists

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut params_values: Vec<String> = Vec::new();

        if let Some(type_id) = update_data.type_id {
            set_clauses.push("type_id = ?");
            params_values.push(type_id.to_string());
        }
        if let Some(title) = &update_data.title {
            set_clauses.push("title = ?");
            params_values.push(title.clone());
        }

        if set_clauses.is_empty() {
            return self.find_by_id_with_tx(id, tx).await;
        }

        set_clauses.push("updated_at = ?");
        params_values.push(now.clone());
        set_clauses.push("updated_by_user_id = ?");
        params_values.push(user_id_str.clone());

        let query_str = format!(
            "UPDATE media_documents SET {} WHERE id = ?",
            set_clauses.join(", ")
        );

        let mut query_builder = query(&query_str);
        for param in &params_values {
            query_builder = query_builder.bind(param);
        }
        query_builder = query_builder.bind(id.to_string());

        query_builder
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        self.find_by_id_with_tx(id, tx).await
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
    ) -> DomainResult<()> {
        // Added updated_at timestamp
        query(
            "UPDATE media_documents SET compression_status = ?, compressed_file_path = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status.as_str())
        .bind(compressed_file_path)
        .bind(Utc::now().to_rfc3339()) // Add updated_at
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }

    async fn update_blob_sync_status(
        &self,
        id: Uuid,
        status: BlobSyncStatus,
        blob_key: Option<&str>,
    ) -> DomainResult<()> {
         // Added updated_at timestamp
         query(
            "UPDATE media_documents SET blob_sync_status = ?, blob_storage_key = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status.as_str())
        .bind(blob_key)
        .bind(Utc::now().to_rfc3339()) // Add updated_at
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;
        Ok(())
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
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let priority_val = priority as i64;
        
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
        
        let result = query_builder.execute(&self.pool).await.map_err(DbError::from)?;
        Ok(result.rows_affected())
    }

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


    async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument> {
        // Delegate to the FindById trait implementation
        <Self as FindById<MediaDocument>>::find_by_id(self, id).await
    }

    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<u64> {
        let result = query(
            "UPDATE media_documents SET related_table = ?, related_id = ?, updated_at = ? WHERE related_table = ? AND related_id = ?"
        )
        .bind(final_related_table)
        .bind(final_related_id.to_string())
        .bind(Utc::now().to_rfc3339()) // Also update timestamp
        .bind(TEMP_RELATED_TABLE)
        .bind(temp_related_id.to_string())
        .execute(&mut **tx) // Use the provided transaction
        .await
        .map_err(DbError::from)?;

        Ok(result.rows_affected())
    }
}

// --- Document Version Repository ---

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
        auth: &AuthContext,
    ) -> DomainResult<DocumentVersion>;

    async fn find_by_document_id(&self, doc_id: Uuid) -> DomainResult<Vec<DocumentVersion>>;
}

pub struct SqliteDocumentVersionRepository {
    pool: Pool<Sqlite>
}

impl SqliteDocumentVersionRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    fn map_row(row: DocumentVersionRow) -> DomainResult<DocumentVersion> {
        Ok(row.into_entity()?)
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
        let user_id = auth.user_id.to_string();

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
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Fetch the created version (optional, could construct from inputs)
        let row = query_as::<_, DocumentVersionRow>("SELECT * FROM document_versions WHERE id = ?")
            .bind(id.to_string()).fetch_one(&self.pool).await.map_err(DbError::from)?;
        Self::map_row(row)
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

// --- Document Access Log Repository ---

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
        Ok(row.into_entity()?)
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
        .bind(new_log.user_id.to_string())
        .bind(&new_log.access_type)
        .bind(&now)
        .bind(&new_log.details)
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch created log (optional)
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
