// src/ffi/project.rs
// =========================================================================
// PROJECT DOMAIN – FFI BINDINGS (CRUD ONLY)
// =========================================================================
use crate::ffi::{handle_status_result};
use crate::ffi::error::FFIError;
use crate::globals;
use crate::auth::AuthContext;
use crate::types::PaginationParams;
use crate::domains::project::types::{NewProject, UpdateProject};

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::Deserialize;
use tokio::runtime::Runtime;

fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let rt = Runtime::new().expect("tokio");
    rt.block_on(future)
}

#[derive(Deserialize)]
struct AuthCtxDto { user_id: String, role: String, device_id: String, offline_mode: bool }

impl TryFrom<AuthCtxDto> for AuthContext {
    type Error = FFIError;
    fn try_from(v: AuthCtxDto) -> Result<Self, Self::Error> {
        use crate::types::UserRole;
        Ok(AuthContext::new(
            Uuid::parse_str(&v.user_id).map_err(|_| FFIError::invalid_argument("user_id"))?,
            UserRole::from_str(&v.role).ok_or_else(|| FFIError::invalid_argument("role"))?,
            v.device_id,
            v.offline_mode,
        ))
    }
}

macro_rules! ensure_ptr { ($p:expr) => { if $p.is_null() { return Err(FFIError::invalid_argument("null ptr")); } }; }

// --------------------------------------------------------------------
// CRUD
// --------------------------------------------------------------------

/// Payload { "new_project": NewProject, "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { new_project: NewProject, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        let resp = block_on_async(svc.create_project(p.new_project, &auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&resp).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

/// Payload { "id": "uuid", "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { id: String, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        let resp = block_on_async(svc.get_project_by_id(id, None, &auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&resp).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

/// Payload { "page": u32, "per_page": u32, "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { page: Option<u32>, per_page: Option<u32>, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = PaginationParams { page: p.page.unwrap_or(1), per_page: p.per_page.unwrap_or(20) };
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        let pg = block_on_async(svc.list_projects(params, None, &auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&pg).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

/// Payload { "id": "uuid", "update": UpdateProject, "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { id: String, update: UpdateProject, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        let updated = block_on_async(svc.update_project(id, p.update, &auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&updated).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

/// Payload { "id": "uuid", "hard_delete": bool, "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_delete(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { id: String, hard_delete: Option<bool>, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        block_on_async(svc.delete_project(id, p.hard_delete.unwrap_or(false), &auth)).map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
} 