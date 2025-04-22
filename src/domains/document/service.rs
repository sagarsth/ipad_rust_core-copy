use crate::auth::AuthContext;
use crate::types::UserRole;
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::file_storage_service::{FileStorageService, FileStorageResult, FileStorageError};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::sync::repository::{TombstoneRepository, ChangeLogRepository};
use crate::domains::document::repository::{
    DocumentAccessLogRepository, DocumentTypeRepository, MediaDocumentRepository,
    DocumentVersionRepository, SqliteDocumentTypeRepository, SqliteMediaDocumentRepository,
    SqliteDocumentVersionRepository, SqliteDocumentAccessLogRepository,
};
use crate::domains::document::types::{
    DocumentType, DocumentTypeResponse, MediaDocument, MediaDocumentResponse, NewDocumentType,
    UpdateDocumentType, NewMediaDocument, UpdateMediaDocument, DocumentVersion, NewDocumentAccessLog,
    DocumentAccessLog, CompressionStatus, BlobSyncStatus, DocumentAccessType,
};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use crate::validation::Validate;
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::Arc;
use uuid::Uuid;

// --- Includes Enum ---

/// Enum to specify related data to include in Document responses
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentInclude {
    DocumentType,
    Versions,
    AccessLogs(PaginationParams), // Allow pagination for logs
}

// --- Service Trait ---

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

    // Media Document Operations
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
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn create_media_document_metadata(
        &self,
        auth: &AuthContext,
        new_doc_metadata: NewMediaDocument,
        file_path: &str,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn get_media_document_by_id(
        &self,
        auth: &AuthContext,
        id: Uuid,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse>;

    /// Download document data (Admin/TL only)
    async fn download_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<(String, Vec<u8>)>;

    async fn list_media_documents_by_related_entity(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<PaginatedResult<MediaDocumentResponse>>;

    async fn update_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
        update_data: UpdateMediaDocument,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse>;

    /// Hard deletes a media document (Admin only)
    async fn delete_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult>;

    // Document Version Operations (Typically created during document creation/update)
    // We might not expose a direct `create_version` if it's internal
    async fn list_document_versions(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> ServiceResult<Vec<DocumentVersion>>; // Consider if a Response DTO is needed

    // Document Access Log Operations
    async fn log_document_access(
        &self,
        new_log: NewDocumentAccessLog,
    ) -> ServiceResult<DocumentAccessLog>; // Consider if a Response DTO is needed

    async fn list_document_access_logs(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        params: PaginationParams,
    ) -> ServiceResult<PaginatedResult<DocumentAccessLog>>; // Consider Response DTO

     // Status Update Operations (Potentially internal or triggered by background jobs)
     async fn update_compression_status(
         &self,
         id: Uuid,
         status: CompressionStatus,
         compressed_file_path: Option<&str>,
     ) -> ServiceResult<()>;

     async fn update_blob_sync_status(
         &self,
         id: Uuid,
         status: BlobSyncStatus,
         blob_key: Option<&str>,
     ) -> ServiceResult<()>;

    // New method
    async fn update_document_sync_priority(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        priority: SyncPriority,
    ) -> ServiceResult<MediaDocument>;
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
}

impl DocumentServiceImpl {
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
    ) -> Self {
        // --- Adapters for Delete Services ---
        // Adapter for DocumentType
        struct DocTypeRepoAdapter(Arc<dyn DocumentTypeRepository>);
        #[async_trait]
        impl FindById<DocumentType> for DocTypeRepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<DocumentType> { self.0.find_by_id(id).await }
        }
        #[async_trait]
        impl SoftDeletable for DocTypeRepoAdapter {
             async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { self.0.soft_delete(id, auth).await }
             async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { self.0.soft_delete_with_tx(id, auth, tx).await }
        }
        #[async_trait]
        impl HardDeletable for DocTypeRepoAdapter {
             fn entity_name(&self) -> &'static str { self.0.entity_name() }
             async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { self.0.hard_delete(id, auth).await }
             async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { self.0.hard_delete_with_tx(id, auth, tx).await }
        }
        let adapted_doc_type_repo: Arc<dyn DeleteServiceRepository<DocumentType>> = Arc::new(DocTypeRepoAdapter(doc_type_repo.clone()));

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
             async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { self.0.soft_delete(id, auth).await }
             async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { self.0.soft_delete_with_tx(id, auth, tx).await }
        }
         #[async_trait]
        impl HardDeletable for MediaDocRepoAdapter {
             fn entity_name(&self) -> &'static str { self.0.entity_name() }
             async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> { self.0.hard_delete(id, auth).await }
             async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut Transaction<'_, Sqlite>) -> DomainResult<()> { self.0.hard_delete_with_tx(id, auth, tx).await }
        }
        
        let adapted_media_doc_repo: Arc<dyn DeleteServiceRepository<MediaDocument>> = Arc::new(MediaDocRepoAdapter(media_doc_repo.clone()));

        // --- Create Delete Services ---
        let delete_service_doc_type = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_doc_type_repo,
            tombstone_repo.clone(),
            change_log_repo.clone(),
            dependency_checker.clone(),
            None,
        ));

        let delete_service_media_doc = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_media_doc_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
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
        }
    }

    /// Helper to enrich MediaDocumentResponse with included data
    async fn enrich_response(
        &self,
        mut response: MediaDocumentResponse,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse> {
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
                                    // Type not found, maybe log a warning but don't fail the request
                                    response.type_name = Some("<Type Not Found>".to_string());
                                }
                                Err(e) => return Err(e.into()), // Propagate other DB errors
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
                    _ => {} // Ignore includes not relevant to MediaDocumentResponse enrichment
                }
            }
        }
        Ok(response)
    }
}

// --- DeleteService Implementations ---

#[async_trait]
impl DeleteService<DocumentType> for DocumentServiceImpl {
    // Delegate all methods to self.delete_service_doc_type
     fn repository(&self) -> &dyn FindById<DocumentType> { self.delete_service_doc_type.repository() }
     fn tombstone_repository(&self) -> &dyn TombstoneRepository { self.delete_service_doc_type.tombstone_repository() }
     fn change_log_repository(&self) -> &dyn ChangeLogRepository { self.delete_service_doc_type.change_log_repository() }
     fn dependency_checker(&self) -> &dyn DependencyChecker { self.delete_service_doc_type.dependency_checker() }
     async fn delete(&self, id: Uuid, auth: &AuthContext, options: DeleteOptions) -> DomainResult<DeleteResult> { self.delete_service_doc_type.delete(id, auth, options).await }
     async fn batch_delete(&self, ids: &[Uuid], auth: &AuthContext, options: DeleteOptions) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { self.delete_service_doc_type.batch_delete(ids, auth, options).await }
     async fn delete_with_dependencies(&self, id: Uuid, auth: &AuthContext) -> DomainResult<DeleteResult> { self.delete_service_doc_type.delete_with_dependencies(id, auth).await }
     async fn get_failed_delete_details(&self, batch_result: &crate::domains::core::delete_service::BatchDeleteResult, auth: &AuthContext) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<DocumentType>>> { self.delete_service_doc_type.get_failed_delete_details(batch_result, auth).await }
}

#[async_trait]
impl DeleteService<MediaDocument> for DocumentServiceImpl {
    // Delegate all methods to self.delete_service_media_doc
     fn repository(&self) -> &dyn FindById<MediaDocument> { self.delete_service_media_doc.repository() }
     fn tombstone_repository(&self) -> &dyn TombstoneRepository { self.delete_service_media_doc.tombstone_repository() }
     fn change_log_repository(&self) -> &dyn ChangeLogRepository { self.delete_service_media_doc.change_log_repository() }
     fn dependency_checker(&self) -> &dyn DependencyChecker { self.delete_service_media_doc.dependency_checker() }
     async fn delete(&self, id: Uuid, auth: &AuthContext, options: DeleteOptions) -> DomainResult<DeleteResult> { self.delete_service_media_doc.delete(id, auth, options).await }
     async fn batch_delete(&self, ids: &[Uuid], auth: &AuthContext, options: DeleteOptions) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { self.delete_service_media_doc.batch_delete(ids, auth, options).await }
     async fn delete_with_dependencies(&self, id: Uuid, auth: &AuthContext) -> DomainResult<DeleteResult> { self.delete_service_media_doc.delete_with_dependencies(id, auth).await }
     async fn get_failed_delete_details(&self, batch_result: &crate::domains::core::delete_service::BatchDeleteResult, auth: &AuthContext) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<MediaDocument>>> { self.delete_service_media_doc.get_failed_delete_details(batch_result, auth).await }
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
        // No permission check needed - any user can create
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

    async fn list_document_types(
        &self,
        params: PaginationParams,
    ) -> ServiceResult<PaginatedResult<DocumentTypeResponse>> {
        let paginated_result = self.doc_type_repo.find_all(params).await?;
        let response_items = paginated_result.items.into_iter().map(DocumentTypeResponse::from).collect();
        Ok(PaginatedResult::new(response_items, paginated_result.total, params))
    }

    async fn update_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
        update_data: UpdateDocumentType,
    ) -> ServiceResult<DocumentTypeResponse> {
        // No permission check needed - any user can update
        update_data.validate()?;
        if let Some(name) = &update_data.name {
             if let Some(existing) = self.doc_type_repo.find_by_name(name).await? {
                 if existing.id != id {
                     return Err(DomainError::Validation(
                        crate::errors::ValidationError::unique("name")
                     ).into());
                 }
             }
        }
        let updated = self.doc_type_repo.update(id, &update_data, auth).await?;
        Ok(DocumentTypeResponse::from(updated))
    }

    /// Hard deletes a document type (Admin only)
    async fn delete_document_type(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult> {
        if auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can hard delete document types.".to_string()).into());
        }
        let options = DeleteOptions { allow_hard_delete: true, fallback_to_soft_delete: false, force: false };
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
        temp_related_id: Option<Uuid>,
    ) -> ServiceResult<MediaDocumentResponse> {
        // No permission check needed - any user can upload
        let doc_type = self.doc_type_repo.find_by_id(document_type_id).await?;

        let entity_or_temp_id_str = if let Some(temp_id) = temp_related_id {
            temp_id.to_string()
        } else {
            related_entity_id.to_string()
        };

        let (relative_path, size_bytes) = self.file_storage_service.save_file(
            file_data,
            &related_entity_type,
            &entity_or_temp_id_str,
            &original_filename,
        ).await.map_err(|e| ServiceError::Domain(e.into()))?;

        let new_doc_metadata = NewMediaDocument {
            id: Uuid::new_v4(),
            related_table: related_entity_type.clone(),
            related_id: if temp_related_id.is_some() { None } else { Some(related_entity_id) },
            temp_related_id,
            type_id: document_type_id,
            original_filename: original_filename.clone(),
            title: title.or_else(|| Some(original_filename.clone())),
            mime_type: guess_mime_type(&original_filename),
            size_bytes: size_bytes as i64,
            field_identifier: linked_field,
            sync_priority,
            created_by_user_id: Some(auth.user_id),
        };

        new_doc_metadata.validate()?;
        let created_doc = self.media_doc_repo.create(&new_doc_metadata, &relative_path).await?;
        let mut response = MediaDocumentResponse::from_doc(&created_doc, Some(doc_type.name));
        response = self.enrich_response(response, None).await?;
        Ok(response)
    }
    
    async fn create_media_document_metadata(
        &self,
        auth: &AuthContext,
        new_doc_metadata: NewMediaDocument,
        file_path: &str,
    ) -> ServiceResult<MediaDocumentResponse> {
        new_doc_metadata.validate()?;
        let doc_type = self.doc_type_repo.find_by_id(new_doc_metadata.type_id).await?;
        let created_doc = self.media_doc_repo.create(&new_doc_metadata, file_path).await?;
        let mut response = MediaDocumentResponse::from_doc(&created_doc, Some(doc_type.name));
        response = self.enrich_response(response, Some(&[DocumentInclude::DocumentType])).await?;
        Ok(response)
    }

    async fn get_media_document_by_id(
        &self,
        auth: &AuthContext,
        id: Uuid,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse> {
        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await?;
        let type_name = if include.map_or(true, |incs| !incs.contains(&DocumentInclude::DocumentType)) {
             self.doc_type_repo.find_by_id(doc.type_id).await.ok().map(|dt| dt.name)
        } else { None };
        let mut response = MediaDocumentResponse::from_doc(&doc, type_name);

        let new_log = NewDocumentAccessLog {
            document_id: id,
            user_id: auth.user_id,
            access_type: DocumentAccessType::View.as_str().to_string(),
            details: None,
        };
        self.doc_log_repo.create(&new_log).await?;

        response = self.enrich_response(response, include).await?;
        Ok(response)
    }

    /// Download document data (Admin/TL only)
    async fn download_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<(String, Vec<u8>)> {
        // Only Admin and Field Team Lead can download
        if auth.role != UserRole::Admin && auth.role != UserRole::FieldTeamLead {
             return Err(DomainError::AuthorizationFailed("Admin or Field Team Lead required to download documents.".to_string()).into());
        }

        let doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await?;
        
        // If from internet/another device, only provide compressed version if available
        let file_path_to_download = if auth.offline_mode {
            // Local device mode - use original file path
            &doc.file_path
        } else {
            // Internet mode - prefer compressed version if available
            doc.compressed_file_path.as_ref().unwrap_or(&doc.file_path)
        };

        let file_data = self.file_storage_service.get_file_data(file_path_to_download)
            .await.map_err(|e| ServiceError::Domain(e.into()))?;

        let new_log = NewDocumentAccessLog {
            document_id: id,
            user_id: auth.user_id,
            access_type: DocumentAccessType::Download.as_str().to_string(),
            details: None,
        };
        self.doc_log_repo.create(&new_log).await?;

        Ok((doc.file_name.clone(), file_data))
    }

    async fn list_media_documents_by_related_entity(
        &self,
        auth: &AuthContext,
        related_table: &str,
        related_id: Uuid,
        params: PaginationParams,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<PaginatedResult<MediaDocumentResponse>> {
        let result = self.media_doc_repo.find_by_related_entity(related_table, related_id, params).await?;
        let mut response_items = Vec::new();
        for item in result.items {
            let response = MediaDocumentResponse::from_doc(&item, None);
            let enriched_response = self.enrich_response(response, include).await?;
            response_items.push(enriched_response);
        }
        Ok(PaginatedResult::new(response_items, result.total, params))
    }

    async fn update_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
        update_data: UpdateMediaDocument,
        include: Option<&[DocumentInclude]>,
    ) -> ServiceResult<MediaDocumentResponse> {
        // No permission check needed - any user can update
        update_data.validate()?;
        let updated_doc = self.media_doc_repo.update(id, &update_data, auth).await?;

        let type_name = if include.map_or(true, |incs| !incs.contains(&DocumentInclude::DocumentType)) {
            self.doc_type_repo.find_by_id(updated_doc.type_id).await.ok().map(|dt| dt.name)
        } else { None };
        let mut response = MediaDocumentResponse::from_doc(&updated_doc, type_name);

        let new_log = NewDocumentAccessLog {
            document_id: id,
            user_id: auth.user_id,
            access_type: DocumentAccessType::EditMetadata.as_str().to_string(),
            details: None,
        };
        self.doc_log_repo.create(&new_log).await?;

        response = self.enrich_response(response, include).await?;
        Ok(response)
    }

    /// Hard deletes a media document (Admin only)
    async fn delete_media_document(
        &self,
        auth: &AuthContext,
        id: Uuid,
    ) -> ServiceResult<DeleteResult> {
        // Only Admin can hard delete documents
        if auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can hard delete documents.".to_string()).into());
        }
        
        // Get the document before deletion for file cleanup
        let doc_to_delete = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, id).await?;
        
        // Hard delete option - admin only
        let options = DeleteOptions { 
            allow_hard_delete: true, 
            fallback_to_soft_delete: false, 
            force: false 
        };
        
        // Perform deletion
        let delete_result = DeleteService::<MediaDocument>::delete(self, id, auth, options).await?;
        
        // Handle file cleanup based on delete result 
        if let DeleteResult::HardDeleted = delete_result {
            // Delete the associated files
            if let Err(e) = self.file_storage_service.delete_file(&doc_to_delete.file_path).await {
                // Log but don't fail the operation if file deletion fails
                eprintln!("Error deleting file {}: {:?}", doc_to_delete.file_path, e);
            }
            
            // Delete compressed file if exists
            if let Some(compressed_path) = &doc_to_delete.compressed_file_path {
                if let Err(e) = self.file_storage_service.delete_file(compressed_path).await {
                    eprintln!("Error deleting compressed file {}: {:?}", compressed_path, e);
                }
            }
        }

        // Log the deletion attempt
        let new_log = NewDocumentAccessLog {
            document_id: id,
            user_id: auth.user_id,
            access_type: DocumentAccessType::Delete.as_str().to_string(),
            details: None,
        };
        if let Err(e) = self.doc_log_repo.create(&new_log).await {
            eprintln!("Error logging document deletion: {:?}", e);
        }

        Ok(delete_result)
    }

    // --- Document Version Methods ---
    async fn list_document_versions(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> ServiceResult<Vec<DocumentVersion>> {
        self.get_media_document_by_id(auth, document_id, None).await?;
        Ok(self.doc_ver_repo.find_by_document_id(document_id).await?)
    }

    // --- Document Access Log Methods ---
    async fn log_document_access(
        &self,
        new_log: NewDocumentAccessLog,
    ) -> ServiceResult<DocumentAccessLog> {
        new_log.validate()?;
        Ok(self.doc_log_repo.create(&new_log).await?)
    }

    async fn list_document_access_logs(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        params: PaginationParams,
    ) -> ServiceResult<PaginatedResult<DocumentAccessLog>> {
        self.get_media_document_by_id(auth, document_id, None).await?;
        Ok(self.doc_log_repo.find_by_document_id(document_id, params).await?)
    }

     // --- Status Update Methods ---
     async fn update_compression_status(
         &self,
         id: Uuid,
         status: CompressionStatus,
         compressed_file_path: Option<&str>,
     ) -> ServiceResult<()> {
         self.media_doc_repo.update_compression_status(id, status, compressed_file_path).await?;
         Ok(())
     }

     async fn update_blob_sync_status(
         &self,
         id: Uuid,
         status: BlobSyncStatus,
         blob_key: Option<&str>,
     ) -> ServiceResult<()> {
         self.media_doc_repo.update_blob_sync_status(id, status, blob_key).await?;
         Ok(())
     }

    // Fix sync priority update
    async fn update_document_sync_priority(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        priority: SyncPriority,
    ) -> ServiceResult<MediaDocument> {
        let ids = [document_id];
        let rows_affected = self.media_doc_repo.update_sync_priority(&ids, priority, auth).await?;
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound("MediaDocument".to_string(), document_id).into());
        }
        let updated_doc = MediaDocumentRepository::find_by_id(&*self.media_doc_repo, document_id).await?;
        Ok(updated_doc)
    }
}

/// Utility function to guess mime type from filename
fn guess_mime_type(filename: &str) -> String {
    // Simple mime type guess based on file extension
    let ext = filename.split('.').last().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        "doc" | "docx" => "application/msword",
        "xls" | "xlsx" => "application/vnd.ms-excel",
        "ppt" | "pptx" => "application/vnd.ms-powerpoint",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }.to_string()
} 