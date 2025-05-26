use crate::ffi::{handle_status_result};
use crate::ffi::error::FFIError;
use crate::globals;
use crate::auth::AuthContext;
use crate::types::PaginationParams;
use crate::domains::livelihood::types::{NewLivelihood, UpdateLivelihood};

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::Deserialize;
use tokio::runtime::Runtime;
use crate::ffi_export;
use crate::domains::livelihood::types::*;

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

ffi_export! {
    service_getter: globals::get_livelihood_service,
    prefix: "livelihood",
    methods: [
        { fn create_livelihood(new_livelihood: NewLivelihood, auth: AuthCtxDto) -> LivelihoodResponse; },
        { fn get_livelihood_by_id(id: uuid::Uuid, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> LivelihoodResponse; },
        { fn list_livelihoods(params: PaginationParams, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> PaginatedResult<LivelihoodResponse>; },
        { fn update_livelihood(id: uuid::Uuid, update_data: UpdateLivelihood, auth: AuthCtxDto) -> LivelihoodResponse; },
        { fn delete_livelihood(id: uuid::Uuid, hard_delete: bool, auth: AuthCtxDto) -> DeleteResult; },
        { fn find_livelihoods_by_project(project_id: uuid::Uuid, params: PaginationParams, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> PaginatedResult<LivelihoodResponse>; },
        { fn find_livelihoods_by_date_range(start_rfc3339: String, end_rfc3339: String, params: PaginationParams, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> PaginatedResult<LivelihoodResponse>; },
        { fn find_livelihoods_by_type(livelihood_type: String, params: PaginationParams, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> PaginatedResult<LivelihoodResponse>; },
        { fn find_livelihoods_by_status(status: String, params: PaginationParams, include: Option<Vec<LivelihoodInclude>>, auth: AuthCtxDto) -> PaginatedResult<LivelihoodResponse>; },
        { fn get_livelihood_statistics(auth: AuthCtxDto) -> LivelihoodStatistics; },
        { fn get_type_distribution(auth: AuthCtxDto) -> std::collections::HashMap<String, i64>; },
    ]
} 

// ---------------------------------------------------------------------------



// ---------------------------------------------------------------------------
// MEMORY
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn livelihood_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

