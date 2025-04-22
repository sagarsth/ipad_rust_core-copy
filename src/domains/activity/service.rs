use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::permission::Permission;
use crate::domains::activity::repository::ActivityRepository;
use crate::domains::activity::types::{NewActivity, Activity, ActivityResponse, UpdateActivity};
use crate::domains::project::repository::ProjectRepository; // Needed for validation
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Trait defining activity service operations
#[async_trait]
pub trait ActivityService: DeleteService<Activity> + Send + Sync {
    async fn create_activity(
        &self,
        new_activity: NewActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;

    async fn get_activity_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;

    async fn list_activities_for_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    async fn update_activity(
        &self,
        id: Uuid,
        update_data: UpdateActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;
    
    async fn delete_activity(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
}

/// Implementation of the activity service
#[derive(Clone)] 
pub struct ActivityServiceImpl {
    repo: Arc<dyn ActivityRepository + Send + Sync>,
    project_repo: Arc<dyn ProjectRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Activity>>,
}

impl ActivityServiceImpl {
    pub fn new(
        pool: SqlitePool,
        activity_repo: Arc<dyn ActivityRepository + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
    ) -> Self {
        // Local adapter struct
        struct RepoAdapter(Arc<dyn ActivityRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Activity> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Activity> {
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
        
        // Blanket impl covers DeleteServiceRepository<Activity>

        let adapted_repo: Arc<dyn DeleteServiceRepository<Activity>> = 
            Arc::new(RepoAdapter(activity_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool,
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
        ));
        
        Self {
            repo: activity_repo, // Keep original repo
            project_repo, // Store project repo for validation
            delete_service,
        }
    }

    // Updated validation helper name for clarity
    async fn validate_project_exists_if_provided(&self, project_id: Option<Uuid>) -> DomainResult<()> {
        if let Some(id) = project_id {
             // If a project_id IS provided, it MUST exist
             match self.project_repo.find_by_id(id).await {
                 Ok(_) => Ok(()),
                 Err(DomainError::EntityNotFound(_, _)) => Err(DomainError::Validation(
                     ValidationError::relationship(&format!("Project with ID {} does not exist", id))
                 )),
                 Err(e) => Err(e), 
             }
         } else {
            // If no project_id is provided, it's valid (activity is independent)
             Ok(())
         }
    }
}

// Implement DeleteService<Activity> by delegating
#[async_trait]
impl DeleteService<Activity> for ActivityServiceImpl {
    // ... (Delegation methods exactly like in ProjectServiceImpl) ...
    fn repository(&self) -> &dyn FindById<Activity> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Activity>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl ActivityService for ActivityServiceImpl {
    async fn create_activity(
        &self,
        new_activity: NewActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if !auth.has_permission(Permission::CreateActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create activities".to_string(),
            ));
        }

        new_activity.validate()?;
        // Validate project exists ONLY if an ID was provided
        self.validate_project_exists_if_provided(new_activity.project_id).await?;

        let created_activity = self.repo.create(&new_activity, auth).await?;
        Ok(ActivityResponse::from(created_activity))
    }

    async fn get_activity_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if !auth.has_permission(Permission::ViewActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view activities".to_string(),
            ));
        }

        let activity = self.repo.find_by_id(id).await?;
        // Optional: Add check if user can access the activity's project (if it has one)
        // if let Some(project_id) = activity.project_id {
        //     self.validate_project_access(project_id, auth).await?;
        // }

        Ok(ActivityResponse::from(activity))
    }

    async fn list_activities_for_project(
        &self,
        project_id: Uuid, // Keep this required, as we are listing FOR a project
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        if !auth.has_permission(Permission::ViewActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view activities".to_string(),
            ));
        }
        
        // Validate the project exists before listing its activities
        self.validate_project_exists_if_provided(Some(project_id)).await?; 

        let paginated_result = self.repo.find_by_project_id(project_id, params).await?;

        let response_items = paginated_result
            .items
            .into_iter()
            .map(ActivityResponse::from)
            .collect();

        Ok(PaginatedResult::new(
            response_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_activity(
        &self,
        id: Uuid,
        mut update_data: UpdateActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if !auth.has_permission(Permission::EditActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit activities".to_string(),
            ));
        }

        update_data.updated_by_user_id = auth.user_id;
        update_data.validate()?;
        
        // Fetch the current activity first to check its project (if any) for access control
        let current_activity = self.repo.find_by_id(id).await?;
        // Optional: Check access to current_activity.project_id
        // if let Some(p_id) = current_activity.project_id { ... }
        
        // Validate the *new* project_id exists if it's being set
        if let Some(opt_p_id) = update_data.project_id { // Check if project_id is part of the update
             self.validate_project_exists_if_provided(opt_p_id).await?;
        }

        let updated_activity = self.repo.update(id, &update_data, auth).await?;
        Ok(ActivityResponse::from(updated_activity))
    }
    
    async fn delete_activity(
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
                 "User does not have permission to {} activities",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Fetch activity first to check existence and potentially project access
        let _activity = self.repo.find_by_id(id).await?; 
        // Optional: Check access to _activity.project_id

        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete, 
            force: false, 
        };
        
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }
}
