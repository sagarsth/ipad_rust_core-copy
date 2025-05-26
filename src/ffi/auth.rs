// Remove the unused imports
use crate::ffi::error::{FFIError, FFIResult};
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;
use tokio::runtime::Runtime;

// Helper function to run async code in a blocking way for FFI
fn block_on_async<F, T, E>(future: F) -> Result<T, E> 
where 
    F: std::future::Future<Output = Result<T, E>>,
{
    // Create a new Tokio runtime for the async operation
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(future)
}

/// Error handling helper for FFI boundaries
fn handle_result<F>(func: F) -> c_int
where
    F: FnOnce() -> FFIResult<()>,
{
    match func() {
        Ok(_) => 0, // Success
        Err(e) => {
            // Log the error (in a real app)
            eprintln!("FFI error: {:?}", e);
            e.code as c_int
        }
    }
}

/// Perform user login
/// 
/// # Safety
///
/// This function should only be called with:
/// - A valid, null-terminated C string for `email` and `password`
/// - A valid pointer to receive the token result
#[unsafe(no_mangle)]
pub unsafe extern "C" fn login(
    email: *const c_char,
    password: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        // Validate pointers
        if email.is_null() || password.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        // Convert C strings to Rust strings - need unsafe blocks for these operations
        let email_str = unsafe { CStr::from_ptr(email) }.to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid email string"))?;
        
        let password_str = unsafe { CStr::from_ptr(password) }.to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid password string"))?;
        
        // Get auth service from globals
        let auth_service = crate::globals::get_auth_service()?;
        
        // Perform login
        let login_result = block_on_async(auth_service.login(email_str, password_str))?;
        
        // Create a JSON response with token and user info
        let response = serde_json::json!({
            "access_token": login_result.access_token,
            "access_expiry": login_result.access_expiry.to_rfc3339(),
            "refresh_token": login_result.refresh_token,
            "refresh_expiry": login_result.refresh_expiry.to_rfc3339(),
            "user_id": login_result.user_id.to_string(),
            "role": login_result.role.as_str(),
        });
        
        // Serialize to JSON
        let json_string = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("Login JSON serialization error: {}", e).to_string()))?;
        
        // Convert to C string and transfer ownership
        let c_json = CString::new(json_string)?;
        
        // Writing to raw pointer requires an unsafe block
        unsafe { *result = c_json.into_raw() };
        
        Ok(())
    })
}

/// Verify a token is valid
/// 
/// # Safety
///
/// This function should only be called with:
/// - A valid, null-terminated C string for `token`
/// - A valid pointer to receive result (optional)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verify_token(
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        // Validate pointers
        if token.is_null() {
            return Err(FFIError::invalid_argument("Null token provided"));
        }
        
        // Convert C strings to Rust strings
        let token_str = unsafe { CStr::from_ptr(token) }.to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        // Get auth service from globals
        let auth_service = crate::globals::get_auth_service()?;
        
        // Verify token
        let auth_context = block_on_async(auth_service.verify_token(token_str))?;
        
        // If result pointer provided, return context information
        if !result.is_null() {
            let context_json = serde_json::json!({
                "user_id": auth_context.user_id.to_string(),
                "role": auth_context.role.as_str(),
                "device_id": auth_context.device_id,
                "offline_mode": auth_context.offline_mode,
            });
            
            let json_string = serde_json::to_string(&context_json)
                .map_err(|e| FFIError::internal(format!("Verify token JSON serialization error: {}", e).to_string()))?;
            
            // Convert to C string and transfer ownership
            let c_json = CString::new(json_string)?;
            
            // Writing to raw pointer requires an unsafe block
            unsafe { *result = c_json.into_raw() };
        }
        
        Ok(())
    })
}

/// Refresh an access token using a refresh token
/// 
/// # Safety
///
/// This function should only be called with:
/// - A valid, null-terminated C string for refresh token
/// - A valid pointer to receive the new token
#[unsafe(no_mangle)]
pub unsafe extern "C" fn refresh_token(
    refresh_token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        // Validate pointers
        if refresh_token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        // Convert C strings to Rust strings
        let token_str = unsafe { CStr::from_ptr(refresh_token) }.to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        // Get auth service from globals
        let auth_service = crate::globals::get_auth_service()?;
        
        // Refresh token
        let (new_token, expiry) = block_on_async(auth_service.refresh_session(token_str))?;
        
        // Create JSON response
        let response = serde_json::json!({
            "access_token": new_token,
            "access_expiry": expiry.to_rfc3339()
        });
        
        // Serialize to JSON
        let json_string = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("Refresh JSON serialization error: {}", e).to_string()))?;
        
        // Convert to C string and transfer ownership
        let c_json = CString::new(json_string)?;
        
        // Writing to raw pointer requires an unsafe block
        unsafe { *result = c_json.into_raw() };
        
        Ok(())
    })
}

/// Log out a user
/// 
/// # Safety
///
/// This function should only be called with:
/// - A valid, null-terminated C string for `token`
/// - A valid, null-terminated C string for refresh token (optional)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logout(
    token: *const c_char,
    refresh_token: *const c_char,
) -> c_int {
    handle_result(|| unsafe {
        // Validate pointers
        if token.is_null() {
            return Err(FFIError::invalid_argument("Null token provided"));
        }
        
        // Convert C strings to Rust strings
        let token_str = unsafe { CStr::from_ptr(token) }.to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        // Get optional refresh token
        let refresh_token_str = if refresh_token.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(refresh_token) }.to_str()
                .map_err(|_| FFIError::invalid_argument("Invalid refresh token string"))?)
        };
        
        // Get auth service from globals
        let auth_service = crate::globals::get_auth_service()?;
        
        // Verify token to get auth context
        let auth_context = block_on_async(auth_service.verify_token(token_str))?;
        
        // Logout
        block_on_async(auth_service.logout(&auth_context, token_str, refresh_token_str))?;
        
        Ok(())
    })
}

/// Free a string allocated by the Rust library
///
/// # Safety
///
/// This function should only be called with:
/// - A pointer returned from one of our other FFI functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // This operation is inherently unsafe but still needs an explicit unsafe block
        unsafe { let _ = CString::from_raw(ptr); }
    }
}

/// Hash a plaintext password using Argon2 and return the hashed string.
///
/// # Safety
///
/// This function should only be called with:
/// - A valid, null-terminated UTF-8 C string for `password`.
/// - A valid pointer to receive the returned hash.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hash_password(
    password: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if password.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }

        let pwd_str = CStr::from_ptr(password).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid password string"))?;

        let auth_service = crate::globals::get_auth_service()?;
        let hash = auth_service.hash_password(pwd_str)?;

        let c_hash = CString::new(hash)?;
        *result = c_hash.into_raw();
        Ok(())
    })
}