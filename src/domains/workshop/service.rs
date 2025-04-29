use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::permission::Permission;
use crate::domains::project::repository::ProjectRepository;
use crate::domains::workshop::repository::{SqliteWorkshopRepository, WorkshopRepository};
use crate::domains::workshop::participant_repository::WorkshopParticipantRepository;
use crate::domains::workshop::types::{
    NewWorkshop, Workshop, WorkshopResponse, UpdateWorkshop, WorkshopInclude, ProjectSummary, ParticipantSummary,
    NewWorkshopParticipant, UpdateWorkshopParticipant, WorkshopParticipant
};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError, DbError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;

/// Trait defining workshop service operations
#[async_trait]
pub trait WorkshopService: DeleteService<Workshop> + Send + Sync {
    async fn create_workshop(
        &self,
        new_workshop: NewWorkshop,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse>;

    async fn get_workshop_by_id(
        &self,
        id: Uuid,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse>;

    async fn list_workshops(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>>;

    async fn update_workshop(
        &self,
        id: Uuid,
        update_data: UpdateWorkshop,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse>;
    
    async fn delete_workshop(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    async fn add_participant_to_workshop(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopParticipant>;
    
    async fn remove_participant_from_workshop(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<()>;
    
    async fn update_participant_evaluation(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        update_data: UpdateWorkshopParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopParticipant>;

    async fn create_workshop_with_documents(
        &self,
        new_workshop: NewWorkshop,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(WorkshopResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    async fn upload_document_for_workshop(
        &self,
        workshop_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_workshop(
        &self,
        workshop_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
}

/// Implementation of the workshop service
#[derive(Clone)]
pub struct WorkshopServiceImpl {
    repo: Arc<dyn WorkshopRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Workshop>>,
    project_repo: Arc<dyn ProjectRepository>,
    workshop_participant_repo: Arc<dyn WorkshopParticipantRepository>,
    document_service: Arc<dyn DocumentService>,
    pool: SqlitePool,
}

impl WorkshopServiceImpl {
    pub fn new(
        pool: SqlitePool,
        workshop_repo: Arc<dyn WorkshopRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository>,
        workshop_participant_repo: Arc<dyn WorkshopParticipantRepository>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // Define a local wrapper struct that implements DeleteServiceRepository
        // Note: This adapter pattern is useful if the main repo trait doesn't directly implement all needed sub-traits.
        struct RepoAdapter(Arc<dyn WorkshopRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Workshop> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Workshop> {
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

        let adapted_repo: Arc<dyn DeleteServiceRepository<Workshop>> = 
            Arc::new(RepoAdapter(workshop_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(), // Clone the pool for the delete service
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
        ));
        
        Self {
            repo: workshop_repo,
            delete_service,
            project_repo,
            workshop_participant_repo,
            document_service,
            pool, // Store the pool for direct queries
        }
    }
    
    // Helper function to enrich a single workshop response
    async fn enrich_response(&self, mut response: WorkshopResponse, include: Option<&[WorkshopInclude]>) -> ServiceResult<WorkshopResponse> {
        if let Some(includes) = include {
            let include_all = includes.contains(&WorkshopInclude::All);
            
            // Include Project
            if include_all || includes.contains(&WorkshopInclude::Project) {
                 if let Some(project_id) = response.project_id {
                      match self.project_repo.find_by_id(project_id).await {
                         Ok(project) => {
                             let summary = ProjectSummary {
                                 id: project.id,
                                 name: project.name,
                             };
                             response.project = Some(summary);
                         },
                         Err(DomainError::EntityNotFound(_, _)) => {
                             eprintln!("Warning: Project with ID {} not found for workshop {}", project_id, response.id);
                             response.project = None;
                         },
                         Err(e) => return Err(ServiceError::Domain(e)),
                      }
                 }
            }
            
            // Include Participants - Use the injected real repo
            if include_all || includes.contains(&WorkshopInclude::Participants) {
                 // Directly use self.workshop_participant_repo
                 match self.workshop_participant_repo.find_participants_for_workshop(response.id).await {
                     Ok(participants) => response.participants = Some(participants),
                     Err(e) => return Err(ServiceError::Domain(e)),
                 }
            }
        }
        Ok(response)
    }
}

#[async_trait]
impl DeleteService<Workshop> for WorkshopServiceImpl {
    // Delegate DeleteService methods
    fn repository(&self) -> &dyn FindById<Workshop> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Workshop>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}


#[async_trait]
impl WorkshopService for WorkshopServiceImpl {
    async fn create_workshop(
        &self,
        mut new_workshop: NewWorkshop,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create workshops".to_string(),
            ));
        }
        
        // Set creator ID
        new_workshop.created_by_user_id = Some(auth.user_id);

        // 2. Validate Input DTO
        new_workshop.validate()?;
        
        // Validate project existence if project_id is provided
        if let Some(proj_id) = new_workshop.project_id {
            match self.project_repo.find_by_id(proj_id).await {
                Ok(_) => (), // Project exists, continue
                Err(DomainError::EntityNotFound(_, _)) => {
                    return Err(ServiceError::Domain(DomainError::Validation(
                        ValidationError::relationship(&format!("Project with ID {} not found", proj_id))
                    )));
                }
                Err(e) => return Err(e.into()), // Propagate other DB errors
            }
        }

        // 3. Perform Creation
        let created_workshop = self.repo.create(&new_workshop, auth).await?;

        // 4. Convert to Response DTO (basic)
        Ok(WorkshopResponse::from_workshop(created_workshop))
    }

    async fn get_workshop_by_id(
        &self,
        id: Uuid,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse> {
        if !auth.has_permission(Permission::ViewWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view workshops".to_string(),
            ));
        }
        let workshop = self.repo.find_by_id(id).await?;
        let base_response = WorkshopResponse::from_workshop(workshop);
        self.enrich_response(base_response, include).await
    }

    async fn list_workshops(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>> {
        if !auth.has_permission(Permission::ViewWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list workshops".to_string(),
            ));
        }
        let paginated_result = self.repo.find_all(params, project_id).await?;
        let mut response_items = Vec::with_capacity(paginated_result.items.len());
        for workshop in paginated_result.items {
            let base_response = WorkshopResponse::from_workshop(workshop);
            let enriched_response = self.enrich_response(base_response, include).await?;
            response_items.push(enriched_response);
        }
        Ok(PaginatedResult::new(
            response_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_workshop(
        &self,
        id: Uuid,
        mut update_data: UpdateWorkshop,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopResponse> {
        if !auth.has_permission(Permission::EditWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit workshops".to_string(),
            ));
        }
        update_data.updated_by_user_id = auth.user_id; 
        update_data.validate()?;
        if let Some(opt_proj_id) = update_data.project_id {
            if let Some(proj_id) = opt_proj_id {
                match self.project_repo.find_by_id(proj_id).await {
                    Ok(_) => (), 
                    Err(DomainError::EntityNotFound(_, _)) => {
                        return Err(ServiceError::Domain(DomainError::Validation(
                            ValidationError::relationship(&format!("Project with ID {} not found", proj_id))
                        )));
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let updated_workshop = self.repo.update(id, &update_data, auth).await?;
        Ok(WorkshopResponse::from_workshop(updated_workshop))
    }
    
    async fn delete_workshop(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteWorkshops
        };
        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} workshops",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete,
            force: false,
        };
        let result = self.delete(id, auth, options).await?;
        match result {
            DeleteResult::DependenciesPrevented { dependencies } => {
                 Err(ServiceError::DependenciesPreventDeletion(dependencies))
            },
            _ => Ok(result)
        }
    }
    
    async fn add_participant_to_workshop(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopParticipant> {
        // 1. Check Permissions (Assume editing workshop implies managing participants)
        if !auth.has_permission(Permission::EditWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to manage workshop participants".to_string(),
            ));
        }
        
        // 2. Validate that Workshop and Participant exist
        // Check workshop exists
        self.repo.find_by_id(workshop_id).await.map_err(|e| match e {
            DomainError::EntityNotFound(_, _) => ServiceError::Domain(
                DomainError::EntityNotFound("Workshop".to_string(), workshop_id)
            ),
            _ => ServiceError::Domain(e)
        })?;
        
        // Check participant exists using a direct query
        // We need to verify the participant exists before linking it to a workshop
        let participant_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM participants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(participant_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::Sqlx(e))))?;
        
        if participant_exists == 0 {
            return Err(ServiceError::Domain(
                DomainError::EntityNotFound("Participant".to_string(), participant_id)
            ));
        }
        
        // 3. Create the link DTO
        let new_link = NewWorkshopParticipant {
            workshop_id,
            participant_id,
            pre_evaluation: None, // Set defaults or pass through DTO if needed
            post_evaluation: None,
            created_by_user_id: Some(auth.user_id),
        };
        
        // 4. Call repository
        let created_link = self.workshop_participant_repo.add_participant(&new_link, auth).await?;
        Ok(created_link)
    }
    
    async fn remove_participant_from_workshop(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<()> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::EditWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to manage workshop participants".to_string(),
            ));
        }
        
        // 2. Validate that Workshop exists (optional but good practice)
        // For removal, we're being lenient - if workshop doesn't exist, removal is still "successful"
        let workshop_exists = match self.repo.find_by_id(workshop_id).await {
            Ok(_) => true,
            Err(DomainError::EntityNotFound(_, _)) => false,
            Err(e) => return Err(ServiceError::Domain(e)),
        };
        
        // If workshop doesn't exist, we can consider the removal "successful" (idempotent)
        if !workshop_exists {
            return Ok(());
        }
        
        // 3. Call repository (repo handles not found cases idempotently)
        self.workshop_participant_repo.remove_participant(workshop_id, participant_id, auth).await?;
        Ok(())
    }
    
    async fn update_participant_evaluation(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        mut update_data: UpdateWorkshopParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopParticipant> {
        // 1. Check Permissions
         if !auth.has_permission(Permission::EditWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to update workshop participant evaluations".to_string(),
            ));
        }
        
        // 2. Set updater ID and Validate DTO
        update_data.updated_by_user_id = auth.user_id;
        update_data.validate()?;
        
        // 3. Call repository
        let updated_link = self.workshop_participant_repo
            .update_participant_evaluation(workshop_id, participant_id, &update_data, auth)
            .await?;
            
        Ok(updated_link)
    }

    async fn create_workshop_with_documents(
        &self,
        mut new_workshop: NewWorkshop,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(WorkshopResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateWorkshops) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create workshops".to_string(),
            ));
        }
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTOs
        new_workshop.created_by_user_id = Some(auth.user_id);
        new_workshop.validate()?;
        
        // 3. Begin transaction for workshop creation
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the workshop first (within transaction)
        let created_workshop = match self.repo.create_with_tx(&new_workshop, auth, &mut tx).await {
            Ok(workshop) => workshop,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };
        
        // 5. Commit transaction 
        if let Err(e) = tx.commit().await {
             return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
        // 6. Upload documents (outside transaction)
        let document_results = if !documents.is_empty() {
            // Use the helper method
            self.upload_documents_for_entity(
                created_workshop.id,
                "workshops", // Entity type
                documents,
                document_type_id,
                created_workshop.sync_priority, // Use workshop's priority
                None, // Default compression priority
                auth,
            ).await
        } else {
            Vec::new()
        };
        
        // 7. Convert to Response DTO and return 
        let response = WorkshopResponse::from_workshop(created_workshop);
        // Optionally enrich here if needed immediately after create
        // let enriched_response = self.enrich_response(response, Some(&[WorkshopInclude::Documents]), auth).await?;
        Ok((response, document_results))
    }

    async fn upload_document_for_workshop(
        &self,
        workshop_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify workshop exists
        let _workshop = self.repo.find_by_id(workshop_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !Workshop::is_document_linkable_field(field) {
                let valid_fields: Vec<String> = Workshop::document_linkable_fields()
                    .into_iter()
                    .collect();
                    
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for workshops. Valid fields: {}",
                    field, valid_fields.join(", ")
                )).into()));
            }
        }

        // 4. Delegate to document service
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            workshop_id,
            "workshops".to_string(),
            linked_field.clone(),
            sync_priority,
            compression_priority,
            None,
        ).await?;

        // 5. Update entity reference if it was a document-only field
        if let Some(field_name) = linked_field {
            if let Some(metadata) = Workshop::get_field_metadata(&field_name) {
                if metadata.is_document_reference_only {
                    self.repo.set_document_reference(
                        workshop_id, 
                        &field_name,
                        document.id,
                        auth
                    ).await?;
                }
            }
        }

        Ok(document)
    }

    async fn bulk_upload_documents_for_workshop(
        &self,
        workshop_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify workshop exists
        let _workshop = self.repo.find_by_id(workshop_id).await
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
            workshop_id,
            "workshops".to_string(),
            sync_priority,
            compression_priority,
            None,
        ).await?;

        Ok(documents)
    }
}
