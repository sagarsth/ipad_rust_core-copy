// src/ffi/core.rs
// ============================================================================
// Core FFI functions for library initialization and management
// ============================================================================

use crate::ffi::{handle_status_result, error::FFIError};
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;

/// Initialize the library with database URL, device ID, offline mode, and JWT secret
/// Returns 0 on success, non-zero on error
#[unsafe(no_mangle)]
pub unsafe extern "C" fn initialize_library(
    db_url: *const c_char,
    device_id: *const c_char,
    offline_mode: bool,
    jwt_secret: *const c_char,
) -> c_int {
    let result = std::panic::catch_unwind(|| {
        if db_url.is_null() || device_id.is_null() || jwt_secret.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided for initialization"));
        }

        let db_url_str = match CStr::from_ptr(db_url).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return Err(FFIError::invalid_argument("Invalid db_url string")),
        };
        
        // Validate that we received a proper SQLite URL, not a file path
        if !db_url_str.starts_with("sqlite://") {
            return Err(FFIError::invalid_argument(
                "db_url must be a SQLite URL starting with 'sqlite://', not a file path"
            ));
        }
        
        let device_id_str = match CStr::from_ptr(device_id).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return Err(FFIError::invalid_argument("Invalid device_id string")),
        };
        
        let jwt_secret_str = match CStr::from_ptr(jwt_secret).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return Err(FFIError::invalid_argument("Invalid jwt_secret string")),
        };

        // Use the centralized runtime to avoid conflicts
        crate::ffi::block_on_async(async {
            crate::initialize(&db_url_str, &device_id_str, offline_mode, &jwt_secret_str).await
        })
    });

    match result {
        Ok(ffi_result) => {
            handle_status_result(|| ffi_result)
        }
        Err(panic_payload) => {
            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Panicked during FFI call, but panic message is not a string".to_string()
            };
            eprintln!("[Rust FFI Panic] in initialize_library: {}", panic_msg);
            handle_status_result(|| Err(FFIError::internal(format!("Panic during initialization: {}", panic_msg))))
        }
    }
}

/// Set offline mode status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_offline_mode(offline_mode: bool) {
    crate::set_offline_mode(offline_mode);
}

/// Get the current device ID
/// Returns allocated string that must be freed with free_string()
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_device_id(result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if result.is_null() {
            return Err(FFIError::invalid_argument("Null result pointer provided"));
        }
        
        let device_id = crate::get_device_id()
            .map_err(|e| FFIError::internal(format!("Failed to get device ID: {}", e)))?;
        
        let c_string = CString::new(device_id)
            .map_err(|e| FFIError::internal(format!("CString creation failed: {}", e)))?;
        
        *result = c_string.into_raw();
        Ok(())
    })
}

/// Check if the app is in offline mode
#[unsafe(no_mangle)]
pub unsafe extern "C" fn is_offline_mode() -> bool {
    crate::is_offline_mode()
}

/// Frees a C string that was allocated by Rust and passed over FFI.
/// This function should be called by the C/Swift side for any string
/// that was created in Rust using `CString::into_raw()`.
#[no_mangle]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // This takes ownership of the CString and drops it when it goes out of scope.
        let _ = CString::from_raw(ptr);
    }
}

/// Get library version
/// Returns allocated string that must be freed with free_string()
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_library_version() -> *mut c_char {
    match CString::new("0.1.0") {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => {
            // Fallback if CString creation fails
            match CString::new("unknown") {
                Ok(c_string) => c_string.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
    }
}

/// Get last error message from thread-local storage
/// Returns allocated string that must be freed with free_string(), or null if no error
#[unsafe(no_mangle)]
pub unsafe extern "C" fn get_last_error() -> *mut c_char {
    crate::ffi::error::get_last_error_message()
}

/// Set the iOS documents directory path for file storage
/// This should be called before initialize_library() on iOS
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_ios_storage_path(path: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        if path.is_null() {
            return Err(FFIError::invalid_argument("Null path pointer provided"));
        }
        
        let path_str = CStr::from_ptr(path)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid UTF-8 in storage path"))?;
        
        println!("🔧 [FFI] Setting iOS storage path: '{}'", path_str);
        
        std::env::set_var("IOS_DOCUMENTS_DIR", path_str);
        
        // Verify it was set
        let verification = std::env::var("IOS_DOCUMENTS_DIR").unwrap_or_else(|_| "NOT SET".to_string());
        println!("🔧 [FFI] Environment variable now: '{}'", verification);
        
        Ok(())
    })
} 