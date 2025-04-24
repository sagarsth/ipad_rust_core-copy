use crate::auth::AuthContext;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService};
use crate::domains::core::repository::{DeleteResult, FindById};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::livelihood::repository::{LivehoodRepository, SubsequentGrantRepository, SqliteLivelihoodRepository, SqliteSubsequentGrantRepository};
use crate::domains::livelihood::types::{Livelihood, LivelihoodInclude, LivelihoodResponse, NewLivelihood, NewSubsequentGrant, ParticipantSummary, ProjectSummary, SubsequentGrantResponse, SubsequentGrantSummary, UpdateLivelihood, UpdateSubsequentGrant};
use crate::domains::participant::repository::ParticipantRepository;
use crate::domains::permission::Permission;
use crate::domains::project::repository::ProjectRepository;
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use uuid::Uuid;

/// Interface for the livelihood service
#[async_trait]
pub trait LivehoodService: DeleteService<Livelihood> + Send + Sync {
    /// Create a new livelihood
    async fn create_livelihood(
        &self,
        new_livelihood: NewLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;

    /// Get a livelihood by ID
    async fn get_livelihood_by_id(
        &self,
        id: Uuid,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;

    /// List all livelihoods with optional filtering
    async fn list_livelihoods(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;

    /// Update a livelihood
    async fn update_livelihood(
        &self,
        id: Uuid,
        update_data: UpdateLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;
    
    /// Delete a livelihood
    async fn delete_livelihood(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    /// Add a subsequent grant to a livelihood
    async fn add_subsequent_grant(
        &self,
        new_grant: NewSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Update a subsequent grant
    async fn update_subsequent_grant(
        &self,
        id: Uuid,
        update_data: UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Get a subsequent grant by ID
    async fn get_subsequent_grant_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Delete a subsequent grant
    async fn delete_subsequent_grant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<()>;
}

/// Implementation of the livelihood service
pub struct LivehoodServiceImpl {
    repo: Arc<SqliteLivelihoodRepository>,
    delete_service: Arc<BaseDeleteService<Livelihood>>,
    subsequent_grant_repo: Arc<SqliteSubsequentGrantRepository>,
    project_repo: Arc<dyn ProjectRepository>,
    participant_repo: Arc<dyn ParticipantRepository>,
    pool: Pool<Sqlite>,
}

impl LivehoodServiceImpl {
    /// Create a new livelihood service implementation
    pub fn new(
        pool: Pool<Sqlite>,
        livelihood_repo: Arc<SqliteLivelihoodRepository>,
        subsequent_grant_repo: Arc<SqliteSubsequentGrantRepository>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository>,
        participant_repo: Arc<dyn ParticipantRepository>,
    ) -> Self {
        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            livelihood_repo.clone(),
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None
        ));
        
        Self {
            repo: livelihood_repo,
            delete_service,
            subsequent_grant_repo,
            project_repo,
            participant_repo,
            pool,
        }
    }
    
    /// Enrich a livelihood response with related entities
    async fn enrich_response(
        &self,
        mut response: LivelihoodResponse,
        include: Option<&[LivelihoodInclude]>,
    ) -> ServiceResult<LivelihoodResponse> {
        let includes = match include {
            Some(includes) => includes,
            None => return Ok(response),
        };
        
        // Check if we need to include all relations
        let include_all = includes.contains(&LivelihoodInclude::All);
        
        // Include project if requested
        if include_all || includes.contains(&LivelihoodInclude::Project) {
            if let Some(project_id) = response.project_id {
                match self.project_repo.find_by_id(project_id).await {
                    Ok(project) => {
                        response = response.with_project(ProjectSummary {
                            id: project.id,
                            name: project.name,
                        });
                    }
                    Err(DomainError::EntityNotFound(_, _)) => {
                        // Project not found, but this is not a critical error
                    }
                    Err(e) => return Err(ServiceError::Domain(e)),
                }
            }
        }
        
        // Include participant if requested
        if include_all || includes.contains(&LivelihoodInclude::Participant) {
            if let Some(participant_id) = response.participant_id {
                match self.participant_repo.find_by_id(participant_id).await {
                    Ok(participant) => {
                        response = response.with_participant(ParticipantSummary {
                            id: participant.id,
                            name: participant.name,
                            gender: participant.gender,
                            age_group: participant.age_group,
                            disability: participant.disability,
                        });
                    }
                    Err(DomainError::EntityNotFound(_, _)) => {
                        // Participant not found, but this is not a critical error
                    }
                    Err(e) => return Err(ServiceError::Domain(e)),
                }
            }
        }
        
        // Include subsequent grants if requested
        if include_all || includes.contains(&LivelihoodInclude::SubsequentGrants) {
            match self.subsequent_grant_repo.find_by_livelihood_id(response.id).await {
                Ok(grants) => {
                    let grant_summaries = grants
                        .into_iter()
                        .map(SubsequentGrantSummary::from)
                        .collect::<Vec<_>>();
                    
                    response = response.with_subsequent_grants(grant_summaries);
                }
                Err(e) => return Err(ServiceError::Domain(e)),
            }
        }
        
        Ok(response)
    }
}

// Implement DeleteService for LivehoodServiceImpl
#[async_trait]
impl DeleteService<Livelihood> for LivehoodServiceImpl {
    fn repository(&self) -> &dyn FindById<Livelihood> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Livelihood>>> {
        self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

// Implement LivehoodService for LivehoodServiceImpl
#[async_trait]
impl LivehoodService for LivehoodServiceImpl {
    async fn create_livelihood(
        &self,
        new_livelihood: NewLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create livelihoods".to_string(),
            ));
        }
        
        // Validate the new livelihood
        new_livelihood.validate().map_err(ServiceError::Domain)?;
        
        // Create the livelihood
        let livelihood = self.repo
            .create(&new_livelihood, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response and enrich if needed
        let response = LivelihoodResponse::from(livelihood);
        Ok(response)
    }
    
    async fn get_livelihood_by_id(
        &self,
        id: Uuid,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }
        
        // Get the livelihood
        let livelihood = self.repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response and enrich
        let response = LivelihoodResponse::from(livelihood);
        self.enrich_response(response, include).await
    }
    
    async fn list_livelihoods(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }
        
        // Get the livelihoods
        let result = self.repo
            .find_all(params, project_id, participant_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Map items to responses and enrich
        let mut responses = Vec::new();
        for livelihood in result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include).await?;
            responses.push(enriched);
        }
        
        // Return paginated result with enriched responses
        Ok(PaginatedResult {
            items: responses,
            total: result.total,
            page: result.page,
            per_page: result.per_page,
            total_pages: result.total_pages,
        })
    }
    
    async fn update_livelihood(
        &self,
        id: Uuid,
        mut update_data: UpdateLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to update livelihoods".to_string(),
            ));
        }
        
        // Set the updated by user ID
        update_data.updated_by_user_id = auth.user_id;
        
        // Validate the update data
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // Update the livelihood
        let livelihood = self.repo
            .update(id, &update_data, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response with subsequent grants
        let response = LivelihoodResponse::from(livelihood);
        let enriched = self.enrich_response(
            response, 
            Some(&[LivelihoodInclude::SubsequentGrants])
        ).await?;
        
        Ok(enriched)
    }
    
    async fn delete_livelihood(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Check permissions
        if hard_delete && !auth.can_hard_delete() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to hard delete records".to_string(),
            ));
        }
        
        if !auth.can_delete_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to delete livelihoods".to_string(),
            ));
        }
        
        // Prepare delete options
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: true,
            force: false,
        };
        
        // Delete the livelihood
        self.delete(id, auth, options)
            .await
            .map_err(ServiceError::Domain)
    }
    
    async fn add_subsequent_grant(
        &self,
        mut new_grant: NewSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to add grants".to_string(),
            ));
        }
        
        // Set the created by user ID if not set
        if new_grant.created_by_user_id.is_none() {
            new_grant.created_by_user_id = Some(auth.user_id);
        }
        
        // Validate the new grant
        new_grant.validate().map_err(ServiceError::Domain)?;
        
        // Check if the livelihood exists
        let livelihood = self.repo
            .find_by_id(new_grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Create the subsequent grant
        let grant = self.subsequent_grant_repo
            .create(&new_grant, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn update_subsequent_grant(
        &self,
        id: Uuid,
        mut update_data: UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to update grants".to_string(),
            ));
        }
        
        // Set the updated by user ID
        update_data.updated_by_user_id = auth.user_id;
        
        // Validate the update data
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // Update the subsequent grant
        let grant = self.subsequent_grant_repo
            .update(id, &update_data, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Get the livelihood to create summary
        let livelihood = self.repo
            .find_by_id(grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn get_subsequent_grant_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view grants".to_string(),
            ));
        }
        
        // Get the subsequent grant
        let grant = self.subsequent_grant_repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Get the livelihood to create summary
        let livelihood = self.repo
            .find_by_id(grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn delete_subsequent_grant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<()> {
        // Check permissions
        if hard_delete && !auth.can_hard_delete() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to hard delete records".to_string(),
            ));
        }
        
        if !auth.can_delete_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to delete grants".to_string(),
            ));
        }
        
        // Delete the subsequent grant
        if hard_delete {
            self.subsequent_grant_repo
                .hard_delete(id, auth)
                .await
                .map_err(ServiceError::Domain)?;
        } else {
            self.subsequent_grant_repo
                .soft_delete(id, auth)
                .await
                .map_err(ServiceError::Domain)?;
        }
        
        Ok(())
    }
}

// Add trait for permission checking to AuthContext
trait LivehoodPermissions {
    fn can_view_livelihoods(&self) -> bool;
    fn can_edit_livelihoods(&self) -> bool;
    fn can_delete_livelihoods(&self) -> bool;
    fn can_hard_delete(&self) -> bool;
}

impl LivehoodPermissions for AuthContext {
    fn can_view_livelihoods(&self) -> bool {
        self.has_permission(Permission::ViewLivelihoods)
    }
    
    fn can_edit_livelihoods(&self) -> bool {
        self.has_permission(Permission::EditLivelihoods) ||
        self.has_permission(Permission::CreateLivelihoods)
    }
    
    fn can_delete_livelihoods(&self) -> bool {
        self.has_permission(Permission::DeleteLivelihoods)
    }
    
    fn can_hard_delete(&self) -> bool {
        self.has_permission(Permission::HardDeleteRecord)
    }
}
