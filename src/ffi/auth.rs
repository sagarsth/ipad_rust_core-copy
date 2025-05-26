// src/ffi/auth.rs
// ============================================================================
// FFI bindings for authentication and user management.
// Combines AuthService and UserService functionality to provide comprehensive
// authentication and user management capabilities to the Swift frontend.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `auth_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//
// ============================================================================

use crate::ffi::error::{FFIError, FFIResult};
use crate::auth::AuthContext;
use crate::domains::user::types::{NewUser, UpdateUser, Credentials};
use crate::validation::Validate;
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;
use uuid::Uuid;
use serde_json::json;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to run async code in a blocking way for FFI
fn block_on_async<F, T, E>(future: F) -> Result<T, E> 
where 
    F: std::future::Future<Output = Result<T, E>>,
{
    crate::ffi::block_on_async(future)
}

/// Error handling helper for FFI boundaries
fn handle_result<F>(func: F) -> c_int
where
    F: FnOnce() -> FFIResult<()>,
{
    match func() {
        Ok(_) => 0, // Success
        Err(e) => {
            eprintln!("Auth FFI error: {:?}", e);
            e.code as c_int
        }
    }
}

/// Helper to create auth context from token
fn create_auth_context_from_token(token: &str) -> FFIResult<AuthContext> {
    let auth_service = crate::globals::get_auth_service()?;
    block_on_async(auth_service.verify_token(token))
        .map_err(|e| FFIError::internal(format!("Token verification failed: {}", e)))
}

/// Helper to parse JSON payload
fn parse_json_payload<T: serde::de::DeserializeOwned>(json_str: &str) -> FFIResult<T> {
    serde_json::from_str(json_str)
        .map_err(|e| FFIError::invalid_argument(&format!("Invalid JSON payload: {}", e)))
}

/// Helper to create JSON response
fn create_json_response<T: serde::Serialize>(data: T) -> FFIResult<*mut c_char> {
    let json_string = serde_json::to_string(&data)
        .map_err(|e| FFIError::internal(format!("JSON serialization failed: {}", e)))?;
    
    let c_string = CString::new(json_string)
        .map_err(|e| FFIError::internal(format!("CString creation failed: {}", e)))?;
    
    Ok(c_string.into_raw())
}

// ============================================================================
// AUTHENTICATION FUNCTIONS
// ============================================================================

/// Perform user login with email and password
/// 
/// # Arguments
/// * `credentials_json` - JSON string containing email and password
/// 
/// # Returns
/// JSON containing access_token, refresh_token, user info, and expiry times
/// 
/// # Safety
/// This function should only be called with valid, null-terminated C strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_login(
    credentials_json: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if credentials_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(credentials_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid credentials JSON string"))?;
        
        let credentials: Credentials = parse_json_payload(json_str)?;
        credentials.validate()
            .map_err(|e| FFIError::invalid_argument(&format!("Validation failed: {}", e)))?;
        
        let auth_service = crate::globals::get_auth_service()?;
        let login_result = block_on_async(auth_service.login(&credentials.email, &credentials.password))
            .map_err(|e| FFIError::internal(format!("Login failed: {}", e)))?;
        
        let response = json!({
            "access_token": login_result.access_token,
            "access_expiry": login_result.access_expiry.to_rfc3339(),
            "refresh_token": login_result.refresh_token,
            "refresh_expiry": login_result.refresh_expiry.to_rfc3339(),
            "user_id": login_result.user_id.to_string(),
            "role": login_result.role.as_str(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Verify a token and return auth context information
/// 
/// # Arguments
/// * `token` - Access token to verify
/// 
/// # Returns
/// JSON containing user_id, role, device_id, and offline_mode
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_verify_token(
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if token.is_null() {
            return Err(FFIError::invalid_argument("Null token provided"));
        }
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        if !result.is_null() {
            let response = json!({
                "user_id": auth_context.user_id.to_string(),
                "role": auth_context.role.as_str(),
                "device_id": auth_context.device_id,
                "offline_mode": auth_context.offline_mode,
            });
            
            *result = create_json_response(response)?;
        }
        
        Ok(())
    })
}

/// Refresh an access token using a refresh token
/// 
/// # Arguments
/// * `refresh_token` - Refresh token to use
/// 
/// # Returns
/// JSON containing new access_token and access_expiry
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_refresh_token(
    refresh_token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if refresh_token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let token_str = CStr::from_ptr(refresh_token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let auth_service = crate::globals::get_auth_service()?;
        let (new_token, expiry) = block_on_async(auth_service.refresh_session(token_str))
            .map_err(|e| FFIError::internal(format!("Token refresh failed: {}", e)))?;
        
        let response = json!({
            "access_token": new_token,
            "access_expiry": expiry.to_rfc3339()
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Log out a user by revoking their tokens
/// 
/// # Arguments
/// * `logout_json` - JSON containing access_token and optional refresh_token
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_logout(
    logout_json: *const c_char,
) -> c_int {
    handle_result(|| unsafe {
        if logout_json.is_null() {
            return Err(FFIError::invalid_argument("Null logout JSON provided"));
        }
        
        let json_str = CStr::from_ptr(logout_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid logout JSON string"))?;
        
        let logout_data: serde_json::Value = parse_json_payload(json_str)?;
        
        let access_token = logout_data["access_token"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing access_token in logout JSON"))?;
        
        let refresh_token = logout_data["refresh_token"].as_str();
        
        let auth_service = crate::globals::get_auth_service()?;
        let auth_context = create_auth_context_from_token(access_token)?;
        
        block_on_async(auth_service.logout(&auth_context, access_token, refresh_token))
            .map_err(|e| FFIError::internal(format!("Logout failed: {}", e)))?;
        
        Ok(())
    })
}

/// Hash a plaintext password using Argon2
/// 
/// # Arguments
/// * `password` - Plain text password to hash
/// 
/// # Returns
/// Hashed password string
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_hash_password(
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
        let hash = auth_service.hash_password(pwd_str)
            .map_err(|e| FFIError::internal(format!("Password hashing failed: {}", e)))?;

        let c_hash = CString::new(hash)
            .map_err(|e| FFIError::internal(format!("CString creation failed: {}", e)))?;
        
        *result = c_hash.into_raw();
        Ok(())
    })
}

// ============================================================================
// USER MANAGEMENT FUNCTIONS
// ============================================================================

/// Create a new user
/// 
/// # Arguments
/// * `user_json` - JSON containing new user data
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing the created user information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_create_user(
    user_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if user_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(user_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid user JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let new_user: NewUser = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        let created_user = block_on_async(user_service.create_user(new_user, &auth_context))
            .map_err(|e| FFIError::internal(format!("User creation failed: {}", e)))?;
        
        let response = json!({
            "id": created_user.id.to_string(),
            "email": created_user.email,
            "name": created_user.name,
            "role": created_user.role.as_str(),
            "active": created_user.active,
            "last_login": created_user.last_login.map(|dt| dt.to_rfc3339()),
            "created_at": created_user.created_at.to_rfc3339(),
            "updated_at": created_user.updated_at.to_rfc3339(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Get a user by ID
/// 
/// # Arguments
/// * `user_id` - UUID of the user to retrieve
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing user information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_get_user(
    user_id: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if user_id.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let id_str = CStr::from_ptr(user_id).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid user ID string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let id = Uuid::parse_str(id_str)
            .map_err(|_| FFIError::invalid_argument("Invalid UUID format"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        let user = block_on_async(user_service.get_user(id, &auth_context))
            .map_err(|e| FFIError::internal(format!("Failed to get user: {}", e)))?;
        
        let response = json!({
            "id": user.id.to_string(),
            "email": user.email,
            "name": user.name,
            "role": user.role.as_str(),
            "active": user.active,
            "last_login": user.last_login.map(|dt| dt.to_rfc3339()),
            "created_at": user.created_at.to_rfc3339(),
            "updated_at": user.updated_at.to_rfc3339(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Get all users
/// 
/// # Arguments
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON array containing all users
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_get_all_users(
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        let users = block_on_async(user_service.get_all_users(&auth_context))
            .map_err(|e| FFIError::internal(format!("Failed to get users: {}", e)))?;
        
        let users_json: Vec<serde_json::Value> = users.into_iter().map(|user| {
            json!({
                "id": user.id.to_string(),
                "email": user.email,
                "name": user.name,
                "role": user.role.as_str(),
                "active": user.active,
                "last_login": user.last_login.map(|dt| dt.to_rfc3339()),
                "created_at": user.created_at.to_rfc3339(),
                "updated_at": user.updated_at.to_rfc3339(),
            })
        }).collect();
        
        *result = create_json_response(users_json)?;
        Ok(())
    })
}

/// Update a user
/// 
/// # Arguments
/// * `user_id` - UUID of the user to update
/// * `update_json` - JSON containing update data
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing updated user information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_update_user(
    user_id: *const c_char,
    update_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if user_id.is_null() || update_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let id_str = CStr::from_ptr(user_id).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid user ID string"))?;
        
        let json_str = CStr::from_ptr(update_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid update JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let id = Uuid::parse_str(id_str)
            .map_err(|_| FFIError::invalid_argument("Invalid UUID format"))?;
        
        let mut update_user: UpdateUser = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        // Set the updated_by_user_id from auth context
        update_user.updated_by_user_id = auth_context.user_id;
        
        let user_service = crate::globals::get_user_service()?;
        let updated_user = block_on_async(user_service.update_user(id, update_user, &auth_context))
            .map_err(|e| FFIError::internal(format!("User update failed: {}", e)))?;
        
        let response = json!({
            "id": updated_user.id.to_string(),
            "email": updated_user.email,
            "name": updated_user.name,
            "role": updated_user.role.as_str(),
            "active": updated_user.active,
            "last_login": updated_user.last_login.map(|dt| dt.to_rfc3339()),
            "created_at": updated_user.created_at.to_rfc3339(),
            "updated_at": updated_user.updated_at.to_rfc3339(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Hard delete a user (permanent deletion)
/// 
/// # Arguments
/// * `user_id` - UUID of the user to delete
/// * `token` - Access token for authentication
/// 
/// # Returns
/// Success/failure status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_hard_delete_user(
    user_id: *const c_char,
    token: *const c_char,
) -> c_int {
    handle_result(|| unsafe {
        if user_id.is_null() || token.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let id_str = CStr::from_ptr(user_id).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid user ID string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let id = Uuid::parse_str(id_str)
            .map_err(|_| FFIError::invalid_argument("Invalid UUID format"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        block_on_async(user_service.hard_delete_user(id, &auth_context))
            .map_err(|e| FFIError::internal(format!("User deletion failed: {}", e)))?;
        
        Ok(())
    })
}

/// Get current user profile
/// 
/// # Arguments
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing current user information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_get_current_user(
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        let user = block_on_async(user_service.get_current_user(&auth_context))
            .map_err(|e| FFIError::internal(format!("Failed to get current user: {}", e)))?;
        
        let response = json!({
            "id": user.id.to_string(),
            "email": user.email,
            "name": user.name,
            "role": user.role.as_str(),
            "active": user.active,
            "last_login": user.last_login.map(|dt| dt.to_rfc3339()),
            "created_at": user.created_at.to_rfc3339(),
            "updated_at": user.updated_at.to_rfc3339(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Update current user's profile
/// 
/// # Arguments
/// * `update_json` - JSON containing update data
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing updated user information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_update_current_user(
    update_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if update_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(update_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid update JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let mut update_user: UpdateUser = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        // Set the updated_by_user_id from auth context
        update_user.updated_by_user_id = auth_context.user_id;
        
        let user_service = crate::globals::get_user_service()?;
        let updated_user = block_on_async(user_service.update_current_user(update_user, &auth_context))
            .map_err(|e| FFIError::internal(format!("Current user update failed: {}", e)))?;
        
        let response = json!({
            "id": updated_user.id.to_string(),
            "email": updated_user.email,
            "name": updated_user.name,
            "role": updated_user.role.as_str(),
            "active": updated_user.active,
            "last_login": updated_user.last_login.map(|dt| dt.to_rfc3339()),
            "created_at": updated_user.created_at.to_rfc3339(),
            "updated_at": updated_user.updated_at.to_rfc3339(),
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Change password with old password verification
/// 
/// # Arguments
/// * `password_change_json` - JSON containing old_password and new_password
/// * `token` - Access token for authentication
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_change_password(
    password_change_json: *const c_char,
    token: *const c_char,
) -> c_int {
    handle_result(|| unsafe {
        if password_change_json.is_null() || token.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(password_change_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid password change JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let password_data: serde_json::Value = parse_json_payload(json_str)?;
        
        let old_password = password_data["old_password"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing old_password in JSON"))?;
        
        let new_password = password_data["new_password"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing new_password in JSON"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        block_on_async(user_service.change_password(old_password, new_password, &auth_context))
            .map_err(|e| FFIError::internal(format!("Password change failed: {}", e)))?;
        
        Ok(())
    })
}

/// Check if email is unique
/// 
/// # Arguments
/// * `email_check_json` - JSON containing email and optional exclude_id
/// 
/// # Returns
/// JSON containing is_unique boolean
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_is_email_unique(
    email_check_json: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_result(|| unsafe {
        if email_check_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(email_check_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid email check JSON string"))?;
        
        let email_data: serde_json::Value = parse_json_payload(json_str)?;
        
        let email = email_data["email"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing email in JSON"))?;
        
        let exclude_id = email_data["exclude_id"].as_str()
            .and_then(|s| Uuid::parse_str(s).ok());
        
        let user_service = crate::globals::get_user_service()?;
        let is_unique = block_on_async(user_service.is_email_unique(email, exclude_id))
            .map_err(|e| FFIError::internal(format!("Email uniqueness check failed: {}", e)))?;
        
        let response = json!({
            "is_unique": is_unique
        });
        
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Initialize default accounts (admin setup)
/// 
/// # Arguments
/// * `token` - Access token for authentication
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_initialize_default_accounts(
    token: *const c_char,
) -> c_int {
    handle_result(|| unsafe {
        if token.is_null() {
            return Err(FFIError::invalid_argument("Null token provided"));
        }
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let user_service = crate::globals::get_user_service()?;
        block_on_async(user_service.initialize_default_accounts(&auth_context))
            .map_err(|e| FFIError::internal(format!("Default accounts initialization failed: {}", e)))?;
        
        Ok(())
    })
}

// ============================================================================
// MEMORY MANAGEMENT
// ============================================================================

/// Free a string allocated by the auth FFI functions
/// 
/// # Safety
/// This function should only be called with pointers returned from auth FFI functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn auth_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); }
    }
}

// ============================================================================
// LEGACY COMPATIBILITY (keeping old function names)
// ============================================================================

/// Legacy login function (redirects to auth_login)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn login(
    email: *const c_char,
    password: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    // Convert individual parameters to JSON format
    if email.is_null() || password.is_null() {
        return handle_result(|| Err(FFIError::invalid_argument("Null pointer(s) provided")));
    }
    
    let email_str = match unsafe { CStr::from_ptr(email) }.to_str() {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::invalid_argument("Invalid email string"))),
    };
    
    let password_str = match unsafe { CStr::from_ptr(password) }.to_str() {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::invalid_argument("Invalid password string"))),
    };
    
    let credentials_json = json!({
        "email": email_str,
        "password": password_str
    });
    
    let json_string = match serde_json::to_string(&credentials_json) {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::internal("JSON serialization failed".to_string()))),
    };
    
    let c_json = match CString::new(json_string) {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::internal("CString creation failed".to_string()))),
    };
    
    unsafe { auth_login(c_json.as_ptr(), result) }
}

/// Legacy verify_token function (redirects to auth_verify_token)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verify_token(
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    unsafe { auth_verify_token(token, result) }
}

/// Legacy refresh_token function (redirects to auth_refresh_token)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn refresh_token(
    refresh_token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    unsafe { auth_refresh_token(refresh_token, result) }
}

/// Legacy logout function (redirects to auth_logout)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn logout(
    token: *const c_char,
    refresh_token: *const c_char,
) -> c_int {
    // Convert individual parameters to JSON format
    if token.is_null() {
        return handle_result(|| Err(FFIError::invalid_argument("Null token provided")));
    }
    
    let token_str = match unsafe { CStr::from_ptr(token) }.to_str() {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::invalid_argument("Invalid token string"))),
    };
    
    let refresh_token_str = if refresh_token.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(refresh_token) }.to_str() {
            Ok(s) => Some(s),
            Err(_) => return handle_result(|| Err(FFIError::invalid_argument("Invalid refresh token string"))),
        }
    };
    
    let logout_json = json!({
        "access_token": token_str,
        "refresh_token": refresh_token_str
    });
    
    let json_string = match serde_json::to_string(&logout_json) {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::internal("JSON serialization failed".to_string()))),
    };
    
    let c_json = match CString::new(json_string) {
        Ok(s) => s,
        Err(_) => return handle_result(|| Err(FFIError::internal("CString creation failed".to_string()))),
    };
    
    unsafe { auth_logout(c_json.as_ptr()) }
}

/// Legacy hash_password function (redirects to auth_hash_password)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hash_password(
    password: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    unsafe { auth_hash_password(password, result) }
}

// Removing the problematic free_string that redirected to auth_free.
// The canonical `free_string` is now defined in `src/ffi/core.rs`.
// `auth_free` (defined earlier in this file) can still be used if needed
// for auth-specific logic, but its current implementation is also a generic CString freer.
// Swift code should primarily use the global `free_string` for generic CString freeing.
// If `auth_free` serves the exact same purpose and is called by Swift, it will also work.
/*
/// Legacy free_string function (redirects to auth_free)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    unsafe { auth_free(ptr) }
}
*/