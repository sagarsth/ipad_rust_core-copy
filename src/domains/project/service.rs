use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::permission::Permission;
use crate::domains::project::repository::ProjectRepository;
use crate::domains::project::types::{NewProject, Project, ProjectResponse, UpdateProject, ProjectInclude};
use crate::domains::strategic_goal::repository::StrategicGoalRepository; // Needed for validation
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::service::DocumentService;

/// Trait defining project service operations
#[async_trait]
pub trait ProjectService: DeleteService<Project> + Send + Sync {
    async fn create_project(
        &self,
        new_project: NewProject,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse>;

    async fn get_project_by_id(
        &self,
        id: Uuid,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse>;

    async fn list_projects(
        &self,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
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
}

/// Implementation of the project service
#[derive(Clone)] 
pub struct ProjectServiceImpl {
    repo: Arc<dyn ProjectRepository + Send + Sync>,
    // Keep a reference to StrategicGoalRepository if needed for validation
    strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Project>>,
    document_service: Arc<dyn DocumentService>,
}

impl ProjectServiceImpl {
    pub fn new(
        pool: SqlitePool,
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // Local adapter struct
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
        
        // Blanket impl covers DeleteServiceRepository<Project>

        let adapted_repo: Arc<dyn DeleteServiceRepository<Project>> = 
            Arc::new(RepoAdapter(project_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool,
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo),
        ));
        
        Self {
            repo: project_repo,
            strategic_goal_repo, // Store the strategic goal repo
            delete_service,
            document_service,
        }
    }

    /// Helper to enrich ProjectResponse with included data
    async fn enrich_response(
        &self, 
        mut response: ProjectResponse, 
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext, 
    ) -> ServiceResult<ProjectResponse> {
        if let Some(includes) = include {
            let include_docs = includes.contains(&ProjectInclude::All) || includes.contains(&ProjectInclude::Documents);
            
            if include_docs {
                let doc_params = PaginationParams::default();
                let docs_result = self.document_service
                    .list_media_documents_by_related_entity(
                        auth,
                        "projects",
                        response.id,
                        doc_params,
                        None
                    ).await?;
                response.documents = Some(docs_result.items);
            }

            // TODO: Add enrichment logic for other includes like StrategicGoal, Status, CreatedBy, Counts
            // if include_strategic_goal && response.strategic_goal.is_none() { ... fetch strategic goal ... }
            // if include_status && response.status.is_none() { ... fetch status ... }
            // ... etc ...
        }
        Ok(response)
    }

    // Helper to validate strategic goal existence if ID is provided
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
                Err(e) => Err(e), // Propagate other DB errors
            }
        } else {
            Ok(())
        }
    }
}

// Implement DeleteService<Project> by delegating
#[async_trait]
impl DeleteService<Project> for ProjectServiceImpl {
    fn repository(&self) -> &dyn FindById<Project> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Project>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl ProjectService for ProjectServiceImpl {
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

        let created_project = self.repo.create(&new_project, auth).await?;
        let response = ProjectResponse::from_project(created_project);
        // No enrichment needed on create
        Ok(response)
    }

    async fn get_project_by_id(
        &self,
        id: Uuid,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectResponse> {
        if !auth.has_permission(Permission::ViewProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view projects".to_string(),
            ));
        }

        let project = self.repo.find_by_id(id).await?;
        let response = ProjectResponse::from_project(project);

        // Pass auth context to enrich_response
        self.enrich_response(response, include, auth).await
    }

    async fn list_projects(
        &self,
        params: PaginationParams,
        include: Option<&[ProjectInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectResponse>> {
        if !auth.has_permission(Permission::ViewProjects) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list projects".to_string(),
            ));
        }

        let paginated_result = self.repo.find_all(params).await?;

        // Enrich items before returning
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = ProjectResponse::from_project(item);
            // Pass auth context to enrich_response
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

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
        
        // Validate strategic goal only if it's being explicitly set (Some(Some(id)) or Some(None))
        if let Some(opt_sg_id) = update_data.strategic_goal_id {
             self.validate_strategic_goal_exists(opt_sg_id).await?;
        }

        let updated_project = self.repo.update(id, &update_data, auth).await?;
        let response = ProjectResponse::from_project(updated_project);
        // No enrichment on update by default
        Ok(response)
    }
    
    async fn delete_project(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteProjects // Use specific delete permission
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
        
        // Delegate to the core delete method provided via DeleteService<Project>
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }
}
