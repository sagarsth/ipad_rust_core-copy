use crate::auth::AuthService;
use crate::domains::user::{UserRepository, UserService};
use crate::domains::user::repository::SqliteUserRepository;
use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex, Once};
use lazy_static::lazy_static;

// Global state definitions
lazy_static! {
    static ref DB_POOL: Mutex<Option<SqlitePool>> = Mutex::new(None);
    static ref USER_REPO: Mutex<Option<Arc<dyn UserRepository>>> = Mutex::new(None);
    static ref USER_SERVICE: Mutex<Option<Arc<UserService>>> = Mutex::new(None);
    static ref AUTH_SERVICE: Mutex<Option<Arc<AuthService>>> = Mutex::new(None);
    static ref DEVICE_ID: Mutex<Option<String>> = Mutex::new(None);
    static ref OFFLINE_MODE: Mutex<bool> = Mutex::new(false);
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
            
            // Create repositories
            let user_repo: Arc<dyn UserRepository> = Arc::new(SqliteUserRepository::new(pool.clone()));
            
            // Create auth service
            let auth_service = Arc::new(AuthService::new(
                pool.clone(),
                device_id.to_string(),
                offline_mode,
            ));
            
            // Create user service
            let user_service = Arc::new(UserService::new(
                user_repo.clone(),
                auth_service.clone(),
            ));
            
            // Store services in globals
            *DB_POOL.lock().map_err(|_| "DB_POOL lock poisoned")? = Some(pool);
            *USER_REPO.lock().map_err(|_| "USER_REPO lock poisoned")? = Some(user_repo);
            *USER_SERVICE.lock().map_err(|_| "USER_SERVICE lock poisoned")? = Some(user_service);
            *AUTH_SERVICE.lock().map_err(|_| "AUTH_SERVICE lock poisoned")? = Some(auth_service);
            
            Ok(())
        })();

        // If the inner closure failed, store the error
        if let Err(e) = result {
            // Convert the Box<dyn Error> to an FFIError
            // This conversion needs to be defined, e.g., in FFIError implementation
            initialization_result = Err(FFIError::internal(format!("Initialization failed: {}", e).to_string()));
        }
    });
    
    // Return the result captured from within call_once
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