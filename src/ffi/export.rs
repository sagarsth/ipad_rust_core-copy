use std::os::raw::{c_char, c_int};
use std::ffi::{CStr, CString};
use tokio::runtime::Runtime;
use crate::ffi::error::{FFIError, FFIResult};
use crate::domains::export::types::{ExportRequest, ExportSummary, EntityFilter};
use crate::domains::export::repository::SqliteExportJobRepository;
use crate::domains::export::service::{ExportService, ExportServiceImpl};
use crate::auth::AuthContext;
use uuid::Uuid;
use serde::Deserialize;
use crate::types::UserRole;
use std::str::FromStr;

// ---------- Helpers ---------------------------------------------------------
fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let rt = Runtime::new().expect("tokio runtime");
    rt.block_on(future)
}

fn build_service() -> FFIResult<impl ExportService> {
    let pool = crate::globals::get_db_pool()?;
    let file_storage = crate::globals::get_file_storage_service()?;
    let job_repo = SqliteExportJobRepository::new(pool);
    Ok(ExportServiceImpl::new(std::sync::Arc::new(job_repo), file_storage))
}

/// C-friendly representation of AuthContext coming from Swift.
#[derive(Deserialize)]
struct AuthCtxDto {
    user_id: String,
    role: String,
    device_id: String,
    offline_mode: bool,
}

fn dto_to_auth(dto: AuthCtxDto) -> Result<AuthContext, FFIError> {
    Ok(AuthContext::new(
        Uuid::parse_str(&dto.user_id).map_err(|_| FFIError::invalid_argument("bad user_id"))?,
        UserRole::from_str(&dto.role).ok_or(FFIError::invalid_argument("bad role"))?,
        dto.device_id,
        dto.offline_mode,
    ))
}

// ---------- FFI functions ---------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_create(request_json: *const c_char, result: *mut *mut c_char) -> c_int {
    crate::ffi::handle_status_result(|| unsafe {
        if request_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let json = CStr::from_ptr(request_json).to_str().map_err(|_| FFIError::invalid_argument("invalid utf8"))?;

        #[derive(Deserialize)]
        struct Payload {
            request: ExportRequest,
            auth: AuthCtxDto,
        }

        let payload: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json parse: {e}")))?;

        let service = build_service()?;
        let auth_ctx = dto_to_auth(payload.auth)?;

        let summary: ExportSummary = block_on_async(service.create_export(payload.request, &auth_ctx))
            .map_err(|e| FFIError::internal(format!("{e}")))?;

        let json_resp = serde_json::to_string(&summary).map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_get_status(job_id_c: *const c_char, result: *mut *mut c_char) -> c_int {
    crate::ffi::handle_status_result(|| unsafe {
        if job_id_c.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("null ptr"));
        }
        let id_str = CStr::from_ptr(job_id_c).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        let job_id = Uuid::parse_str(id_str).map_err(|_| FFIError::invalid_argument("uuid"))?;

        let service = build_service()?;
        let summary = block_on_async(service.get_export_status(job_id))
            .map_err(|e| FFIError::internal(format!("{e}")))?;
        let json = serde_json::to_string(&summary).map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Allow Swift to free strings allocated by Rust.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
} 