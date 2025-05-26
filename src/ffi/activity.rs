// src/ffi/activity.rs
// ============================================================================
// FFI bindings for the `ActivityService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `activity_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `activity_free` more than
//      once – double-free will crash.
//   •  All pointers received from Swift are assumed to be valid, non-NULL,
//      null-terminated UTF-8 strings. We defensively validate this and return
//      `ErrorCode::InvalidArgument` when the contract is violated.
//
// JSON contracts:
//   For calls that need a complex payload we expect a single JSON object that
//   bundles the request data together with an `auth` context. The exact shape
//   of each payload is documented above every function.
// ----------------------------------------------------------------------------

use crate::ffi::{handle_status_result, error::FFIError};
use crate::domains::activity::types::{
    NewActivity, UpdateActivity, ActivityResponse, ActivityInclude
};
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::domains::core::repository::DeleteResult;
use crate::auth::AuthContext;
use crate::types::{UserRole, PaginationParams};
use crate::globals;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use base64;

// ---------------------------------------------------------------------------
// Helper utilities
// ---------------------------------------------------------------------------

/// Run an async future to completion on a freshly-spun Tokio runtime.
fn block_on_async<F, T, E>(future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    crate::ffi::block_on_async(future)
}

/// Ensure pointer is not null
macro_rules! ensure_ptr {
    ($ptr:expr) => {
        if $ptr.is_null() {
            return Err(FFIError::invalid_argument("null pointer"));
        }
    };
}

/// DTO mirroring the subset of `AuthContext` that we expect to receive from Swift
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

/// DTO for pagination parameters
#[derive(Deserialize)]
struct PaginationDto {
    page: Option<u32>,
    per_page: Option<u32>,
}

impl From<PaginationDto> for PaginationParams {
    fn from(dto: PaginationDto) -> Self {
        PaginationParams {
            page: dto.page.unwrap_or(1),
            per_page: dto.per_page.unwrap_or(20),
        }
    }
}

/// DTO for activity includes
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActivityIncludeDto {
    Project,
    Status,
    Documents,
    CreatedBy,
    UpdatedBy,
    All,
}

impl From<ActivityIncludeDto> for ActivityInclude {
    fn from(dto: ActivityIncludeDto) -> Self {
        match dto {
            ActivityIncludeDto::Project => ActivityInclude::Project,
            ActivityIncludeDto::Status => ActivityInclude::Status,
            ActivityIncludeDto::Documents => ActivityInclude::Documents,
            ActivityIncludeDto::CreatedBy => ActivityInclude::CreatedBy,
            ActivityIncludeDto::UpdatedBy => ActivityInclude::UpdatedBy,
            ActivityIncludeDto::All => ActivityInclude::All,
        }
    }
}

// ---------------------------------------------------------------------------
// Basic CRUD Operations
// ---------------------------------------------------------------------------

/// Create a new activity
/// Expected JSON payload:
/// {
///   "activity": { NewActivity },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            activity: NewActivity,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let activity = block_on_async(svc.create_activity(p.activity, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activity)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Create a new activity with documents
/// Expected JSON payload:
/// {
///   "activity": { NewActivity },
///   "documents": [{"file_data": "base64", "filename": "string", "linked_field": "optional_string"}, ...],
///   "document_type_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_create_with_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct DocumentData {
            file_data: String, // base64 encoded
            filename: String,
            linked_field: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            activity: NewActivity,
            documents: Vec<DocumentData>,
            document_type_id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        // Decode all documents
        let mut documents = Vec::new();
        for doc in p.documents {
            let data = base64::decode(&doc.file_data)
                .map_err(|_| FFIError::invalid_argument("invalid base64 file data"))?;
            documents.push((data, doc.filename, doc.linked_field));
        }
        
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let (activity, doc_results) = block_on_async(svc.create_activity_with_documents(
            p.activity, documents, document_type_id, &auth
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct CreateWithDocsResponse {
            activity: ActivityResponse,
            document_results: Vec<Result<crate::domains::document::types::MediaDocumentResponse, String>>,
        }
        
        let response = CreateWithDocsResponse {
            activity,
            document_results: doc_results.into_iter().map(|r| r.map_err(|e| e.to_string())).collect(),
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activity by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let svc = globals::get_activity_service()?;
        let activity = block_on_async(svc.get_activity_by_id(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activity)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List activities for a project with pagination
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "pagination": { PaginationDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_list_for_project(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
            pagination: Option<PaginationDto>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let project_id = Uuid::parse_str(&p.project_id).map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let svc = globals::get_activity_service()?;
        let activities = block_on_async(svc.list_activities_for_project(project_id, params, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activities)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Update activity
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "update": { UpdateActivity },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateActivity,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let activity = block_on_async(svc.update_activity(id, p.update, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activity)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete activity (soft or hard delete) - RETURNS DeleteResult!
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "hard_delete": bool,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_delete(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            hard_delete: Option<bool>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        // Important: Capture the DeleteResult
        let delete_result = block_on_async(svc.delete_activity(id, p.hard_delete.unwrap_or(false), &auth))
            .map_err(FFIError::from_service_error)?;
        
        // Serialize and return the DeleteResult
        let json_resp = serde_json::to_string(&delete_result)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Filtered Queries
// ---------------------------------------------------------------------------

/// Find activities by date range
/// Expected JSON payload:
/// {
///   "start_date": "2023-01-01T00:00:00Z",
///   "end_date": "2023-12-31T23:59:59Z",
///   "pagination": { PaginationDto },
///   "include": [ActivityIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_by_date_range(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            start_date: String,
            end_date: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<ActivityInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_activity_service()?;
        let activities = block_on_async(svc.find_activities_by_date_range(&p.start_date, &p.end_date, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activities)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activity progress summary for a project
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_project_progress_summary(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let project_id = Uuid::parse_str(&p.project_id).map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        // Get all activities for the project and calculate summary
        let activities = block_on_async(svc.list_activities_for_project(project_id, PaginationParams::default(), &auth))
            .map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct ProgressSummary {
            project_id: Uuid,
            total_activities: usize,
            activities_with_targets: usize,
            activities_with_actuals: usize,
            average_progress_percentage: Option<f64>,
            activities_completed: usize, // progress >= 100%
            activities_in_progress: usize, // 0% < progress < 100%
            activities_not_started: usize, // progress = 0% or no actual value
        }
        
        let total_activities = activities.items.len();
        let mut activities_with_targets = 0;
        let mut activities_with_actuals = 0;
        let mut total_progress = 0.0;
        let mut progress_count = 0;
        let mut activities_completed = 0;
        let mut activities_in_progress = 0;
        let mut activities_not_started = 0;
        
        for activity in &activities.items {
            if activity.target_value.is_some() {
                activities_with_targets += 1;
            }
            if activity.actual_value.is_some() {
                activities_with_actuals += 1;
            }
            
            if let Some(progress) = activity.progress_percentage {
                total_progress += progress;
                progress_count += 1;
                
                if progress >= 100.0 {
                    activities_completed += 1;
                } else if progress > 0.0 {
                    activities_in_progress += 1;
                } else {
                    activities_not_started += 1;
                }
            } else {
                activities_not_started += 1;
            }
        }
        
        let average_progress_percentage = if progress_count > 0 {
            Some(total_progress / progress_count as f64)
        } else {
            None
        };
        
        let summary = ProgressSummary {
            project_id,
            total_activities,
            activities_with_targets,
            activities_with_actuals,
            average_progress_percentage,
            activities_completed,
            activities_in_progress,
            activities_not_started,
        };
        
        let json_resp = serde_json::to_string(&summary)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activities that are behind target (actual < target)
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "pagination": { PaginationDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_behind_target(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
            pagination: Option<PaginationDto>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let project_id = Uuid::parse_str(&p.project_id).map_err(|_| FFIError::invalid_argument("project_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let params: PaginationParams = p.pagination.map(|dto| dto.into()).unwrap_or_default();
        let svc = globals::get_activity_service()?;

        // Fetch all activities for the project (or a large enough page to cover most cases for client-side-like filtering)
        // Consider if the service layer should offer more dedicated filtering if performance becomes an issue.
        let all_activities_result = block_on_async(svc.list_activities_for_project(
            project_id, 
            PaginationParams { page: 1, per_page: 10000 }, // Fetch a large set
            &auth
        )).map_err(FFIError::from_service_error)?;

        let behind_target: Vec<ActivityResponse> = all_activities_result.items.into_iter()
            .filter(|activity| {
                if let (Some(actual), Some(target)) = (activity.actual_value, activity.target_value) {
                    actual < target && target > 0.0 // Ensure target is positive to avoid trivial matches
                } else {
                    false
                }
            })
            .collect();
        
        let total = behind_target.len() as u64;
        let start = ((params.page.saturating_sub(1)) * params.per_page) as usize; // u32 to usize
        let end = std::cmp::min(start + (params.per_page as usize), total as usize);
        
        let paginated_items = if start < total as usize {
            behind_target[start..end].to_vec()
        } else {
            Vec::new()
        };

        let paginated_response = crate::types::PaginatedResult {
            items: paginated_items,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages: if params.per_page > 0 { (total as f64 / params.per_page as f64).ceil() as u32 } else { 0 },
        };
        
        let json_resp = serde_json::to_string(&paginated_response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activities that have exceeded their target (actual > target)
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "pagination": { PaginationDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_exceeding_target(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
            pagination: Option<PaginationDto>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let project_id = Uuid::parse_str(&p.project_id).map_err(|_| FFIError::invalid_argument("project_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let params: PaginationParams = p.pagination.map(|dto| dto.into()).unwrap_or_default();
        let svc = globals::get_activity_service()?;

        let all_activities_result = block_on_async(svc.list_activities_for_project(
            project_id, 
            PaginationParams { page: 1, per_page: 10000 }, // Fetch a large set
            &auth
        )).map_err(FFIError::from_service_error)?;

        let exceeding_target: Vec<ActivityResponse> = all_activities_result.items.into_iter()
            .filter(|activity| {
                if let (Some(actual), Some(target)) = (activity.actual_value, activity.target_value) {
                    actual > target && target > 0.0 // Ensure target is positive
                } else {
                    false
                }
            })
            .collect();
        
        let total = exceeding_target.len() as u64;
        let start = ((params.page.saturating_sub(1)) * params.per_page) as usize;
        let end = std::cmp::min(start + (params.per_page as usize), total as usize);
        
        let paginated_items = if start < total as usize {
            exceeding_target[start..end].to_vec()
        } else {
            Vec::new()
        };

        let paginated_response = crate::types::PaginatedResult {
            items: paginated_items,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages: if params.per_page > 0 { (total as f64 / params.per_page as f64).ceil() as u32 } else { 0 },
        };
        
        let json_resp = serde_json::to_string(&paginated_response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activities without targets set
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "pagination": { PaginationDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_without_targets(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
            pagination: Option<PaginationDto>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let project_id = Uuid::parse_str(&p.project_id).map_err(|_| FFIError::invalid_argument("project_id"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let params: PaginationParams = p.pagination.map(|dto| dto.into()).unwrap_or_default();
        let svc = globals::get_activity_service()?;

        let all_activities_result = block_on_async(svc.list_activities_for_project(
            project_id, 
            PaginationParams { page: 1, per_page: 10000 }, // Fetch a large set
            &auth
        )).map_err(FFIError::from_service_error)?;

        let without_targets: Vec<ActivityResponse> = all_activities_result.items.into_iter()
            .filter(|activity| activity.target_value.is_none())
            .collect();
        
        let total = without_targets.len() as u64;
        let start = ((params.page.saturating_sub(1)) * params.per_page) as usize;
        let end = std::cmp::min(start + (params.per_page as usize), total as usize);
        
        let paginated_items = if start < total as usize {
            without_targets[start..end].to_vec()
        } else {
            Vec::new()
        };

        let paginated_response = crate::types::PaginatedResult {
            items: paginated_items,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages: if params.per_page > 0 { (total as f64 / params.per_page as f64).ceil() as u32 } else { 0 },
        };
        
        let json_resp = serde_json::to_string(&paginated_response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Document Integration
// ---------------------------------------------------------------------------

/// Upload a single document for activity
/// Expected JSON payload:
/// {
///   "activity_id": "uuid",
///   "file_data": "base64_encoded_file_data",
///   "original_filename": "string",
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "linked_field": "optional_string",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_upload_document(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            activity_id: String,
            file_data: String, // base64 encoded
            original_filename: String,
            title: Option<String>,
            document_type_id: String,
            linked_field: Option<String>,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        // Decode base64 file data
        let file_data = base64::decode(&p.file_data)
            .map_err(|_| FFIError::invalid_argument("invalid base64 file data"))?;
        
        let activity_id = Uuid::parse_str(&p.activity_id)
            .map_err(|_| FFIError::invalid_argument("invalid activity_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let document = block_on_async(svc.upload_document_for_activity(
            activity_id,
            file_data,
            p.original_filename,
            p.title,
            document_type_id,
            p.linked_field,
            sync_priority,
            compression_priority,
            &auth,
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&document)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Upload multiple documents for an existing activity record
/// Expected JSON payload:
/// {
///   "activity_id": "uuid",
///   "documents": [
///     {
///       "file_data": "base64_encoded_file_data",
///       "original_filename": "string"
///     },
///     ...
///   ],
///   "title": "optional_string_applied_to_all_documents",
///   "document_type_id": "uuid",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
/// Note: All documents will share the same title, document_type, sync_priority, and compression_priority.
/// For individual metadata per document, use the single upload method multiple times.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_upload_documents_bulk(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct DocumentData {
            file_data: String, // base64 encoded
            original_filename: String,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            activity_id: String,
            documents: Vec<DocumentData>,
            title: Option<String>,
            document_type_id: String,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        let activity_id = Uuid::parse_str(&p.activity_id)
            .map_err(|_| FFIError::invalid_argument("invalid activity_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        // Decode all documents first
        let mut files = Vec::new();
        let total_documents = p.documents.len();
        
        for doc in p.documents {
            let file_data = base64::decode(&doc.file_data)
                .map_err(|_| FFIError::invalid_argument("invalid base64 file data"))?;
            files.push((file_data, doc.original_filename));
        }
        
        // Use the bulk upload method from the service
        let doc_results = block_on_async(svc.bulk_upload_documents_for_activity(
            activity_id,
            files,
            p.title, // title applied to all documents
            document_type_id,
            sync_priority,
            compression_priority,
            &auth,
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct BulkUploadResponse {
            activity_id: Uuid,
            document_results: Vec<crate::domains::document::types::MediaDocumentResponse>,
            total_documents: usize,
            successful_uploads: usize,
            failed_uploads: usize,
        }
        
        let successful_uploads = doc_results.len();
        let failed_uploads = total_documents - successful_uploads;
        
        let response = BulkUploadResponse {
            activity_id,
            document_results: doc_results,
            total_documents,
            successful_uploads,
            failed_uploads,
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Memory Management
// ---------------------------------------------------------------------------

/// Free memory allocated by Rust for C strings
/// MUST be called by Swift for every *mut c_char returned by activity functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
} 