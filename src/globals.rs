use crate::auth::AuthService;
use crate::domains::user::{UserRepository, UserService, User};
use crate::domains::user::repository::SqliteUserRepository;
use crate::domains::sync::repository::{ChangeLogRepository, SqliteChangeLogRepository, TombstoneRepository, SqliteTombstoneRepository};
use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex, Once};
use lazy_static::lazy_static;
use crate::domains::core::dependency_checker::{DependencyChecker, SqliteDependencyChecker};
use crate::domains::core::delete_service::{DeleteService, BaseDeleteService, PendingDeletionManager};
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::auth::AuthContext;
use uuid::Uuid;
use crate::domains::donor::repository::{DonorRepository, SqliteDonorRepository};
use crate::domains::donor::Donor;
use crate::domains::sync::entity_merger::{DonorEntityMerger, UserEntityMerger};
use crate::domains::document::repository::{MediaDocumentRepository, SqliteMediaDocumentRepository};
use crate::domains::compression::repository::{CompressionRepository, SqliteCompressionRepository};
use crate::domains::compression::service::{CompressionService, CompressionServiceImpl};
use crate::domains::compression::manager::{CompressionManager, CompressionManagerImpl};
use crate::domains::core::file_storage_service::{FileStorageService, LocalFileStorageService};
use crate::domains::document::file_deletion_worker::FileDeletionWorker;

// Global state definitions
lazy_static! {
    static ref DB_POOL: Mutex<Option<SqlitePool>> = Mutex::new(None);
    static ref USER_REPO: Mutex<Option<Arc<dyn UserRepository>>> = Mutex::new(None);
    static ref DONOR_REPO: Mutex<Option<Arc<dyn DonorRepository>>> = Mutex::new(None);
    static ref USER_SERVICE: Mutex<Option<Arc<UserService>>> = Mutex::new(None);
    static ref AUTH_SERVICE: Mutex<Option<Arc<AuthService>>> = Mutex::new(None);
    static ref DEVICE_ID: Mutex<Option<String>> = Mutex::new(None);
    static ref OFFLINE_MODE: Mutex<bool> = Mutex::new(false);
    static ref CHANGE_LOG_REPO: Mutex<Option<Arc<dyn ChangeLogRepository>>> = Mutex::new(None);
    static ref TOMBSTONE_REPO: Mutex<Option<Arc<dyn TombstoneRepository>>> = Mutex::new(None);
    static ref MEDIA_DOCUMENT_REPO: Mutex<Option<Arc<dyn MediaDocumentRepository>>> = Mutex::new(None);
    static ref DEPENDENCY_CHECKER: Mutex<Option<Arc<dyn DependencyChecker>>> = Mutex::new(None);
    static ref DELETE_SERVICE_USER: Mutex<Option<Arc<dyn DeleteService<User>>>> = Mutex::new(None);
    static ref DELETE_SERVICE_DONOR: Mutex<Option<Arc<dyn DeleteService<Donor>>>> = Mutex::new(None);
    static ref DELETION_MANAGER: Mutex<Option<Arc<PendingDeletionManager>>> = Mutex::new(None);
    static ref USER_ENTITY_MERGER: Mutex<Option<Arc<UserEntityMerger>>> = Mutex::new(None);
    static ref DONOR_ENTITY_MERGER: Mutex<Option<Arc<DonorEntityMerger>>> = Mutex::new(None);
    static ref COMPRESSION_MANAGER: Mutex<Option<Arc<dyn CompressionManager>>> = Mutex::new(None);
    static ref FILE_STORAGE_SERVICE: Mutex<Option<Arc<dyn FileStorageService>>> = Mutex::new(None);
}

static INIT: Once = Once::new();

/// Initialize global services
pub fn initialize(
    db_url: &str, 
    device_id: &str, 
    offline_mode: bool, 
    jwt_secret: &str
) -> FFIResult<()> {
    let mut initialization_result = Ok(()); 

    INIT.call_once(|| {
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = (|| { 
            crate::auth::jwt::initialize(jwt_secret);
            
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect_lazy(db_url)?;
                
            *DEVICE_ID.lock().map_err(|_| "DEVICE_ID lock poisoned")? = Some(device_id.to_string());
            *OFFLINE_MODE.lock().map_err(|_| "OFFLINE_MODE lock poisoned")? = offline_mode;
            
            let change_log_repo: Arc<dyn ChangeLogRepository> = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
            let tombstone_repo: Arc<dyn TombstoneRepository> = Arc::new(SqliteTombstoneRepository::new(pool.clone()));
            let media_document_repo: Arc<dyn MediaDocumentRepository> = Arc::new(SqliteMediaDocumentRepository::new(pool.clone(), change_log_repo.clone()));

            let user_repo: Arc<dyn UserRepository> = Arc::new(SqliteUserRepository::new(
                pool.clone(),
                change_log_repo.clone()
            ));
            let donor_repo: Arc<dyn DonorRepository> = Arc::new(SqliteDonorRepository::new(pool.clone()));

            let dependency_checker: Arc<dyn DependencyChecker> = Arc::new(SqliteDependencyChecker::new(pool.clone()));
            let deletion_manager = Arc::new(PendingDeletionManager::new(pool.clone()));

            let auth_service = Arc::new(AuthService::new(
                pool.clone(),
                device_id.to_string(),
                offline_mode,
            ));

            // Adapter for DeleteServiceRepository<User>
            struct UserRepoAdapter(Arc<dyn UserRepository>); 
             #[async_trait::async_trait]
             impl FindById<User> for UserRepoAdapter {
                async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<User> { self.0.find_by_id(id).await }
             }
             #[async_trait::async_trait]
             impl HardDeletable for UserRepoAdapter {
                  fn entity_name(&self) -> &'static str { "users" } 
                  async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> {
                      self.0.hard_delete(id, auth).await 
                  }
                  async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     self.0.hard_delete_with_tx(id, auth, tx).await 
                 }
             }
             #[async_trait::async_trait]
             impl SoftDeletable for UserRepoAdapter {
                 async fn soft_delete(&self, _id: Uuid, _auth: &AuthContext) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
                 async fn soft_delete_with_tx(&self, _id: Uuid, _auth: &AuthContext, _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
             }
            let adapted_user_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<User>> = Arc::new(UserRepoAdapter(user_repo.clone()));

            let delete_service_user: Arc<dyn DeleteService<User>> = Arc::new(BaseDeleteService::new(
                pool.clone(),
                adapted_user_repo, 
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                None,
                deletion_manager.clone(), 
            ));

            // Adapter for DeleteServiceRepository<Donor>
            struct DonorRepoAdapter(Arc<dyn DonorRepository>);

            #[async_trait::async_trait]
            impl FindById<Donor> for DonorRepoAdapter {
                async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<Donor> { self.0.find_by_id(id).await }
            }

            #[async_trait::async_trait]
            impl HardDeletable for DonorRepoAdapter {
                 fn entity_name(&self) -> &'static str { "donors" }
                 async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> {
                     self.0.hard_delete(id, auth).await 
                 }
                 async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                    self.0.hard_delete_with_tx(id, auth, tx).await 
                }
            }

            #[async_trait::async_trait]
            impl SoftDeletable for DonorRepoAdapter {
                async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> { 
                    self.0.soft_delete(id, auth).await 
                }
                async fn soft_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> { 
                    self.0.soft_delete_with_tx(id, auth, tx).await
                }
            }
            let adapted_donor_repo: Arc<dyn crate::domains::core::delete_service::DeleteServiceRepository<Donor>> = Arc::new(DonorRepoAdapter(donor_repo.clone()));

            let delete_service_donor: Arc<dyn DeleteService<Donor>> = Arc::new(BaseDeleteService::new(
                pool.clone(),
                adapted_donor_repo,
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                Some(media_document_repo.clone()),
                deletion_manager.clone(),
            ));

            let user_service = Arc::new(UserService::new(
                user_repo.clone(),
                auth_service.clone(),
                delete_service_user.clone(), 
            ));

            let user_entity_merger = Arc::new(UserEntityMerger::new(user_repo.clone(), pool.clone()));
            let donor_entity_merger = Arc::new(DonorEntityMerger::new(
                donor_repo.clone(), 
                pool.clone(), 
                delete_service_donor.clone()
            ));

            // Initialize FileStorageService (local filesystem under ./storage)
            let file_storage_service: Arc<dyn FileStorageService> = Arc::new(
                LocalFileStorageService::new("./storage").map_err(|e| format!("File storage init failed: {}", e))?
            );

            // Initialize Compression components
            let compression_repo: Arc<dyn CompressionRepository> = Arc::new(SqliteCompressionRepository::new(pool.clone()));
            let compression_service: Arc<dyn CompressionService> = Arc::new(CompressionServiceImpl::new(
                pool.clone(),
                compression_repo.clone(),
                file_storage_service.clone(),
                media_document_repo.clone(),
                None, // ghostscript_path
            ));
            let compression_manager: Arc<dyn CompressionManager> = Arc::new(CompressionManagerImpl::new(
                compression_service.clone(),
                compression_repo.clone(),
                2,        // max_concurrent_jobs
                5_000,    // poll interval ms
            ));
            // Start the compression manager (worker started inside constructor but we keep sender alive)
            let _ = compression_manager.start();

            // Spawn FileDeletionWorker in background
            let fd_pool = pool.clone();
            let fd_storage = file_storage_service.clone();
            tokio::spawn(async move {
                let worker = FileDeletionWorker::new(fd_pool, fd_storage);
                if let Err(e) = worker.start().await {
                    log::error!("FileDeletionWorker exited: {:?}", e);
                }
            });

            *DB_POOL.lock().map_err(|_| "DB_POOL lock poisoned")? = Some(pool);
            *CHANGE_LOG_REPO.lock().map_err(|_| "CHANGE_LOG_REPO lock poisoned")? = Some(change_log_repo); 
            *TOMBSTONE_REPO.lock().map_err(|_| "TOMBSTONE_REPO lock poisoned")? = Some(tombstone_repo);
            *MEDIA_DOCUMENT_REPO.lock().map_err(|_| "MEDIA_DOCUMENT_REPO lock poisoned")? = Some(media_document_repo);
            *DEPENDENCY_CHECKER.lock().map_err(|_| "DEPENDENCY_CHECKER lock poisoned")? = Some(dependency_checker);
            *DELETION_MANAGER.lock().map_err(|_| "DELETION_MANAGER lock poisoned")? = Some(deletion_manager);
            *USER_REPO.lock().map_err(|_| "USER_REPO lock poisoned")? = Some(user_repo);
            *DONOR_REPO.lock().map_err(|_| "DONOR_REPO lock poisoned")? = Some(donor_repo);
            *DELETE_SERVICE_USER.lock().map_err(|_| "DELETE_SERVICE_USER lock poisoned")? = Some(delete_service_user); 
            *DELETE_SERVICE_DONOR.lock().map_err(|_| "DELETE_SERVICE_DONOR lock poisoned")? = Some(delete_service_donor);
            *USER_SERVICE.lock().map_err(|_| "USER_SERVICE lock poisoned")? = Some(user_service);
            *AUTH_SERVICE.lock().map_err(|_| "AUTH_SERVICE lock poisoned")? = Some(auth_service);
            *USER_ENTITY_MERGER.lock().map_err(|_| "USER_ENTITY_MERGER lock poisoned")? = Some(user_entity_merger);
            *DONOR_ENTITY_MERGER.lock().map_err(|_| "DONOR_ENTITY_MERGER lock poisoned")? = Some(donor_entity_merger);
            *FILE_STORAGE_SERVICE.lock().map_err(|_| "FILE_STORAGE_SERVICE lock poisoned")? = Some(file_storage_service);
            *COMPRESSION_MANAGER.lock().map_err(|_| "COMPRESSION_MANAGER lock poisoned")? = Some(compression_manager);
            
            Ok(())
        })();

        if let Err(e) = result {
            initialization_result = Err(FFIError::internal(format!("Initialization failed: {}", e).to_string()));
        }
    });
    
    initialization_result
}

/// Get database pool
pub fn get_db_pool() -> FFIResult<SqlitePool> {
    DB_POOL.lock()
        .map_err(|_| FFIError::internal("DB_POOL lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Database pool not initialized".to_string()))
}

/// Get user repository
pub fn get_user_repo() -> FFIResult<Arc<dyn UserRepository>> {
    USER_REPO.lock()
        .map_err(|_| FFIError::internal("USER_REPO lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("User repository not initialized".to_string()))
}

/// Get user service
pub fn get_user_service() -> FFIResult<Arc<UserService>> {
    USER_SERVICE.lock()
        .map_err(|_| FFIError::internal("USER_SERVICE lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("User service not initialized".to_string()))
}

/// Get auth service
pub fn get_auth_service() -> FFIResult<Arc<AuthService>> {
    AUTH_SERVICE.lock()
        .map_err(|_| FFIError::internal("AUTH_SERVICE lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Auth service not initialized".to_string()))
}

/// Get device ID
pub fn get_device_id() -> FFIResult<String> {
    DEVICE_ID.lock()
        .map_err(|_| FFIError::internal("DEVICE_ID lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Device ID not initialized".to_string()))
}

/// Get offline mode status
pub fn is_offline_mode() -> bool {
    OFFLINE_MODE.lock()
        .map(|guard| *guard)
        .unwrap_or(false)
}

/// Set offline mode status
pub fn set_offline_mode(offline: bool) {
    if let Ok(mut guard) = OFFLINE_MODE.lock() {
        *guard = offline;
    }
}

/// Get user delete service
pub fn get_user_delete_service() -> FFIResult<Arc<dyn DeleteService<User>>> {
    DELETE_SERVICE_USER.lock()
        .map_err(|_| FFIError::internal("DELETE_SERVICE_USER lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("User delete service not initialized".to_string()))
}

/// Get pending deletion manager
pub fn get_deletion_manager() -> FFIResult<Arc<PendingDeletionManager>> {
    DELETION_MANAGER.lock()
        .map_err(|_| FFIError::internal("DELETION_MANAGER lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Pending deletion manager not initialized".to_string()))
}

pub fn get_donor_repo() -> FFIResult<Arc<dyn DonorRepository>> {
    DONOR_REPO.lock()
        .map_err(|_| FFIError::internal("DONOR_REPO lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Donor repository not initialized".to_string()))
}

pub fn get_donor_delete_service() -> FFIResult<Arc<dyn DeleteService<Donor>>> {
    DELETE_SERVICE_DONOR.lock()
        .map_err(|_| FFIError::internal("DELETE_SERVICE_DONOR lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Donor delete service not initialized".to_string()))
}

pub fn get_media_document_repo() -> FFIResult<Arc<dyn MediaDocumentRepository>> {
    MEDIA_DOCUMENT_REPO.lock()
        .map_err(|_| FFIError::internal("MEDIA_DOCUMENT_REPO lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("Media document repository not initialized".to_string()))
} 