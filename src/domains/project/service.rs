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

            // TODO: Add enrichment logic for other includes like StrategicGoal, Status, CreatedBy, Counts
            let _include_strategic_goal = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::StrategicGoal);
            // if _include_strategic_goal && response.strategic_goal.is_none() { ... fetch strategic goal ... }
            // if include_status && response.status.is_none() { ... fetch status ... }
            // ... etc ...
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

    // ADDED: Helper method copied from StrategicGoalService
    /// Helper method to upload documents for any entity and handle errors individually
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
            // Use the injected document_service
            let upload_result = self.document_service.upload_document(
                auth,
                file_data,
                filename,
                None, // No title, will use filename as default
                document_type_id,
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
        document_type_id: Uuid,
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

        // 4. Delegate to document service, passing linked_field
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            project_id,
            "projects".to_string(), // Correct entity type
            linked_field.clone(), // Pass the validated field name
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        // 5. --- NEW: Update entity reference if it was a document-only field ---
        // REMOVED INCORRECT BLOCK: The document service already handles the linked_field correctly
        // by storing it in media_documents.field_identifier. No direct update to the
        // project table is needed or possible according to the schema.
        // if let Some(field_name) = linked_field {
        //     if let Some(metadata) = Project::get_field_metadata(&field_name) {
        //         if metadata.is_document_reference_only {
        //             self.repo.set_document_reference(
        //                 project_id,
        //                 &field_name, // e.g., "proposal_document"
        //                 document.id, // The ID of the newly created MediaDocument
        //                 auth
        //             ).await?;
        //         }
        //     }
        // }

        Ok(document)
    }

    async fn bulk_upload_documents_for_project(
        &self,
        project_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        auth.authorize(Permission::UploadDocuments)?;

        let _project = self.repo.find_by_id(project_id).await.map_err(ServiceError::Domain)?;

        let mut results = Vec::new();
        for (file_data, original_filename) in files {
            let result = self.document_service.upload_document(
                auth,
                file_data,
                original_filename,
                title.clone(),
                document_type_id,
                project_id,
                "projects".to_string(),
                None, // No specific linked field for bulk uploads
                sync_priority,
                compression_priority,
                None, // No transaction needed here, handled by document service
            ).await?;
            results.push(result);
        }
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
}