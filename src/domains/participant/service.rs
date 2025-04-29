use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::permission::Permission;
use crate::domains::participant::repository::ParticipantRepository;
use crate::domains::participant::types::{NewParticipant, Participant, ParticipantResponse, UpdateParticipant, ParticipantInclude};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::errors::{DbError, DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Trait defining participant service operations
#[async_trait]
pub trait ParticipantService: DeleteService<Participant> + Send + Sync {
    async fn create_participant(
        &self,
        new_participant: NewParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;

    async fn get_participant_by_id(
        &self,
        id: Uuid,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;

    async fn list_participants(
        &self,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    async fn update_participant(
        &self,
        id: Uuid,
        update_data: UpdateParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;
    
    async fn delete_participant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    async fn upload_document_for_participant(
        &self,
        participant_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_participant(
        &self,
        participant_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
    
    async fn create_participant_with_documents(
        &self,
        new_participant: NewParticipant,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ParticipantResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;
    
    // Add methods for workshop/livelihood management if needed
    // async fn add_participant_to_workshop(...)
    // async fn remove_participant_from_workshop(...)
}

/// Implementation of the participant service
#[derive(Clone)] 
pub struct ParticipantServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn ParticipantRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Participant>>,
    document_service: Arc<dyn DocumentService>,
}

impl ParticipantServiceImpl {
    pub fn new(
        pool: SqlitePool,
        participant_repo: Arc<dyn ParticipantRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // Local adapter struct
        struct RepoAdapter(Arc<dyn ParticipantRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Participant> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Participant> {
                self.0.find_by_id(id).await
            }
        }

        #[async_trait]
        impl SoftDeletable for RepoAdapter {
             async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
                 self.0.soft_delete(id, auth).await
             }
             async fn soft_delete_with_tx(
                 &self,
                 id: Uuid,
                 auth: &AuthContext,
                 tx: &mut Transaction<'_, Sqlite>,
             ) -> DomainResult<()> {
                 self.0.soft_delete_with_tx(id, auth, tx).await
             }
        }

        #[async_trait]
        impl HardDeletable for RepoAdapter {
             fn entity_name(&self) -> &'static str {
                 self.0.entity_name()
             }
             async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
                 self.0.hard_delete(id, auth).await
             }
             async fn hard_delete_with_tx(
                 &self,
                 id: Uuid,
                 auth: &AuthContext,
                 tx: &mut Transaction<'_, Sqlite>,
             ) -> DomainResult<()> {
                 self.0.hard_delete_with_tx(id, auth, tx).await
             }
        }
        
        // Blanket impl covers DeleteServiceRepository<Participant>

        let adapted_repo: Arc<dyn DeleteServiceRepository<Participant>> = 
            Arc::new(RepoAdapter(participant_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
        ));
        
        Self {
            pool,
            repo: participant_repo,
            delete_service,
            document_service,
        }
    }
    
    async fn enrich_response(
        &self,
        mut response: ParticipantResponse,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if let Some(includes) = include {
            let include_docs = includes.contains(&ParticipantInclude::Documents) || 
                               includes.contains(&ParticipantInclude::All);

            if include_docs {
                let doc_params = PaginationParams::default();
                let docs_result = self.document_service
                    .list_media_documents_by_related_entity(
                        auth,
                        "participants",
                        response.id,
                        doc_params,
                        None
                    ).await?;
                response.documents = Some(docs_result.items);
            }
        }
        
        Ok(response)
    }
    
    async fn upload_documents_for_entity(
        &self,
        entity_id: Uuid,
        entity_type: &str,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> Vec<Result<MediaDocumentResponse, ServiceError>> {
        let mut results = Vec::new();
        
        for (file_data, filename, linked_field) in documents {
            let upload_result = self.document_service.upload_document(
                auth,
                file_data,
                filename.clone(),
                None,
                document_type_id,
                entity_id,
                entity_type.to_string(),
                linked_field,
                sync_priority,
                compression_priority,
                None,
            ).await;
            
            results.push(upload_result);
        }
        
        results
    }
}

// Implement DeleteService<Participant> by delegating
#[async_trait]
impl DeleteService<Participant> for ParticipantServiceImpl {
    fn repository(&self) -> &dyn FindById<Participant> {
        self.delete_service.repository()
    }
    fn tombstone_repository(&self) -> &dyn TombstoneRepository {
        self.delete_service.tombstone_repository()
    }
    fn change_log_repository(&self) -> &dyn ChangeLogRepository {
        self.delete_service.change_log_repository()
    }
    fn dependency_checker(&self) -> &dyn DependencyChecker {
        self.delete_service.dependency_checker()
    }
    async fn delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<DeleteResult> {
        self.delete_service.delete(id, auth, options).await
    }
    async fn batch_delete(
        &self,
        ids: &[Uuid],
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> {
        self.delete_service.batch_delete(ids, auth, options).await
    }
    async fn delete_with_dependencies(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<DeleteResult> {
        self.delete_service.delete_with_dependencies(id, auth).await
    }
    async fn get_failed_delete_details(
        &self,
        batch_result: &crate::domains::core::delete_service::BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Participant>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl ParticipantService for ParticipantServiceImpl {
    async fn create_participant(
        &self,
        new_participant: NewParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if !auth.has_permission(Permission::CreateParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create participants".to_string(),
            ));
        }

        new_participant.validate()?;

        let created_participant = self.repo.create(&new_participant, auth).await?;
        Ok(ParticipantResponse::from(created_participant))
    }

    async fn get_participant_by_id(
        &self,
        id: Uuid,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view participants".to_string(),
            ));
        }

        let participant = self.repo.find_by_id(id).await?;
        let response = ParticipantResponse::from(participant);
        
        self.enrich_response(response, include, auth).await
    }

    async fn list_participants(
        &self,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list participants".to_string(),
            ));
        }

        let paginated_result = self.repo.find_all(params).await?;

        let mut enriched_items = Vec::with_capacity(paginated_result.items.len());
        for item in paginated_result.items {
            let response = ParticipantResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; 
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_participant(
        &self,
        id: Uuid,
        mut update_data: UpdateParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if !auth.has_permission(Permission::EditParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit participants".to_string(),
            ));
        }

        update_data.updated_by_user_id = auth.user_id;
        update_data.validate()?;
        
        // Ensure participant exists before update
        let _ = self.repo.find_by_id(id).await?;

        let updated_participant = self.repo.update(id, &update_data, auth).await?;
        Ok(ParticipantResponse::from(updated_participant))
    }
    
    async fn delete_participant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteRecord
        };

        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} participants",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Fetch participant first to check existence
        let _ = self.repo.find_by_id(id).await?;

        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            // Note: Consider dependencies (workshops, livelihoods) - may need DependencyChecker
            // If hard delete is disallowed due to dependencies, it might error or soft delete instead.
            fallback_to_soft_delete: !hard_delete, 
            force: false, 
        };
        
        // Use the delete method inherited from DeleteService<Participant>
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }
    
    async fn upload_document_for_participant(
        &self,
        participant_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify participant exists
        let _participant = self.repo.find_by_id(participant_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !Participant::is_document_linkable_field(field) {
                let valid_fields: Vec<String> = Participant::document_linkable_fields()
                    .into_iter()
                    .collect();
                    
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for participants. Valid fields: {}",
                    field, valid_fields.join(", ")
                )).into())); 
            }
        }

        // 4. Delegate to document service, passing linked_field
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            participant_id,
            "participants".to_string(), // Entity type
            linked_field.clone(), // Pass the validated field name
            sync_priority,
            compression_priority,
            None, 
        ).await?;

        // 5. --- NEW: Update entity reference if it was a document-only field ---
        if let Some(field_name) = linked_field {
            if let Some(metadata) = Participant::get_field_metadata(&field_name) {
                if metadata.is_document_reference_only {
                    // Call repo method to update the specific reference column
                    self.repo.set_document_reference(
                        participant_id, 
                        &field_name, // e.g., "profile_photo"
                        document.id, // The ID of the newly created MediaDocument
                        auth
                    ).await?;
                }
            }
        }

        Ok(document)
    }

    async fn bulk_upload_documents_for_participant(
        &self,
        participant_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify participant exists
        let _participant = self.repo.find_by_id(participant_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Delegate to document service
        let documents = self.document_service.bulk_upload_documents(
            auth,
            files,
            title,
            document_type_id,
            participant_id,
            "participants".to_string(),
            sync_priority,
            compression_priority,
            None,
        ).await?;

        Ok(documents)
    }
    
    async fn create_participant_with_documents(
        &self,
        new_participant: NewParticipant,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ParticipantResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create participants".to_string(),
            ));
        }
        
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_participant.validate()?;
        
        // 3. Begin transaction for participant creation
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the participant first (within transaction)
        let created_participant = match self.repo.create_with_tx(&new_participant, auth, &mut tx).await {
            Ok(participant) => participant,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };
        
        // 5. Commit transaction to ensure participant is created before attaching docs
        if let Err(e) = tx.commit().await {
             return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
        // 6. Now upload documents (outside transaction)
        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_participant.id,
                "participants",
                documents,
                document_type_id,
                new_participant.sync_priority.unwrap_or(SyncPriority::Normal),
                None,
                auth,
            ).await
        } else {
            Vec::new()
        };
        
        // 7. Convert to Response DTO and return with document results
        let response = ParticipantResponse::from(created_participant);
        Ok((response, document_results))
    }
}
