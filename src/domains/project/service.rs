use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::permission::Permission;
use crate::domains::project::repository::ProjectRepository;
use crate::domains::project::types::{ // Added new types
    NewProject, Project, ProjectResponse, UpdateProject, ProjectInclude, 
    ProjectStatistics, ProjectStatusBreakdown, ProjectMetadataCounts, ProjectDocumentReference,
    ProjectWithDocumentTimeline
};
use crate::domains::strategic_goal::repository::StrategicGoalRepository;
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError, DbError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use std::str::FromStr;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// Import necessary types related to documents and sync/compression
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse; // Ensure this is imported
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::domains::core::delete_service::PendingDeletionManager;

// ADDED: Import additional repositories for enrichment
use crate::domains::user::repository::UserRepository;
use crate::domains::activity::repository::ActivityRepository;
use crate::domains::workshop::repository::WorkshopRepository;

/// Trait defining project service operations
#[async_trait]
pub trait ProjectService: DeleteService<Project> + Send + Sync {
    async fn create_project(
        &self,
        new_project: NewProject,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse>;

    // ADDED: Create with documents method
    async fn create_project_with_documents(
        &self,
        new_project: NewProject,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ProjectResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    async fn get_project_by_id(
        &self,
        id: Uuid,
        include: Option<&[ProjectInclude]>, // Include is used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse>;

    async fn list_projects(
        &self,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>, // Include is used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;

    async fn update_project(
        &self,
        id: Uuid,
        update_data: UpdateProject,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse>;

    async fn delete_project(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;

    // ADDED: Document integration methods
    async fn upload_document_for_project(
        &self,
        project_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_project(
        &self,
        project_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;

    // --- New Methods ---
    
    /// Get comprehensive project statistics for dashboard
    async fn get_project_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectStatistics>;
    
    /// Get project status breakdown
    async fn get_project_status_breakdown(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ProjectStatusBreakdown>>;
    
    /// Get project metadata counts
    async fn get_project_metadata_counts(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectMetadataCounts>;
    
    /// Find projects by status
    async fn find_projects_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;
    
    /// Find projects by responsible team
    async fn find_projects_by_responsible_team(
        &self,
        team: &str,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;
    
    /// Get project with document timeline
    async fn get_project_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectWithDocumentTimeline>;
    
    /// Get project document references
    async fn get_project_document_references(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ProjectDocumentReference>>;
    
    /// Search projects by name, objective, or outcome
    async fn search_projects(
        &self,
        query: &str,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;

    /// Find projects within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_projects_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;

    /// ADDED: Gets a list of project IDs based on complex filter criteria.
    /// This is ideal for UI bulk operations (selection, export, etc.).
    /// Follows the same pattern as StrategicGoalService::get_filtered_goal_ids.
    async fn get_filtered_project_ids(
        &self,
        filter: crate::domains::project::types::ProjectFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;

    // --- ADDED: Advanced Dashboard Aggregations ---
    
    /// Get team workload distribution - count of projects by responsible team
    /// Perfect for dashboard widgets showing team capacity and workload
    async fn get_team_workload_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;
    
    /// Get projects by strategic goal distribution - count of projects grouped by parent strategic goal
    /// Ideal for showing strategic goal progress and project allocation
    async fn get_projects_by_strategic_goal_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;
    
    /// Find stale projects that haven't been updated since a specific date
    /// Useful for project management dashboards to identify projects needing attention
    async fn find_stale_projects(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>>;
    
    /// Get document coverage analysis - projects with/without documents, document counts
    /// Perfect for compliance and documentation tracking dashboards
    async fn get_document_coverage_analysis(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<crate::domains::project::types::DocumentCoverageAnalysis>;
    
    /// Get project activity timeline - projects with recent activity vs inactive
    /// Useful for project health monitoring dashboards
    async fn get_project_activity_timeline(
        &self,
        days_active: u32,
        auth: &AuthContext,
    ) -> ServiceResult<crate::domains::project::types::ProjectActivityTimeline>;
}

/// Implementation of the project service
#[derive(Clone)]
pub struct ProjectServiceImpl {
    // ADDED: Pool for transactions
    pool: SqlitePool,
    repo: Arc<dyn ProjectRepository + Send + Sync>,
    strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Project>>,
    document_service: Arc<dyn DocumentService>,
    // ADDED: Additional repositories for enrichment
    user_repo: Arc<dyn UserRepository + Send + Sync>,
    activity_repo: Arc<dyn ActivityRepository + Send + Sync>,
    workshop_repo: Arc<dyn WorkshopRepository + Send + Sync>,
}

impl ProjectServiceImpl {
    pub fn new(
        // ADDED: pool parameter
        pool: SqlitePool,
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>, // Still needed for BaseDeleteService
        document_service: Arc<dyn DocumentService>,
        deletion_manager: Arc<PendingDeletionManager>,
        // ADDED: Additional repositories for enrichment
        user_repo: Arc<dyn UserRepository + Send + Sync>,
        activity_repo: Arc<dyn ActivityRepository + Send + Sync>,
        workshop_repo: Arc<dyn WorkshopRepository + Send + Sync>,
    ) -> Self {
        // --- Adapter setup remains the same ---
        struct RepoAdapter(Arc<dyn ProjectRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Project> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Project> {
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
                 "projects"
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

        let adapted_repo: Arc<dyn DeleteServiceRepository<Project>> =
            Arc::new(RepoAdapter(project_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(), // Clone pool for delete service
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo), // Pass media repo for delete service's file handling
            deletion_manager,
        ));

        Self {
            // ADDED: store pool
            pool,
            repo: project_repo,
            strategic_goal_repo,
            delete_service,
            document_service,
            // ADDED: Additional repositories for enrichment
            user_repo,
            activity_repo,
            workshop_repo,
        }
    }

    /// Helper to enrich ProjectResponse with included data
    /// PRESERVED: Document enrichment logic is kept
    async fn enrich_response(
        &self,
        mut response: ProjectResponse,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext, // Auth context is needed for listing documents
    ) -> ServiceResult<ProjectResponse> {
        if let Some(includes) = include {
            // --- PRESERVED Document Enrichment ---
            let include_docs = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::Documents);

            if include_docs {
                // Use default pagination or define specific params if needed
                let doc_params = PaginationParams::default();
                // Fetch documents using the document service
                let docs_result = self.document_service
                    .list_media_documents_by_related_entity(
                        auth,         // Pass auth context
                        "projects",   // Correct entity type
                        response.id,  // Project ID
                        doc_params,
                        None // No nested includes needed for the document list itself here
                    ).await?;
                // Attach the fetched documents to the response
                response.documents = Some(docs_result.items);
            }
             // --- END PRESERVED Document Enrichment ---

            // ADDED: Username Enrichment - resolve user IDs to usernames
            let include_usernames = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::CreatedBy);
            if include_usernames {
                // Resolve created_by_user_id to username
                if let Some(created_by_id) = response.created_by_user_id {
                    if let Ok(user) = self.user_repo.find_by_id(created_by_id).await {
                        response.created_by_username = Some(user.name.clone());
                    }
                }
                // Resolve updated_by_user_id to username
                if let Some(updated_by_id) = response.updated_by_user_id {
                    if let Ok(user) = self.user_repo.find_by_id(updated_by_id).await {
                        response.updated_by_username = Some(user.name.clone());
                    }
                }
            }

            // ADDED: Count-based Enrichment - pre-compute related entity counts
            let include_counts = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::Counts);
            if include_counts {
                // Count activities for this project
                match self.activity_repo.find_by_project_id(response.id, PaginationParams::default()).await {
                    Ok(activities_result) => {
                        response.activity_count = Some(activities_result.total as i64);
                    }
                    Err(_) => {
                        // If there's an error, set count to 0 rather than failing enrichment
                        response.activity_count = Some(0);
                    }
                }

                // Count workshops for this project
                match self.workshop_repo.find_by_project_id(response.id, PaginationParams::default()).await {
                    Ok(workshops_result) => {
                        response.workshop_count = Some(workshops_result.total as i64);
                    }
                    Err(_) => {
                        // If there's an error, set count to 0 rather than failing enrichment
                        response.workshop_count = Some(0);
                    }
                }
            }

            // ADDED: Strategic Goal Enrichment
            let include_strategic_goal = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::StrategicGoal);
            if include_strategic_goal && response.strategic_goal.is_none() {
                                 if let Some(sg_id) = response.strategic_goal_id {
                     if let Ok(strategic_goal) = self.strategic_goal_repo.find_by_id(sg_id).await {
                         response.strategic_goal = Some(crate::domains::project::types::StrategicGoalSummary {
                             id: strategic_goal.id,
                             objective_code: strategic_goal.objective_code,
                             outcome: strategic_goal.outcome,
                         });
                     }
                 }
            }

            // TODO: Add status enrichment when status repository is available
            // let include_status = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::Status);
            // if include_status && response.status.is_none() { ... fetch status ... }
        }
        Ok(response)
    }

    // Helper to validate strategic goal existence if ID is provided - Remains the same
    async fn validate_strategic_goal_exists(&self, sg_id: Option<Uuid>) -> DomainResult<()> {
        if let Some(id) = sg_id {
            match self.strategic_goal_repo.find_by_id(id).await {
                Ok(_) => Ok(()),
                Err(DomainError::EntityNotFound(_, _)) => {
                    let error_message = format!("Strategic Goal with ID {} does not exist", id);
                    Err(DomainError::Validation(
                        ValidationError::relationship(&error_message)
                    ))
                },
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }

    // ENHANCED: Helper method with auto-detection (copied from StrategicGoalService and enhanced)
    /// Helper method to upload documents for any entity and handle errors individually
    /// UPDATED: Now uses smart auto-detection instead of provided document_type_id
    async fn upload_documents_for_entity(
        &self,
        entity_id: Uuid,
        entity_type: &str,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid, // UPDATED: Still in signature for compatibility, but ignored
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> Vec<Result<MediaDocumentResponse, ServiceError>> {
        let mut results = Vec::new();

        for (file_data, filename, linked_field) in documents {
            // ENHANCED: Auto-detect document type from file extension
            let extension = filename.split('.').last().unwrap_or("").to_lowercase();
            
            let document_type_name = match crate::domains::document::initialization::get_document_type_for_extension(&extension) {
                Some(type_name) => type_name,
                None => {
                    // Store error result and continue with other files
                    results.push(Err(ServiceError::Domain(
                        DomainError::Validation(ValidationError::custom(&format!(
                            "Unsupported file type: .{}", extension
                        )))
                    )));
                    continue;
                }
            };
            
            // Get document type ID by name
            let auto_detected_document_type = match self.document_service.get_document_type_by_name(document_type_name).await {
                Ok(Some(doc_type)) => doc_type,
                Ok(None) => {
                    results.push(Err(ServiceError::Domain(
                        DomainError::Validation(ValidationError::custom(&format!(
                            "Document type '{}' not found in database", document_type_name
                        )))
                    )));
                    continue;
                }
                Err(e) => {
                    results.push(Err(e));
                    continue;
                }
            };

            // Upload with auto-detected document type
            let upload_result = self.document_service.upload_document(
                auth,
                file_data,
                filename,
                None, // No title, will use filename as default
                auto_detected_document_type.id, // Use auto-detected type instead of provided one
                entity_id,
                entity_type.to_string(),
                linked_field,
                sync_priority,
                compression_priority,
                None, // No temp ID needed since entity exists
            ).await;

            // Store the result (success or error) without failing the whole operation
            results.push(upload_result);
        }

        results
    }
}

// Implement DeleteService<Project> by delegating - Remains the same
#[async_trait]
impl DeleteService<Project> for ProjectServiceImpl {
    fn repository(&self) -> &dyn FindById<Project> { self.delete_service.repository() }
    fn tombstone_repository(&self) -> &dyn TombstoneRepository { self.delete_service.tombstone_repository() }
    fn change_log_repository(&self) -> &dyn ChangeLogRepository { self.delete_service.change_log_repository() }
    fn dependency_checker(&self) -> &dyn DependencyChecker { self.delete_service.dependency_checker() }
    async fn delete( &self, id: Uuid, auth: &AuthContext, options: DeleteOptions ) -> DomainResult<DeleteResult> { self.delete_service.delete(id, auth, options).await }
    async fn batch_delete( &self, ids: &[Uuid], auth: &AuthContext, options: DeleteOptions ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { self.delete_service.batch_delete(ids, auth, options).await }
    async fn delete_with_dependencies( &self, id: Uuid, auth: &AuthContext ) -> DomainResult<DeleteResult> { self.delete_service.delete_with_dependencies(id, auth).await }
    async fn get_failed_delete_details( &self, batch_result: &crate::domains::core::delete_service::BatchDeleteResult, auth: &AuthContext ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Project>>> { self.delete_service.get_failed_delete_details(batch_result, auth).await }
}

#[async_trait]
impl ProjectService for ProjectServiceImpl {
    // create_project remains mostly the same (core logic unchanged)
    async fn create_project(
        &self,
        new_project: NewProject,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse> {
        if !auth.has_permission(Permission::CreateProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create projects".to_string(),
            ));
        }

        new_project.validate()?;
        self.validate_strategic_goal_exists(new_project.strategic_goal_id).await?;

        // Assume repo.create doesn't use a transaction internally for this basic method
        let created_project = self.repo.create(&new_project, auth).await?;
        let response = ProjectResponse::from_project(created_project);
        // No enrichment needed on *basic* create
        Ok(response)
    }

    // ADDED: Implementation for create_project_with_documents
    async fn create_project_with_documents(
        &self,
        new_project: NewProject,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ProjectResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create projects".to_string(),
            ));
        }
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_project.validate()?;
        self.validate_strategic_goal_exists(new_project.strategic_goal_id).await?;

        // 3. Begin transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

        // 4. Create the project first (within transaction)
        // ASSUMPTION: self.repo has create_with_tx method
        let created_project = match self.repo.create_with_tx(&new_project, auth, &mut tx).await {
            Ok(project) => project,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };

        // 5. Commit transaction to ensure project is created
        tx.commit().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

        // 6. Now upload documents (outside transaction, linking to created_project.id)
        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_project.id,
                "projects", // Use correct entity type string
                documents,
                document_type_id,
                SyncPriority::Normal, // Default or could be passed in
                None, // Use default compression priority
                auth,
            ).await
        } else {
            Vec::new()
        };

        // 7. Convert to Response DTO and return with document results
        let response = ProjectResponse::from_project(created_project);
        Ok((response, document_results))
    }


    // get_project_by_id - PRESERVED call to self.enrich_response
    async fn get_project_by_id(
        &self,
        id: Uuid,
        include: Option<&[ProjectInclude]>, // Include parameter IS used
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse> {
        if !auth.has_permission(Permission::ViewProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view projects".to_string(),
            ));
        }

        let project = self.repo.find_by_id(id).await?;
        let response = ProjectResponse::from_project(project);

        // PRESERVED: Pass auth context and include options to enrich_response
        self.enrich_response(response, include, auth).await
    }

    // list_projects - PRESERVED enrichment loop
    async fn list_projects(
        &self,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>, // Include parameter IS used
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        if !auth.has_permission(Permission::ViewProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list projects".to_string(),
            ));
        }

        let paginated_result = self.repo.find_all(params).await?;

        // PRESERVED: Enrich items before returning
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = ProjectResponse::from_project(item);
            // Pass auth context and include options to enrich_response for each item
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params, // Pass original params back
        ))
    }

    // update_project remains the same (core logic unchanged)
    async fn update_project(
        &self,
        id: Uuid,
        mut update_data: UpdateProject,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse> {
        if !auth.has_permission(Permission::EditProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit projects".to_string(),
            ));
        }

        update_data.updated_by_user_id = auth.user_id;
        update_data.validate()?;

        if let Some(opt_sg_id) = update_data.strategic_goal_id {
             self.validate_strategic_goal_exists(opt_sg_id).await?;
        }

        let updated_project = self.repo.update(id, &update_data, auth).await?;
        let response = ProjectResponse::from_project(updated_project);
         // No enrichment needed on update response by default
        Ok(response)
    }

    // delete_project remains the same (core logic unchanged)
    async fn delete_project(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteProjects
        };

        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} projects",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }

        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete,
            force: false,
        };

        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }

    // --- ADDED: Document integration methods ---

    // Copied from StrategicGoalService, adapted for Project
    async fn upload_document_for_project(
        &self,
        project_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid, // UPDATED: Still in signature for FFI compatibility, but will be ignored in favor of auto-detection
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify project exists
        let _project = self.repo.find_by_id(project_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !Project::is_document_linkable_field(field) {
                let valid_fields: Vec<String> = Project::document_linkable_fields()
                    .into_iter()
                    .collect();
                    
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for projects. Valid fields: {}",
                    field, valid_fields.join(", ")
                )).into()));
            }
        }

        // 4. ENHANCED: Auto-detect document type from file extension (same as strategic_goal)
        let extension = original_filename.split('.').last().unwrap_or("").to_lowercase();
        
        let document_type_name = match crate::domains::document::initialization::get_document_type_for_extension(&extension) {
            Some(type_name) => type_name,
            None => {
                return Err(ServiceError::Domain(
                    DomainError::Validation(ValidationError::custom(&format!(
                        "Unsupported file type: .{}", extension
                    )))
                ));
            }
        };
        
        // Get document type ID by name
        let auto_detected_document_type = match self.document_service.get_document_type_by_name(document_type_name).await {
            Ok(Some(doc_type)) => doc_type,
            Ok(None) => {
                return Err(ServiceError::Domain(
                    DomainError::Validation(ValidationError::custom(&format!(
                        "Document type '{}' not found in database", document_type_name
                    )))
                ));
            }
            Err(e) => return Err(e),
        };

        // 5. Delegate to document service with auto-detected type
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            auto_detected_document_type.id, // Use auto-detected type instead of provided one
            project_id,
            "projects".to_string(), // Correct entity type
            linked_field.clone(), // Pass the validated field name
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        Ok(document)
    }

    async fn bulk_upload_documents_for_project(
        &self,
        project_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid, // UPDATED: Still in signature for FFI compatibility, but will be ignored in favor of auto-detection
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify project exists
        let _project = self.repo.find_by_id(project_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. ENHANCED: Process each file with auto-detected document type
        let mut results = Vec::new();
        
        for (file_data, filename) in files {
            // Extract file extension
            let extension = filename.split('.').last().unwrap_or("").to_lowercase();
            
            // Auto-detect document type using existing initialization logic
            let document_type_name = match crate::domains::document::initialization::get_document_type_for_extension(&extension) {
                Some(type_name) => type_name,
                None => {
                    // Handle unsupported extension - add as error result but continue with others
                    log::warn!("Skipping file '{}' with unsupported extension: .{}", filename, extension);
                    continue; // Skip this file, continue with others
                }
            };
            
            // Get document type ID by name
            let document_type = match self.document_service.get_document_type_by_name(document_type_name).await {
                Ok(Some(doc_type)) => doc_type,
                Ok(None) => {
                    log::warn!("Skipping file '{}': Document type '{}' not found in database", filename, document_type_name);
                    continue;
                }
                Err(e) => {
                    log::warn!("Skipping file '{}': Error fetching document type: {}", filename, e);
                    continue;
                }
            };

            // Upload individual document with auto-detected type
            let upload_result = self.document_service.upload_document(
                auth,
                file_data,
                filename.clone(),
                title.clone(),
                document_type.id,
                project_id,
                "projects".to_string(),
                None, // No specific field linking for bulk uploads
                sync_priority,
                compression_priority,
                None,
            ).await;
            
            match upload_result {
                Ok(document) => results.push(document),
                Err(e) => {
                    log::warn!("Failed to upload file '{}': {}", filename, e);
                    // Continue with other files rather than failing the entire batch
                }
            }
        }

        // Return successful uploads (allowing partial success)
        Ok(results)
    }

    // --- New Method Implementations ---

    async fn get_project_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectStatistics> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Get statistics from repository
        let statistics = self.repo.get_project_statistics().await
            .map_err(ServiceError::Domain)?;
        
        Ok(statistics)
    }
    
    async fn get_project_status_breakdown(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ProjectStatusBreakdown>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Get breakdown from repository
        let breakdown = self.repo.get_project_status_breakdown().await
            .map_err(ServiceError::Domain)?;
        
        Ok(breakdown)
    }
    
    async fn get_project_metadata_counts(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectMetadataCounts> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Get counts from repository
        let counts = self.repo.get_project_metadata_counts().await
            .map_err(ServiceError::Domain)?;
        
        Ok(counts)
    }
    
    async fn find_projects_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Find projects by status
        let paginated_result = self.repo.find_by_status(status_id, params).await
            .map_err(ServiceError::Domain)?;

        // 3. Convert and enrich each project
        let mut enriched_items = Vec::new();
        for project in paginated_result.items {
            let response = ProjectResponse::from_project(project);
            let enriched = self.enrich_response(response, include, auth).await?; // Pass auth
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    async fn find_projects_by_responsible_team(
        &self,
        team: &str,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Find projects by team
        let paginated_result = self.repo.find_by_responsible_team(team, params).await
            .map_err(ServiceError::Domain)?;

        // 3. Convert and enrich each project
        let mut enriched_items = Vec::new();
        for project in paginated_result.items {
            let response = ProjectResponse::from_project(project);
            let enriched = self.enrich_response(response, include, auth).await?; // Pass auth
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    async fn get_project_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectWithDocumentTimeline> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the project
        let project = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        let project_response = ProjectResponse::from_project(project);
        
        // 3. Get all documents for this project
        let documents = self.document_service.list_media_documents_by_related_entity(
            auth,
            "projects",
            id,
            PaginationParams { page: 1, per_page: 100 }, // Use correct field name 'per_page'
            None,
        ).await?.items;
        
        // 4. Organize documents by type/category
        let mut documents_by_type: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        let mut total_document_count = 0;
        
        for doc in documents {
            // Use field_identifier if available, otherwise use a default category
            let document_type: String = match &doc.field_identifier {
                Some(field) => field.clone(), // Clone here for owned String
                None => "General".to_string(), // Also creates owned String
            };
            
            documents_by_type
                .entry(document_type) // Using owned String is fine
                .or_insert_with(Vec::new)
                .push(doc);
            total_document_count += 1;
        }
        
        // 5. Create and return combined response
        Ok(ProjectWithDocumentTimeline {
            project: project_response,
            documents_by_type,
            total_document_count: total_document_count as u64,
        })
    }
    
    async fn get_project_document_references(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ProjectDocumentReference>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Verify project exists
        let _project = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        // 3. Get document references from repository
        let references = self.repo.get_project_document_references(id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(references)
    }
    
    async fn search_projects(
        &self,
        query: &str,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Validate query length
        if query.trim().len() < 2 {
            return Err(ServiceError::Domain(
                DomainError::Validation(ValidationError::custom("Search query must be at least 2 characters"))
            ));
        }

        // 3. Search projects
        let paginated_result = self.repo.search_projects(query, params).await
            .map_err(ServiceError::Domain)?;

        // 4. Convert and enrich each project
        let mut enriched_items = Vec::new();
        for project in paginated_result.items {
            let response = ProjectResponse::from_project(project);
            let enriched = self.enrich_response(response, include, auth).await?; // Pass auth
            enriched_items.push(enriched);
        }

        // 5. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    /// Find projects within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_projects_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Parse RFC3339 datetime strings
        let start_datetime = DateTime::parse_from_rfc3339(start_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(
                ValidationError::format("start_date", &format!("Invalid RFC3339 date format: {}", e))
            )))?
            .with_timezone(&Utc);

        let end_datetime = DateTime::parse_from_rfc3339(end_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(
                ValidationError::format("end_date", &format!("Invalid RFC3339 date format: {}", e))
            )))?
            .with_timezone(&Utc);

        // 3. Validate date range
        if start_datetime > end_datetime {
            return Err(ServiceError::Domain(DomainError::Validation(
                ValidationError::custom("Start date must be before end date")
            )));
        }

        // 4. Find projects by date range
        let paginated_result = self.repo.find_by_date_range(start_datetime, end_datetime, params).await
            .map_err(ServiceError::Domain)?;

        // 5. Convert and enrich each project
        let mut enriched_items = Vec::new();
        for project in paginated_result.items {
            let response = ProjectResponse::from_project(project);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 6. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    /// ADDED: Gets a list of project IDs based on complex filter criteria.
    /// Follows the same pattern as StrategicGoalService::get_filtered_goal_ids.
    async fn get_filtered_project_ids(
        &self,
        filter: crate::domains::project::types::ProjectFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Use repository filter method to get matching IDs
        let ids = self.repo
            .find_ids_by_filter(filter)
            .await
            .map_err(ServiceError::Domain)?;

        Ok(ids)
    }

    // --- ADDED: Advanced Dashboard Aggregations Implementation ---
    
    /// Get team workload distribution - count of projects by responsible team
    /// Perfect for dashboard widgets showing team capacity and workload
    async fn get_team_workload_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Get team counts from repository
        let team_counts = self.repo.count_by_responsible_team().await
            .map_err(ServiceError::Domain)?;
        
        // 3. Convert to HashMap with proper team names
        let mut distribution = HashMap::new();
        for (team_name, count) in team_counts {
            let display_name = team_name.unwrap_or_else(|| "No Team Assigned".to_string());
            distribution.insert(display_name, count);
        }
        
        Ok(distribution)
    }
    
    /// Get projects by strategic goal distribution - count of projects grouped by parent strategic goal
    /// Ideal for showing strategic goal progress and project allocation
    async fn get_projects_by_strategic_goal_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Get strategic goal counts from repository
        let sg_counts = self.repo.count_by_strategic_goal().await
            .map_err(ServiceError::Domain)?;
        
        // 3. Convert to HashMap with strategic goal names
        let mut distribution = HashMap::new();
        for (sg_id, count) in sg_counts {
            let display_name = match sg_id {
                Some(id) => {
                    // Try to get the strategic goal name
                    match self.strategic_goal_repo.find_by_id(id).await {
                        Ok(goal) => goal.objective_code.clone(),
                        Err(_) => format!("Strategic Goal {}", id),
                    }
                }
                None => "No Strategic Goal".to_string(),
            };
            distribution.insert(display_name, count);
        }
        
        Ok(distribution)
    }
    
    /// Find stale projects that haven't been updated since a specific date
    /// Useful for project management dashboards to identify projects needing attention
    async fn find_stale_projects(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Calculate cutoff date
        let cutoff_date = Utc::now() - chrono::Duration::days(days_stale as i64);
        
        // 3. Find stale projects using date range method
        let start_date = DateTime::from_timestamp(0, 0).unwrap_or_else(|| Utc::now());
        let paginated_result = self.repo.find_by_date_range(start_date, cutoff_date, params).await
            .map_err(ServiceError::Domain)?;
            
        // 4. Convert and enrich each project
        let mut enriched_items = Vec::new();
        for project in paginated_result.items {
            let response = ProjectResponse::from_project(project);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 5. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    /// Get document coverage analysis - projects with/without documents, document counts
    /// Perfect for compliance and documentation tracking dashboards
    async fn get_document_coverage_analysis(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<crate::domains::project::types::DocumentCoverageAnalysis> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get total project count
        let total_projects = self.repo.find_all(PaginationParams { page: 1, per_page: 1 }).await
            .map_err(ServiceError::Domain)?
            .total as i64;

        // 3. Get document counts by entity type
        let mut projects_with_documents = 0i64;
        let mut total_documents = 0i64;
        let mut document_count_by_type = HashMap::new();
        
        // This is a simplified implementation - in a real system, you'd want to optimize this
        // with a dedicated repository method that joins projects and documents
        let all_projects = self.repo.find_all(PaginationParams { page: 1, per_page: 1000 }).await
            .map_err(ServiceError::Domain)?;
            
        for project in all_projects.items {
            // Check if project has documents
            let docs_result = self.document_service.list_media_documents_by_related_entity(
                auth,
                "projects",
                project.id,
                PaginationParams { page: 1, per_page: 100 },
                None,
            ).await;
            
            if let Ok(docs) = docs_result {
                if !docs.items.is_empty() {
                    projects_with_documents += 1;
                    total_documents += docs.items.len() as i64;
                    
                    // Count documents by type
                    for doc in docs.items {
                        if let Some(doc_type) = &doc.type_name {
                            *document_count_by_type.entry(doc_type.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // 4. Calculate metrics
        let projects_without_documents = total_projects - projects_with_documents;
        let coverage_percentage = if total_projects > 0 {
            (projects_with_documents as f64 / total_projects as f64) * 100.0
        } else {
            0.0
        };
        let average_documents_per_project = if total_projects > 0 {
            total_documents as f64 / total_projects as f64
        } else {
            0.0
        };

        // 5. Return analysis
        Ok(crate::domains::project::types::DocumentCoverageAnalysis {
            projects_with_documents,
            projects_without_documents,
            total_projects,
            average_documents_per_project,
            coverage_percentage,
            document_count_by_type,
        })
    }
    
    /// Get project activity timeline - projects with recent activity vs inactive
    /// Useful for project health monitoring dashboards
    async fn get_project_activity_timeline(
        &self,
        days_active: u32,
        auth: &AuthContext,
    ) -> ServiceResult<crate::domains::project::types::ProjectActivityTimeline> {
        // 1. Check permissions
        auth.authorize(Permission::ViewProjects)?;

        // 2. Calculate date thresholds
        let active_cutoff = Utc::now() - chrono::Duration::days(days_active as i64);
        let stale_cutoff = Utc::now() - chrono::Duration::days((days_active * 2) as i64);
        
        // 3. Get total project count
        let total_projects = self.repo.find_all(PaginationParams { page: 1, per_page: 1 }).await
            .map_err(ServiceError::Domain)?
            .total as i64;

        // 4. Get recently updated projects (active)
        let recently_updated = self.repo.find_by_date_range(active_cutoff, Utc::now(), PaginationParams { page: 1, per_page: 1 }).await
            .map_err(ServiceError::Domain)?
            .total as i64;

        // 5. Get stale projects (very old)
        let stale_projects = self.repo.find_by_date_range(
            DateTime::from_timestamp(0, 0).unwrap_or_else(|| Utc::now()), 
            stale_cutoff, 
            PaginationParams { page: 1, per_page: 1 }
        ).await
            .map_err(ServiceError::Domain)?
            .total as i64;

        // 6. Calculate metrics
        let active_projects = recently_updated;
        let inactive_projects = total_projects - active_projects;
        let activity_percentage = if total_projects > 0 {
            (active_projects as f64 / total_projects as f64) * 100.0
        } else {
            0.0
        };

        // 7. Return timeline analysis
        Ok(crate::domains::project::types::ProjectActivityTimeline {
            active_projects,
            inactive_projects,
            total_projects,
            activity_percentage,
            stale_projects,
            recently_updated_projects: recently_updated,
        })
    }
}