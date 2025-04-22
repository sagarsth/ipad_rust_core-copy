use std::sync::Arc;

// Public modules
pub mod auth;
pub mod domains;
pub mod errors;
pub mod ffi;
pub mod globals;
pub mod types;
pub mod validation;

// Private modules
mod db_migration;
mod utils;

// Entry point for initialization
/// Initialize the library with the given database path, device ID, offline mode, and JWT secret.
/// This function must be called before any other function in the library.
pub fn initialize(
    db_path: &str, 
    device_id: &str, 
    offline_mode: bool, 
    jwt_secret: &str
) -> ffi::FFIResult<()> {
    // Initialize global services, passing the secret
    globals::initialize(db_path, device_id, offline_mode, jwt_secret)?;
    
    // Initialize database with migrations
    db_migration::initialize_database()?;
    
    Ok(())
}

/// Set offline mode status
pub fn set_offline_mode(offline_mode: bool) {
    globals::set_offline_mode(offline_mode);
}

/// Get the current device ID
pub fn get_device_id() -> ffi::FFIResult<String> {
    globals::get_device_id()
}

/// Check if the app is in offline mode
pub fn is_offline_mode() -> bool {
    globals::is_offline_mode()
}

/// Get a reference to the SQLite connection pool
/// This is primarily for internal use
pub fn get_db_pool() -> ffi::FFIResult<sqlx::SqlitePool> {
    globals::get_db_pool()
}