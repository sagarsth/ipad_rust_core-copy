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
    NewActivity, UpdateActivity, ActivityResponse, ActivityInclude, 
    ActivityDocumentReference, ActivityFilter, ActivityStatistics, 
    ActivityStatusBreakdown, ActivityMetadataCounts, ActivityProgressAnalysis
};
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::auth::AuthContext;
use crate::types::{UserRole, PaginationParams};
use crate::globals;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::str::FromStr;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Helper function to parse includes
fn parse_includes(includes: Option<Vec<ActivityIncludeDto>>) -> Option<Vec<ActivityInclude>> {
    includes.map(|inc| inc.into_iter().map(Into::into).collect())
}

/// Helper function to parse pagination
fn parse_pagination(pagination: Option<PaginationDto>) -> PaginationParams {
    pagination.map(Into::into).unwrap_or_default()
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
    CreatedBy,
    UpdatedBy,
    Documents,
    All,
}

impl From<ActivityIncludeDto> for ActivityInclude {
    fn from(dto: ActivityIncludeDto) -> Self {
        match dto {
            ActivityIncludeDto::Project => ActivityInclude::Project,
            ActivityIncludeDto::Status => ActivityInclude::Status,
            ActivityIncludeDto::CreatedBy => ActivityInclude::CreatedBy,
            ActivityIncludeDto::UpdatedBy => ActivityInclude::UpdatedBy,
            ActivityIncludeDto::Documents => ActivityInclude::Documents,
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
///   "activity": { NewActivityDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct NewActivityDto {
            description: Option<String>,
            kpi: Option<String>,
            target_value: Option<f64>,
            actual_value: Option<f64>,
            status_id: Option<i64>,
            project_id: Option<String>,
            sync_priority: String,
            created_by_user_id: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            activity: NewActivityDto,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert DTO to domain struct with UUID parsing
        let project_id = p.activity.project_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        
        let created_by_user_id = p.activity.created_by_user_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid created_by_user_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.activity.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        
        let new_activity = NewActivity {
            description: p.activity.description,
            kpi: p.activity.kpi,
            target_value: p.activity.target_value,
            actual_value: p.activity.actual_value,
            status_id: p.activity.status_id,
            project_id,
            sync_priority,
            created_by_user_id,
        };
        
        let svc = globals::get_activity_service()?;
        let activity = block_on_async(svc.create_activity(new_activity, &auth))
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
        struct NewActivityDto {
            description: Option<String>,
            kpi: Option<String>,
            target_value: Option<f64>,
            actual_value: Option<f64>,
            status_id: Option<i64>,
            project_id: Option<String>,
            sync_priority: String,
            created_by_user_id: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            activity: NewActivityDto,
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
        
        // Convert DTO to domain struct with UUID parsing
        let project_id = p.activity.project_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        
        let created_by_user_id = p.activity.created_by_user_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid created_by_user_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.activity.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        
        let new_activity = NewActivity {
            description: p.activity.description,
            kpi: p.activity.kpi,
            target_value: p.activity.target_value,
            actual_value: p.activity.actual_value,
            status_id: p.activity.status_id,
            project_id,
            sync_priority,
            created_by_user_id,
        };
        
        let svc = globals::get_activity_service()?;
        let (activity, doc_results) = block_on_async(svc.create_activity_with_documents(
            new_activity, documents, document_type_id, &auth
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
///   "include": [ActivityIncludeDto, ...],
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
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
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

/// List activities with pagination and includes
/// Expected JSON payload:
/// {
///   "pagination": { PaginationDto },
///   "include": [ActivityIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
/// NOTE: This function returns an empty list as the service doesn't support 
/// listing all activities without constraints. Use search or filtered methods instead.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            pagination: Option<PaginationDto>,
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        // Return empty paginated result since the service doesn't support listing all activities
        #[derive(Serialize)]
        struct EmptyPaginatedResult {
            items: Vec<serde_json::Value>,
            total: u64,
            page: u32,
            per_page: u32,
            total_pages: u32,
        }
        
        let empty_result = EmptyPaginatedResult {
            items: vec![],
            total: 0,
            page: params.page,
            per_page: params.per_page,
            total_pages: 0,
        };
        
        let json_resp = serde_json::to_string(&empty_result)
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

/// Delete activity (soft or hard delete)
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
        
        let delete_result = block_on_async(svc.delete_activity(id, p.hard_delete.unwrap_or(false), &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&delete_result)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Document Integration
// ---------------------------------------------------------------------------

/// Upload a single document for an activity
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

/// Bulk upload multiple documents for an activity
/// Expected JSON payload:
/// {
///   "activity_id": "uuid",
///   "files": [{"file_data": "base64", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_bulk_upload_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct FileData {
            file_data: String, // base64 encoded
            filename: String,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            activity_id: String,
            files: Vec<FileData>,
            title: Option<String>,
            document_type_id: String,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        // Decode all files
        let mut files = Vec::new();
        for file in p.files {
            let data = base64::decode(&file.file_data)
                .map_err(|_| FFIError::invalid_argument("invalid base64 file data"))?;
            files.push((data, file.filename));
        }
        
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
        
        let documents = block_on_async(svc.bulk_upload_documents_for_activity(
            activity_id,
            files,
            p.title,
            document_type_id,
            sync_priority,
            compression_priority,
            &auth,
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&documents)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Analytics and Statistics
// ---------------------------------------------------------------------------

/// Get activity statistics for dashboard
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_statistics(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let stats = block_on_async(svc.get_activity_statistics(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stats)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activity status breakdown
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_status_breakdown(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let breakdown = block_on_async(svc.get_activity_status_breakdown(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&breakdown)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activity metadata counts
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_metadata_counts(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let counts = block_on_async(svc.get_activity_metadata_counts(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&counts)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Query Operations
// ---------------------------------------------------------------------------

/// Find activities by status
/// Expected JSON payload:
/// {
///   "status_id": i64,
///   "pagination": { PaginationDto },
///   "include": [ActivityIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_by_status(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            status_id: i64,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_activity_service()?;
        let activities = block_on_async(svc.find_activities_by_status(p.status_id, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activities)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find activities by date range
/// Expected JSON payload:
/// {
///   "start_date": "2024-01-01T00:00:00Z",
///   "end_date": "2024-12-31T23:59:59Z",
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
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
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

/// Search activities by text
/// Expected JSON payload:
/// {
///   "query": "string",
///   "pagination": { PaginationDto },
///   "include": [ActivityIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_search(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            query: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_activity_service()?;
        let activities = block_on_async(svc.search_activities(&p.query, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&activities)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Detailed Views
// ---------------------------------------------------------------------------

/// Get activity document references
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_document_references(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let doc_refs = block_on_async(svc.get_activity_document_references(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&doc_refs)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Filtering and Bulk Operations
// ---------------------------------------------------------------------------

/// Get filtered activity IDs for bulk operations
/// Expected JSON payload:
/// {
///   "filter": {
///     "status_ids": [1, 2, 3],
///     "project_ids": ["uuid1", "uuid2"],
///     "search_text": "optional search text",
///     "date_range": ["2024-01-01T00:00:00Z", "2024-12-31T23:59:59Z"],
///     "target_value_range": [0.0, 100.0],
///     "actual_value_range": [0.0, 100.0],
///     "exclude_deleted": true
///   },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_filtered_ids(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct FilterDto {
            status_ids: Option<Vec<i64>>,
            project_ids: Option<Vec<String>>,
            search_text: Option<String>,
            date_range: Option<(String, String)>,
            target_value_range: Option<(f64, f64)>,
            actual_value_range: Option<(f64, f64)>,
            exclude_deleted: Option<bool>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            filter: FilterDto,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert FilterDto to ActivityFilter
        let filter = ActivityFilter {
            status_ids: p.filter.status_ids,
            project_ids: p.filter.project_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|id| Uuid::parse_str(&id).ok())
                    .collect()
            }),
            search_text: p.filter.search_text,
            date_range: p.filter.date_range,
            target_value_range: p.filter.target_value_range,
            actual_value_range: p.filter.actual_value_range,
            exclude_deleted: p.filter.exclude_deleted,
        };
        
        let svc = globals::get_activity_service()?;
        let ids = block_on_async(svc.get_filtered_activity_ids(filter, &auth))
            .map_err(FFIError::from_service_error)?;
        
        // Convert UUIDs to strings for FFI
        let id_strings: Vec<String> = ids.into_iter().map(|id| id.to_string()).collect();
        
        let json_resp = serde_json::to_string(&id_strings)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Bulk update activity status
/// Expected JSON payload:
/// {
///   "activity_ids": ["uuid1", "uuid2", "uuid3"],
///   "status_id": i64,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_bulk_update_status(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            activity_ids: Vec<String>,
            status_id: i64,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert string UUIDs to Uuid type
        let mut activity_ids = Vec::new();
        for id_str in p.activity_ids {
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| FFIError::invalid_argument("invalid activity_id in list"))?;
            activity_ids.push(id);
        }
        
        let svc = globals::get_activity_service()?;
        let update_count = block_on_async(svc.bulk_update_activity_status(&activity_ids, p.status_id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct BulkUpdateResponse {
            updated_count: u64,
            status_id: i64,
        }
        
        let response = BulkUpdateResponse {
            updated_count: update_count,
            status_id: p.status_id,
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Advanced Dashboard Aggregations
// ---------------------------------------------------------------------------

/// Get activity workload distribution by project for dashboard widgets
/// Expected JSON payload: { "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_workload_by_project(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let distribution = block_on_async(svc.get_activity_workload_by_project(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find stale activities for dashboard widgets
/// Expected JSON payload:
/// {
///   "days_stale": 30,
///   "pagination": { "page": 1, "per_page": 20 },
///   "include": ["Documents", "CreatedBy"],
///   "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_find_stale(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            days_stale: u32,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ActivityIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let params = parse_pagination(p.pagination);
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        let svc = globals::get_activity_service()?;
        
        let stale_activities = block_on_async(svc.find_stale_activities(p.days_stale, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stale_activities)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get activity progress analysis for dashboard tracking
/// Expected JSON payload: { "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activity_get_progress_analysis(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_activity_service()?;
        
        let analysis = block_on_async(svc.get_activity_progress_analysis(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&analysis)
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