use crate::ffi::{handle_status_result};
use crate::ffi::error::FFIError;
use crate::globals;
use crate::auth::AuthContext;
use crate::types::PaginationParams;
use crate::domains::workshop::types::{NewWorkshop, UpdateWorkshop, WorkshopParticipant, WorkshopStatistics};

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::Deserialize;
use tokio::runtime::Runtime;
use crate::ffi_export;

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

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

use crate::domains::workshop::types::*;
ffi_export! {
    service_getter: globals::get_workshop_service,
    prefix: "workshop",
    methods: [
        { fn create_workshop(new_workshop: NewWorkshop, auth: AuthCtxDto) -> WorkshopResponse; },
        { fn create_workshop_with_documents(new_workshop: NewWorkshop,
                                    documents: Vec<(Vec<u8>, String, Option<String>)>,
                                    document_type_id: uuid::Uuid,
                                    auth: AuthCtxDto)
      -> (WorkshopResponse, Vec<Result<MediaDocumentResponse, ServiceError>>); },
        { fn get_workshop_by_id(id: uuid::Uuid, include: Option<Vec<WorkshopInclude>>, auth: AuthCtxDto) -> WorkshopResponse; },
        { fn list_workshops(params: PaginationParams, include: Option<Vec<WorkshopInclude>>, auth: AuthCtxDto) -> PaginatedResult<WorkshopResponse>; },
        { fn update_workshop(id: uuid::Uuid, update_data: UpdateWorkshop, auth: AuthCtxDto) -> WorkshopResponse; },
        { fn delete_workshop(id: uuid::Uuid, hard_delete: bool, auth: AuthCtxDto) -> DeleteResult; },
        { fn find_workshops_by_project(project_id: uuid::Uuid, params: PaginationParams, include: Option<Vec<WorkshopInclude>>, auth: AuthCtxDto) -> PaginatedResult<WorkshopResponse>; },
        { fn find_workshops_by_date_range(start_rfc3339: String, end_rfc3339: String, params: PaginationParams, include: Option<Vec<WorkshopInclude>>, auth: AuthCtxDto) -> PaginatedResult<WorkshopResponse>; },
    ]
}

/// Payload { "workshop_id": "uuid", "participant_id": "uuid", "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn workshop_add_participant(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { workshop_id: String, participant_id: String, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let workshop_id = Uuid::parse_str(&p.workshop_id).map_err(|_| FFIError::invalid_argument("workshop_id"))?;
        let participant_id = Uuid::parse_str(&p.participant_id).map_err(|_| FFIError::invalid_argument("participant_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_workshop_service()?;
        let resp: WorkshopParticipant = block_on_async(svc.add_participant_to_workshop(workshop_id, participant_id, &auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&resp).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

/// Payload { "workshop_id": "uuid", "participant_id": "uuid", "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn workshop_remove_participant(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { workshop_id: String, participant_id: String, auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let workshop_id = Uuid::parse_str(&p.workshop_id).map_err(|_| FFIError::invalid_argument("workshop_id"))?;
        let participant_id = Uuid::parse_str(&p.participant_id).map_err(|_| FFIError::invalid_argument("participant_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_workshop_service()?;
        block_on_async(svc.remove_participant_from_workshop(workshop_id, participant_id, &auth)).map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

/// Payload { "auth": AuthCtxDto }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn workshop_get_statistics(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json); ensure_ptr!(result);
        let s = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct P { auth: AuthCtxDto }
        let p: P = serde_json::from_str(s).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_workshop_service()?;
        let stats: WorkshopStatistics = block_on_async(svc.get_workshop_statistics(&auth)).map_err(FFIError::from_service_error)?;
        *result = CString::new(serde_json::to_string(&stats).unwrap()).unwrap().into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// MEMORY
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn workshop_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
} 