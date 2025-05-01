use crate::auth::AuthService;
use crate::domains::user::{UserRepository, UserService};
use crate::domains::user::repository::SqliteUserRepository;
use crate::domains::sync::repository::{ChangeLogRepository, SqliteChangeLogRepository, TombstoneRepository, SqliteTombstoneRepository};
use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex, Once};
use lazy_static::lazy_static;
use crate::domains::core::dependency_checker::{DependencyChecker, SqliteDependencyChecker};
use crate::domains::core::delete_service::{DeleteService, BaseDeleteService, DeleteServiceRepository};
use crate::domains::user::User;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::auth::AuthContext;
use uuid::Uuid;

// Global state definitions
lazy_static! {
    static ref DB_POOL: Mutex<Option<SqlitePool>> = Mutex::new(None);
    static ref USER_REPO: Mutex<Option<Arc<dyn UserRepository>>> = Mutex::new(None);
    static ref USER_SERVICE: Mutex<Option<Arc<UserService>>> = Mutex::new(None);
    static ref AUTH_SERVICE: Mutex<Option<Arc<AuthService>>> = Mutex::new(None);
    static ref DEVICE_ID: Mutex<Option<String>> = Mutex::new(None);
    static ref OFFLINE_MODE: Mutex<bool> = Mutex::new(false);
    static ref CHANGE_LOG_REPO: Mutex<Option<Arc<dyn ChangeLogRepository>>> = Mutex::new(None);
    static ref TOMBSTONE_REPO: Mutex<Option<Arc<dyn TombstoneRepository>>> = Mutex::new(None);
    static ref DEPENDENCY_CHECKER: Mutex<Option<Arc<dyn DependencyChecker>>> = Mutex::new(None);
    static ref DELETE_SERVICE_USER: Mutex<Option<Arc<dyn DeleteService<User>>>> = Mutex::new(None);
}

static INIT: Once = Once::new();

/// Initialize global services
pub fn initialize(
    db_url: &str, 
    device_id: &str, 
    offline_mode: bool, 
    jwt_secret: &str
) -> FFIResult<()> {
    // Use a variable to capture potential errors inside call_once
    let mut initialization_result = Ok(()); 

    INIT.call_once(|| {
        // Wrap the initialization logic in a closure that returns a Result
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = (|| { // Using Box<dyn Error> for simplicity inside closure
            // Initialize JWT module FIRST
            crate::auth::jwt::initialize(jwt_secret);
            
            // Set up SQLx connection pool
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(5)
                .connect_lazy(db_url)?;
                
            // Store device ID
            *DEVICE_ID.lock().map_err(|_| "DEVICE_ID lock poisoned")? = Some(device_id.to_string());
            
            // Store offline mode
            *OFFLINE_MODE.lock().map_err(|_| "OFFLINE_MODE lock poisoned")? = offline_mode;
            
            // --- Create Repositories ---
            let change_log_repo: Arc<dyn ChangeLogRepository> = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
            let tombstone_repo: Arc<dyn TombstoneRepository> = Arc::new(SqliteTombstoneRepository::new(pool.clone()));
            let user_repo: Arc<dyn UserRepository> = Arc::new(SqliteUserRepository::new(
                pool.clone(),
                change_log_repo.clone()
            ));
            // Instantiate other domain repositories here...

            // --- Create Core Services ---
            // Create DependencyChecker
            let dependency_checker: Arc<dyn DependencyChecker> = Arc::new(SqliteDependencyChecker::new(pool.clone()));

            // Create Auth Service
            let auth_service = Arc::new(AuthService::new(
                pool.clone(),
                device_id.to_string(),
                offline_mode,
            ));

            // --- Create Delete Services ---
            // Adapter for DeleteServiceRepository<User>
            struct UserRepoAdapter(Arc<dyn UserRepository>); 

             #[async_trait::async_trait]
             impl FindById<User> for UserRepoAdapter {
                 async fn find_by_id(&self, id: Uuid) -> crate::errors::DomainResult<User> {
                     self.0.find_by_id(id).await
                 }
             }
             #[async_trait::async_trait]
             impl HardDeletable for UserRepoAdapter {
                  fn entity_name(&self) -> &'static str { "users" } 
                  async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> crate::errors::DomainResult<()> {
                      log::warn!("Standalone hard_delete called on UserRepoAdapter");
                      self.0.hard_delete(id, auth).await 
                  }
                  async fn hard_delete_with_tx(&self, id: Uuid, auth: &AuthContext, tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> {
                     self.0.hard_delete_with_tx(id, auth, tx).await 
                 }
             }
             // SoftDeletable is not needed for User
             #[async_trait::async_trait]
             impl SoftDeletable for UserRepoAdapter {
                 async fn soft_delete(&self, _id: Uuid, _auth: &AuthContext) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
                 async fn soft_delete_with_tx(&self, _id: Uuid, _auth: &AuthContext, _tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> crate::errors::DomainResult<()> { unimplemented!("User does not support soft delete") }
             }

             // Create the adapter instance
             let adapted_user_repo: Arc<dyn DeleteServiceRepository<User>> = Arc::new(UserRepoAdapter(user_repo.clone()));

            // Create BaseDeleteService for User
            let delete_service_user: Arc<dyn DeleteService<User>> = Arc::new(BaseDeleteService::new(
                pool.clone(),
                adapted_user_repo, // Use the adapter
                tombstone_repo.clone(),
                change_log_repo.clone(),
                dependency_checker.clone(),
                None, // Assuming User doesn't link to MediaDocumentRepository
            ));

            // --- Create Domain Services ---
            // Create User Service, now passing delete_service_user
            let user_service = Arc::new(UserService::new(
                user_repo.clone(),
                auth_service.clone(),
                delete_service_user.clone(), // Pass the delete service
            ));

            // --- Store Globals ---
            *DB_POOL.lock().map_err(|_| "DB_POOL lock poisoned")? = Some(pool);
            *CHANGE_LOG_REPO.lock().map_err(|_| "CHANGE_LOG_REPO lock poisoned")? = Some(change_log_repo); 
            *TOMBSTONE_REPO.lock().map_err(|_| "TOMBSTONE_REPO lock poisoned")? = Some(tombstone_repo);
            *DEPENDENCY_CHECKER.lock().map_err(|_| "DEPENDENCY_CHECKER lock poisoned")? = Some(dependency_checker);
            *USER_REPO.lock().map_err(|_| "USER_REPO lock poisoned")? = Some(user_repo);
            *DELETE_SERVICE_USER.lock().map_err(|_| "DELETE_SERVICE_USER lock poisoned")? = Some(delete_service_user); 
            *USER_SERVICE.lock().map_err(|_| "USER_SERVICE lock poisoned")? = Some(user_service);
            *AUTH_SERVICE.lock().map_err(|_| "AUTH_SERVICE lock poisoned")? = Some(auth_service);
            
            Ok(())
        })();

        // If the inner closure failed, store the error
        if let Err(e) = result {
            // Convert the Box<dyn Error> to an FFIError
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
    // Assuming lock poisoning is unlikely/unrecoverable for a simple bool read
    OFFLINE_MODE.lock()
        .map(|guard| *guard) // Dereference the guard if lock succeeds
        .unwrap_or(false)    // Return false if lock was poisoned
}

/// Set offline mode status
pub fn set_offline_mode(offline: bool) {
    if let Ok(mut guard) = OFFLINE_MODE.lock() {
        *guard = offline;
    }
    // Otherwise, the lock is poisoned, maybe log an error?
}

/// Get user delete service
pub fn get_user_delete_service() -> FFIResult<Arc<dyn DeleteService<User>>> {
    DELETE_SERVICE_USER.lock()
        .map_err(|_| FFIError::internal("DELETE_SERVICE_USER lock poisoned".to_string()))?
        .clone()
        .ok_or_else(|| FFIError::internal("User delete service not initialized".to_string()))
} 