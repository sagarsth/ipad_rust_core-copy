// src/ffi/user.rs
// ============================================================================
// FFI bindings for the `UserService`.
// All heavy–lifting logic lives in the domain/service layer.  These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `user_free` function exported below.  Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `user_free` more than
//      once – double-free will crash.
//   •  All pointers received from Swift are assumed to be valid, non-NULL,
//      null-terminated UTF-8 strings.  We defensively validate this and return
//      `ErrorCode::InvalidArgument` when the contract is violated.
//
// JSON contracts:
//   For calls that need a complex payload we expect a single JSON object that
//   bundles the request data together with an `auth` context.  The exact shape
//   of each payload is documented above every function.
// ----------------------------------------------------------------------------

use crate::ffi::{handle_status_result, error::{FFIError}};
use crate::domains::user::types::{NewUser, UpdateUser, UserResponse};
use crate::auth::AuthContext;
use crate::types::UserRole;
use crate::globals;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::Deserialize;
use tokio::runtime::Runtime;

// ---------------------------------------------------------------------------
// Helper utilities
// ---------------------------------------------------------------------------

/// Run an async future to completion on a freshly-spun Tokio runtime.
fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let rt = Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(future)
}

/// DTO mirroring the subset of `AuthContext` that we expect to receive from
/// Swift.  We purposefully keep this separate so that the public JSON contract
/// is stable even if the internal `AuthContext` struct evolves.
#[derive(Deserialize)]
struct AuthCtxDto {
    user_id: String,
    role: String,
    device_id: String,
    offline_mode: bool,
}

impl TryFrom<AuthCtxDto> for AuthContext {
    type Error = FFIError;

    fn try_from(value: AuthCtxDto) -> Result<Self, Self::Error> {
        Ok(AuthContext::new(
            Uuid::parse_str(&value.user_id)
                .map_err(|_| FFIError::invalid_argument("invalid user_id"))?,
            UserRole::from_str(&value.role)
                .ok_or_else(|| FFIError::invalid_argument("invalid role"))?,
            value.device_id,
            value.offline_mode,
        ))
    }
}

// ---------------------------------------------------------------------------
// FFI – Create User
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "user": { NewUser },
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload {
            user: NewUser,
            auth: AuthCtxDto,
        }

        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        let user = block_on_async(svc.create_user(payload.user, &auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;
        let resp: UserResponse = user.into();

        let json_resp = serde_json::to_string(&resp)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Get User by ID
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "id": "uuid",
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload {
            id: String,
            auth: AuthCtxDto,
        }
        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let id = Uuid::parse_str(&payload.id)
            .map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        let user = block_on_async(svc.get_user_response(id, &auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;

        let json_resp = serde_json::to_string(&user)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Get All Users
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_get_all(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;
        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        let users = block_on_async(svc.get_all_user_responses(&auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;

        let json_resp = serde_json::to_string(&users)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Update User
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "id": "uuid",
//   "update": { UpdateUser (sans updated_by_user_id) },
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateUserOptional,
            auth: AuthCtxDto,
        }

        // Helper DTO for optional fields because `UpdateUser` requires
        // `updated_by_user_id` – we'll inject that server-side.
        #[derive(Deserialize, Default)]
        struct UpdateUserOptional {
            email: Option<String>,
            password: Option<String>,
            name: Option<String>,
            role: Option<String>,
            active: Option<bool>,
        }

        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let id = Uuid::parse_str(&payload.id)
            .map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        // Build full UpdateUser by injecting `updated_by_user_id`.
        let update = UpdateUser {
            email: payload.update.email,
            password: payload.update.password,
            name: payload.update.name,
            role: payload.update.role,
            active: payload.update.active,
            updated_by_user_id: auth_ctx.user_id,
        };

        let user = block_on_async(svc.update_user(id, update, &auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;
        let resp: UserResponse = user.into();
        let json_resp = serde_json::to_string(&resp)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Hard Delete User
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "id": "uuid",
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_hard_delete(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let id = Uuid::parse_str(&payload.id)
            .map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;
        block_on_async(svc.hard_delete_user(id, &auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Is Email Unique
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "email": "foo@example.com",
//   "exclude_id": "optional-uuid-or-null",
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_is_email_unique(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload {
            email: String,
            exclude_id: Option<String>,
            auth: AuthCtxDto,
        }
        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let exclude = match payload.exclude_id {
            Some(ref id_str) => Some(Uuid::parse_str(id_str)
                .map_err(|_| FFIError::invalid_argument("uuid"))?),
            None => None,
        };
        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        let is_unique = block_on_async(svc.is_email_unique(&payload.email, exclude))
            .map_err(|e| FFIError::from_service_error(e))?;

        let json_resp = serde_json::to_string(&is_unique)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// FFI – Change Password
// ---------------------------------------------------------------------------
// Expected JSON payload:
// {
//   "old_password": "...",
//   "new_password": "...",
//   "auth": { AuthCtxDto }
// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_change_password(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        if payload_json.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json_str = CStr::from_ptr(payload_json)
            .to_str()
            .map_err(|_| FFIError::invalid_argument("utf8"))?;

        #[derive(Deserialize)]
        struct Payload { old_password: String, new_password: String, auth: AuthCtxDto }
        let payload: Payload = serde_json::from_str(json_str)
            .map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let auth_ctx: AuthContext = payload.auth.try_into()?;
        let svc = globals::get_user_service()?;

        block_on_async(svc.change_password(&payload.old_password, &payload.new_password, &auth_ctx))
            .map_err(|e| FFIError::from_service_error(e))?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Memory management helper
// ---------------------------------------------------------------------------
#[unsafe(no_mangle)]
pub unsafe extern "C" fn user_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); }
    }
} 