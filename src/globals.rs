use crate::auth::AuthService;
use crate::domains::user::{UserRepository, UserService, User};
use crate::domains::user::repository::SqliteUserRepository;
use crate::domains::sync::repository::{ChangeLogRepository, SqliteChangeLogRepository, TombstoneRepository, SqliteTombstoneRepository, SyncRepository, SqliteSyncRepository};
use crate::domains::sync::service::{SyncService, SyncServiceImpl};
use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex, Once};
use lazy_static::lazy_static;
use crate::domains::core::dependency_checker::{DependencyChecker, SqliteDependencyChecker};
use crate::domains::core::delete_service::{DeleteService, BaseDeleteService, PendingDeletionManager};
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::auth::AuthContext;
use uuid::Uuid;

// Existing Domains
use crate::domains::donor::repository::{DonorRepository, SqliteDonorRepository};
use crate::domains::donor::Donor;
use crate::domains::document::repository::{MediaDocumentRepository, SqliteMediaDocumentRepository, DocumentTypeRepository, SqliteDocumentTypeRepository};
use crate::domains::document::types::{MediaDocument, DocumentType}; // Assuming MediaDocument type
use crate::domains::compression::repository::{CompressionRepository, SqliteCompressionRepository};
use crate::domains::compression::service::{CompressionService, CompressionServiceImpl};
use crate::domains::compression::manager::{CompressionManager, CompressionManagerImpl};
use crate::domains::core::file_storage_service::{FileStorageService, LocalFileStorageService};
use crate::domains::document::file_deletion_worker::FileDeletionWorker;
use crate::domains::sync::cloud_storage::{CloudStorageService, ApiCloudStorageService};

// New Domain Imports (assuming paths and types)
use crate::domains::project::repository::{ProjectRepository, SqliteProjectRepository};
use crate::domains::project::types::Project;
use crate::domains::activity::repository::{ActivityRepository, SqliteActivityRepository};
use crate::domains::activity::types::Activity;
use crate::domains::funding::repository::{ProjectFundingRepository, SqliteProjectFundingRepository};
use crate::domains::funding::types::ProjectFunding;
use crate::domains::workshop::repository::{WorkshopRepository, SqliteWorkshopRepository};
use crate::domains::workshop::types::Workshop;
use crate::domains::livelihood::repository::{LivehoodRepository, SqliteLivelihoodRepository, SubsequentGrantRepository, SqliteSubsequentGrantRepository};
use crate::domains::livelihood::types::{Livelihood, SubsequentGrant};
use crate::domains::participant::repository::{ParticipantRepository, SqliteParticipantRepository};
use crate::domains::participant::types::Participant;
use crate::domains::strategic_goal::repository::{StrategicGoalRepository, SqliteStrategicGoalRepository};
use crate::domains::strategic_goal::types::StrategicGoal;
use crate::domains::workshop::participant_repository::{WorkshopParticipantRepository, SqliteWorkshopParticipantRepository};
use crate::domains::workshop::types::WorkshopParticipant;

// ... after existing use statements for domain imports, add new imports ...
use crate::domains::document::repository::{DocumentVersionRepository, SqliteDocumentVersionRepository, DocumentAccessLogRepository, SqliteDocumentAccessLogRepository};
use crate::domains::document::service::{DocumentService, DocumentServiceImpl};
use crate::domains::project::service::{ProjectService, ProjectServiceImpl};
use crate::domains::activity::service::{ActivityService, ActivityServiceImpl};

// Entity Mergers
use crate::domains::sync::entity_merger::{
    UserEntityMerger, DonorEntityMerger, ProjectEntityMerger, ActivityEntityMerger,
    FundingEntityMerger, WorkshopEntityMerger, LivelihoodEntityMerger,
    SubsequentGrantEntityMerger, DocumentEntityMerger, ParticipantEntityMerger,
    StrategicGoalEntityMerger, DocumentTypeEntityMerger, WorkshopParticipantEntityMerger, EntityMerger, DomainEntityMerger
};

// After ActivityService import line
use crate::domains::donor::service::{DonorService, DonorServiceImpl};
use crate::domains::funding::service::{ProjectFundingService, ProjectFundingServiceImpl};
use crate::domains::participant::service::{ParticipantService, ParticipantServiceImpl};

// New Domain Imports (assuming paths and types)
use crate::domains::workshop::service::{WorkshopService, WorkshopServiceImpl};
use crate::domains::livelihood::service::{LivehoodService, LivehoodServiceImpl};
use crate::domains::strategic_goal::service::{StrategicGoalService, StrategicGoalServiceImpl};

// Global state definitions
lazy_static! {
    static ref DB_POOL: Mutex<Option<SqlitePool>> = Mutex::new(None);
    static ref DEVICE_ID: Mutex<Option<String>> = Mutex::new(None);
    static ref OFFLINE_MODE: Mutex<bool> = Mutex::new(false);

    // Core Services
    static ref CHANGE_LOG_REPO: Mutex<Option<Arc<dyn ChangeLogRepository>>> = Mutex::new(None);
    static ref TOMBSTONE_REPO: Mutex<Option<Arc<dyn TombstoneRepository>>> = Mutex::new(None);
    static ref DEPENDENCY_CHECKER: Mutex<Option<Arc<dyn DependencyChecker>>> = Mutex::new(None);
    static ref DELETION_MANAGER: Mutex<Option<Arc<PendingDeletionManager>>> = Mutex::new(None);
    static ref AUTH_SERVICE: Mutex<Option<Arc<AuthService>>> = Mutex::new(None);
    static ref FILE_STORAGE_SERVICE: Mutex<Option<Arc<dyn FileStorageService>>> = Mutex::new(None);
    static ref COMPRESSION_MANAGER: Mutex<Option<Arc<dyn CompressionManager>>> = Mutex::new(None);
    static ref COMPRESSION_REPO: Mutex<Option<Arc<dyn CompressionRepository>>> = Mutex::new(None); // Added for completeness
    static ref COMPRESSION_SERVICE: Mutex<Option<Arc<dyn CompressionService>>> = Mutex::new(None);
    static ref CLOUD_STORAGE_SERVICE: Mutex<Option<Arc<dyn CloudStorageService>>> = Mutex::new(None);

    // User Domain
    static ref USER_REPO: Mutex<Option<Arc<dyn UserRepository>>> = Mutex::new(None);
    static ref USER_SERVICE: Mutex<Option<Arc<UserService>>> = Mutex::new(None);
    static ref DELETE_SERVICE_USER: Mutex<Option<Arc<dyn DeleteService<User>>>> = Mutex::new(None);
    static ref USER_ENTITY_MERGER: Mutex<Option<Arc<UserEntityMerger>>> = Mutex::new(None);

    // Donor Domain
    static ref DONOR_REPO: Mutex<Option<Arc<dyn DonorRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_DONOR: Mutex<Option<Arc<dyn DeleteService<Donor>>>> = Mutex::new(None);
    static ref DONOR_ENTITY_MERGER: Mutex<Option<Arc<DonorEntityMerger>>> = Mutex::new(None);

    // Document Domain
    static ref MEDIA_DOCUMENT_REPO: Mutex<Option<Arc<dyn MediaDocumentRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_MEDIA_DOCUMENT: Mutex<Option<Arc<dyn DeleteService<MediaDocument>>>> = Mutex::new(None);
    static ref MEDIA_DOCUMENT_ENTITY_MERGER: Mutex<Option<Arc<DocumentEntityMerger>>> = Mutex::new(None);
    static ref DOCUMENT_TYPE_REPO: Mutex<Option<Arc<dyn DocumentTypeRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_DOCUMENT_TYPE: Mutex<Option<Arc<dyn DeleteService<DocumentType>>>> = Mutex::new(None);
    static ref DOCUMENT_TYPE_ENTITY_MERGER: Mutex<Option<Arc<DocumentTypeEntityMerger>>> = Mutex::new(None);


    // Project Domain
    static ref PROJECT_REPO: Mutex<Option<Arc<dyn ProjectRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_PROJECT: Mutex<Option<Arc<dyn DeleteService<Project>>>> = Mutex::new(None);
    static ref PROJECT_ENTITY_MERGER: Mutex<Option<Arc<ProjectEntityMerger>>> = Mutex::new(None);

    // Activity Domain
    static ref ACTIVITY_REPO: Mutex<Option<Arc<dyn ActivityRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_ACTIVITY: Mutex<Option<Arc<dyn DeleteService<Activity>>>> = Mutex::new(None);
    static ref ACTIVITY_ENTITY_MERGER: Mutex<Option<Arc<ActivityEntityMerger>>> = Mutex::new(None);

    // Funding Domain (ProjectFunding)
    static ref PROJECT_FUNDING_REPO: Mutex<Option<Arc<dyn ProjectFundingRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_PROJECT_FUNDING: Mutex<Option<Arc<dyn DeleteService<ProjectFunding>>>> = Mutex::new(None);
    static ref PROJECT_FUNDING_ENTITY_MERGER: Mutex<Option<Arc<FundingEntityMerger>>> = Mutex::new(None);

    // Workshop Domain
    static ref WORKSHOP_REPO: Mutex<Option<Arc<dyn WorkshopRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_WORKSHOP: Mutex<Option<Arc<dyn DeleteService<Workshop>>>> = Mutex::new(None);
    static ref WORKSHOP_ENTITY_MERGER: Mutex<Option<Arc<WorkshopEntityMerger>>> = Mutex::new(None);

    // Livelihood Domain
    static ref LIVELIHOOD_REPO: Mutex<Option<Arc<dyn LivehoodRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_LIVELIHOOD: Mutex<Option<Arc<dyn DeleteService<Livelihood>>>> = Mutex::new(None);
    static ref LIVELIHOOD_ENTITY_MERGER: Mutex<Option<Arc<LivelihoodEntityMerger>>> = Mutex::new(None);

    // SubsequentGrant Domain
    static ref SUBSEQUENT_GRANT_REPO: Mutex<Option<Arc<dyn SubsequentGrantRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_SUBSEQUENT_GRANT: Mutex<Option<Arc<dyn DeleteService<SubsequentGrant>>>> = Mutex::new(None);
    static ref SUBSEQUENT_GRANT_ENTITY_MERGER: Mutex<Option<Arc<SubsequentGrantEntityMerger>>> = Mutex::new(None);

    // Participant Domain
    static ref PARTICIPANT_REPO: Mutex<Option<Arc<dyn ParticipantRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_PARTICIPANT: Mutex<Option<Arc<dyn DeleteService<Participant>>>> = Mutex::new(None);
    static ref PARTICIPANT_ENTITY_MERGER: Mutex<Option<Arc<ParticipantEntityMerger>>> = Mutex::new(None);

    // StrategicGoal Domain
    static ref STRATEGIC_GOAL_REPO: Mutex<Option<Arc<dyn StrategicGoalRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_STRATEGIC_GOAL: Mutex<Option<Arc<dyn DeleteService<StrategicGoal>>>> = Mutex::new(None);
    static ref STRATEGIC_GOAL_ENTITY_MERGER: Mutex<Option<Arc<StrategicGoalEntityMerger>>> = Mutex::new(None);

    // WorkshopParticipant Domain
    static ref WORKSHOP_PARTICIPANT_REPO: Mutex<Option<Arc<dyn WorkshopParticipantRepository>>> = Mutex::new(None);
    static ref DELETE_SERVICE_WORKSHOP_PARTICIPANT: Mutex<Option<Arc<dyn DeleteService<WorkshopParticipant>>>> = Mutex::new(None);
    static ref WORKSHOP_PARTICIPANT_ENTITY_MERGER: Mutex<Option<Arc<WorkshopParticipantEntityMerger>>> = Mutex::new(None);

    // ... within lazy_static! block after CLOUD_STORAGE_SERVICE definition ...
    static ref DOCUMENT_SERVICE: Mutex<Option<Arc<dyn DocumentService>>> = Mutex::new(None);
    static ref PROJECT_SERVICE: Mutex<Option<Arc<dyn ProjectService>>> = Mutex::new(None);
    static ref ACTIVITY_SERVICE: Mutex<Option<Arc<dyn ActivityService>>> = Mutex::new(None);

    // Donor Domain
    static ref DONOR_SERVICE: Mutex<Option<Arc<dyn DonorService>>> = Mutex::new(None);
    static ref PROJECT_FUNDING_SERVICE: Mutex<Option<Arc<dyn ProjectFundingService>>> = Mutex::new(None);
    static ref PARTICIPANT_SERVICE: Mutex<Option<Arc<dyn ParticipantService>>> = Mutex::new(None);

    // Workshop Service
    static ref WORKSHOP_SERVICE: Mutex<Option<Arc<dyn WorkshopService>>> = Mutex::new(None);
    static ref LIVELIHOOD_SERVICE: Mutex<Option<Arc<dyn LivehoodService>>> = Mutex::new(None);
    static ref STRATEGIC_GOAL_SERVICE: Mutex<Option<Arc<dyn StrategicGoalService>>> = Mutex::new(None);

    // ---- Sync / Merge ----
    static ref SYNC_REPO: Mutex<Option<Arc<dyn SyncRepository>>> = Mutex::new(None);
    static ref ENTITY_MERGER_GLOBAL: Mutex<Option<Arc<EntityMerger>>> = Mutex::new(None);
    static ref SYNC_SERVICE_GLOBAL: Mutex<Option<Arc<dyn SyncService>>> = Mutex::new(None);

}

static INIT: Once = Once::new();

/// Initialize global services
pub fn initialize(
    db_url: &str,
    device_id_str: &str, // Renamed to avoid conflict
    offline_mode_flag: bool, // Renamed to avoid conflict
    jwt_secret: &str
) -> FFIResult<()> {
    let mut initialization_result = Ok(());

    INIT.call_once(|| {
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = (|| {
            crate::auth::jwt::initialize(jwt_secret);

            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5) // Consider increasing if many domains
                .connect_lazy(db_url)?;

            *DEVICE_ID.lock().map_err(|_| "DEVICE_ID lock poisoned")? = Some(device_id_str.to_string());
            *OFFLINE_MODE.lock().map_err(|_| "OFFLINE_MODE lock poisoned")? = offline_mode_flag;

            // Core services
            let change_log_repo: Arc<dyn ChangeLogRepository> = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
            let tombstone_repo: Arc<dyn TombstoneRepository> = Arc::new(SqliteTombstoneRepository::new(pool.clone()));
            let dependency_checker: Arc<dyn DependencyChecker> = Arc::new(SqliteDependencyChecker::new(pool.clone()));
            let deletion_manager = Arc::new(PendingDeletionManager::new(pool.clone()));
            let auth_service = Arc::new(AuthService::new(
                pool.clone(),
                device_id_str.to_string(),
                offline_mode_flag,
            ));
            let file_storage_service: Arc<dyn FileStorageService> = Arc::new(
                LocalFileStorageService::new("./storage").map_err(|e| format!("File storage init failed: {}", e))?
            );
            let compression_repo: Arc<dyn CompressionRepository> = Arc::new(SqliteCompressionRepository::new(pool.clone()));

            // Cloud storage service (API-backed). In the future the base URL should come from config or FFI parameters.
            let cloud_storage_service: Arc<dyn CloudStorageService> = Arc::new(ApiCloudStorageService::new(
                "https://example.com/api",   // TODO: replace with configurable base URL
                "./storage",                 // Re-use local storage directory
            ));

            // Repositories
            let user_repo: Arc<dyn UserRepository> = Arc::new(SqliteUserRepository::new(pool.clone(), change_log_repo.clone()));
            let donor_repo: Arc<dyn DonorRepository> = Arc::new(SqliteDonorRepository::new(pool.clone()));
            let media_document_repo: Arc<dyn MediaDocumentRepository> = Arc::new(SqliteMediaDocumentRepository::new(pool.clone(), change_log_repo.clone()));
            let document_type_repo: Arc<dyn DocumentTypeRepository> = Arc::new(SqliteDocumentTypeRepository::new(pool.clone(), change_log_repo.clone()));
            let project_repo: Arc<dyn ProjectRepository> = Arc::new(SqliteProjectRepository::new(pool.clone(), change_log_repo.clone()));
            let activity_repo: Arc<dyn ActivityRepository> = Arc::new(SqliteActivityRepository::new(pool.clone(), change_log_repo.clone()));
            let project_funding_repo: Arc<dyn ProjectFundingRepository> = Arc::new(SqliteProjectFundingRepository::new(pool.clone(), change_log_repo.clone()));
            let workshop_repo: Arc<dyn WorkshopRepository> = Arc::new(SqliteWorkshopRepository::new(pool.clone(), change_log_repo.clone()));
            let livelihood_repo: Arc<dyn LivehoodRepository> = Arc::new(SqliteLivelihoodRepository::new(pool.clone(), change_log_repo.clone()));
            let subsequent_grant_repo: Arc<dyn SubsequentGrantRepository> = Arc::new(SqliteSubsequentGrantRepository::new(pool.clone()));
            let participant_repo: Arc<dyn ParticipantRepository> = Arc::new(SqliteParticipantRepository::new(pool.clone(), change_log_repo.clone()));
            let strategic_goal_repo: Arc<dyn StrategicGoalRepository> = Arc::new(SqliteStrategicGoalRepository::new(pool.clone(), change_log_repo.clone()));
            let workshop_participant_repo: Arc<dyn WorkshopParticipantRepository> = Arc::new(SqliteWorkshopParticipantRepository::new(pool.clone(), change_log_repo.clone()));


            // --- Delete Service Adapters & Services ---
            // User
            struct UserRepoAdapter(Arc<dyn UserRepository>);
             #[async_trait::async_trait]
             impl FindById<User> for UserRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<User> { self.0.find_by_id(id).await } }
             #[async_trait::async_trait]
             impl HardDeletable for UserRepoAdapter {
                  fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                  async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                  async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                  }
             }
             #[async_trait::async_trait]
             impl SoftDeletable for UserRepoAdapter {
                 async fn soft_delete(&self, _id: Uuid, _auth: &AuthContext) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
                 async fn soft_delete_with_tx(&self, _id: Uuid, _auth: &AuthContext, _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
             }
            let adapted_user_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<User>> = Arc::new(UserRepoAdapter(user_repo.clone()));
            let delete_service_user: Arc<dyn DeleteService<User>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_user_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), None, deletion_manager.clone()));

            // User Service and entity merger (re-added)
            let user_service = Arc::new(UserService::new(
                user_repo.clone(),
                auth_service.clone(),
                delete_service_user.clone(),
            ));

            let user_entity_merger = Arc::new(UserEntityMerger::new(user_repo.clone(), pool.clone()));

            // Donor
            struct DonorRepoAdapter(Arc<dyn DonorRepository>);
            #[async_trait::async_trait]
            impl FindById<Donor> for DonorRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Donor> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for DonorRepoAdapter {
                 fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                 async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                 async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                 }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for DonorRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_donor_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Donor>> = Arc::new(DonorRepoAdapter(donor_repo.clone()));
            let delete_service_donor: Arc<dyn DeleteService<Donor>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_donor_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // MediaDocument
            struct MediaDocumentRepoAdapter(Arc<dyn MediaDocumentRepository>);
            #[async_trait::async_trait]
            impl FindById<MediaDocument> for MediaDocumentRepoAdapter {
                async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<MediaDocument> {
                    crate::domains::core::repository::FindById::find_by_id(&*self.0, id).await
                }
            }
            #[async_trait::async_trait]
            impl HardDeletable for MediaDocumentRepoAdapter {
                 fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                 async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                 async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                 }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for MediaDocumentRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_media_document_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<MediaDocument>> = Arc::new(MediaDocumentRepoAdapter(media_document_repo.clone()));
            let delete_service_media_document: Arc<dyn DeleteService<MediaDocument>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_media_document_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), None, deletion_manager.clone()));

            // DocumentType
            struct DocumentTypeRepoAdapter(Arc<dyn DocumentTypeRepository>);
            #[async_trait::async_trait]
            impl FindById<DocumentType> for DocumentTypeRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<DocumentType> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for DocumentTypeRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for DocumentTypeRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_document_type_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<DocumentType>> = Arc::new(DocumentTypeRepoAdapter(document_type_repo.clone()));
            let delete_service_document_type: Arc<dyn DeleteService<DocumentType>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_document_type_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), None, deletion_manager.clone()));

            // Project
            struct ProjectRepoAdapter(Arc<dyn ProjectRepository>);
            #[async_trait::async_trait]
            impl FindById<Project> for ProjectRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Project> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for ProjectRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for ProjectRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_project_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Project>> = Arc::new(ProjectRepoAdapter(project_repo.clone()));
            let delete_service_project: Arc<dyn DeleteService<Project>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_project_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // Activity
            struct ActivityRepoAdapter(Arc<dyn ActivityRepository>);
            #[async_trait::async_trait]
            impl FindById<Activity> for ActivityRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Activity> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for ActivityRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for ActivityRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_activity_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Activity>> = Arc::new(ActivityRepoAdapter(activity_repo.clone()));
            let delete_service_activity: Arc<dyn DeleteService<Activity>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_activity_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // ProjectFunding
            struct ProjectFundingRepoAdapter(Arc<dyn ProjectFundingRepository>);
            #[async_trait::async_trait]
            impl FindById<ProjectFunding> for ProjectFundingRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<ProjectFunding> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for ProjectFundingRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for ProjectFundingRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_project_funding_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<ProjectFunding>> = Arc::new(ProjectFundingRepoAdapter(project_funding_repo.clone()));
            let delete_service_project_funding: Arc<dyn DeleteService<ProjectFunding>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_project_funding_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

             // Workshop
            struct WorkshopRepoAdapter(Arc<dyn WorkshopRepository>);
            #[async_trait::async_trait]
            impl FindById<Workshop> for WorkshopRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Workshop> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for WorkshopRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for WorkshopRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_workshop_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Workshop>> = Arc::new(WorkshopRepoAdapter(workshop_repo.clone()));
            let delete_service_workshop: Arc<dyn DeleteService<Workshop>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_workshop_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // Livelihood
            struct LivelihoodRepoAdapter(Arc<dyn LivehoodRepository>);
            #[async_trait::async_trait]
            impl FindById<Livelihood> for LivelihoodRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Livelihood> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for LivelihoodRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for LivelihoodRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_livelihood_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Livelihood>> = Arc::new(LivelihoodRepoAdapter(livelihood_repo.clone()));
            let delete_service_livelihood: Arc<dyn DeleteService<Livelihood>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_livelihood_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // SubsequentGrant - Assuming it might not have direct document links or complex dependencies for this example
            // If it does, a more specific DeleteService might be needed. For now, let's assume simpler.
            struct SubsequentGrantRepoAdapter(Arc<dyn SubsequentGrantRepository>);
            #[async_trait::async_trait]
            impl FindById<SubsequentGrant> for SubsequentGrantRepoAdapter {
                async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<SubsequentGrant> { self.0.find_by_id(id).await }
            }

            #[async_trait::async_trait]
            impl HardDeletable for SubsequentGrantRepoAdapter {
                fn entity_name(&self) -> &'static str { "subsequent_grants" }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { crate::domains::core::repository::HardDeletable::hard_delete(&*self.0, id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for SubsequentGrantRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { crate::domains::core::repository::SoftDeletable::soft_delete(&*self.0, id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_subsequent_grant_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<SubsequentGrant>> = Arc::new(SubsequentGrantRepoAdapter(subsequent_grant_repo.clone()));
            let delete_service_subsequent_grant: Arc<dyn DeleteService<SubsequentGrant>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_subsequent_grant_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), None, deletion_manager.clone()));

            // Participant
            struct ParticipantRepoAdapter(Arc<dyn ParticipantRepository>);
            #[async_trait::async_trait]
            impl FindById<Participant> for ParticipantRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Participant> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for ParticipantRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for ParticipantRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_participant_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Participant>> = Arc::new(ParticipantRepoAdapter(participant_repo.clone()));
            let delete_service_participant: Arc<dyn DeleteService<Participant>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_participant_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));


            // StrategicGoal
            struct StrategicGoalRepoAdapter(Arc<dyn StrategicGoalRepository>);
            #[async_trait::async_trait]
            impl FindById<StrategicGoal> for StrategicGoalRepoAdapter { async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<StrategicGoal> { self.0.find_by_id(id).await } }
            #[async_trait::async_trait]
            impl HardDeletable for StrategicGoalRepoAdapter {
                fn entity_name(&self) -> &'static str { HardDeletable::entity_name(&*self.0) }
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.hard_delete(id, auth).await }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::HardDeletable::hard_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for StrategicGoalRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { self.0.soft_delete(id, auth).await }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    crate::domains::core::repository::SoftDeletable::soft_delete_with_tx(&*self.0, id, auth, tx).await
                }
            }
            let adapted_strategic_goal_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<StrategicGoal>> = Arc::new(StrategicGoalRepoAdapter(strategic_goal_repo.clone()));
            let delete_service_strategic_goal: Arc<dyn DeleteService<StrategicGoal>> = Arc::new(BaseDeleteService::new(pool.clone(), adapted_strategic_goal_repo, tombstone_repo.clone(), change_log_repo.clone(), dependency_checker.clone(), Some(media_document_repo.clone()), deletion_manager.clone()));

            // WorkshopParticipant - This is a linking table, soft delete is primary. Hard delete might not be exposed/needed via generic DeleteService.
            // Or it might be handled differently (e.g., cascade delete or specific service logic).
            // For now, let's assume it needs a DeleteService if it can be independently deleted.
            // The WorkshopParticipantRepository has soft_delete, but not hard_delete directly in its trait.
            // If BaseDeleteService is to be used, it might need a hard_delete or an adapter.
            // For this example, let's assume soft delete is primary and hard delete might be a direct call or not used via generic service.
            // This might mean WorkshopParticipantEntityMerger handles its own "deletion" logic if it's just a soft delete.
            // Or if a full DeleteService is needed:
            struct WorkshopParticipantRepoAdapter(Arc<dyn WorkshopParticipantRepository>);
            #[async_trait::async_trait]
            impl FindById<WorkshopParticipant> for WorkshopParticipantRepoAdapter {
                 async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<WorkshopParticipant> {
                    // Delegate to the repository's find_by_id, which should operate on workshop_participants.id
                    self.0.find_by_id(id).await
                 }
            }
             #[async_trait::async_trait]
            impl HardDeletable for WorkshopParticipantRepoAdapter {
                fn entity_name(&self) -> &'static str { "workshop_participants" } // This is the table name
                async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> {
                    // This requires hard_delete_link_by_id_with_tx to be on the repo trait and a tx managed here.
                    // For simplicity with BaseDeleteService, which expects hard_delete_with_tx,
                    // we'll assume BaseDeleteService will primarily use the _with_tx variant.
                    // If hard_delete (non-tx) is directly called on this adapter, it would need to create a tx.
                    // However, BaseDeleteService typically calls hard_delete_with_tx.
                    // We need to ensure WorkshopParticipantRepository has a hard_delete_link_by_id_with_tx.
                    // Let's assume it will be called via hard_delete_with_tx by the BaseDeleteService.
                    // So, this direct one can be less critical if BaseDeleteService is the only consumer.
                    // For completeness, if called, it should start a transaction.
                    // However, to align with BaseDeleteService, we'll make this call its _with_tx version.
                    // This is a slight simplification for adapter purposes.
                    let mut tx = get_db_pool().map_err(|e| crate::errors::DomainError::Internal(format!("Failed to get DB pool: {}", e)))?.begin().await.map_err(crate::errors::DbError::from)?;
                    let result = self.0.hard_delete_link_by_id_with_tx(id, auth, &mut tx).await;
                    if result.is_ok() {
                        tx.commit().await.map_err(crate::errors::DbError::from)?;
                    } else {
                        let _ = tx.rollback().await;
                    }
                    result
                }
                async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     // Delegate to the repository's method that deletes by the workshop_participant link's own ID
                     self.0.hard_delete_link_by_id_with_tx(id, auth, tx).await
                }
            }
            #[async_trait::async_trait]
            impl SoftDeletable for WorkshopParticipantRepoAdapter {
                async fn soft_delete(&self, _id: Uuid, _auth: &AuthContext) -> crate::errors::DomainResult<()> {
                    // Soft delete by the link's own single ID is not the primary mechanism.
                    // Soft deletes are typically by (workshop_id, participant_id) via remove_participant.
                    // Forcing this through generic DeleteService by single ID can be marked as not supported.
                    Err(crate::errors::DomainError::Internal("Soft delete for WorkshopParticipant by its own single ID is not supported. Use domain-specific service.".to_string()))
                }
                async fn soft_delete_with_tx(&self, _id: Uuid, _auth: &AuthContext, _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    Err(crate::errors::DomainError::Internal("Soft delete for WorkshopParticipant by its own single ID is not supported. Use domain-specific service.".to_string()))
                }
            }
            // Create a minimal DeleteService for WorkshopParticipant so that the EntityMerger constructor signature is satisfied.
            let adapted_workshop_participant_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<WorkshopParticipant>> = Arc::new(WorkshopParticipantRepoAdapter(workshop_participant_repo.clone()));
            let delete_service_workshop_participant: Arc<dyn DeleteService<WorkshopParticipant>> = Arc::new(BaseDeleteService::new(
                pool.clone(),
                adapted_workshop_participant_repo,
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                None,
                deletion_manager.clone(),
            ));
            
            // Store the service
            *DELETE_SERVICE_WORKSHOP_PARTICIPANT.lock().map_err(|_| "DELETE_SERVICE_WORKSHOP_PARTICIPANT lock poisoned")? = Some(delete_service_workshop_participant.clone());
            
            // Now create the merger using the delete service
            // WorkshopParticipantEntityMerger - if it has its own delete logic, it won't take a DeleteService.
            let workshop_participant_entity_merger = Arc::new(WorkshopParticipantEntityMerger::new(workshop_participant_repo.clone(), pool.clone(), delete_service_workshop_participant.clone()));
            
            *WORKSHOP_PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| "WORKSHOP_PARTICIPANT_ENTITY_MERGER lock poisoned")? = Some(workshop_participant_entity_merger.clone());

            let media_document_entity_merger = Arc::new(DocumentEntityMerger::new(
                media_document_repo.clone(),
                pool.clone(),
                delete_service_media_document.clone(),
            ));

            let document_type_entity_merger = Arc::new(DocumentTypeEntityMerger::new(document_type_repo.clone(), pool.clone(), delete_service_document_type.clone()));

            // Create other entity mergers that were previously missing or out of order
            let project_entity_merger = Arc::new(ProjectEntityMerger::new(project_repo.clone(), pool.clone(), delete_service_project.clone()));
            let activity_entity_merger = Arc::new(ActivityEntityMerger::new(activity_repo.clone(), pool.clone(), delete_service_activity.clone()));
            let project_funding_entity_merger = Arc::new(FundingEntityMerger::new(project_funding_repo.clone(), pool.clone(), delete_service_project_funding.clone()));
            let workshop_entity_merger = Arc::new(WorkshopEntityMerger::new(workshop_repo.clone(), pool.clone(), delete_service_workshop.clone()));
            let livelihood_entity_merger = Arc::new(LivelihoodEntityMerger::new(livelihood_repo.clone(), pool.clone(), delete_service_livelihood.clone()));
            let subsequent_grant_entity_merger = Arc::new(SubsequentGrantEntityMerger::new(subsequent_grant_repo.clone(), pool.clone(), delete_service_subsequent_grant.clone()));
            let participant_entity_merger = Arc::new(ParticipantEntityMerger::new(participant_repo.clone(), pool.clone(), delete_service_participant.clone()));
            let strategic_goal_entity_merger = Arc::new(StrategicGoalEntityMerger::new(strategic_goal_repo.clone(), pool.clone(), delete_service_strategic_goal.clone()));
            let donor_entity_merger = Arc::new(DonorEntityMerger::new(donor_repo.clone(), pool.clone(), delete_service_donor.clone()));


  // Compression components
 let compression_service: Arc<dyn CompressionService> = Arc::new(CompressionServiceImpl::new(
    pool.clone(),
    compression_repo.clone(),
    file_storage_service.clone(),
    media_document_repo.clone(),
    None, // ghostscript_path
));
let compression_manager: Arc<dyn CompressionManager> = Arc::new(CompressionManagerImpl::new(
    compression_service.clone(), // Corrected: pass the service, not the repo
    compression_repo.clone(),
    2,        // max_concurrent_jobs
    5_000,    // poll interval ms
));
let _ = compression_manager.start();

// FileDeletionWorker
let fd_pool = pool.clone();
let fd_storage = file_storage_service.clone();
tokio::spawn(async move {
    let worker = FileDeletionWorker::new(fd_pool, fd_storage);
    if let Err(e) = worker.start().await {
        log::error!("FileDeletionWorker exited: {:?}", e);
    }
});


            // --- Additional Document repositories required by DocumentService ---
            let document_version_repo: Arc<dyn DocumentVersionRepository> = Arc::new(SqliteDocumentVersionRepository::new(pool.clone(), change_log_repo.clone()));
            let document_access_log_repo: Arc<dyn DocumentAccessLogRepository> = Arc::new(SqliteDocumentAccessLogRepository::new(pool.clone()));

            // Document Service (needed by Project & Activity)
            let document_service: Arc<dyn DocumentService> = Arc::new(DocumentServiceImpl::new(
                pool.clone(),
                document_type_repo.clone(),
                media_document_repo.clone(),
                document_version_repo.clone(),
                document_access_log_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                file_storage_service.clone(),
                compression_service.clone(),
                deletion_manager.clone(),
            ));

            // Project Service
            let project_service: Arc<dyn ProjectService> = Arc::new(ProjectServiceImpl::new(
                pool.clone(),
                project_repo.clone(),
                strategic_goal_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                media_document_repo.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Activity Service
            let activity_service_singleton: Arc<dyn ActivityService> = Arc::new(ActivityServiceImpl::new(
                pool.clone(),
                activity_repo.clone(),
                project_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Donor Service
            let donor_service: Arc<dyn DonorService> = Arc::new(DonorServiceImpl::new(
                pool.clone(),
                donor_repo.clone(),
                project_funding_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                media_document_repo.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Project Funding Service
            let project_funding_service: Arc<dyn ProjectFundingService> = Arc::new(ProjectFundingServiceImpl::new(
                pool.clone(),
                project_funding_repo.clone(),
                project_repo.clone(),
                donor_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Participant Service
            let participant_service: Arc<dyn ParticipantService> = Arc::new(ParticipantServiceImpl::new(
                pool.clone(),
                participant_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Workshop Service
            let workshop_service: Arc<dyn WorkshopService> = Arc::new(WorkshopServiceImpl::new(
                pool.clone(),
                workshop_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                project_repo.clone(),
                workshop_participant_repo.clone(),
                document_service.clone(),
                deletion_manager.clone(),
            ));

            // Livelihood Service
            let livelihood_service: Arc<dyn LivehoodService> = Arc::new(LivehoodServiceImpl::new(
                pool.clone(),
                Arc::new(SqliteLivelihoodRepository::new(pool.clone(), change_log_repo.clone())),
                Arc::new(SqliteSubsequentGrantRepository::new(pool.clone())),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                project_repo.clone(),
                participant_repo.clone(),
                document_service.clone(),
                media_document_repo.clone(),
                deletion_manager.clone(),
            ));

            // Strategic Goal Service
            let strategic_goal_service: Arc<dyn StrategicGoalService> = Arc::new(StrategicGoalServiceImpl::new(
                pool.clone(),
                strategic_goal_repo.clone(),
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                document_service.clone(),
                project_repo.clone(),
                deletion_manager.clone(),
            ));

            // --- END services creation ---

            
            // Store all components
            *DB_POOL.lock().map_err(|_| "DB_POOL lock poisoned")? = Some(pool.clone());
            *CHANGE_LOG_REPO.lock().map_err(|_| "CHANGE_LOG_REPO lock poisoned")? = Some(change_log_repo.clone());
            *TOMBSTONE_REPO.lock().map_err(|_| "TOMBSTONE_REPO lock poisoned")? = Some(tombstone_repo.clone());
            *DEPENDENCY_CHECKER.lock().map_err(|_| "DEPENDENCY_CHECKER lock poisoned")? = Some(dependency_checker.clone());
            *DELETION_MANAGER.lock().map_err(|_| "DELETION_MANAGER lock poisoned")? = Some(deletion_manager.clone());
            *AUTH_SERVICE.lock().map_err(|_| "AUTH_SERVICE lock poisoned")? = Some(auth_service.clone());
            *FILE_STORAGE_SERVICE.lock().map_err(|_| "FILE_STORAGE_SERVICE lock poisoned")? = Some(file_storage_service.clone());
            *COMPRESSION_REPO.lock().map_err(|_| "COMPRESSION_REPO lock poisoned")? = Some(compression_repo.clone());
            *COMPRESSION_SERVICE.lock().map_err(|_| "COMPRESSION_SERVICE lock poisoned")? = Some(compression_service.clone());
            *COMPRESSION_MANAGER.lock().map_err(|_| "COMPRESSION_MANAGER lock poisoned")? = Some(compression_manager.clone());
            *CLOUD_STORAGE_SERVICE.lock().map_err(|_| "CLOUD_STORAGE_SERVICE lock poisoned")? = Some(cloud_storage_service.clone());

            *USER_REPO.lock().map_err(|_| "USER_REPO lock poisoned")? = Some(user_repo);
            *USER_SERVICE.lock().map_err(|_| "USER_SERVICE lock poisoned")? = Some(user_service);
            *DELETE_SERVICE_USER.lock().map_err(|_| "DELETE_SERVICE_USER lock poisoned")? = Some(delete_service_user);
            *USER_ENTITY_MERGER.lock().map_err(|_| "USER_ENTITY_MERGER lock poisoned")? = Some(user_entity_merger.clone());

            *DONOR_REPO.lock().map_err(|_| "DONOR_REPO lock poisoned")? = Some(donor_repo);
            *DELETE_SERVICE_DONOR.lock().map_err(|_| "DELETE_SERVICE_DONOR lock poisoned")? = Some(delete_service_donor);
            *DONOR_ENTITY_MERGER.lock().map_err(|_| "DONOR_ENTITY_MERGER lock poisoned")? = Some(donor_entity_merger.clone());

            *MEDIA_DOCUMENT_REPO.lock().map_err(|_| "MEDIA_DOCUMENT_REPO lock poisoned")? = Some(media_document_repo);
            *DELETE_SERVICE_MEDIA_DOCUMENT.lock().map_err(|_| "DELETE_SERVICE_MEDIA_DOCUMENT lock poisoned")? = Some(delete_service_media_document);
            *MEDIA_DOCUMENT_ENTITY_MERGER.lock().map_err(|_| "MEDIA_DOCUMENT_ENTITY_MERGER lock poisoned")? = Some(media_document_entity_merger.clone());

            *DOCUMENT_TYPE_REPO.lock().map_err(|_| "DOCUMENT_TYPE_REPO lock poisoned")? = Some(document_type_repo);
            *DELETE_SERVICE_DOCUMENT_TYPE.lock().map_err(|_| "DELETE_SERVICE_DOCUMENT_TYPE lock poisoned")? = Some(delete_service_document_type);
            *DOCUMENT_TYPE_ENTITY_MERGER.lock().map_err(|_| "DOCUMENT_TYPE_ENTITY_MERGER lock poisoned")? = Some(document_type_entity_merger.clone());

            *PROJECT_REPO.lock().map_err(|_| "PROJECT_REPO lock poisoned")? = Some(project_repo);
            *DELETE_SERVICE_PROJECT.lock().map_err(|_| "DELETE_SERVICE_PROJECT lock poisoned")? = Some(delete_service_project);
            *PROJECT_ENTITY_MERGER.lock().map_err(|_| "PROJECT_ENTITY_MERGER lock poisoned")? = Some(project_entity_merger.clone());

            *ACTIVITY_REPO.lock().map_err(|_| "ACTIVITY_REPO lock poisoned")? = Some(activity_repo);
            *DELETE_SERVICE_ACTIVITY.lock().map_err(|_| "DELETE_SERVICE_ACTIVITY lock poisoned")? = Some(delete_service_activity);
            *ACTIVITY_ENTITY_MERGER.lock().map_err(|_| "ACTIVITY_ENTITY_MERGER lock poisoned")? = Some(activity_entity_merger.clone());

            *PROJECT_FUNDING_REPO.lock().map_err(|_| "PROJECT_FUNDING_REPO lock poisoned")? = Some(project_funding_repo);
            *DELETE_SERVICE_PROJECT_FUNDING.lock().map_err(|_| "DELETE_SERVICE_PROJECT_FUNDING lock poisoned")? = Some(delete_service_project_funding);
            *PROJECT_FUNDING_ENTITY_MERGER.lock().map_err(|_| "PROJECT_FUNDING_ENTITY_MERGER lock poisoned")? = Some(project_funding_entity_merger.clone());

            *WORKSHOP_REPO.lock().map_err(|_| "WORKSHOP_REPO lock poisoned")? = Some(workshop_repo);
            *DELETE_SERVICE_WORKSHOP.lock().map_err(|_| "DELETE_SERVICE_WORKSHOP lock poisoned")? = Some(delete_service_workshop);
            *WORKSHOP_ENTITY_MERGER.lock().map_err(|_| "WORKSHOP_ENTITY_MERGER lock poisoned")? = Some(workshop_entity_merger.clone());

            *LIVELIHOOD_REPO.lock().map_err(|_| "LIVELIHOOD_REPO lock poisoned")? = Some(livelihood_repo);
            *DELETE_SERVICE_LIVELIHOOD.lock().map_err(|_| "DELETE_SERVICE_LIVELIHOOD lock poisoned")? = Some(delete_service_livelihood);
            *LIVELIHOOD_ENTITY_MERGER.lock().map_err(|_| "LIVELIHOOD_ENTITY_MERGER lock poisoned")? = Some(livelihood_entity_merger.clone());

            *SUBSEQUENT_GRANT_REPO.lock().map_err(|_| "SUBSEQUENT_GRANT_REPO lock poisoned")? = Some(subsequent_grant_repo);
            *DELETE_SERVICE_SUBSEQUENT_GRANT.lock().map_err(|_| "DELETE_SERVICE_SUBSEQUENT_GRANT lock poisoned")? = Some(delete_service_subsequent_grant);
            *SUBSEQUENT_GRANT_ENTITY_MERGER.lock().map_err(|_| "SUBSEQUENT_GRANT_ENTITY_MERGER lock poisoned")? = Some(subsequent_grant_entity_merger.clone());

            *PARTICIPANT_REPO.lock().map_err(|_| "PARTICIPANT_REPO lock poisoned")? = Some(participant_repo);
            *DELETE_SERVICE_PARTICIPANT.lock().map_err(|_| "DELETE_SERVICE_PARTICIPANT lock poisoned")? = Some(delete_service_participant);
            *PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| "PARTICIPANT_ENTITY_MERGER lock poisoned")? = Some(participant_entity_merger.clone());

            *STRATEGIC_GOAL_REPO.lock().map_err(|_| "STRATEGIC_GOAL_REPO lock poisoned")? = Some(strategic_goal_repo);
            *DELETE_SERVICE_STRATEGIC_GOAL.lock().map_err(|_| "DELETE_SERVICE_STRATEGIC_GOAL lock poisoned")? = Some(delete_service_strategic_goal);
            *STRATEGIC_GOAL_ENTITY_MERGER.lock().map_err(|_| "STRATEGIC_GOAL_ENTITY_MERGER lock poisoned")? = Some(strategic_goal_entity_merger.clone());

            *WORKSHOP_PARTICIPANT_REPO.lock().map_err(|_| "WORKSHOP_PARTICIPANT_REPO lock poisoned")? = Some(workshop_participant_repo);
            *WORKSHOP_PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| "WORKSHOP_PARTICIPANT_ENTITY_MERGER lock poisoned")? = Some(workshop_participant_entity_merger.clone());

            *DOCUMENT_SERVICE.lock().map_err(|_| "DOCUMENT_SERVICE lock poisoned")? = Some(document_service);
            *PROJECT_SERVICE.lock().map_err(|_| "PROJECT_SERVICE lock poisoned")? = Some(project_service);
            *ACTIVITY_SERVICE.lock().map_err(|_| "ACTIVITY_SERVICE lock poisoned")? = Some(activity_service_singleton);

            *DONOR_SERVICE.lock().map_err(|_| "DONOR_SERVICE lock poisoned")? = Some(donor_service);
            *PROJECT_FUNDING_SERVICE.lock().map_err(|_| "PROJECT_FUNDING_SERVICE lock poisoned")? = Some(project_funding_service);
            *PARTICIPANT_SERVICE.lock().map_err(|_| "PARTICIPANT_SERVICE lock poisoned")? = Some(participant_service);

            *WORKSHOP_SERVICE.lock().map_err(|_| "WORKSHOP_SERVICE lock poisoned")? = Some(workshop_service);
            *LIVELIHOOD_SERVICE.lock().map_err(|_| "LIVELIHOOD_SERVICE lock poisoned")? = Some(livelihood_service);
            *STRATEGIC_GOAL_SERVICE.lock().map_err(|_| "STRATEGIC_GOAL_SERVICE lock poisoned")? = Some(strategic_goal_service);

            // --- Central Entity Merger & Sync components ---
            let mut central_merger = EntityMerger::new(pool.clone());
            central_merger.register_merger(user_entity_merger.clone());
            central_merger.register_merger(donor_entity_merger.clone());
            central_merger.register_merger(media_document_entity_merger.clone());
            central_merger.register_merger(document_type_entity_merger.clone());
            central_merger.register_merger(project_entity_merger.clone());
            central_merger.register_merger(activity_entity_merger.clone());
            central_merger.register_merger(project_funding_entity_merger.clone());
            central_merger.register_merger(workshop_entity_merger.clone());
            central_merger.register_merger(livelihood_entity_merger.clone());
            central_merger.register_merger(subsequent_grant_entity_merger.clone());
            central_merger.register_merger(participant_entity_merger.clone());
            central_merger.register_merger(strategic_goal_entity_merger.clone());
            central_merger.register_merger(workshop_participant_entity_merger.clone());

            let central_merger = Arc::new(central_merger);

            // Sync repository & service
            let sync_repo: Arc<dyn SyncRepository> = Arc::new(SqliteSyncRepository::new(pool.clone()));
            let sync_service: Arc<dyn SyncService> = Arc::new(SyncServiceImpl::new(
                sync_repo.clone(),
                change_log_repo.clone(),
                tombstone_repo.clone(),
                central_merger.clone(),
                file_storage_service.clone(),
                cloud_storage_service.clone(),
                Some(compression_service.clone()),
                None,
                None,
            ));

            *SYNC_REPO.lock().map_err(|_| "SYNC_REPO lock poisoned")? = Some(sync_repo);
            *ENTITY_MERGER_GLOBAL.lock().map_err(|_| "ENTITY_MERGER_GLOBAL lock poisoned")? = Some(central_merger);
            *SYNC_SERVICE_GLOBAL.lock().map_err(|_| "SYNC_SERVICE_GLOBAL lock poisoned")? = Some(sync_service);

            Ok(())
        })();

        if let Err(e) = result {
            initialization_result = Err(FFIError::internal(format!("Initialization failed: {}", e).to_string()));
        }
    });

    initialization_result
}

// --- Getter Functions ---

pub fn get_db_pool() -> FFIResult<SqlitePool> {
    DB_POOL.lock().map_err(|_| FFIError::internal("DB_POOL lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Database pool not initialized".to_string()))
}
pub fn get_device_id() -> FFIResult<String> {
    DEVICE_ID.lock().map_err(|_| FFIError::internal("DEVICE_ID lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Device ID not initialized".to_string()))
}
pub fn is_offline_mode() -> bool { OFFLINE_MODE.lock().map(|guard| *guard).unwrap_or(false) }
pub fn set_offline_mode(offline: bool) { if let Ok(mut guard) = OFFLINE_MODE.lock() { *guard = offline; } }

pub fn get_change_log_repo() -> FFIResult<Arc<dyn ChangeLogRepository>> {
    CHANGE_LOG_REPO.lock().map_err(|_| FFIError::internal("CHANGE_LOG_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ChangeLogRepository not initialized".to_string()))
}
pub fn get_tombstone_repo() -> FFIResult<Arc<dyn TombstoneRepository>> {
    TOMBSTONE_REPO.lock().map_err(|_| FFIError::internal("TOMBSTONE_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("TombstoneRepository not initialized".to_string()))
}
pub fn get_dependency_checker() -> FFIResult<Arc<dyn DependencyChecker>> {
    DEPENDENCY_CHECKER.lock().map_err(|_| FFIError::internal("DEPENDENCY_CHECKER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DependencyChecker not initialized".to_string()))
}
pub fn get_deletion_manager() -> FFIResult<Arc<PendingDeletionManager>> {
    DELETION_MANAGER.lock().map_err(|_| FFIError::internal("DELETION_MANAGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("PendingDeletionManager not initialized".to_string()))
}
pub fn get_auth_service() -> FFIResult<Arc<AuthService>> {
    AUTH_SERVICE.lock().map_err(|_| FFIError::internal("AUTH_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("AuthService not initialized".to_string()))
}
pub fn get_file_storage_service() -> FFIResult<Arc<dyn FileStorageService>> {
    FILE_STORAGE_SERVICE.lock().map_err(|_| FFIError::internal("FILE_STORAGE_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("FileStorageService not initialized".to_string()))
}
pub fn get_compression_repo() -> FFIResult<Arc<dyn CompressionRepository>> {
    COMPRESSION_REPO.lock().map_err(|_| FFIError::internal("COMPRESSION_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("CompressionRepository not initialized".to_string()))
}
pub fn get_compression_service() -> FFIResult<Arc<dyn CompressionService>> {
    COMPRESSION_SERVICE.lock().map_err(|_| FFIError::internal("COMPRESSION_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("CompressionService not initialized".to_string()))
}
pub fn get_compression_manager() -> FFIResult<Arc<dyn CompressionManager>> {
    COMPRESSION_MANAGER.lock().map_err(|_| FFIError::internal("COMPRESSION_MANAGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("CompressionManager not initialized".to_string()))
}


// User
pub fn get_user_repo() -> FFIResult<Arc<dyn UserRepository>> {
    USER_REPO.lock().map_err(|_| FFIError::internal("USER_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("UserRepository not initialized".to_string()))
}
pub fn get_user_service() -> FFIResult<Arc<UserService>> {
    USER_SERVICE.lock().map_err(|_| FFIError::internal("USER_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("UserService not initialized".to_string()))
}
pub fn get_user_delete_service() -> FFIResult<Arc<dyn DeleteService<User>>> {
    DELETE_SERVICE_USER.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_USER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("User DeleteService not initialized".to_string()))
}
pub fn get_user_entity_merger() -> FFIResult<Arc<UserEntityMerger>> {
    USER_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("USER_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("UserEntityMerger not initialized".to_string()))
}

// Donor
pub fn get_donor_repo() -> FFIResult<Arc<dyn DonorRepository>> {
    DONOR_REPO.lock().map_err(|_| FFIError::internal("DONOR_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DonorRepository not initialized".to_string()))
}
pub fn get_donor_delete_service() -> FFIResult<Arc<dyn DeleteService<Donor>>> {
    DELETE_SERVICE_DONOR.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_DONOR lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Donor DeleteService not initialized".to_string()))
}
pub fn get_donor_entity_merger() -> FFIResult<Arc<DonorEntityMerger>> {
    DONOR_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("DONOR_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DonorEntityMerger not initialized".to_string()))
}

// Document
pub fn get_media_document_repo() -> FFIResult<Arc<dyn MediaDocumentRepository>> {
    MEDIA_DOCUMENT_REPO.lock().map_err(|_| FFIError::internal("MEDIA_DOCUMENT_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("MediaDocumentRepository not initialized".to_string()))
}
pub fn get_media_document_delete_service() -> FFIResult<Arc<dyn DeleteService<MediaDocument>>> {
    DELETE_SERVICE_MEDIA_DOCUMENT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_MEDIA_DOCUMENT lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("MediaDocument DeleteService not initialized".to_string()))
}
pub fn get_media_document_entity_merger() -> FFIResult<Arc<DocumentEntityMerger>> {
    MEDIA_DOCUMENT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("MEDIA_DOCUMENT_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("MediaDocumentEntityMerger not initialized".to_string()))
}
pub fn get_document_type_repo() -> FFIResult<Arc<dyn DocumentTypeRepository>> {
    DOCUMENT_TYPE_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_TYPE_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentTypeRepository not initialized".to_string()))
}
pub fn get_document_type_delete_service() -> FFIResult<Arc<dyn DeleteService<DocumentType>>> {
    DELETE_SERVICE_DOCUMENT_TYPE.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_DOCUMENT_TYPE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentType DeleteService not initialized".to_string()))
}
pub fn get_document_type_entity_merger() -> FFIResult<Arc<DocumentTypeEntityMerger>> {
    DOCUMENT_TYPE_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("DOCUMENT_TYPE_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentTypeEntityMerger not initialized".to_string()))
}


// Project
pub fn get_project_repo() -> FFIResult<Arc<dyn ProjectRepository>> {
    PROJECT_REPO.lock().map_err(|_| FFIError::internal("PROJECT_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectRepository not initialized".to_string()))
}
pub fn get_project_delete_service() -> FFIResult<Arc<dyn DeleteService<Project>>> {
    DELETE_SERVICE_PROJECT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PROJECT lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Project DeleteService not initialized".to_string()))
}
pub fn get_project_entity_merger() -> FFIResult<Arc<ProjectEntityMerger>> {
    PROJECT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PROJECT_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectEntityMerger not initialized".to_string()))
}

// Activity
pub fn get_activity_repo() -> FFIResult<Arc<dyn ActivityRepository>> {
    ACTIVITY_REPO.lock().map_err(|_| FFIError::internal("ACTIVITY_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ActivityRepository not initialized".to_string()))
}
pub fn get_activity_delete_service() -> FFIResult<Arc<dyn DeleteService<Activity>>> {
    DELETE_SERVICE_ACTIVITY.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_ACTIVITY lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Activity DeleteService not initialized".to_string()))
}
pub fn get_activity_entity_merger() -> FFIResult<Arc<ActivityEntityMerger>> {
    ACTIVITY_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("ACTIVITY_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ActivityEntityMerger not initialized".to_string()))
}

// ProjectFunding
pub fn get_project_funding_repo() -> FFIResult<Arc<dyn ProjectFundingRepository>> {
    PROJECT_FUNDING_REPO.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectFundingRepository not initialized".to_string()))
}
pub fn get_project_funding_delete_service() -> FFIResult<Arc<dyn DeleteService<ProjectFunding>>> {
    DELETE_SERVICE_PROJECT_FUNDING.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PROJECT_FUNDING lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectFunding DeleteService not initialized".to_string()))
}
pub fn get_project_funding_entity_merger() -> FFIResult<Arc<FundingEntityMerger>> {
    PROJECT_FUNDING_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectFundingEntityMerger not initialized".to_string()))
}

// Workshop
pub fn get_workshop_repo() -> FFIResult<Arc<dyn WorkshopRepository>> {
    WORKSHOP_REPO.lock().map_err(|_| FFIError::internal("WORKSHOP_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("WorkshopRepository not initialized".to_string()))
}
pub fn get_workshop_delete_service() -> FFIResult<Arc<dyn DeleteService<Workshop>>> {
    DELETE_SERVICE_WORKSHOP.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_WORKSHOP lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Workshop DeleteService not initialized".to_string()))
}
pub fn get_workshop_entity_merger() -> FFIResult<Arc<WorkshopEntityMerger>> {
    WORKSHOP_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("WORKSHOP_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("WorkshopEntityMerger not initialized".to_string()))
}

// Livelihood
pub fn get_livelihood_repo() -> FFIResult<Arc<dyn LivehoodRepository>> {
    LIVELIHOOD_REPO.lock().map_err(|_| FFIError::internal("LIVELIHOOD_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("LivelihoodRepository not initialized".to_string()))
}
pub fn get_livelihood_delete_service() -> FFIResult<Arc<dyn DeleteService<Livelihood>>> {
    DELETE_SERVICE_LIVELIHOOD.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_LIVELIHOOD lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Livelihood DeleteService not initialized".to_string()))
}
pub fn get_livelihood_entity_merger() -> FFIResult<Arc<LivelihoodEntityMerger>> {
    LIVELIHOOD_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("LIVELIHOOD_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("LivelihoodEntityMerger not initialized".to_string()))
}

// SubsequentGrant
pub fn get_subsequent_grant_repo() -> FFIResult<Arc<dyn SubsequentGrantRepository>> {
    SUBSEQUENT_GRANT_REPO.lock().map_err(|_| FFIError::internal("SUBSEQUENT_GRANT_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("SubsequentGrantRepository not initialized".to_string()))
}
pub fn get_subsequent_grant_delete_service() -> FFIResult<Arc<dyn DeleteService<SubsequentGrant>>> {
    DELETE_SERVICE_SUBSEQUENT_GRANT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_SUBSEQUENT_GRANT lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("SubsequentGrant DeleteService not initialized".to_string()))
}
pub fn get_subsequent_grant_entity_merger() -> FFIResult<Arc<SubsequentGrantEntityMerger>> {
    SUBSEQUENT_GRANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("SUBSEQUENT_GRANT_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("SubsequentGrantEntityMerger not initialized".to_string()))
}

// Participant
pub fn get_participant_repo() -> FFIResult<Arc<dyn ParticipantRepository>> {
    PARTICIPANT_REPO.lock().map_err(|_| FFIError::internal("PARTICIPANT_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ParticipantRepository not initialized".to_string()))
}
pub fn get_participant_delete_service() -> FFIResult<Arc<dyn DeleteService<Participant>>> {
    DELETE_SERVICE_PARTICIPANT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PARTICIPANT lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("Participant DeleteService not initialized".to_string()))
}
pub fn get_participant_entity_merger() -> FFIResult<Arc<ParticipantEntityMerger>> {
    PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PARTICIPANT_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ParticipantEntityMerger not initialized".to_string()))
}

// StrategicGoal
pub fn get_strategic_goal_repo() -> FFIResult<Arc<dyn StrategicGoalRepository>> {
    STRATEGIC_GOAL_REPO.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("StrategicGoalRepository not initialized".to_string()))
}
pub fn get_strategic_goal_delete_service() -> FFIResult<Arc<dyn DeleteService<StrategicGoal>>> {
    DELETE_SERVICE_STRATEGIC_GOAL.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_STRATEGIC_GOAL lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("StrategicGoal DeleteService not initialized".to_string()))
}
pub fn get_strategic_goal_entity_merger() -> FFIResult<Arc<StrategicGoalEntityMerger>> {
    STRATEGIC_GOAL_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("StrategicGoalEntityMerger not initialized".to_string()))
}

// WorkshopParticipant
pub fn get_workshop_participant_repo() -> FFIResult<Arc<dyn WorkshopParticipantRepository>> {
    WORKSHOP_PARTICIPANT_REPO.lock().map_err(|_| FFIError::internal("WORKSHOP_PARTICIPANT_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("WorkshopParticipantRepository not initialized".to_string()))
}
pub fn get_workshop_participant_entity_merger() -> FFIResult<Arc<WorkshopParticipantEntityMerger>> {
    WORKSHOP_PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("WORKSHOP_PARTICIPANT_ENTITY_MERGER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("WorkshopParticipantEntityMerger not initialized".to_string()))
}

// Cloud storage
pub fn get_cloud_storage_service() -> FFIResult<Arc<dyn CloudStorageService>> {
    CLOUD_STORAGE_SERVICE.lock().map_err(|_| FFIError::internal("CLOUD_STORAGE_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("CloudStorageService not initialized".to_string()))
}

// ... after compression_manager getter functions, add new getters ...
pub fn get_document_service() -> FFIResult<Arc<dyn DocumentService>> {
    DOCUMENT_SERVICE.lock().map_err(|_| FFIError::internal("DOCUMENT_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentService not initialized".to_string()))
}

pub fn get_project_service() -> FFIResult<Arc<dyn ProjectService>> {
    PROJECT_SERVICE.lock().map_err(|_| FFIError::internal("PROJECT_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectService not initialized".to_string()))
}

pub fn get_activity_service() -> FFIResult<Arc<dyn ActivityService>> {
    ACTIVITY_SERVICE.lock().map_err(|_| FFIError::internal("ACTIVITY_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ActivityService not initialized".to_string()))
}

pub fn get_donor_service() -> FFIResult<Arc<dyn DonorService>> {
    DONOR_SERVICE.lock().map_err(|_| FFIError::internal("DONOR_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DonorService not initialized".to_string()))
}

pub fn get_project_funding_service() -> FFIResult<Arc<dyn ProjectFundingService>> {
    PROJECT_FUNDING_SERVICE.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ProjectFundingService not initialized".to_string()))
}

pub fn get_participant_service() -> FFIResult<Arc<dyn ParticipantService>> {
    PARTICIPANT_SERVICE.lock().map_err(|_| FFIError::internal("PARTICIPANT_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("ParticipantService not initialized".to_string()))
}

pub fn get_workshop_service() -> FFIResult<Arc<dyn WorkshopService>> {
    WORKSHOP_SERVICE.lock().map_err(|_| FFIError::internal("WORKSHOP_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("WorkshopService not initialized".to_string()))
}

pub fn get_livelihood_service() -> FFIResult<Arc<dyn LivehoodService>> {
    LIVELIHOOD_SERVICE.lock().map_err(|_| FFIError::internal("LIVELIHOOD_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("LivelihoodService not initialized".to_string()))
}

pub fn get_strategic_goal_service() -> FFIResult<Arc<dyn StrategicGoalService>> {
    STRATEGIC_GOAL_SERVICE.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("StrategicGoalService not initialized".to_string()))
}

pub fn get_sync_repo() -> FFIResult<Arc<dyn SyncRepository>> {
    SYNC_REPO.lock().map_err(|_| FFIError::internal("SYNC_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("SyncRepository not initialized".to_string()))
}

pub fn get_entity_merger() -> FFIResult<Arc<EntityMerger>> {
    ENTITY_MERGER_GLOBAL.lock().map_err(|_| FFIError::internal("ENTITY_MERGER_GLOBAL lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("EntityMerger not initialized".to_string()))
}

pub fn get_sync_service() -> FFIResult<Arc<dyn SyncService>> {
    SYNC_SERVICE_GLOBAL.lock().map_err(|_| FFIError::internal("SYNC_SERVICE_GLOBAL lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("SyncService not initialized".to_string()))
}
