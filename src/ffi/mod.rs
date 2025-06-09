// In src/ffi/mod.rs
use std::os::raw::{c_int, c_char};
use std::ffi::CString;
// Corrected imports for FFIError and FFIResult
use crate::ffi::error::{FFIError, ErrorCode};
use serde::Serialize;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use std::future::Future;

// Declare necessary FFI submodules
pub mod auth;
pub mod core; // Core library functions
pub mod error; // Ensure error module is declared
pub mod export;
pub mod user; // Include if you have src/ffi/user.rs
// pub mod init; // Include if you have src/ffi/init.rs
// Consider removing jwt_init
pub mod document;
pub mod compression;
pub mod strategic_goal;
pub mod project;
pub mod donor;
pub mod funding;
pub mod activity;
pub mod livelihood;
pub mod workshop;
pub mod participant;

// iOS-safe single-threaded runtime
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Initialize or get the iOS-safe Tokio runtime
pub fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create iOS-safe Tokio runtime")
    })
}

/// Block on async operation in iOS-safe manner
pub fn block_on_async<F: Future>(future: F) -> F::Output {
    get_runtime().block_on(future)
}

/// Error handling helper for FFI boundaries (returns error code)
pub fn handle_status_result<F>(func: F) -> c_int
where
    F: FnOnce() -> FFIResult<()>,
{
    match func() {
        Ok(_) => {
            // Clear any previous error on success
            error::clear_last_error();
            ErrorCode::Success as c_int
        }
        Err(e) => {
            // Store the error in thread-local storage
            error::store_last_error(&e);
            eprintln!("[Rust FFI Error] Code: {:?}, Message: {}, Details: {:?}",
                      e.code, e.message, e.details.as_deref().unwrap_or("None"));
            e.code as c_int
        }
    }
}

/// Handles results for FFI functions that return data, serializing Ok(T) or Err(FFIError) to JSON.
/// Returns a pointer to a C string (must be freed by the caller).
pub fn handle_json_result<F, T>(func: F) -> *mut c_char
where
    F: FnOnce() -> FFIResult<T>,
    T: Serialize,
{
    let result = func();
    let json_string = match result {
        Ok(value) => {
            // Clear any previous error on success
            error::clear_last_error();
            // Wrap the successful value in a standard structure if desired, or serialize directly
            // Example: Serialize directly
            serde_json::to_string(&value)
        },
        Err(ffi_error) => {
            // Store the error in thread-local storage
            error::store_last_error(&ffi_error);
            // Serialize the FFIError itself
            serde_json::to_string(&ffi_error)
        },
    };

    let final_json = match json_string {
        Ok(s) => s,
        Err(e) => {
            // Handle serialization error: Create an FFIError JSON manually
            // It's crucial the FFI caller can always parse the response
            let error_code = ErrorCode::InternalError;
            let error_msg = format!("Failed to serialize result: {}", e);
            eprintln!("[Rust FFI Error] Serialization failed: {}", error_msg);
            // Manually construct JSON string for the serialization error
            format!("{{\"code\":\"{:?}\",\"message\":\"{}\",\"details\":null}}", error_code, error_msg)
        }
    };

    // Convert the JSON string to CString and return the raw pointer
    match CString::new(final_json) {
        Ok(c_string) => c_string.into_raw(),
        Err(e) => {
            // Handle CString creation error (e.g., null bytes in string)
            eprintln!("[Rust FFI Error] Failed to create CString: {}", e);
            // Return a specific error JSON string or null pointer
            let error_code = ErrorCode::InternalError;
            let error_msg = format!("Failed to create CString: {}", e);
            let error_json = format!("{{\"code\":\"{:?}\",\"message\":\"{}\",\"details\":null}}", error_code, error_msg);
            // Try creating CString from the error JSON itself
            CString::new(error_json).map_or(std::ptr::null_mut(), |cs| cs.into_raw())
        }
    }
}

/// Convert any error implementing Clone + 'static to FFIError
// Note: The 'static bound might be restrictive, adjust if necessary.
pub fn to_ffi_error<E: std::error::Error + Clone + 'static>(error: E) -> FFIError {
    // Corrected path to the helper function
    // Pass a reference to the error
    crate::ffi::error::to_ffi_error(&error)
}

// Re-export FFIResult for convenience if needed within ffi module
pub use error::FFIResult;

// Memory safety utilities for FFI
use std::collections::BTreeMap;
use std::sync::Mutex;

// Simple counter for tracking allocations instead of using raw pointers
static ALLOCATION_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Track string allocation for debugging (simplified approach)
pub fn track_string_allocation(_ptr: *mut std::os::raw::c_char) {
    ALLOCATION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Track string deallocation for debugging (simplified approach)
pub fn track_string_deallocation(_ptr: *mut std::os::raw::c_char) {
    ALLOCATION_COUNTER.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
}

/// Get count of currently allocated strings
pub fn get_allocated_string_count() -> usize {
    ALLOCATION_COUNTER.load(std::sync::atomic::Ordering::Relaxed)
}
