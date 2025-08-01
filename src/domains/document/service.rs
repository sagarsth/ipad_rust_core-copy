use crate::auth::AuthContext;
use crate::types::UserRole;
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::sync::repository::{TombstoneRepository, ChangeLogRepository};
use crate::domains::sync::types::{Tombstone, ChangeLogEntry, ChangeOperationType, SyncPriority};
use crate::domains::document::repository::{
    DocumentAccessLogRepository, DocumentTypeRepository, MediaDocumentRepository,
    DocumentVersionRepository,
};
use crate::domains::document::types::{
    DocumentType, DocumentTypeResponse, MediaDocument, MediaDocumentResponse, NewDocumentType,
    UpdateDocumentType,
    NewMediaDocument,
    DocumentVersion, NewDocumentAccessLog,
    DocumentAccessLog, CompressionStatus, BlobSyncStatus, DocumentAccessType,
    DocumentSummary,
    DocumentFileInfo,
    SourceOfChange,
};
use crate::domains::compression::service::CompressionService;
use crate::domains::compression::types::CompressionPriority;
use crate::errors::{DbError, DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite, query_scalar};
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use tokio::fs;
use std::str::FromStr;
use std::path::Path;
use chrono::{Utc, DateTime};
use serde_json;
use crate::domains::core::delete_service::PendingDeletionManager;
use serde::{Serialize, Deserialize};
// --- Includes Enum ---
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum DocumentInclude {
    DocumentType,
    Versions,
    AccessLogs(PaginationParams),
}

// --- Document Service Trait ---
#[async_trait]
pub trait DocumentService:
    DeleteService<DocumentType> + DeleteService<MediaDocument> + Send + Sync
{
    // Document Type Operations
    async fn create_document_type(
        &self,
        auth: &AuthContext,
        new_type: NewDocumentType,
    ) -> ServiceResult<DocumentTypeResponse>;

    async fn get_document_type_by_id(
        &self,
        id: Uuid,
    ) -> ServiceResult<DocumentTypeResponse>;

    async fn get_document_type_by_name(
        &self,
        name: &str,
    ) -> ServiceResult<Option<DocumentTypeResponse>>;

    async fn list_document_types(
        &self,
        params: PaginationParams,
    ) -> ServiceResult<PaginatedResult<DocumentTypeResponse>>;

    async fn update_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
        update_data: UpdateDocumentType,
    ) -> ServiceResult<DocumentTypeResponse>;

    /// Hard deletes a document type (Admin only)
    async fn delete_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult>;

    // Media Document Operations - Create Only
    async fn upload_document(
        &self,
        auth: &AuthContext,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse>;

    /// Bulk upload multiple documents with a single shared title
    async fn bulk_upload_documents(
        &self,
        auth: &AuthContext,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;

    // Media Document Operations - Read Only
    async fn get_media_document_by_id(
        &self,
        auth: &AuthContext,
        id: Uuid,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn list_media_documents_by_related_entity(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<PaginatedResult<MediaDocumentResponse>>;

    // Media Document Operations - Download and Access
    /// Download document data (Admin/TL only), returns None for data if not local
    async fn download_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<(String, Option<Vec<u8>>)>;

    /// Get file path for opening (if available locally)
    async fn open_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> ServiceResult<Option<String>>;

    /// Check if document is available on current device
    async fn is_document_available_on_device(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<bool>;

    // Delete Operation (Admin only)
    /// Hard deletes a media document (Admin only)
    async fn delete_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult>;

    // Document summarization
    /// Calculate document summary by linked fields
    async fn calculate_document_summary_by_linked_fields(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
    ) -> ServiceResult<DocumentSummary>;
    
    /// Link previously uploaded documents with a temporary ID to their final entity
    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
    ) -> ServiceResult<u64>; // Returns number of documents linked

    // Active File Usage Tracking
    async fn register_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
        use_type: &str,  // "view" or "edit"
    ) -> ServiceResult<()>;

    async fn unregister_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
    ) -> ServiceResult<()>;

    // --- iOS Optimized Path-Based Upload Methods ---
    
    /// Upload document from local file path (iOS optimized - no Base64 encoding)
    async fn upload_document_from_path(
        &self,
        auth: &AuthContext,
        file_path: String,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse>;

    /// Bulk upload documents from file paths (iOS optimized - no Base64 encoding)
    async fn bulk_upload_documents_from_paths(
        &self,
        auth: &AuthContext,
        file_paths: Vec<(String, String)>, // (path, filename)
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
}

// --- Service Implementation ---
pub struct DocumentServiceImpl {
    pool: SqlitePool,
    doc_type_repo: Arc<dyn DocumentTypeRepository>,
    media_doc_repo: Arc<dyn MediaDocumentRepository>,
    doc_ver_repo: Arc<dyn DocumentVersionRepository>,
    doc_log_repo: Arc<dyn DocumentAccessLogRepository>,
    delete_service_doc_type: Arc<BaseDeleteService<DocumentType>>,
    delete_service_media_doc: Arc<BaseDeleteService<MediaDocument>>,
    file_storage_service: Arc<dyn FileStorageService>,
    compression_service: Arc<dyn CompressionService>,
}

impl DocumentServiceImpl {
    /// Configure SQLite connection pool for better concurrency
    fn configure_sqlite_pool(pool: &SqlitePool) {
        // SQLite-specific optimizations are typically done at connection level
        // This is a placeholder for any pool-level configurations
        println!("🔧 [DOC_SERVICE] SQLite pool configured for concurrency");
    }
    
    /// Retry database operations that might fail due to locking
    async fn retry_db_operation<T, F, Fut>(operation: F, max_retries: u32) -> ServiceResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = ServiceResult<T>>,
    {
        let mut retries = 0;
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if Self::is_database_locked_error(&e) && retries < max_retries => {
                    retries += 1;
                    let delay_ms = 50 * (2_u64.pow(retries)); // Exponential backoff
                    println!("🔄 [DOC_SERVICE] Database locked, retrying in {}ms (attempt {}/{})", delay_ms, retries, max_retries);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
    
    /// Check if error is related to database locking
    fn is_database_locked_error(error: &ServiceError) -> bool {
        match error {
            ServiceError::Domain(domain_err) => {
                let error_str = domain_err.to_string().to_lowercase();
                error_str.contains("database is locked") || 
                error_str.contains("sqlite_busy") ||
                error_str.contains("database table is locked")
            }
            _ => false,
        }
    }

    pub fn new(
        pool: SqlitePool,
        doc_type_repo: Arc<dyn DocumentTypeRepository>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
        doc_ver_repo: Arc<dyn DocumentVersionRepository>,
        doc_log_repo: Arc<dyn DocumentAccessLogRepository>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        file_storage_service: Arc<dyn FileStorageService>,
        compression_service: Arc<dyn CompressionService>,
        deletion_manager: Arc<PendingDeletionManager>,
    ) -> Self {
        // Configure SQLite connection pool for better concurrency
        Self::configure_sqlite_pool(&pool);
        
        // --- Adapters for Delete Services ---
        struct DocTypeRepoAdapter(Arc<dyn DocumentTypeRepository>);

        #[async_trait]
        impl FindById<DocumentType> for DocTypeRepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<DocumentType> { 
                self.0.find_by_id(id).await 
            }
        }

        #[async_trait]
        impl SoftDeletable for DocTypeRepoAdapter {
            async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { 
                self.0.soft_delete(id, auth).await 
            }

            async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { 
                self.0.soft_delete_with_tx(id, auth, tx).await 
            }
        }

        #[async_trait]
        impl HardDeletable for DocTypeRepoAdapter {
            fn entity_name(&self) -> &'static str { 
                self.0.entity_name() 
            }

            async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { 
                self.0.hard_delete(id, auth).await 
            }

            async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { 
                self.0.hard_delete_with_tx(id, auth, tx).await 
            }
        }

        let adapted_doc_type_repo: Arc<dyn DeleteServiceRepository<DocumentType>> = 
            Arc::new(DocTypeRepoAdapter(doc_type_repo.clone()));

        // Adapter for MediaDocument
        struct MediaDocRepoAdapter(Arc<dyn MediaDocumentRepository>);

        #[async_trait]
        impl FindById<MediaDocument> for MediaDocRepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<MediaDocument> { 
                MediaDocumentRepository::find_by_id(&*self.0, id).await 
            }
        }

        #[async_trait]
        impl SoftDeletable for MediaDocRepoAdapter {
            async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { 
                self.0.soft_delete(id, auth).await 
            }

            async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { 
                self.0.soft_delete_with_tx(id, auth, tx).await 
            }
        }

        #[async_trait]
        impl HardDeletable for MediaDocRepoAdapter {
            fn entity_name(&self) -> &'static str { 
                HardDeletable::entity_name(self.0.as_ref())
            }

            async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { 
                self.0.hard_delete(id, auth).await 
            }

            async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { 
                self.0.hard_delete_with_tx(id, auth, tx).await 
            }
        }

        let adapted_media_doc_repo: Arc<dyn DeleteServiceRepository<MediaDocument>> = 
            Arc::new(MediaDocRepoAdapter(media_doc_repo.clone()));

        // --- Create Delete Services ---
        let delete_service_doc_type = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_doc_type_repo,
            tombstone_repo.clone(),
            change_log_repo.clone(),
            dependency_checker.clone(),
            None,
            deletion_manager.clone(),
        ));

        let delete_service_media_doc = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_media_doc_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo.clone()),
            deletion_manager,
        ));

        Self {
            pool,
            doc_type_repo,
            media_doc_repo,
            doc_ver_repo,
            doc_log_repo,
            delete_service_doc_type,
            delete_service_media_doc,
            file_storage_service,
            compression_service,
        }
    }

    /// Helper to enrich MediaDocumentResponse with included data and check availability
    async fn enrich_response(
        &self,
        mut response: MediaDocumentResponse,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse> {
        // Determine the primary path and size to display based on compression
        let mut _display_path = response.file_path.clone();
        let mut _display_size = response.size_bytes;

        // Check if the relevant file (original or completed compressed) is available locally
        let path_to_check = if response.compression_status == CompressionStatus::Completed.as_str() && response.compressed_file_path.is_some() {
            if let Some(compressed_path) = &response.compressed_file_path {
                _display_path = compressed_path.clone();
                if let Some(compressed_size) = response.compressed_size_bytes {
                    _display_size = compressed_size;
                }
                compressed_path
            } else {
                &response.file_path
            }
        } else {
            &response.file_path
        };
        
        // Only mark as unavailable if file doesn't actually exist or has actual file path errors
        // Don't mark as unavailable just because compression failed
        if response.file_path == "ERROR" {
            response.is_available_locally = false;
        } else {
            let absolute_path = self.file_storage_service.get_absolute_path(path_to_check);
            response.is_available_locally = match tokio::fs::metadata(&absolute_path).await {
                Ok(_) => true,
                Err(_) => false,
            };
        }

        // Handle includes
        if let Some(includes) = include {
            for inc in includes {
                match inc {
                    DocumentInclude::DocumentType => {
                        if response.type_name.is_none() {
                            match self.doc_type_repo.find_by_id(response.type_id).await {
                                Ok(doc_type) => {
                                    response.type_name = Some(doc_type.name);
                                }
                                Err(DomainError::EntityNotFound(_, _)) => {
                                    response.type_name = Some("<Type Not Found>".to_string());
                                }
                                Err(e) => return Err(ServiceError::Domain(e)),
                            }
                        }
                    }
                    DocumentInclude::Versions => {
                        let versions = self.doc_ver_repo.find_by_document_id(response.id).await?;
                        response.versions = Some(versions);
                    }
                    DocumentInclude::AccessLogs(params) => {
                        let logs = self.doc_log_repo.find_by_document_id(response.id, params.clone()).await?;
                        response.access_logs = Some(logs);
                    }
                }
            }
        }

        Ok(response)
    }

    // --- Sync Service Interface Methods ---
    
    /// Update compression status - Called by Compression Service
    pub async fn update_compression_status(
        &self,
        id: Uuid,
        status: CompressionStatus,
        compressed_file_path: Option<&str>,
        compressed_size_bytes: Option<i64>,
    ) -> ServiceResult<()> {
        self.media_doc_repo.update_compression_status(id, status, compressed_file_path, compressed_size_bytes).await
            .map_err(|e| ServiceError::Domain(e))
    }

    /// Update blob sync status - Called by Sync Service
    pub async fn update_blob_sync_status(
        &self,
        id: Uuid,
        status: BlobSyncStatus,
        blob_key: Option<&str>,
    ) -> ServiceResult<()> {
        self.media_doc_repo.update_blob_sync_status(id, status, blob_key).await
            .map_err(|e| ServiceError::Domain(e))
    }

    /// Get document file info for sync - Called by Sync Service
    pub async fn get_document_file_info(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<DocumentFileInfo> {
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| ServiceError::Domain(e))?;

        if doc.file_path == "ERROR" {
            return Err(ServiceError::Domain(DomainError::Internal("Cannot get file info for error document".to_string())));
        }

        let use_compressed_for_sync = doc.compressed_file_path.is_some() &&
                                      doc.compression_status == CompressionStatus::Completed.as_str();

        let (sync_file_path, sync_size_bytes) = if use_compressed_for_sync {
            (doc.compressed_file_path.as_ref().unwrap().clone(), doc.compressed_size_bytes.unwrap_or(doc.size_bytes))
        } else {
            (doc.file_path.clone(), doc.size_bytes)
        };

        let absolute_path = self.file_storage_service.get_absolute_path(&sync_file_path);

        let file_exists_locally = match fs::metadata(&absolute_path).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                eprintln!("Error checking file metadata for sync info {}: {}", absolute_path.display(), e);
                false
            }
        };

        Ok(DocumentFileInfo {
            id: doc.id,
            file_path: sync_file_path,
            absolute_path: absolute_path.to_string_lossy().to_string(),
            is_compressed: use_compressed_for_sync,
            size_bytes: sync_size_bytes,
            mime_type: doc.mime_type.clone(),
            file_exists_locally,
            blob_status: doc.blob_status.clone(),
            blob_key: doc.blob_key.clone(),
            sync_priority: doc.sync_priority.clone(),
            original_file_path: doc.file_path.clone(),
            original_size_bytes: doc.size_bytes,
            compression_status: CompressionStatus::from_str(&doc.compression_status)
                .unwrap_or(CompressionStatus::Pending)
        })
    }

    /// Update file paths after download - Called by Sync Service
    pub async fn update_document_file_paths(
        &self,
        document_id: Uuid,
        file_path: Option<&str>,
        compressed_file_path: Option<&str>,
        compression_status: Option<CompressionStatus>,
        compressed_size_bytes: Option<i64>,
    ) -> ServiceResult<()> {
        self.media_doc_repo.update_paths_and_status(
            document_id,
            file_path,
            compressed_file_path,
            compressed_size_bytes,
            compression_status,
        ).await.map_err(|e| ServiceError::Domain(e))
    }
}

// --- DeleteService Implementations ---
#[async_trait]
impl DeleteService<DocumentType> for DocumentServiceImpl {
    fn repository(&self) -> &dyn FindById<DocumentType> { 
        self.delete_service_doc_type.repository() 
    }

    fn tombstone_repository(&self) -> &dyn TombstoneRepository { 
        self.delete_service_doc_type.tombstone_repository() 
    }

    fn change_log_repository(&self) -> &dyn ChangeLogRepository { 
        self.delete_service_doc_type.change_log_repository() 
    }

    fn dependency_checker(&self) -> &dyn DependencyChecker { 
        self.delete_service_doc_type.dependency_checker() 
    }

    async fn delete(
        &self, 
        id: Uuid, 
        auth: &AuthContext, 
        options: DeleteOptions
    ) -> DomainResult<DeleteResult> { 
        self.delete_service_doc_type.delete(id, auth, options).await 
    }

    async fn batch_delete(
        &self, 
        ids: &[Uuid], 
        auth: &AuthContext, 
        options: DeleteOptions
    ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { 
        self.delete_service_doc_type.batch_delete(ids, auth, options).await 
    }

    async fn delete_with_dependencies(
        &self, 
        id: Uuid, 
        auth: &AuthContext
    ) -> DomainResult<DeleteResult> { 
        self.delete_service_doc_type.delete_with_dependencies(id, auth).await 
    }

    async fn get_failed_delete_details(
        &self,
        batch_result: &crate::domains::core::delete_service::BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<DocumentType>>> { 
        self.delete_service_doc_type.get_failed_delete_details(batch_result, auth).await 
    }
}

#[async_trait]
impl DeleteService<MediaDocument> for DocumentServiceImpl {
    fn repository(&self) -> &dyn FindById<MediaDocument> { 
        self.delete_service_media_doc.repository() 
    }

    fn tombstone_repository(&self) -> &dyn TombstoneRepository { 
        self.delete_service_media_doc.tombstone_repository() 
    }

    fn change_log_repository(&self) -> &dyn ChangeLogRepository { 
        self.delete_service_media_doc.change_log_repository() 
    }

    fn dependency_checker(&self) -> &dyn DependencyChecker { 
        self.delete_service_media_doc.dependency_checker() 
    }

    async fn delete(
        &self, 
        id: Uuid, 
        auth: &AuthContext, 
        options: DeleteOptions
    ) -> DomainResult<DeleteResult> { 
        self.delete_service_media_doc.delete(id, auth, options).await 
    }

    async fn batch_delete(
        &self, 
        ids: &[Uuid], 
        auth: &AuthContext, 
        options: DeleteOptions
    ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { 
        self.delete_service_media_doc.batch_delete(ids, auth, options).await 
    }

    async fn delete_with_dependencies(
        &self, 
        id: Uuid, 
        auth: &AuthContext
    ) -> DomainResult<DeleteResult> { 
        self.delete_service_media_doc.delete_with_dependencies(id, auth).await 
    }

    async fn get_failed_delete_details(
        &self,
        batch_result: &crate::domains::core::delete_service::BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<MediaDocument>>> { 
        self.delete_service_media_doc.get_failed_delete_details(batch_result, auth).await 
    }
}

// --- DocumentService Implementation ---
#[async_trait]
impl DocumentService for DocumentServiceImpl {
    // --- Document Type Methods ---
    async fn create_document_type(
        &self,
        auth: &AuthContext,
        new_type: NewDocumentType,
    ) -> ServiceResult<DocumentTypeResponse> {
        new_type.validate()?;
        let created = self.doc_type_repo.create(&new_type, auth).await?;
        Ok(DocumentTypeResponse::from(created))
    }

    async fn get_document_type_by_id(
        &self,
        id: Uuid,
    ) -> ServiceResult<DocumentTypeResponse> {
        let doc_type = self.doc_type_repo.find_by_id(id).await?;
        Ok(DocumentTypeResponse::from(doc_type))
    }

    async fn get_document_type_by_name(
        &self,
        name: &str,
    ) -> ServiceResult<Option<DocumentTypeResponse>> {
        let doc_type_opt = self.doc_type_repo.find_by_name(name).await?;
        Ok(doc_type_opt.map(DocumentTypeResponse::from))
    }

    async fn list_document_types(
        &self,
        params: PaginationParams,
    ) -> ServiceResult<PaginatedResult<DocumentTypeResponse>> {
        let paginated_result = self.doc_type_repo.find_all(params).await?;
        let response_items = paginated_result.items.into_iter()
            .map(DocumentTypeResponse::from).collect();
        Ok(PaginatedResult::new(response_items, paginated_result.total, params))
    }

    async fn update_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
        update_data: UpdateDocumentType,
    ) -> ServiceResult<DocumentTypeResponse> {
        update_data.validate()?;
        
        if let Some(name) = &update_data.name {
            if let Some(existing) = self.doc_type_repo.find_by_name(name).await? {
                if existing.id != id {
                    return Err(ServiceError::Domain(DomainError::Validation(
                        ValidationError::unique("name")
                    )));
                }
            }
        }
        
        let updated = self.doc_type_repo.update(id, &update_data, auth).await?;
        Ok(DocumentTypeResponse::from(updated))
    }

    async fn delete_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult> {
        if auth.role != UserRole::Admin {
            return Err(ServiceError::PermissionDenied(
                "Only admins can hard delete document types".to_string()
            ));
        }

        let options = DeleteOptions { 
            allow_hard_delete: true, 
            fallback_to_soft_delete: false, 
            force: false 
        };

        let result = DeleteService::<DocumentType>::delete(self, id, auth, options).await?;
        Ok(result)
    }

    // --- Media Document Methods ---
    async fn upload_document(
        &self,
        auth: &AuthContext,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse> {
        println!("🎯 [DOC_SERVICE] *** UPLOAD_DOCUMENT CALLED *** - File: {}", original_filename);
        
        println!("📄 [DOC_SERVICE] Starting upload_document");
        println!("   📄 Filename: {}", original_filename);
        println!("   📊 File size: {} bytes", file_data.len());
        println!("   🆔 Entity ID: {}", related_entity_id);
        println!("   🏷️ Entity type: {}", related_entity_type);
        println!("   🗜️ Compression priority: {:?}", compression_priority);
        
        let doc_type = self.doc_type_repo.find_by_id(document_type_id).await?;
        println!("📄 [DOC_SERVICE] Document type found: {} (compression: {}, method: {:?})", 
                doc_type.name, doc_type.compression_level, doc_type.compression_method);
        
        let entity_or_temp_id_str = if let Some(temp_id) = temp_related_id {
            temp_id.to_string()
        } else {
            related_entity_id.to_string()
        };
        
        println!("📄 [DOC_SERVICE] Calling file_storage_service.save_file...");
        let file_save_result = self.file_storage_service.save_file(
            file_data.clone(),
            &if temp_related_id.is_some() { "temp" } else { &related_entity_type },
            &entity_or_temp_id_str,
            &original_filename,
        ).await;
        
        match file_save_result {
            Ok((relative_path, size_bytes)) => {
                println!("📄 [DOC_SERVICE] File save successful!");
                println!("   🔗 Relative path: '{}'", relative_path);
                println!("   📊 Size: {} bytes", size_bytes);
                
                // Check if relative_path is empty or null-like
                if relative_path.is_empty() {
                    println!("❌ [DOC_SERVICE] ERROR: relative_path is EMPTY!");
                    return Err(ServiceError::Domain(DomainError::Internal("File storage returned empty path".to_string())));
                }
                
                let new_doc_metadata = NewMediaDocument {
                    id: Uuid::new_v4(),
                    related_table: if temp_related_id.is_some() { "TEMP".to_string() } else { related_entity_type },
                    related_id: if temp_related_id.is_some() { None } else { Some(related_entity_id) },
                    temp_related_id,
                    type_id: document_type_id,
                    original_filename: original_filename.clone(),
                    title: title.or_else(|| Some(original_filename.clone())),
                    mime_type: guess_mime_type(&original_filename),
                    size_bytes: size_bytes as i64,
                    field_identifier: linked_field,
                    sync_priority: sync_priority.as_str().to_string(),
                    created_by_user_id: Some(auth.user_id),
                    file_path: relative_path.clone(),
                    description: None,
                    compression_status: CompressionStatus::Pending.as_str().to_string(),
                    blob_status: BlobSyncStatus::Pending.as_str().to_string(),
                    blob_key: None,
                    compressed_file_path: None,
                    compressed_size_bytes: None,
                    source_of_change: SourceOfChange::Local,
                };
                
                println!("📄 [DOC_SERVICE] About to create document record:");
                println!("   🔗 file_path: '{}'", new_doc_metadata.file_path);
                println!("   🆔 Document ID: {}", new_doc_metadata.id);
                
                new_doc_metadata.validate()?;
                
                // *** HYBRID APPROACH: Individual Document Creation ***
                // Create document individually (resilient - if this fails, only this document fails)
                let created_doc = self.media_doc_repo.create(&new_doc_metadata).await?;
                println!("📄 [DOC_SERVICE] Document record created successfully!");
                
                let final_compression_priority = compression_priority
                    .or_else(|| CompressionPriority::from_str(&doc_type.default_priority).ok())
                    .unwrap_or(CompressionPriority::Normal);
                
                println!("📄 [DOC_SERVICE] About to queue for compression:");
                println!("   🔄 Final compression priority: {:?}", final_compression_priority);
                println!("   📄 Document ID: {}", created_doc.id);
                println!("   📏 File size: {} bytes (min for compression: {})", 
                        size_bytes, doc_type.min_size_for_compression.unwrap_or(0));
                
                // Check if file is large enough for compression
                let should_compress = size_bytes as i64 >= doc_type.min_size_for_compression.unwrap_or(0);
                println!("   🤔 Should compress? {} (file: {} >= min: {})", 
                        should_compress, size_bytes, doc_type.min_size_for_compression.unwrap_or(0));
                
                if should_compress {
                    // *** SEPARATE COMPRESSION QUEUING WITH RETRY ***
                    // Queue for compression separately (with retry logic for locking issues)
                    match Self::retry_db_operation(|| async {
                        self.compression_service
                            .queue_document_for_compression(created_doc.id, final_compression_priority)
                            .await
                    }, 3).await {
                        Ok(()) => {
                            println!("✅ [DOC_SERVICE] Successfully queued document {} for compression", created_doc.id);
                        },
                        Err(e) => {
                            println!("❌ [DOC_SERVICE] Failed to queue document {} for compression: {:?}", created_doc.id, e);
                            // Don't fail the entire upload - document is still created successfully
                            eprintln!("Warning: Document {} created but compression queuing failed: {:?}", created_doc.id, e);
                        }
                    }
                } else {
                    println!("⏭️ [DOC_SERVICE] File too small for compression, marking as skipped");
                    // Update status to skipped with retry logic
                    if let Err(e) = Self::retry_db_operation(|| async {
                        self.media_doc_repo.update_compression_status(
                            created_doc.id, 
                            CompressionStatus::Skipped, 
                            None, 
                            None
                        ).await.map_err(|e| ServiceError::Domain(e))
                    }, 3).await {
                        println!("⚠️ [DOC_SERVICE] Failed to update compression status to skipped: {:?}", e);
                        // Don't fail the upload - document is still created
                    }
                }
                
                let mut response = MediaDocumentResponse::from_doc(&created_doc, Some(doc_type.name));
                response = self.enrich_response(response, None).await?;
                Ok(response)
            },
            Err(e) => {
                let error_message = format!("Failed to save file: {}", e);
                println!("❌ [DOC_SERVICE] File save failed: {}", error_message);
                
                // Instead of creating a database record with "ERROR" file_path, 
                // return the error directly since file_path is NOT NULL
                Err(ServiceError::Domain(DomainError::Internal(error_message)))
            }
        }
    }

    async fn bulk_upload_documents(
        &self,
        auth: &AuthContext,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        let mut results = Vec::new();
        let doc_type = self.doc_type_repo.find_by_id(document_type_id).await?;
        let final_compression_priority = compression_priority
            .or_else(|| CompressionPriority::from_str(&doc_type.default_priority).ok())
            .unwrap_or(CompressionPriority::Normal);

        let total_files = files.len();
        println!("📦 [DOC_SERVICE] Starting bulk upload of {} documents", total_files);

        for (file_data, original_filename) in files {
            println!("📄 [DOC_SERVICE] Processing file: {}", original_filename);
            
            // *** HYBRID APPROACH: Process each document individually ***
            // This ensures that if one document fails, others can still succeed
            match self.upload_document(
                auth,
                file_data,
                original_filename.clone(),
                title.clone(),
                document_type_id,
                related_entity_id,
                related_entity_type.clone(),
                None,
                sync_priority,
                Some(final_compression_priority),
                temp_related_id,
            ).await {
                Ok(response) => {
                    println!("✅ [DOC_SERVICE] Successfully uploaded: {}", original_filename);
                    results.push(response);
                },
                Err(e) => {
                    // Log the error but continue with other files
                    println!("❌ [DOC_SERVICE] Failed to upload '{}': {}", original_filename, e);
                    eprintln!("🚫 Bulk upload failed for '{}': {}", original_filename, e);
                    // Skip this file instead of creating an error response with invalid file_path
                    // The client can handle partial success responses
                }
            }
        }
        
        println!("📦 [DOC_SERVICE] Bulk upload completed: {}/{} files successful", results.len(), total_files);
        Ok(results)
    }

    async fn get_media_document_by_id(
        &self,
        auth: &AuthContext,
        id: Uuid,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse> {
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await?;
        let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::View.as_str().to_string(), details: None };
        if let Err(e) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log document access for {}: {:?}", id, e); }
        let type_name = if include.map_or(false, |incs| incs.contains(&DocumentInclude::DocumentType)) { None } else { self.doc_type_repo.find_by_id(doc.type_id).await.ok().map(|dt| dt.name) };
        let mut response = MediaDocumentResponse::from_doc(&doc, type_name);
        response = self.enrich_response(response, include).await?;
        Ok(response)
    }

    async fn list_media_documents_by_related_entity(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<PaginatedResult<MediaDocumentResponse>> {
        let result = self.media_doc_repo.find_by_related_entity(related_table, related_id, params.clone()).await?;
        let mut response_items = Vec::new();
        let type_map = HashMap::<Uuid, String>::new();
        for item in result.items {
            let initial_type_name = type_map.get(&item.type_id).cloned();
            let response = MediaDocumentResponse::from_doc(&item, initial_type_name);
            let enriched_response = self.enrich_response(response, include).await?;
            response_items.push(enriched_response);
        }
        Ok(PaginatedResult::new(response_items, result.total, params))
    }

    async fn download_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<(String, Option<Vec<u8>>)> {
        if auth.role != UserRole::Admin && auth.role != UserRole::FieldTeamLead { return Err(ServiceError::PermissionDenied("Admin or Field Team Lead required to download documents".to_string())); }
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await?;
        if doc.file_path == "ERROR" {
            let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptDownload.as_str().to_string(), details: Some("Attempted to download error document".to_string()) };
            if let Err(e) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log error document download attempt for {}: {:?}", id, e); }
            return Err(ServiceError::Ui(format!("Document has an error status: {}", doc.description.unwrap_or_else(|| "Unknown error".to_string()))));
        }
        
        // *** MODIFIED: Allow downloading during compression using original file ***
        // Similar to open_document, we'll use the original file during compression
        if doc.compression_status == CompressionStatus::Processing.as_str() {
            // Log that user is downloading during compression (for analytics)
            let new_log = NewDocumentAccessLog { 
                document_id: id, 
                user_id: auth.user_id, 
                access_type: DocumentAccessType::Download.as_str().to_string(), 
                details: Some("Downloading original file during compression".to_string()) 
            };
            if let Err(log_err) = self.doc_log_repo.create(&new_log).await { 
                eprintln!("Failed to log download during compression for {}: {:?}", id, log_err); 
            }
            
            println!("📥 [DOCUMENT_DOWNLOAD] Document {} is being compressed, serving original file", id);
        }
        
        // *** MODIFIED: Smart file path selection (same logic as open_document) ***
        let file_path_to_check = if let Some(compressed_path) = &doc.compressed_file_path {
            if doc.compression_status == CompressionStatus::Completed.as_str() {
                // Use compressed file if compression is completed
                compressed_path
            } else {
                // Use original file if compression is in progress, pending, failed, or skipped
                &doc.file_path
            }
        } else {
            // No compressed file exists, use original
            &doc.file_path
        };
        
        let absolute_path = self.file_storage_service.get_absolute_path(file_path_to_check);
        match fs::metadata(&absolute_path).await {
            Ok(_) => {
                match fs::File::open(&absolute_path).await {
                    Ok(_) => {
                        match self.file_storage_service.get_file_data(file_path_to_check).await {
                            Ok(file_data) => {
                                let details = if doc.compression_status == CompressionStatus::Processing.as_str() {
                                    Some("Downloaded original file during compression".to_string())
                                } else if doc.compression_status == CompressionStatus::Completed.as_str() && file_path_to_check == doc.compressed_file_path.as_ref().unwrap() {
                                    Some("Downloaded compressed file".to_string())
                                } else {
                                    None
                                };
                                
                                let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::Download.as_str().to_string(), details };
                                if let Err(e) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log document download for {}: {:?}", id, e); }
                                Ok((doc.original_filename.clone(), Some(file_data)))
                            },
                            Err(e) => {
                                eprintln!("Error reading file data for {}: {}", absolute_path.display(), e);
                                Err(ServiceError::Domain(DomainError::Internal(format!("Error reading document file: {}", e))))
                            }
                        }
                    },
                    Err(e) => {
                        let details = Some(format!("File exists but is locked/inaccessible: {}", e));
                        let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptDownload.as_str().to_string(), details };
                        if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log locked download attempt for {}: {:?}", id, log_err); }
                        
                        // *** IMPROVED: Better error messages ***
                        if doc.compression_status == CompressionStatus::Processing.as_str() { 
                            Err(ServiceError::Ui("Document file is temporarily locked during compression. Please try again in a moment.".to_string())) 
                        } else { 
                            Err(ServiceError::Ui("Cannot access document file. It may be in use by another process.".to_string())) 
                        }
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let details = Some(format!("File not found: {}", absolute_path.display()));
                let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptDownload.as_str().to_string(), details };
                if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log missing file download attempt for {}: {:?}", id, log_err); }
                
                // *** IMPROVED: Suggest solutions ***
                if doc.compression_status == CompressionStatus::Processing.as_str() {
                    Err(ServiceError::Ui("Document is being compressed and the original file is temporarily unavailable. Please try again shortly.".to_string()))
                } else {
                    Err(ServiceError::Ui("Document file not found on device. It may need to be downloaded from the cloud.".to_string()))
                }
            },
            Err(e) => {
                let details = Some(format!("File system error: {}", e));
                let new_log = NewDocumentAccessLog { document_id: id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptDownload.as_str().to_string(), details };
                if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log file system error download attempt for {}: {:?}", id, log_err); }
                Err(ServiceError::Domain(DomainError::Internal(format!("File system error: {}", e))))
            }
        }
    }

    async fn open_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> ServiceResult<Option<String>> {
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, document_id).await?;
        
        // Check for error documents
        if doc.file_path == "ERROR" {
            let new_log = NewDocumentAccessLog { document_id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptView.as_str().to_string(), details: Some("Attempted to open error document".to_string()) };
            if let Err(e) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log error document open attempt for {}: {:?}", document_id, e); }
            return Err(ServiceError::Ui(format!("Document has an error status: {}", doc.description.unwrap_or_else(|| "Unknown error".to_string()))));
        }

        // *** MODIFIED: Allow viewing during compression using original file ***
        // Instead of blocking access during compression, we'll use the original file
        // This provides better user experience while compression happens in background
        if doc.compression_status == CompressionStatus::Processing.as_str() {
            // Log that user is viewing during compression (for analytics)
            let new_log = NewDocumentAccessLog { 
                document_id, 
                user_id: auth.user_id, 
                access_type: DocumentAccessType::View.as_str().to_string(), 
                details: Some("Viewing original file during compression".to_string()) 
            };
            if let Err(log_err) = self.doc_log_repo.create(&new_log).await { 
                eprintln!("Failed to log view during compression for {}: {:?}", document_id, log_err); 
            }
            
            // Continue with original file access instead of blocking
            println!("📖 [DOCUMENT_OPEN] Document {} is being compressed, serving original file", document_id);
        }

        // *** ADDED: Register usage ***
        // Attempt to parse device_id, use a placeholder if invalid
        let parsed_device_id: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse().ok() };
        let device_id_for_registration: Uuid = parsed_device_id.unwrap_or_else(Uuid::nil);
        if let Err(e) = self.register_document_in_use(document_id, auth.user_id, device_id_for_registration, "view").await {
            eprintln!("Failed to register document {} in use: {:?}", document_id, e);
            // Log error, but proceed with opening anyway
        }

        // *** MODIFIED: Smart file path selection ***
        // Priority: 1) Completed compressed file, 2) Original file (even during compression)
        let file_path_to_check = if let Some(compressed_path) = &doc.compressed_file_path {
            if doc.compression_status == CompressionStatus::Completed.as_str() {
                // Use compressed file if compression is completed
                compressed_path
            } else {
                // Use original file if compression is in progress, pending, failed, or skipped
                &doc.file_path
            }
        } else {
            // No compressed file exists, use original
            &doc.file_path
        };
        
        let absolute_path = self.file_storage_service.get_absolute_path(file_path_to_check);
        match fs::metadata(&absolute_path).await {
            Ok(_) => {
                match fs::File::open(&absolute_path).await {
                    Ok(_) => {
                        let details = if doc.compression_status == CompressionStatus::Processing.as_str() {
                            Some("Opened original file during compression".to_string())
                        } else if doc.compression_status == CompressionStatus::Completed.as_str() && file_path_to_check == doc.compressed_file_path.as_ref().unwrap() {
                            Some("Opened compressed file".to_string())
                        } else {
                            Some("Opened locally".to_string())
                        };
                        
                        let new_log = NewDocumentAccessLog { document_id, user_id: auth.user_id, access_type: DocumentAccessType::View.as_str().to_string(), details };
                        if let Err(e) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log document view for {}: {:?}", document_id, e); }
                        #[cfg(target_os = "ios")] { let ios_path = format!("file://{}", absolute_path.display()); Ok(Some(ios_path)) }
                        #[cfg(not(target_os = "ios"))] { Ok(Some(absolute_path.to_string_lossy().to_string())) }
                    },
                    Err(e) => {
                        let details = Some(format!("File exists but is locked/inaccessible: {}", e));
                        let new_log = NewDocumentAccessLog { document_id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptView.as_str().to_string(), details };
                        if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log locked view attempt for {}: {:?}", document_id, log_err); }
                        
                        // *** IMPROVED: Better error messages ***
                        if doc.compression_status == CompressionStatus::Processing.as_str() {
                            Err(ServiceError::Ui("Document file is temporarily locked during compression. Please try again in a moment.".to_string()))
                        } else {
                            Err(ServiceError::Ui("Cannot access document file. It may be in use by another process.".to_string()))
                        }
                    }
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let details = Some(format!("File not found: {}", absolute_path.display()));
                let new_log = NewDocumentAccessLog { document_id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptView.as_str().to_string(), details };
                if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log missing file view attempt for {}: {:?}", document_id, log_err); }
                
                // *** IMPROVED: Suggest solutions ***
                if doc.compression_status == CompressionStatus::Processing.as_str() {
                    Err(ServiceError::Ui("Document is being compressed and the original file is temporarily unavailable. Please try again shortly.".to_string()))
                } else {
                    Err(ServiceError::Ui("Document file not found on device. It may need to be downloaded from the cloud.".to_string()))
                }
            },
            Err(e) => {
                let details = Some(format!("File system error: {}", e));
                let new_log = NewDocumentAccessLog { document_id, user_id: auth.user_id, access_type: DocumentAccessType::AttemptView.as_str().to_string(), details };
                if let Err(log_err) = self.doc_log_repo.create(&new_log).await { eprintln!("Failed to log file system error view attempt for {}: {:?}", document_id, log_err); }
                Err(ServiceError::Domain(DomainError::Internal(format!("File system error: {}", e))))
            }
        }
    }

    async fn is_document_available_on_device(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<bool> {
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, document_id)
            .await.map_err(|e| ServiceError::Domain(e))?;
        
        if doc.file_path == "ERROR" {
            return Ok(false);
        }

        let file_path_to_check = if let Some(compressed_path) = &doc.compressed_file_path {
            if doc.compression_status == CompressionStatus::Completed.as_str() {
                compressed_path
            } else {
                &doc.file_path
            }
        } else {
            &doc.file_path
        };

        let absolute_path = self.file_storage_service.get_absolute_path(file_path_to_check);

        match fs::metadata(&absolute_path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ServiceError::Domain(DomainError::Internal(format!("Failed to check file availability: {}", e)))),
        }
    }

    /// Implements proper individual document deletion with file cleanup
    async fn delete_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult> {
        if auth.role != UserRole::Admin { return Err(ServiceError::PermissionDenied("Only admins can hard delete documents".to_string())); }
        let doc_to_delete = match MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await {
            Ok(doc) => doc,
            Err(e @ DomainError::EntityNotFound(_, _)) => return Err(ServiceError::Domain(e)), 
            Err(e) => return Err(ServiceError::Domain(e)), 
        };
        let doc_id_str = id.to_string();
        let active_users_result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM active_file_usage WHERE document_id = ? AND last_active_at > datetime('now', '-5 minutes')"#,
            doc_id_str
        ).fetch_one(&self.pool).await;
        let active_users = match active_users_result { Ok(result) => result.count, Err(e) => { eprintln!("Failed to query active file usage for {}: {:?}. Assuming 0.", id, e); 0 } };
        
        // For testing, you might want to log but not block:
        if active_users > 0 { 
            eprintln!("Warning: Document {} is in use by {} user(s), but proceeding with delete", id, active_users);
            // return Err(ServiceError::Ui(format!("Document is currently in use by {} user(s). Please try again later.", active_users)));
        }
        
        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        let device_id_op_uuid: Option<Uuid> = if auth.device_id.is_empty() { None } else { auth.device_id.as_str().parse().ok() };
        let mut tombstone = Tombstone::new(id, "media_documents", auth.user_id, device_id_op_uuid);
        let metadata = serde_json::json!({
            "file_path": doc_to_delete.file_path,
            "compressed_file_path": doc_to_delete.compressed_file_path,
            "original_filename": doc_to_delete.original_filename,
            "mime_type": doc_to_delete.mime_type,
            "size_bytes": doc_to_delete.size_bytes,
            "related_table": doc_to_delete.related_table,
            "related_id": doc_to_delete.related_id,
            "type_id": doc_to_delete.type_id,
            "deletion_type": "individual",
            "timestamp": Utc::now().to_rfc3339()
        });
        tombstone.additional_metadata = Some(metadata.to_string());
        let operation_id = tombstone.operation_id;
        if let Err(e) = self.delete_service_media_doc.tombstone_repository().create_tombstone_with_tx(&tombstone, &mut tx).await {
             let _ = tx.rollback().await;
             return Err(ServiceError::Domain(e));
        }
        
        let change_log = ChangeLogEntry {
            operation_id, entity_table: "media_documents".to_string(), entity_id: id, operation_type: ChangeOperationType::HardDelete, field_name: None, old_value: None, new_value: None,
            document_metadata: Some(metadata.to_string()), timestamp: Utc::now(), user_id: auth.user_id, device_id: auth.device_id.parse().ok(), sync_batch_id: None, processed_at: None, sync_error: None,
        };
        if let Err(e) = self.delete_service_media_doc.change_log_repository().create_change_log_with_tx(&change_log, &mut tx).await {
             let _ = tx.rollback().await;
             return Err(ServiceError::Domain(e));
        }
        
        if doc_to_delete.compression_status == CompressionStatus::Processing.as_str() {
            if let Err(e) = self.compression_service.cancel_compression(id).await { eprintln!("Failed to cancel compression for deleted document {}: {:?}", id, e); }
        }
        
        let queue_id_str = Uuid::new_v4().to_string();
        let doc_id_q_str = id.to_string();
        let requested_at_str = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let grace_period: i64 = 86400;
        if let Err(e) = sqlx::query!(
            r#"INSERT INTO file_deletion_queue (id, document_id, file_path, compressed_file_path, requested_at, requested_by, grace_period_seconds) VALUES (?, ?, ?, ?, ?, ?, ?)"#,
            queue_id_str, doc_id_q_str, doc_to_delete.file_path, doc_to_delete.compressed_file_path, requested_at_str, user_id_str, grace_period
        ).execute(&mut *tx).await {
             eprintln!("Failed to queue file for deletion DB insert {}: {:?}. Rolling back.", id, e);
             let _ = tx.rollback().await;
             return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
        if let Err(e) = self.media_doc_repo.hard_delete_with_tx(id, auth, &mut tx).await {
             let _ = tx.rollback().await;
             return Err(ServiceError::Domain(e));
        }
        
        let log_id_str = Uuid::new_v4().to_string();
        let log_doc_id_str = id.to_string();
        let log_user_id_str = auth.user_id.to_string();
        let log_access_type = DocumentAccessType::Delete.as_str();
        let log_timestamp_str = Utc::now().to_rfc3339();
        let log_details = "Admin hard delete".to_string();
        if let Err(e) = sqlx::query!(
            r#"INSERT INTO document_access_logs (id, document_id, user_id, access_type, access_date, details) VALUES (?, ?, ?, ?, ?, ?)"#,
            log_id_str, log_doc_id_str, log_user_id_str, log_access_type, log_timestamp_str, log_details
        ).execute(&mut *tx).await {
             eprintln!("Failed to log hard delete access for document {}: {:?}", id, e);
        }
        
        if let Err(e) = tx.commit().await {
            return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
            if doc_to_delete.file_path != "ERROR" {
            let mut original_deleted = false;
            let mut compressed_deleted = false;
    
            // Try to delete original file
                if let Err(e) = self.file_storage_service.delete_file(&doc_to_delete.file_path).await {
                 // Only log if it's not a NotFound error, as that's expected if already deleted
                 if !matches!(e, crate::domains::core::file_storage_service::FileStorageError::NotFound(_)) {
                    eprintln!("Warning: Could not immediately delete file {}: {:?}", doc_to_delete.file_path, e);
                 }
                 // Consider NotFound as success for queue update purposes
                 original_deleted = matches!(e, crate::domains::core::file_storage_service::FileStorageError::NotFound(_));
            } else {
                original_deleted = true;
                }

            // Try to delete compressed file if it exists
                if let Some(compressed_path) = &doc_to_delete.compressed_file_path {
                    if let Err(e) = self.file_storage_service.delete_file(compressed_path).await {
                    if !matches!(e, crate::domains::core::file_storage_service::FileStorageError::NotFound(_)) {
                        eprintln!("Warning: Could not immediately delete compressed file {}: {:?}", compressed_path, e);
                    }
                    compressed_deleted = matches!(e, crate::domains::core::file_storage_service::FileStorageError::NotFound(_));
                } else {
                    compressed_deleted = true;
                }
            } else {
                // No compressed file, consider it "deleted" for the check
                compressed_deleted = true;
            }
            
            // ** ADDED OPTIMIZATION: Update queue if both files deleted **
            if original_deleted && compressed_deleted {
                let doc_id_str = id.to_string();
                let now_str = Utc::now().to_rfc3339();
                let attempts_val: i64 = 1; // Set attempts to 1 since we tried
                
                if let Err(e) = sqlx::query!(
                    r#"
                    UPDATE file_deletion_queue 
                     SET completed_at = ?, 
                         last_attempt_at = ?, 
                         attempts = ? 
                     WHERE document_id = ? AND completed_at IS NULL 
                    "#,
                    now_str, // completed_at
                    now_str, // last_attempt_at
                    attempts_val, // attempts
                    doc_id_str // document_id
                ).execute(&self.pool).await {
                    // Log but don't fail the overall operation if this update fails
                    eprintln!("Warning: Could not update file deletion queue entry after successful immediate delete for doc {}: {:?}", doc_id_str, e);
        }
            }
        }
        Ok(DeleteResult::HardDeleted)
    }

    async fn calculate_document_summary_by_linked_fields(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
    ) -> ServiceResult<DocumentSummary> {
        let doc_params = PaginationParams { page: 1, per_page: 10000 };
        let docs_result = self.media_doc_repo.find_by_related_entity(related_table, related_id, doc_params).await?;
        let mut linked_fields = HashMap::new();
        let mut unlinked_count: i64 = 0;
        let total_count = docs_result.items.len() as i64;
        for doc in &docs_result.items {
            if let Some(field) = &doc.field_identifier { let cleaned_field = field.trim().to_lowercase(); if !cleaned_field.is_empty() { *linked_fields.entry(cleaned_field).or_insert(0i64) += 1; } else { unlinked_count += 1; } } else { unlinked_count += 1; }
        }
        Ok(DocumentSummary { total_count, unlinked_count, linked_fields })
    }
    
    /// Link previously uploaded documents with a temporary ID to their final entity
    async fn link_temp_documents(
        &self,
        temp_related_id: Uuid,
        final_related_table: &str,
        final_related_id: Uuid,
    ) -> ServiceResult<u64> {
        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        let count = match self.media_doc_repo.link_temp_documents_with_tx(temp_related_id, final_related_table, final_related_id, &mut tx).await {
            Ok(count) => count,
            Err(e) => { let _ = tx.rollback().await; return Err(ServiceError::Domain(e)); }
        };
        tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        Ok(count)
    }

    // Active File Usage Tracking
    async fn register_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
        use_type: &str,  // "view" or "edit"
    ) -> ServiceResult<()> {
        let id_str = Uuid::new_v4().to_string();
        let doc_id_str = document_id.to_string();
        let user_id_str = user_id.to_string();
        let device_id_str = device_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let use_type_str = use_type.to_string();
        
        sqlx::query!(
            r#"INSERT INTO active_file_usage (id, document_id, user_id, device_id, started_at, last_active_at, use_type) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(document_id, user_id, device_id) DO UPDATE SET last_active_at = excluded.last_active_at, use_type = excluded.use_type"#,
            id_str, doc_id_str, user_id_str, device_id_str, now_str, now_str, use_type_str
        ).execute(&self.pool).await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        Ok(())
    }

    async fn unregister_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
    ) -> ServiceResult<()> {
        let doc_id_str = document_id.to_string();
        let user_id_str = user_id.to_string();
        let device_id_str = device_id.to_string();
        
        sqlx::query!(
            r#"DELETE FROM active_file_usage WHERE document_id = ? AND user_id = ? AND device_id = ?"#,
            doc_id_str, user_id_str, device_id_str
        ).execute(&self.pool).await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        Ok(())
    }

    // --- iOS Optimized Path-Based Upload Method Implementations ---

    async fn upload_document_from_path(
        &self,
        auth: &AuthContext,
        file_path: String,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse> {
        println!("🚀 [DOCUMENT_SERVICE] Processing upload from path: {}", file_path);
        
        // 1. Read file from path (fast, no Base64 overhead)
        let file_data = match fs::read(&file_path).await {
            Ok(data) => {
                println!("✅ [DOCUMENT_SERVICE] File read successfully: {} bytes", data.len());
                data
            },
            Err(e) => {
                println!("❌ [DOCUMENT_SERVICE] Failed to read file {}: {}", file_path, e);
                return Err(ServiceError::Domain(DomainError::Internal(
                    format!("Failed to read file from path {}: {}", file_path, e)
                )));
            }
        };

        // 2. Clean up the temporary file (convert copy->move operation)
        if let Err(e) = fs::remove_file(&file_path).await {
            println!("⚠️ [DOCUMENT_SERVICE] Failed to clean up temp file {}: {}", file_path, e);
            // Don't fail the upload, just log the warning
        } else {
            println!("🧹 [DOCUMENT_SERVICE] Cleaned up temp file: {}", file_path);
        }

        // 3. Delegate to existing upload_document method (reuse all existing logic)
        self.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            related_entity_id,
            related_entity_type,
            linked_field,
            sync_priority,
            compression_priority,
            temp_related_id,
        ).await
    }

    async fn bulk_upload_documents_from_paths(
        &self,
        auth: &AuthContext,
        file_paths: Vec<(String, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        related_entity_id: Uuid,
        related_entity_type: String,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        println!("🚀 [DOCUMENT_SERVICE] Processing bulk upload from {} paths", file_paths.len());
        
        // Read all files and clean up paths
        let mut files_data = Vec::new();
        for (path, filename) in file_paths {
            match fs::read(&path).await {
                Ok(data) => {
                    println!("✅ [DOCUMENT_SERVICE] Read file {}: {} bytes", filename, data.len());
                    files_data.push((data, filename));
                    
                    // Clean up temp file immediately after reading
                    if let Err(e) = fs::remove_file(&path).await {
                        println!("⚠️ [DOCUMENT_SERVICE] Failed to clean up temp file {}: {}", path, e);
                    } else {
                        println!("🧹 [DOCUMENT_SERVICE] Cleaned up temp file: {}", path);
                    }
                },
                Err(e) => {
                    println!("❌ [DOCUMENT_SERVICE] Failed to read file {}: {}", path, e);
                    return Err(ServiceError::Domain(DomainError::Internal(
                        format!("Failed to read file from path {}: {}", path, e)
                    )));
                }
            }
        }

        // Delegate to existing bulk_upload_documents method (reuse all existing logic)
        self.bulk_upload_documents(
            auth,
            files_data,
            title,
            document_type_id,
            related_entity_id,
            related_entity_type,
            sync_priority,
            compression_priority,
            temp_related_id,
        ).await
    }
}

/// Utility function to guess mime type from filename
fn guess_mime_type(filename: &str) -> String {
    let ext = filename.split('.').last().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg", "png" => "image/png", "gif" => "image/gif", "pdf" => "application/pdf", "doc" => "application/msword", "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "xls" => "application/vnd.ms-excel", "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "ppt" => "application/vnd.ms-powerpoint", "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation", "txt" => "text/plain", "html" | "htm" => "text/html", "csv" => "text/csv", "mp3" => "audio/mpeg", "mp4" => "video/mp4", "mov" => "video/quicktime", "zip" => "application/zip", _ => "application/octet-stream",
    }.to_string()
}

/// Extension methods for the document service to handle deferred deletions and worker start
impl DocumentServiceImpl {
    /// Register document as being actively viewed or edited
    pub async fn register_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
        use_type: &str, // "view" or "edit"
    ) -> ServiceResult<()> {
        let id_str = Uuid::new_v4().to_string();
        let doc_id_str = document_id.to_string();
        let user_id_str = user_id.to_string();
        let device_id_str = device_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let use_type_str = use_type.to_string();
        
        sqlx::query!(
            r#"INSERT INTO active_file_usage (id, document_id, user_id, device_id, started_at, last_active_at, use_type) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(document_id, user_id, device_id) DO UPDATE SET last_active_at = excluded.last_active_at, use_type = excluded.use_type"#,
            id_str, doc_id_str, user_id_str, device_id_str, now_str, now_str, use_type_str
        ).execute(&self.pool).await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        Ok(())
    }
    
    /// Mark document as no longer in use
    pub async fn unregister_document_in_use(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        device_id: Uuid,
    ) -> ServiceResult<()> {
        let doc_id_str = document_id.to_string();
        let user_id_str = user_id.to_string();
        let device_id_str = device_id.to_string();
        
        sqlx::query!(
            r#"DELETE FROM active_file_usage WHERE document_id = ? AND user_id = ? AND device_id = ?"#,
            doc_id_str, user_id_str, device_id_str
        ).execute(&self.pool).await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        Ok(())
    }
    
    /// Start the file deletion worker
    pub fn start_file_deletion_worker(
        pool: SqlitePool,
        file_storage_service: Arc<dyn FileStorageService>,
    ) -> tokio::sync::oneshot::Sender<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        
        // Need to import FileDeletionWorker here
        use crate::domains::document::file_deletion_worker::FileDeletionWorker;

        let worker = FileDeletionWorker::new(pool, file_storage_service)
            .with_shutdown_signal(shutdown_rx);
        
        tokio::spawn(async move {
            if let Err(e) = worker.start().await {
                eprintln!("File deletion worker stopped with error: {:?}", e);
            }
        });
        
        shutdown_tx
    }
}