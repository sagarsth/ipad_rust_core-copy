use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::permission::Permission;
use crate::domains::participant::repository::ParticipantRepository;
use crate::domains::participant::types::{NewParticipant, Participant, ParticipantResponse, UpdateParticipant};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainResult, ServiceError, ServiceResult, ValidationError};
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
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;

    async fn list_participants(
        &self,
        params: PaginationParams,
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
    
    // Add methods for workshop/livelihood management if needed
    // async fn add_participant_to_workshop(...)
    // async fn remove_participant_from_workshop(...)
}

/// Implementation of the participant service
#[derive(Clone)] 
pub struct ParticipantServiceImpl {
    repo: Arc<dyn ParticipantRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Participant>>,
}

impl ParticipantServiceImpl {
    pub fn new(
        pool: SqlitePool,
        participant_repo: Arc<dyn ParticipantRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
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
            pool,
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
        ));
        
        Self {
            repo: participant_repo,
            delete_service,
        }
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
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view participants".to_string(),
            ));
        }

        let participant = self.repo.find_by_id(id).await?;
        Ok(ParticipantResponse::from(participant))
    }

    async fn list_participants(
        &self,
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list participants".to_string(),
            ));
        }

        let paginated_result = self.repo.find_all(params).await?;

        let response_items = paginated_result
            .items
            .into_iter()
            .map(ParticipantResponse::from)
            .collect();

        Ok(PaginatedResult::new(
            response_items,
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
}
