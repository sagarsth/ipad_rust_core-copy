use crate::ffi::{handle_status_result};
use crate::ffi::error::FFIError;
use crate::globals;
use crate::auth::AuthContext;
use crate::types::PaginationParams;
use crate::domains::funding::types::{NewProjectFunding, UpdateProjectFunding};

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::Deserialize;
use tokio::runtime::Runtime;
use crate::ffi_export;
use crate::domains::funding::types::*;

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

ffi_export! {
    service_getter: globals::get_funding_service,
    prefix: "funding",
    methods: [
        { fn create_funding(new_funding: NewFunding, auth: AuthCtxDto) -> FundingResponse; },
        { fn create_funding_with_documents(new_funding: NewFunding,
                                    documents: Vec<(Vec<u8>, String, Option<String>)>,
                                    document_type_id: uuid::Uuid,
                                    auth: AuthCtxDto)
      -> (FundingResponse, Vec<Result<MediaDocumentResponse, ServiceError>>); },
        { fn get_funding_by_id(id: uuid::Uuid, include: Option<Vec<FundingInclude>>, auth: AuthCtxDto) -> FundingResponse; },
        { fn list_fundings(params: PaginationParams, include: Option<Vec<FundingInclude>>, auth: AuthCtxDto) -> PaginatedResult<FundingResponse>; },
        { fn update_funding(id: uuid::Uuid, update_data: UpdateFunding, auth: AuthCtxDto) -> FundingResponse; },
        { fn delete_funding(id: uuid::Uuid, hard_delete: bool, auth: AuthCtxDto) -> DeleteResult; },
        { fn find_fundings_by_donor(donor_id: uuid::Uuid, params: PaginationParams, include: Option<Vec<FundingInclude>>, auth: AuthCtxDto) -> PaginatedResult<FundingResponse>; },
        { fn find_fundings_by_project(project_id: uuid::Uuid, params: PaginationParams, include: Option<Vec<FundingInclude>>, auth: AuthCtxDto) -> PaginatedResult<FundingResponse>; },
        { fn find_fundings_by_date_range(start_rfc3339: String, end_rfc3339: String, params: PaginationParams, include: Option<Vec<FundingInclude>>, auth: AuthCtxDto) -> PaginatedResult<FundingResponse>; },
    ]
}

// ---------------------------------------------------------------------------
// MEMORY
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn funding_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
} 