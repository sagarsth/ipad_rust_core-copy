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
    NewWorkshopParticipant, UpdateWorkshopParticipant, WorkshopParticipant,
    WorkshopStatistics, WorkshopWithParticipants, WorkshopWithDocumentTimeline, WorkshopBudgetSummary, ProjectWorkshopMetrics, ParticipantAttendance,
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
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use crate::domains::core::delete_service::PendingDeletionManager;

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

    /// Get comprehensive workshop statistics for dashboard
    async fn get_workshop_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopStatistics>;

    /// Get workshop budget statistics
    async fn get_budget_statistics(
        &self,
        project_id: Option<Uuid>,
        auth: &AuthContext,
    ) -> ServiceResult<(Decimal, Decimal, Decimal, f64)>; // (total_budget, total_actuals, total_variance, avg_variance_pct)

    /// Find workshops by date range
    async fn find_workshops_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>>;

    /// Find past workshops
    async fn find_past_workshops(
        &self,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>>;

    /// Find upcoming workshops
    async fn find_upcoming_workshops(
        &self,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>>;

    /// Find workshops by location
    async fn find_workshops_by_location(
        &self,
        location: &str,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>>;

    /// Get workshop with detailed participant information
    async fn get_workshop_with_participants(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopWithParticipants>;

    /// Get workshop with document timeline
    async fn get_workshop_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopWithDocumentTimeline>;

    /// Get budget summaries for project workshops
    async fn get_workshop_budget_summaries_for_project(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<WorkshopBudgetSummary>>;

    /// Get project workshop metrics
    async fn get_project_workshop_metrics(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectWorkshopMetrics>;

    /// Get participant attendance record
    async fn get_participant_attendance(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantAttendance>;

    /// Batch add participants to a workshop
    async fn batch_add_participants_to_workshop(
        &self,
        workshop_id: Uuid,
        participant_ids: Vec<Uuid>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<(Uuid, Result<(), ServiceError>)>>;

    /// Find participants with missing evaluations
    async fn find_participants_with_missing_evaluations(
        &self,
        workshop_id: Uuid,
        eval_type: &str, // "pre" or "post"
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ParticipantSummary>>;
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
        deletion_manager: Arc<PendingDeletionManager>,
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
                "workshop"
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
            None,
            deletion_manager,
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
        update_data.updated_by_user_id = Some(auth.user_id);
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
        auth.authorize(Permission::UploadDocuments)?;

        let _workshop = self.repo.find_by_id(workshop_id).await.map_err(ServiceError::Domain)?;

        let mut results = Vec::new();
        for (file_data, original_filename) in files {
            let result = self.document_service.upload_document(
                auth,
                file_data,
                original_filename,
                title.clone(),
                document_type_id,
                workshop_id,
                "workshops".to_string(),
                None, // No specific linked field for bulk uploads
                sync_priority,
                compression_priority,
                None, // No transaction needed here, handled by document service
            ).await?;
            results.push(result);
        }
        Ok(results)
    }

    async fn get_workshop_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopStatistics> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Get statistics from repository
        let statistics = self.repo.get_workshop_statistics().await
            .map_err(ServiceError::Domain)?;
        
        Ok(statistics)
    }

    async fn get_budget_statistics(
        &self,
        project_id: Option<Uuid>,
        auth: &AuthContext,
    ) -> ServiceResult<(Decimal, Decimal, Decimal, f64)> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Check project existence if provided
        if let Some(pid) = project_id {
            match self.project_repo.find_by_id(pid).await {
                Ok(_) => (), // Project exists
                Err(DomainError::EntityNotFound(_, _)) => {
                    return Err(ServiceError::Domain(
                        DomainError::EntityNotFound("Project".to_string(), pid)
                    ));
                },
                Err(e) => return Err(ServiceError::Domain(e)),
            }
        }

        // 3. Get budget statistics from repository
        let stats = self.repo.get_budget_statistics(project_id).await
            .map_err(ServiceError::Domain)?;
        
        Ok(stats)
    }

    async fn find_workshops_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Parse date range
        let start_date = DateTime::parse_from_rfc3339(start_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(ValidationError::custom(
                &format!("Invalid start date format: {}", e)
            ))))?
            .with_timezone(&Utc);
        let end_date = DateTime::parse_from_rfc3339(end_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(ValidationError::custom(
                &format!("Invalid end date format: {}", e)
            ))))?
            .with_timezone(&Utc);

        // 3. Validate date range
        if start_date > end_date {
            return Err(ServiceError::Domain(
                DomainError::Validation(ValidationError::custom(
                    "Start date cannot be after end date"
                ))
            ));
        }

        // 4. Find workshops in date range
        let paginated_result = self.repo.find_by_date_range(start_date, end_date, params).await
            .map_err(ServiceError::Domain)?;

        // 5. Convert and enrich each workshop
        let mut enriched_items = Vec::new();
        for workshop in paginated_result.items {
            let response = WorkshopResponse::from_workshop(workshop);
            let enriched = self.enrich_response(response, include).await?;
            enriched_items.push(enriched);
        }

        // 6. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_past_workshops(
        &self,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Find past workshops
        let paginated_result = self.repo.find_past_workshops(params).await
            .map_err(ServiceError::Domain)?;

        // 3. Convert and enrich each workshop
        let mut enriched_items = Vec::new();
        for workshop in paginated_result.items {
            let response = WorkshopResponse::from_workshop(workshop);
            let enriched = self.enrich_response(response, include).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_upcoming_workshops(
        &self,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Find upcoming workshops
        let paginated_result = self.repo.find_upcoming_workshops(params).await
            .map_err(ServiceError::Domain)?;

        // 3. Convert and enrich each workshop
        let mut enriched_items = Vec::new();
        for workshop in paginated_result.items {
            let response = WorkshopResponse::from_workshop(workshop);
            let enriched = self.enrich_response(response, include).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_workshops_by_location(
        &self,
        location: &str,
        params: PaginationParams,
        include: Option<&[WorkshopInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<WorkshopResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Find workshops by location
        let paginated_result = self.repo.find_by_location(location, params).await
            .map_err(ServiceError::Domain)?;

        // 3. Convert and enrich each workshop
        let mut enriched_items = Vec::new();
        for workshop in paginated_result.items {
            let response = WorkshopResponse::from_workshop(workshop);
            let enriched = self.enrich_response(response, include).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_workshop_with_participants(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopWithParticipants> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get the workshop
        let workshop = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        let workshop_response = WorkshopResponse::from_workshop(workshop);
        
        // 3. Get detailed participant information
        let participants = self.workshop_participant_repo.find_participants_with_details(id).await
            .map_err(ServiceError::Domain)?;
        
        // 4. Get evaluation completion counts
        let (total_participants, _pre_eval_count, _post_eval_count) = // Ignoring pre/post for rate calc
            self.workshop_participant_repo.get_evaluation_completion_counts(id).await
            .map_err(ServiceError::Domain)?;
            
        // 5. Calculate evaluation completion rate
        let evaluation_completion_rate = if total_participants > 0 {
            // Consider a participant's evaluation complete only if both pre and post are done
            let complete_evals = participants.iter()
                .filter(|p| p.evaluation_complete)
                .count() as f64;
                
            (complete_evals / total_participants as f64) * 100.0 // As percentage
        } else {
            0.0
        };
        
        // 6. Create and return combined response
        Ok(WorkshopWithParticipants {
            workshop: workshop_response,
            participants,
            total_participants,
            evaluation_completion_rate,
        })
    }

    async fn get_workshop_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<WorkshopWithDocumentTimeline> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the workshop
        let workshop = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        let workshop_response = WorkshopResponse::from_workshop(workshop);
        
        // 3. Get all documents for this workshop
        let documents = self.document_service.list_media_documents_by_related_entity(
            auth,
            "workshops",
            id,
            PaginationParams { page: 1, per_page: 100 }, // Corrected instantiation
            None,
        ).await?.items;
        
        // 4. Organize documents by category
        let mut documents_by_category: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        let mut total_document_count = 0;

        for doc in documents {
            // Use field_identifier if available, otherwise use a default category
            let category = doc.field_identifier.clone().unwrap_or_else(|| "General".to_string());
            
            documents_by_category
                .entry(category)
                .or_insert_with(Vec::new)
                .push(doc);
            total_document_count += 1;
        }
        
        // 5. Create and return combined response
        Ok(WorkshopWithDocumentTimeline {
            workshop: workshop_response,
            documents_by_category,
            total_document_count,
        })
    }

    async fn get_workshop_budget_summaries_for_project(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<WorkshopBudgetSummary>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;
        
        // 2. Verify project exists
        match self.project_repo.find_by_id(project_id).await {
            Ok(_) => (), // Project exists
            Err(DomainError::EntityNotFound(_, _)) => {
                return Err(ServiceError::Domain(
                    DomainError::EntityNotFound("Project".to_string(), project_id)
                ));
            },
            Err(e) => return Err(ServiceError::Domain(e)),
        }
        
        // 3. Get budget summaries from repository
        let summaries = self.repo.get_workshop_budget_summaries_for_project(project_id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(summaries)
    }

    async fn get_project_workshop_metrics(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectWorkshopMetrics> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;
        auth.authorize(Permission::ViewProjects)?;
        
        // 2. Get metrics from repository
        let metrics = self.repo.get_project_workshop_metrics(project_id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(metrics)
    }

    async fn get_participant_attendance(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantAttendance> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewWorkshops)?;
        
        // 2. Get attendance metrics from repository
        let attendance = self.workshop_participant_repo.get_participant_attendance(participant_id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(attendance)
    }

    async fn batch_add_participants_to_workshop(
        &self,
        workshop_id: Uuid,
        participant_ids: Vec<Uuid>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<(Uuid, Result<(), ServiceError>)>> {
        // 1. Check permissions
        auth.authorize(Permission::EditWorkshops)?;
        
        // 2. Verify workshop exists
        match self.repo.find_by_id(workshop_id).await {
            Ok(_) => (), // Workshop exists
            Err(e) => return Err(ServiceError::Domain(e)),
        }
        
        // 3. Batch add participants
        let results = self.workshop_participant_repo.batch_add_participants(
            workshop_id, 
            &participant_ids, 
            auth
        ).await
            .map_err(ServiceError::Domain)?;
            
        // 4. Convert domain results to service results
        let service_results = results.into_iter()
            .map(|(id, result)| {
                let service_result = result.map_err(ServiceError::Domain);
                (id, service_result)
            })
            .collect();
            
        Ok(service_results)
    }

    async fn find_participants_with_missing_evaluations(
        &self,
        workshop_id: Uuid,
        eval_type: &str,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ParticipantSummary>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewWorkshops)?;
        auth.authorize(Permission::ViewParticipants)?;
        
        // 2. Verify workshop exists
        match self.repo.find_by_id(workshop_id).await {
            Ok(_) => (), // Workshop exists
            Err(e) => return Err(ServiceError::Domain(e)),
        }
        
        // 3. Validate eval_type
        match eval_type.to_lowercase().as_str() {
            "pre" | "post" => (), // Valid
            _ => return Err(ServiceError::Domain(
                DomainError::Validation(ValidationError::custom(
                    &format!("Invalid evaluation type: {}. Must be 'pre' or 'post'", eval_type)
                ))
            )),
        }
        
        // 4. Find participants with missing evaluations
        let participants = self.workshop_participant_repo.find_participants_with_missing_evaluations(
            workshop_id, 
            eval_type
        ).await
            .map_err(ServiceError::Domain)?;
            
        Ok(participants)
    }
}

// Add this helper method to WorkshopServiceImpl for document uploads
impl WorkshopServiceImpl {
    // Helper method for document uploads (used in create_workshop_with_documents)
    async fn upload_documents_for_entity(
        &self,
        entity_id: Uuid,
        entity_type: &str,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (data, filename, linked_field)
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
                None, // Assume no separate title for batch upload?
                document_type_id,
                entity_id,
                entity_type.to_string(),
                linked_field.clone(),
                sync_priority,
                compression_priority,
                None, // No transaction needed here, handled by document service
            ).await;
            
            results.push(upload_result);
        }
        
        results
    }
}
