use crate::auth::AuthService;
use crate::domains::user::{UserRepository, UserService, User};
use crate::domains::user::repository::SqliteUserRepository;
use crate::domains::sync::repository::{ChangeLogRepository, SqliteChangeLogRepository, TombstoneRepository, SqliteTombstoneRepository, SyncRepository, SqliteSyncRepository};
use crate::domains::sync::service::{SyncService, SyncServiceImpl};
use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use lazy_static::lazy_static;
use chrono;
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
use crate::domains::document::initialization; // Add document initialization import
use crate::domains::compression::repository::{CompressionRepository, SqliteCompressionRepository};
use crate::domains::compression::service::{CompressionService, CompressionServiceImpl};
use crate::domains::compression::manager::{CompressionManager, CompressionManagerImpl, StubCompressionManager};
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
use crate::domains::funding::service::{ProjectFundingService, ProjectFundingServiceImpl};

// Entity Mergers
use crate::domains::sync::entity_merger::{
    UserEntityMerger, DonorEntityMerger, ProjectEntityMerger, ActivityEntityMerger,
    FundingEntityMerger, WorkshopEntityMerger, LivelihoodEntityMerger,
    SubsequentGrantEntityMerger, DocumentEntityMerger, ParticipantEntityMerger,
    StrategicGoalEntityMerger, DocumentTypeEntityMerger, WorkshopParticipantEntityMerger, EntityMerger, DomainEntityMerger
};

// After ActivityService import line
use crate::domains::donor::service::{DonorService, DonorServiceImpl};

use crate::domains::participant::service::{ParticipantService, ParticipantServiceImpl};


// New Domain Imports (assuming paths and types)
use crate::domains::workshop::service::{WorkshopService, WorkshopServiceImpl};
use crate::domains::livelihood::service::{LivehoodService, LivehoodServiceImpl};
use crate::domains::strategic_goal::service::{StrategicGoalService, StrategicGoalServiceImpl};

// Global state definitions
lazy_static! {
    static ref INIT_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
    static ref INITIALIZED: AtomicBool = AtomicBool::new(false);
    
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
    static ref COMPRESSION_WORKER_SENDER: Mutex<Option<tokio::sync::mpsc::Sender<crate::domains::compression::worker::CompressionWorkerMessage>>> = Mutex::new(None);
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

    // Document Version and Access Log Repositories
    static ref DOCUMENT_VERSION_REPO: Mutex<Option<Arc<dyn DocumentVersionRepository>>> = Mutex::new(None);
    static ref DOCUMENT_ACCESS_LOG_REPO: Mutex<Option<Arc<dyn DocumentAccessLogRepository>>> = Mutex::new(None);

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
    
    // Funding Service (alias for ProjectFundingService)
    static ref FUNDING_SERVICE: Mutex<Option<Arc<dyn ProjectFundingService>>> = Mutex::new(None);

    // ---- Sync / Merge ----
    static ref SYNC_REPO: Mutex<Option<Arc<dyn SyncRepository>>> = Mutex::new(None);
    static ref ENTITY_MERGER_GLOBAL: Mutex<Option<Arc<EntityMerger>>> = Mutex::new(None);
    static ref SYNC_SERVICE_GLOBAL: Mutex<Option<Arc<dyn SyncService>>> = Mutex::new(None);

}

// --- Getter Functions (moved before initialization to avoid ordering issues) ---

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
pub fn get_compression_worker_sender() -> FFIResult<tokio::sync::mpsc::Sender<crate::domains::compression::worker::CompressionWorkerMessage>> {
    COMPRESSION_WORKER_SENDER.lock().map_err(|_| FFIError::internal("COMPRESSION_WORKER_SENDER lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("CompressionWorkerSender not initialized".to_string()))
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

// Document Version Repository
pub fn get_document_version_repo() -> FFIResult<Arc<dyn DocumentVersionRepository>> {
    DOCUMENT_VERSION_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_VERSION_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentVersionRepository not initialized".to_string()))
}

// Document Access Log Repository  
pub fn get_document_access_log_repo() -> FFIResult<Arc<dyn DocumentAccessLogRepository>> {
    DOCUMENT_ACCESS_LOG_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_ACCESS_LOG_REPO lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("DocumentAccessLogRepository not initialized".to_string()))
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

// Services
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

pub fn get_funding_service() -> FFIResult<Arc<dyn ProjectFundingService>> {
    FUNDING_SERVICE.lock().map_err(|_| FFIError::internal("FUNDING_SERVICE lock poisoned".to_string()))?.clone().ok_or_else(|| FFIError::internal("FundingService not initialized".to_string()))
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

/// Initialize global services
pub async fn initialize(
    db_url: &str,
    device_id_str: &str,
    offline_mode_flag: bool,
    jwt_secret: &str
) -> FFIResult<()> {
    // Acquire the async mutex to ensure single initialization
    let _guard = INIT_MUTEX.lock().await;
    
    // Check if already initialized
    if INITIALIZED.load(Ordering::Acquire) {
        return Ok(());
    }
    
    // Perform initialization
    let result = initialize_internal(db_url, device_id_str, offline_mode_flag, jwt_secret).await;
    
    // Mark as initialized only if successful
    if result.is_ok() {
        INITIALIZED.store(true, Ordering::Release);
    }
    
    result
}

async fn initialize_internal(
    db_url: &str,
    device_id_str: &str,
    offline_mode_flag: bool,
    jwt_secret: &str
) -> FFIResult<()> {
    // Initialize logging first
    if std::env::var("RUST_LOG").is_err() {
        #[cfg(debug_assertions)]
        std::env::set_var("RUST_LOG", "debug");
        #[cfg(not(debug_assertions))]
        std::env::set_var("RUST_LOG", "info");
    }
    
    // Initialize env_logger if not already initialized
    let _ = env_logger::try_init();
    
    log::info!("Starting internal initialization");
    log::debug!("Database URL: {}", db_url);
    log::debug!("Device ID: {}", device_id_str);
    log::debug!("Offline mode: {}", offline_mode_flag);
    
    // Initialize JWT
    log::debug!("Initializing JWT");
    crate::auth::jwt::initialize(jwt_secret);
    log::debug!("JWT initialized");

    // Create async database connection
    println!("🗄️ [GLOBALS] Creating database connection...");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await
        .map_err(|e| {
            println!("❌ [GLOBALS] Database connection failed: {}", e);
            FFIError::internal(format!("Database connection failed: {}", e))
        })?;

    println!("✅ [GLOBALS] Database connection established");

    // Store the pool first
    println!("💾 [GLOBALS] Storing database pool...");
    *DB_POOL.lock().map_err(|_| {
        println!("❌ [GLOBALS] DB_POOL lock poisoned");
        FFIError::internal("DB_POOL lock poisoned".to_string())
    })? = Some(pool.clone());
    println!("✅ [GLOBALS] Database pool stored");

    // Run database migrations BEFORE creating services
    println!("🔄 [GLOBALS] Running database initialization with consolidated schema...");
    // Since we now use a single consolidated schema, we call our custom initializer.
    // This approach is not compatible with `cargo sqlx prepare` in the traditional sense,
    // but it ensures the entire schema is applied from the single file.
    crate::db_migration::initialize_database().await
        .map_err(|e| {
            println!("❌ [GLOBALS] Database initialization failed: {}", e);
            e
        })?;
    println!("✅ [GLOBALS] Database initialization completed");

    // Ensure critical lookup data exists
    println!("🔧 [GLOBALS] Ensuring critical lookup data...");
    ensure_status_types_initialized(&pool).await
        .map_err(|e| {
            println!("❌ [GLOBALS] Status types initialization failed: {}", e);
            e
        })?;
    println!("✅ [GLOBALS] Status types verified");
    
    // Initialize document types IMMEDIATELY after status types and BEFORE starting any background workers
    // This prevents database concurrency issues with compression worker polling
    println!("📄 [GLOBALS] Ensuring document types initialized...");
    ensure_document_types_initialized(&pool).await
        .map_err(|e| {
            println!("❌ [GLOBALS] Document types initialization failed: {}", e);
            e
        })?;
    println!("✅ [GLOBALS] Document types verified");
    
    // Initialize document types after repositories are created
    // Note: We'll do this after the document_type_repo is created below

    // Store device ID and offline mode
    *DEVICE_ID.lock().map_err(|_| FFIError::internal("DEVICE_ID lock poisoned".to_string()))? = Some(device_id_str.to_string());
    *OFFLINE_MODE.lock().map_err(|_| FFIError::internal("OFFLINE_MODE lock poisoned".to_string()))? = offline_mode_flag;

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
    // For iOS, use a proper storage path
    let storage_path = if cfg!(target_os = "ios") {
        println!("🔍 [GLOBALS] Detected iOS target, checking IOS_DOCUMENTS_DIR...");
        // This will be replaced by the iOS app with the actual documents directory
        match std::env::var("IOS_DOCUMENTS_DIR") {
            Ok(path) => {
                println!("✅ [GLOBALS] IOS_DOCUMENTS_DIR found: '{}'", path);
                path
            },
            Err(e) => {
                println!("❌ [GLOBALS] IOS_DOCUMENTS_DIR not found: {:?}, using fallback", e);
                "./storage".to_string()
            }
        }
    } else {
        println!("🔍 [GLOBALS] Not iOS target, but checking IOS_DOCUMENTS_DIR anyway...");
        // Even if not iOS target, check if IOS_DOCUMENTS_DIR is set (for simulator builds)
        match std::env::var("IOS_DOCUMENTS_DIR") {
            Ok(path) => {
                println!("✅ [GLOBALS] IOS_DOCUMENTS_DIR found even on non-iOS target: '{}'", path);
                path
            },
            Err(_) => {
                println!("📁 [GLOBALS] Using default ./storage");
        "./storage".to_string()
            }
        }
    };
    
    println!("🗂️ [GLOBALS] Initializing file storage with path: '{}'", storage_path);

    let file_storage_service: Arc<dyn FileStorageService> = Arc::new(
        LocalFileStorageService::new(&storage_path).map_err(|e| FFIError::internal(format!("File storage init failed: {}", e)))?
    );
    let compression_repo: Arc<dyn CompressionRepository> = Arc::new(SqliteCompressionRepository::new(pool.clone()));

    // Cloud storage service (API-backed)
    let cloud_storage_service: Arc<dyn CloudStorageService> = Arc::new(ApiCloudStorageService::new(
        "https://example.com/api",
        "./storage",
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

    // User Service and entity merger
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

    // SubsequentGrant
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

    // WorkshopParticipant
    struct WorkshopParticipantRepoAdapter(Arc<dyn WorkshopParticipantRepository>);
    #[async_trait::async_trait]
    impl FindById<WorkshopParticipant> for WorkshopParticipantRepoAdapter {
        async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<WorkshopParticipant> {
            self.0.find_by_id(id).await
        }
    }
    #[async_trait::async_trait]
    impl HardDeletable for WorkshopParticipantRepoAdapter {
        fn entity_name(&self) -> &'static str { "workshop_participants" }
        async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> {
            let mut tx = get_db_pool().map_err(|e| crate::errors::DomainError::Internal(format!("Failed to get DB pool for hard_delete: {}", e)))?.begin().await.map_err(crate::errors::DbError::from)?;
            let result = self.0.hard_delete_link_by_id_with_tx(id, auth, &mut tx).await;
            if result.is_ok() {
                tx.commit().await.map_err(crate::errors::DbError::from)?;
            } else {
                let _ = tx.rollback().await;
            }
            result
        }
        async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
            self.0.hard_delete_link_by_id_with_tx(id, auth, tx).await
        }
    }
    #[async_trait::async_trait]
    impl SoftDeletable for WorkshopParticipantRepoAdapter {
        async fn soft_delete(&self, _id: Uuid, _auth: &AuthContext) -> crate::errors::DomainResult<()> {
            Err(crate::errors::DomainError::Internal("Soft delete for WorkshopParticipant by its own single ID is not supported. Use domain-specific service.".to_string()))
        }
        async fn soft_delete_with_tx(&self, _id: Uuid, _auth: &AuthContext, _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
            Err(crate::errors::DomainError::Internal("Soft delete for WorkshopParticipant by its own single ID is not supported. Use domain-specific service.".to_string()))
        }
    }
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
    *DELETE_SERVICE_WORKSHOP_PARTICIPANT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_WORKSHOP_PARTICIPANT lock poisoned".to_string()))? = Some(delete_service_workshop_participant.clone());
    
    // Create the merger
    let workshop_participant_entity_merger = Arc::new(WorkshopParticipantEntityMerger::new(workshop_participant_repo.clone(), pool.clone(), delete_service_workshop_participant.clone()));
    
    *WORKSHOP_PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("WORKSHOP_PARTICIPANT_ENTITY_MERGER lock poisoned".to_string()))? = Some(workshop_participant_entity_merger.clone());

    let media_document_entity_merger = Arc::new(DocumentEntityMerger::new(
        media_document_repo.clone(),
        pool.clone(),
        delete_service_media_document.clone(),
    ));

    let document_type_entity_merger = Arc::new(DocumentTypeEntityMerger::new(document_type_repo.clone(), pool.clone(), delete_service_document_type.clone()));

    // Create other entity mergers
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
    
    // Use CompressionWorker instead of the problematic CompressionManager
    // This avoids the unsafe global pool access that was causing crashes
    let comp_pool = pool.clone();
    let comp_service = compression_service.clone();
    let comp_repo = compression_repo.clone();
    let worker = crate::domains::compression::worker::CompressionWorker::new(
        comp_service,
        comp_repo,
        comp_pool,
        Some(5_000), // poll interval ms
        Some(2),     // max_concurrent_jobs
    );
    let worker_sender = worker.get_message_sender();
    
    // Store the worker sender globally for FFI access
    *COMPRESSION_WORKER_SENDER.lock().unwrap() = Some(worker_sender);
    
    tokio::spawn(async move {
        let (handle, _shutdown_tx) = worker.start();
        if let Err(e) = handle.await {
            log::error!("CompressionWorker exited: {:?}", e);
        }
    });
    
    // Create a stub manager for FFI compatibility
    let compression_manager: Arc<dyn CompressionManager> = Arc::new(StubCompressionManager::new(
        compression_service.clone(),
        compression_repo.clone(),
    ));

    // FileDeletionWorker
    let fd_pool = pool.clone();
    let fd_storage = file_storage_service.clone();
    tokio::spawn(async move {
        let worker = FileDeletionWorker::new(fd_pool, fd_storage);
        if let Err(e) = worker.start().await {
            log::error!("FileDeletionWorker exited: {:?}", e);
        }
    });

    // Additional Document repositories
    let document_version_repo: Arc<dyn DocumentVersionRepository> = Arc::new(SqliteDocumentVersionRepository::new(pool.clone(), change_log_repo.clone()));
    let document_access_log_repo: Arc<dyn DocumentAccessLogRepository> = Arc::new(SqliteDocumentAccessLogRepository::new(pool.clone()));

    // Document Service
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
        // ADDED: Additional repositories for enrichment
        user_repo.clone(),
        activity_repo.clone(),
        workshop_repo.clone(),
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
        user_repo.clone(),
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
        user_repo.clone(),
        deletion_manager.clone(),
        delete_service_strategic_goal.clone(), // Pass the properly configured delete service
    ));

    // Funding Service (alias)
    let funding_service: Arc<dyn ProjectFundingService> = Arc::new(ProjectFundingServiceImpl::new(
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

    // Store all components (DB_POOL already stored above after connection)
    *CHANGE_LOG_REPO.lock().map_err(|_| FFIError::internal("CHANGE_LOG_REPO lock poisoned".to_string()))? = Some(change_log_repo.clone());
    *TOMBSTONE_REPO.lock().map_err(|_| FFIError::internal("TOMBSTONE_REPO lock poisoned".to_string()))? = Some(tombstone_repo.clone());
    *DEPENDENCY_CHECKER.lock().map_err(|_| FFIError::internal("DEPENDENCY_CHECKER lock poisoned".to_string()))? = Some(dependency_checker.clone());
    *DELETION_MANAGER.lock().map_err(|_| FFIError::internal("DELETION_MANAGER lock poisoned".to_string()))? = Some(deletion_manager.clone());
    *AUTH_SERVICE.lock().map_err(|_| FFIError::internal("AUTH_SERVICE lock poisoned".to_string()))? = Some(auth_service.clone());
    *FILE_STORAGE_SERVICE.lock().map_err(|_| FFIError::internal("FILE_STORAGE_SERVICE lock poisoned".to_string()))? = Some(file_storage_service.clone());
    *COMPRESSION_REPO.lock().map_err(|_| FFIError::internal("COMPRESSION_REPO lock poisoned".to_string()))? = Some(compression_repo.clone());
    *COMPRESSION_SERVICE.lock().map_err(|_| FFIError::internal("COMPRESSION_SERVICE lock poisoned".to_string()))? = Some(compression_service.clone());
    *COMPRESSION_MANAGER.lock().map_err(|_| FFIError::internal("COMPRESSION_MANAGER lock poisoned".to_string()))? = Some(compression_manager.clone());
    *CLOUD_STORAGE_SERVICE.lock().map_err(|_| FFIError::internal("CLOUD_STORAGE_SERVICE lock poisoned".to_string()))? = Some(cloud_storage_service.clone());

    *USER_REPO.lock().map_err(|_| FFIError::internal("USER_REPO lock poisoned".to_string()))? = Some(user_repo);
    *USER_SERVICE.lock().map_err(|_| FFIError::internal("USER_SERVICE lock poisoned".to_string()))? = Some(user_service);
    *DELETE_SERVICE_USER.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_USER lock poisoned".to_string()))? = Some(delete_service_user);
    *USER_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("USER_ENTITY_MERGER lock poisoned".to_string()))? = Some(user_entity_merger.clone());

    *DONOR_REPO.lock().map_err(|_| FFIError::internal("DONOR_REPO lock poisoned".to_string()))? = Some(donor_repo);
    *DELETE_SERVICE_DONOR.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_DONOR lock poisoned".to_string()))? = Some(delete_service_donor);
    *DONOR_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("DONOR_ENTITY_MERGER lock poisoned".to_string()))? = Some(donor_entity_merger.clone());

    *MEDIA_DOCUMENT_REPO.lock().map_err(|_| FFIError::internal("MEDIA_DOCUMENT_REPO lock poisoned".to_string()))? = Some(media_document_repo);
    *DELETE_SERVICE_MEDIA_DOCUMENT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_MEDIA_DOCUMENT lock poisoned".to_string()))? = Some(delete_service_media_document);
    *MEDIA_DOCUMENT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("MEDIA_DOCUMENT_ENTITY_MERGER lock poisoned".to_string()))? = Some(media_document_entity_merger.clone());

    *DOCUMENT_TYPE_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_TYPE_REPO lock poisoned".to_string()))? = Some(document_type_repo);
    *DELETE_SERVICE_DOCUMENT_TYPE.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_DOCUMENT_TYPE lock poisoned".to_string()))? = Some(delete_service_document_type);
    *DOCUMENT_TYPE_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("DOCUMENT_TYPE_ENTITY_MERGER lock poisoned".to_string()))? = Some(document_type_entity_merger.clone());

    *PROJECT_REPO.lock().map_err(|_| FFIError::internal("PROJECT_REPO lock poisoned".to_string()))? = Some(project_repo);
    *DELETE_SERVICE_PROJECT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PROJECT lock poisoned".to_string()))? = Some(delete_service_project);
    *PROJECT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PROJECT_ENTITY_MERGER lock poisoned".to_string()))? = Some(project_entity_merger.clone());

    *ACTIVITY_REPO.lock().map_err(|_| FFIError::internal("ACTIVITY_REPO lock poisoned".to_string()))? = Some(activity_repo);
    *DELETE_SERVICE_ACTIVITY.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_ACTIVITY lock poisoned".to_string()))? = Some(delete_service_activity);
    *ACTIVITY_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("ACTIVITY_ENTITY_MERGER lock poisoned".to_string()))? = Some(activity_entity_merger.clone());

    *PROJECT_FUNDING_REPO.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_REPO lock poisoned".to_string()))? = Some(project_funding_repo);
    *DELETE_SERVICE_PROJECT_FUNDING.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PROJECT_FUNDING lock poisoned".to_string()))? = Some(delete_service_project_funding);
    *PROJECT_FUNDING_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_ENTITY_MERGER lock poisoned".to_string()))? = Some(project_funding_entity_merger.clone());

    *WORKSHOP_REPO.lock().map_err(|_| FFIError::internal("WORKSHOP_REPO lock poisoned".to_string()))? = Some(workshop_repo);
    *DELETE_SERVICE_WORKSHOP.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_WORKSHOP lock poisoned".to_string()))? = Some(delete_service_workshop);
    *WORKSHOP_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("WORKSHOP_ENTITY_MERGER lock poisoned".to_string()))? = Some(workshop_entity_merger.clone());

    *LIVELIHOOD_REPO.lock().map_err(|_| FFIError::internal("LIVELIHOOD_REPO lock poisoned".to_string()))? = Some(livelihood_repo);
    *DELETE_SERVICE_LIVELIHOOD.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_LIVELIHOOD lock poisoned".to_string()))? = Some(delete_service_livelihood);
    *LIVELIHOOD_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("LIVELIHOOD_ENTITY_MERGER lock poisoned".to_string()))? = Some(livelihood_entity_merger.clone());

    *SUBSEQUENT_GRANT_REPO.lock().map_err(|_| FFIError::internal("SUBSEQUENT_GRANT_REPO lock poisoned".to_string()))? = Some(subsequent_grant_repo);
    *DELETE_SERVICE_SUBSEQUENT_GRANT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_SUBSEQUENT_GRANT lock poisoned".to_string()))? = Some(delete_service_subsequent_grant);
    *SUBSEQUENT_GRANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("SUBSEQUENT_GRANT_ENTITY_MERGER lock poisoned".to_string()))? = Some(subsequent_grant_entity_merger.clone());

    *PARTICIPANT_REPO.lock().map_err(|_| FFIError::internal("PARTICIPANT_REPO lock poisoned".to_string()))? = Some(participant_repo);
    *DELETE_SERVICE_PARTICIPANT.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_PARTICIPANT lock poisoned".to_string()))? = Some(delete_service_participant);
    *PARTICIPANT_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("PARTICIPANT_ENTITY_MERGER lock poisoned".to_string()))? = Some(participant_entity_merger.clone());

    *STRATEGIC_GOAL_REPO.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_REPO lock poisoned".to_string()))? = Some(strategic_goal_repo);
    *DELETE_SERVICE_STRATEGIC_GOAL.lock().map_err(|_| FFIError::internal("DELETE_SERVICE_STRATEGIC_GOAL lock poisoned".to_string()))? = Some(delete_service_strategic_goal);
    *STRATEGIC_GOAL_ENTITY_MERGER.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_ENTITY_MERGER lock poisoned".to_string()))? = Some(strategic_goal_entity_merger.clone());

    *WORKSHOP_PARTICIPANT_REPO.lock().map_err(|_| FFIError::internal("WORKSHOP_PARTICIPANT_REPO lock poisoned".to_string()))? = Some(workshop_participant_repo);

    *DOCUMENT_VERSION_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_VERSION_REPO lock poisoned".to_string()))? = Some(document_version_repo.clone());
    *DOCUMENT_ACCESS_LOG_REPO.lock().map_err(|_| FFIError::internal("DOCUMENT_ACCESS_LOG_REPO lock poisoned".to_string()))? = Some(document_access_log_repo.clone());

    *DOCUMENT_SERVICE.lock().map_err(|_| FFIError::internal("DOCUMENT_SERVICE lock poisoned".to_string()))? = Some(document_service);
    *PROJECT_SERVICE.lock().map_err(|_| FFIError::internal("PROJECT_SERVICE lock poisoned".to_string()))? = Some(project_service);
    *ACTIVITY_SERVICE.lock().map_err(|_| FFIError::internal("ACTIVITY_SERVICE lock poisoned".to_string()))? = Some(activity_service_singleton);

    *DONOR_SERVICE.lock().map_err(|_| FFIError::internal("DONOR_SERVICE lock poisoned".to_string()))? = Some(donor_service);
    *PROJECT_FUNDING_SERVICE.lock().map_err(|_| FFIError::internal("PROJECT_FUNDING_SERVICE lock poisoned".to_string()))? = Some(project_funding_service);
    *PARTICIPANT_SERVICE.lock().map_err(|_| FFIError::internal("PARTICIPANT_SERVICE lock poisoned".to_string()))? = Some(participant_service);

    *WORKSHOP_SERVICE.lock().map_err(|_| FFIError::internal("WORKSHOP_SERVICE lock poisoned".to_string()))? = Some(workshop_service);
    *LIVELIHOOD_SERVICE.lock().map_err(|_| FFIError::internal("LIVELIHOOD_SERVICE lock poisoned".to_string()))? = Some(livelihood_service);
    *STRATEGIC_GOAL_SERVICE.lock().map_err(|_| FFIError::internal("STRATEGIC_GOAL_SERVICE lock poisoned".to_string()))? = Some(strategic_goal_service);
    *FUNDING_SERVICE.lock().map_err(|_| FFIError::internal("FUNDING_SERVICE lock poisoned".to_string()))? = Some(funding_service);

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

    *SYNC_REPO.lock().map_err(|_| FFIError::internal("SYNC_REPO lock poisoned".to_string()))? = Some(sync_repo);
    *ENTITY_MERGER_GLOBAL.lock().map_err(|_| FFIError::internal("ENTITY_MERGER_GLOBAL lock poisoned".to_string()))? = Some(central_merger);
    *SYNC_SERVICE_GLOBAL.lock().map_err(|_| FFIError::internal("SYNC_SERVICE_GLOBAL lock poisoned".to_string()))? = Some(sync_service);

    // Document types are already initialized before workers started - no need to do it again here

    Ok(())
}



/// Ensure status types lookup table is properly initialized
async fn ensure_status_types_initialized(pool: &SqlitePool) -> FFIResult<()> {

    println!("🔍 [GLOBALS] Checking status_types table...");
    
    // Check if status_types table exists and has data
    let count_result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM status_types")
        .fetch_one(pool)
        .await;
    
    match count_result {
        Ok(count) => {
            if count >= 4 {
                println!("✅ [GLOBALS] Status types table has {} entries, looks good", count);
                return Ok(());
            } else {
                println!("⚠️ [GLOBALS] Status types table only has {} entries, re-seeding...", count);
            }
        },
        Err(e) => {
            println!("❌ [GLOBALS] Error checking status_types table: {}", e);
            return Err(FFIError::internal(format!("Failed to check status_types: {}", e)));
        }
    }
    
    // Re-seed the status types (using INSERT OR IGNORE to avoid conflicts)
    println!("🌱 [GLOBALS] Seeding status types...");
    let now = chrono::Utc::now().to_rfc3339();
    
    let seed_queries = [
        ("INSERT OR IGNORE INTO status_types (id, value, created_at, updated_at) VALUES (1, 'On Track', ?, ?)", "On Track"),
        ("INSERT OR IGNORE INTO status_types (id, value, created_at, updated_at) VALUES (2, 'At Risk', ?, ?)", "At Risk"),
        ("INSERT OR IGNORE INTO status_types (id, value, created_at, updated_at) VALUES (3, 'Delayed', ?, ?)", "Delayed"),
        ("INSERT OR IGNORE INTO status_types (id, value, created_at, updated_at) VALUES (4, 'Completed', ?, ?)", "Completed"),
    ];
    
    for (query, status_name) in &seed_queries {
        match sqlx::query(query)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
        {
            Ok(_) => println!("✅ [GLOBALS] Seeded status type: {}", status_name),
            Err(e) => {
                println!("⚠️ [GLOBALS] Warning seeding {}: {}", status_name, e);
                // Don't fail on individual seed errors, might already exist
            }
        }
    }
    
    // Verify the seeding worked
    let final_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM status_types")
        .fetch_one(pool)
        .await
        .map_err(|e| FFIError::internal(format!("Failed to verify status_types after seeding: {}", e)))?;
    
    if final_count >= 4 {
        println!("✅ [GLOBALS] Status types successfully verified: {} entries", final_count);
        Ok(())
    } else {
        Err(FFIError::internal(format!("Status types seeding failed: only {} entries after seeding", final_count)))
    }
}

/// Ensure document types are properly initialized
async fn ensure_document_types_initialized(pool: &SqlitePool) -> FFIResult<()> {
    println!("🔍 [GLOBALS] Checking document_types table...");
    
    // Use an explicit transaction to ensure atomicity and handle strict SQLite configurations
    // on newer iOS versions (iOS 18+) which have stricter isolation
    let mut tx = pool.begin().await
        .map_err(|e| FFIError::internal(format!("Failed to begin document types transaction: {}", e)))?;
    
    // Get the standard document types from our initialization module
    let standard_types = initialization::initialize_standard_document_types();
    let expected_count = standard_types.len();
    
    println!("🌱 [GLOBALS] Ensuring {} standard document types are present...", expected_count);
    let now = chrono::Utc::now().to_rfc3339();
    
    // Use upsert logic (INSERT OR REPLACE) to handle existing types properly
    for doc_type in standard_types {
        let doc_type_id = Uuid::new_v4().to_string();
        
        // First, check if this document type already exists by name
        let existing_id = sqlx::query_scalar::<_, String>(
            "SELECT id FROM document_types WHERE name = ? AND deleted_at IS NULL"
        )
        .bind(&doc_type.name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| FFIError::internal(format!("Failed to check existing document type {}: {}", doc_type.name, e)))?;
        
        if let Some(existing_id) = existing_id {
            // Update existing document type to ensure it has all current settings
            let query = r#"
                UPDATE document_types SET
                    description = ?, icon = ?, default_priority = ?,
                    allowed_extensions = ?, max_size = ?, compression_level = ?, 
                    compression_method = ?, min_size_for_compression = ?, related_tables = ?,
                    updated_at = ?,
                    allowed_extensions_updated_at = ?, max_size_updated_at = ?,
                    compression_level_updated_at = ?, compression_method_updated_at = ?,
                    min_size_for_compression_updated_at = ?, description_updated_at = ?,
                    default_priority_updated_at = ?, icon_updated_at = ?,
                    related_tables_updated_at = ?
                WHERE id = ? AND deleted_at IS NULL
            "#;
            
            match sqlx::query(query)
                .bind(&doc_type.description)
                .bind(&doc_type.icon)
                .bind(&doc_type.default_priority)
                .bind(&doc_type.allowed_extensions)
                .bind(doc_type.max_size)
                .bind(doc_type.compression_level)
                .bind(&doc_type.compression_method)
                .bind(doc_type.min_size_for_compression)
                .bind(&doc_type.related_tables)
                .bind(&now)
                .bind(&now) // allowed_extensions_updated_at
                .bind(&now) // max_size_updated_at
                .bind(&now) // compression_level_updated_at
                .bind(&now) // compression_method_updated_at
                .bind(&now) // min_size_for_compression_updated_at
                .bind(&now) // description_updated_at
                .bind(&now) // default_priority_updated_at
                .bind(&now) // icon_updated_at
                .bind(&now) // related_tables_updated_at
                .bind(&existing_id)
                .execute(&mut *tx)
                .await
            {
                Ok(_) => println!("✅ [GLOBALS] Updated document type: {}", doc_type.name),
                Err(e) => {
                    println!("⚠️ [GLOBALS] Warning updating {}: {}", doc_type.name, e);
                    // Continue with other types
                }
            }
        } else {
            // Insert new document type
            let query = r#"
                INSERT INTO document_types (
                    id, name, description, icon, default_priority,
                    allowed_extensions, max_size, compression_level, 
                    compression_method, min_size_for_compression, related_tables,
                    created_at, updated_at,
                    name_updated_at, allowed_extensions_updated_at,
                    max_size_updated_at, compression_level_updated_at,
                    compression_method_updated_at, min_size_for_compression_updated_at,
                    description_updated_at, default_priority_updated_at,
                    icon_updated_at, related_tables_updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#;
            
            match sqlx::query(query)
                .bind(&doc_type_id)
                .bind(&doc_type.name)
                .bind(&doc_type.description)
                .bind(&doc_type.icon)
                .bind(&doc_type.default_priority)
                .bind(&doc_type.allowed_extensions)
                .bind(doc_type.max_size)
                .bind(doc_type.compression_level)
                .bind(&doc_type.compression_method)
                .bind(doc_type.min_size_for_compression)
                .bind(&doc_type.related_tables)
                .bind(&now)
                .bind(&now)
                .bind(&now) // name_updated_at
                .bind(&now) // allowed_extensions_updated_at
                .bind(&now) // max_size_updated_at
                .bind(&now) // compression_level_updated_at
                .bind(&now) // compression_method_updated_at
                .bind(&now) // min_size_for_compression_updated_at
                .bind(&now) // description_updated_at
                .bind(&now) // default_priority_updated_at
                .bind(&now) // icon_updated_at
                .bind(&now) // related_tables_updated_at
                .execute(&mut *tx)
                .await
            {
                Ok(_) => println!("✅ [GLOBALS] Inserted document type: {}", doc_type.name),
                Err(e) => {
                    println!("⚠️ [GLOBALS] Warning inserting {}: {}", doc_type.name, e);
                    // Continue with other types
                }
            }
        }
    }
    
    // Commit the transaction before verification to ensure changes are visible
    tx.commit().await
        .map_err(|e| FFIError::internal(format!("Failed to commit document types transaction: {}", e)))?;
    
    // Verify that we have at least the expected number of standard document types
    let final_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM document_types WHERE deleted_at IS NULL")
        .fetch_one(pool)
        .await
        .map_err(|e| FFIError::internal(format!("Failed to verify document_types after initialization: {}", e)))?;
    
    // More flexible validation - ensure we have at least the standard types
    let standard_type_names: Vec<String> = initialization::initialize_standard_document_types()
        .into_iter()
        .map(|dt| dt.name)
        .collect();
    
    let mut missing_types = Vec::new();
    for type_name in &standard_type_names {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM document_types WHERE name = ? AND deleted_at IS NULL"
        )
        .bind(type_name)
        .fetch_one(pool)
        .await
        .map_err(|e| FFIError::internal(format!("Failed to check for document type {}: {}", type_name, e)))?;
        
        if exists == 0 {
            missing_types.push(type_name.clone());
        }
    }
    
    if missing_types.is_empty() {
        println!("✅ [GLOBALS] Document types successfully verified: {} total entries, all {} standard types present", final_count, expected_count);
        Ok(())
    } else {
        Err(FFIError::internal(format!(
            "Document types initialization incomplete: missing types: {:?}. Total count: {}, expected at least: {}", 
            missing_types, final_count, expected_count
        )))
    }
}
