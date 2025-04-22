use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::permission::{Permission, UserRole};
use crate::domains::strategic_goal::repository::{SqliteStrategicGoalRepository, StrategicGoalRepository};
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalResponse, UpdateStrategicGoal, StrategicGoalInclude,
};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;

/// Trait defining strategic goal service operations
#[async_trait]
pub trait StrategicGoalService: DeleteService<StrategicGoal> + Send + Sync {
    async fn create_strategic_goal(
        &self,
        new_goal: NewStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;

    async fn get_strategic_goal_by_id(
        &self,
        id: Uuid,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;

    async fn list_strategic_goals(
        &self,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;

    async fn update_strategic_goal(
        &self,
        id: Uuid,
        update_data: UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;
    
    async fn delete_strategic_goal(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
}

/// Implementation of the strategic goal service
#[derive(Clone)] // Clone needed if you store this service in another struct
pub struct StrategicGoalServiceImpl {
    repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<StrategicGoal>>,
    document_service: Arc<dyn DocumentService>,
}

impl StrategicGoalServiceImpl {
    pub fn new(
        pool: SqlitePool,
        strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // Define a local wrapper struct that implements DeleteServiceRepository
        struct RepoAdapter(Arc<dyn StrategicGoalRepository + Send + Sync>);

        #[async_trait]
        impl FindById<StrategicGoal> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<StrategicGoal> {
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
                 "strategic_goals"
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

        // Wrap the specific repo in the adapter
        let adapted_repo: Arc<dyn DeleteServiceRepository<StrategicGoal>> = 
            Arc::new(RepoAdapter(strategic_goal_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool,
            adapted_repo, // Pass the adapted repo
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo),
        ));
        
        Self {
            repo: strategic_goal_repo, // Keep the original repo for other methods
            delete_service,
            document_service,
        }
    }

    /// Helper to enrich response with included data
    async fn enrich_response(
        &self, 
        mut response: StrategicGoalResponse, 
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        if let Some(includes) = include {
            if includes.contains(&StrategicGoalInclude::Documents) {
                let doc_params = PaginationParams::default(); 
                let docs_result = self.document_service
                    .list_media_documents_by_related_entity(
                        auth,
                        "strategic_goals",
                        response.id,
                        doc_params,
                        None,
                    ).await?;
                response.documents = Some(docs_result.items);
            }
        }
        Ok(response)
    }
}

#[async_trait]
impl DeleteService<StrategicGoal> for StrategicGoalServiceImpl {
    // Delegate DeleteService methods to the inner BaseDeleteService
    fn repository(&self) -> &dyn crate::domains::core::repository::FindById<StrategicGoal> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<StrategicGoal>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}


#[async_trait]
impl StrategicGoalService for StrategicGoalServiceImpl {
    async fn create_strategic_goal(
        &self,
        new_goal: NewStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create strategic goals".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_goal.validate()?; // Propagates DomainError::Validation

        // 3. Perform Creation
        let created_goal = self.repo.create(&new_goal, auth).await?;

        // 4. Convert to Response DTO
        let response = StrategicGoalResponse::from(created_goal);
        // No enrichment needed on create
        Ok(response)
    }

    async fn get_strategic_goal_by_id(
        &self,
        id: Uuid,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) { // Assumes ViewStrategicGoals permission
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view strategic goals".to_string(),
            ));
        }

        // 2. Fetch from Repository
        let goal = self.repo.find_by_id(id).await?;

        // 3. Convert to Response DTO
        let response = StrategicGoalResponse::from(goal);
        
        // Pass auth to enrich_response
        self.enrich_response(response, include, auth).await
    }

    async fn list_strategic_goals(
        &self,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) { // Assumes ViewStrategicGoals permission
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list strategic goals".to_string(),
            ));
        }

        // 2. Fetch Paginated Data
        let paginated_result = self.repo.find_all(params).await?;

        // 3. Convert items to Response DTOs
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            // Pass auth to enrich_response
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Create Paginated Response - Fix arguments
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params, // Pass the original params struct
        ))
    }

    async fn update_strategic_goal(
        &self,
        id: Uuid,
        mut update_data: UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::EditStrategicGoals) { // Assumes EditStrategicGoals permission
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit strategic goals".to_string(),
            ));
        }

        // Ensure the updater's ID is set (already required by DTO)
        update_data.updated_by_user_id = auth.user_id; 

        // 2. Validate Input DTO
        update_data.validate()?;
        
        // Check if objective_code is being updated and if it's unique (if required)
        // if let Some(code) = &update_data.objective_code {
        //     let existing = self.repo.find_by_objective_code(code).await?;
        //     if let Some(goal) = existing {
        //         if goal.id != id {
        //             return Err(DomainError::Validation(ValidationError::unique("objective_code")).into());
        //         }
        //     }
        // }

        // 3. Perform Update
        let updated_goal = self.repo.update(id, &update_data, auth).await?;

        // 4. Convert to Response DTO
        let response = StrategicGoalResponse::from(updated_goal);
        // Enrich response after update, maybe only if requested?
        // For simplicity, let's not enrich on update by default.
        Ok(response)
    }
    
    async fn delete_strategic_goal(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Determine required permission
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteStrategicGoals // Or Permission::DeleteRecord
        };

        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} strategic goals",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Configure delete options
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete, // Only fallback if initial request was NOT for hard delete
            force: false, // Default to no force delete, admin might use delete_with_dependencies
        };
        
        // Delegate to the core delete method
        let result = self.delete(id, auth, options).await?;
        
        Ok(result)
    }
}
