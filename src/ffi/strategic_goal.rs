// src/ffi/strategic_goal.rs
// ============================================================================
// FFI bindings for the `StrategicGoalService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `strategic_goal_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `strategic_goal_free` more than
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
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, UpdateStrategicGoal, StrategicGoalResponse, StrategicGoalInclude,
    UserGoalRole, GoalValueSummaryResponse
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

/// Helper function to parse includes
fn parse_includes(includes: Option<Vec<StrategicGoalIncludeDto>>) -> Option<Vec<StrategicGoalInclude>> {
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

/// DTO for strategic goal includes
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StrategicGoalIncludeDto {
    Documents,
    Status,
    Activities,
    Projects,
    ProjectCount,
    Participants,
    DocumentCounts,
}

impl From<StrategicGoalIncludeDto> for StrategicGoalInclude {
    fn from(dto: StrategicGoalIncludeDto) -> Self {
        match dto {
            StrategicGoalIncludeDto::Documents => StrategicGoalInclude::Documents,
            StrategicGoalIncludeDto::Status => StrategicGoalInclude::Status,
            StrategicGoalIncludeDto::Activities => StrategicGoalInclude::Activities,
            StrategicGoalIncludeDto::Projects => StrategicGoalInclude::Projects,
            StrategicGoalIncludeDto::ProjectCount => StrategicGoalInclude::ProjectCount,
            StrategicGoalIncludeDto::Participants => StrategicGoalInclude::Participants,
            StrategicGoalIncludeDto::DocumentCounts => StrategicGoalInclude::DocumentCounts,
        }
    }
}

// ---------------------------------------------------------------------------
// Basic CRUD Operations
// ---------------------------------------------------------------------------

/// Create a new strategic goal
/// Expected JSON payload:
/// {
///   "goal": { NewStrategicGoal },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            goal: NewStrategicGoal,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_strategic_goal_service()?;
        
        let goal = block_on_async(svc.create_strategic_goal(p.goal, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goal)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Create a new strategic goal with documents
/// Expected JSON payload:
/// {
///   "goal": { NewStrategicGoal },
///   "documents": [{"file_data": "base64", "filename": "string", "linked_field": "optional_string"}, ...],
///   "document_type_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_create_with_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            goal: NewStrategicGoal,
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
        let svc = globals::get_strategic_goal_service()?;
        
        let (goal, doc_results) = block_on_async(svc.create_strategic_goal_with_documents(
            p.goal, documents, document_type_id, &auth
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct CreateWithDocsResponse {
            goal: StrategicGoalResponse,
            document_results: Vec<Result<crate::domains::document::types::MediaDocumentResponse, String>>,
        }
        
        let response = CreateWithDocsResponse {
            goal,
            document_results: doc_results.into_iter().map(|r| r.map_err(|e| e.to_string())).collect(),
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get strategic goal by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goal = block_on_async(svc.get_strategic_goal_by_id(id, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goal)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List strategic goals with pagination and includes
/// Expected JSON payload:
/// {
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.list_strategic_goals(params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Update strategic goal
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "update": { UpdateStrategicGoal },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateStrategicGoal,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_strategic_goal_service()?;
        
        let goal = block_on_async(svc.update_strategic_goal(id, p.update, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goal)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete strategic goal (soft or hard delete)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "hard_delete": bool,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_delete(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
        let svc = globals::get_strategic_goal_service()?;
        
        let delete_result = block_on_async(svc.delete_strategic_goal(id, p.hard_delete.unwrap_or(false), &auth))
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

/// Upload a single document for a strategic goal (with auto-detection)
/// Expected JSON payload:
/// {
///   "goal_id": "uuid",
///   "file_data": "base64_encoded_file_data",
///   "original_filename": "string",
///   "title": "optional_string",
///   "document_type_id": "uuid", // IGNORED - document type auto-detected from file extension
///   "linked_field": "optional_string",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_upload_document(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            goal_id: String,
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
        
        let goal_id = Uuid::parse_str(&p.goal_id)
            .map_err(|_| FFIError::invalid_argument("invalid goal_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_strategic_goal_service()?;
        
        let document = block_on_async(svc.upload_document_for_goal(
            goal_id,
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

/// Bulk upload documents to a strategic goal (legacy base64 method)
/// Expected JSON payload:
/// {
///   "goal_id": "uuid",
///   "files": [{"file_data": "base64", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "sync_priority": "low|normal|high",
///   "compression_priority": "low|normal|high",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_bulk_upload_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            goal_id: String,
            files: Vec<FileData>,
            title: Option<String>,
            document_type_id: String,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let goal_uuid = Uuid::parse_str(&p.goal_id)
            .map_err(|_| FFIError::invalid_argument("invalid goal_id"))?;
        let doc_type_uuid = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = if let Some(cp) = p.compression_priority {
            CompressionPriority::from_str(&cp)
                .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?
        } else {
            CompressionPriority::Normal
        };
        
        // Decode base64 files
        let mut files_data = Vec::new();
        for file in p.files {
            let file_bytes = base64::decode(&file.file_data)
                .map_err(|_| FFIError::invalid_argument("invalid base64 in file_data"))?;
            files_data.push((file_bytes, file.filename));
        }
        
        let svc = globals::get_strategic_goal_service()?;
        let documents = block_on_async(svc.bulk_upload_documents_for_goal(
            goal_uuid,
            files_data,
            p.title,
            sync_priority,
            Some(compression_priority),
            &auth,
        ))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&documents)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Upload a single document to a strategic goal using file path (iOS optimized)
/// Expected JSON payload:
/// {
///   "goal_id": "uuid",
///   "file_path": "/path/to/file",
///   "original_filename": "string",
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "linked_field": "optional_string",
///   "sync_priority": "low|normal|high",
///   "compression_priority": "low|normal|high",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_upload_document_from_path(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            goal_id: String,
            file_path: String,
            original_filename: String,
            title: Option<String>,
            document_type_id: String,
            linked_field: Option<String>,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let goal_uuid = Uuid::parse_str(&p.goal_id)
            .map_err(|_| FFIError::invalid_argument("invalid goal_id"))?;
        let doc_type_uuid = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = if let Some(cp) = p.compression_priority {
            CompressionPriority::from_str(&cp)
                .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?
        } else {
            CompressionPriority::Normal
        };
        
        let svc = globals::get_strategic_goal_service()?;
        let document = block_on_async(svc.upload_document_from_path(
            goal_uuid,
            &p.file_path,
            &p.original_filename,
            p.title,
            doc_type_uuid,
            p.linked_field,
            sync_priority,
            compression_priority,
            &auth,
        ))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&document)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Bulk upload documents to a strategic goal using file paths (iOS optimized)
/// Expected JSON payload:
/// {
///   "goal_id": "uuid",
///   "file_paths": [{"file_path": "/path/to/file", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "sync_priority": "low|normal|high",
///   "compression_priority": "low|normal|high",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_bulk_upload_documents_from_paths(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct FilePathData {
            file_path: String,
            filename: String,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            goal_id: String,
            file_paths: Vec<FilePathData>,
            title: Option<String>,
            document_type_id: String,
            sync_priority: String,
            compression_priority: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let goal_uuid = Uuid::parse_str(&p.goal_id)
            .map_err(|_| FFIError::invalid_argument("invalid goal_id"))?;
        let doc_type_uuid = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = if let Some(cp) = p.compression_priority {
            CompressionPriority::from_str(&cp)
                .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?
        } else {
            CompressionPriority::Normal
        };
        
        // Convert file paths
        let file_paths: Vec<(String, String)> = p.file_paths
            .into_iter()
            .map(|fp| (fp.file_path, fp.filename))
            .collect();
        
        let svc = globals::get_strategic_goal_service()?;
        let documents = block_on_async(svc.bulk_upload_documents_from_paths(
            goal_uuid,
            file_paths,
            p.title,
            doc_type_uuid,
            sync_priority,
            compression_priority,
            &auth,
        ))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&documents)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Query Operations
// ---------------------------------------------------------------------------

/// Find strategic goals by status
/// Expected JSON payload:
/// {
///   "status_id": 1,
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_find_by_status(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            status_id: i64,
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<StrategicGoalInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.find_goals_by_status(p.status_id, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find strategic goals by responsible team
/// Expected JSON payload:
/// {
///   "team_name": "string",
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_find_by_team(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            team_name: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<StrategicGoalInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.find_goals_by_responsible_team(&p.team_name, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find strategic goals by user role
/// Expected JSON payload:
/// {
///   "user_id": "uuid",
///   "role": "created|updated",
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_find_by_user_role(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            user_id: String,
            role: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let user_id = Uuid::parse_str(&p.user_id).map_err(|_| FFIError::invalid_argument("invalid user_id"))?;
        let role = match p.role.as_str() {
            "created" => UserGoalRole::Created,
            "updated" => UserGoalRole::Updated,
            _ => return Err(FFIError::invalid_argument("invalid role, must be 'created' or 'updated'")),
        };
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<StrategicGoalInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.find_goals_by_user_role(user_id, role, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find stale strategic goals
/// Expected JSON payload:
/// {
///   "days_stale": u32,
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_find_stale(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            days_stale: u32,
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<StrategicGoalInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.find_stale_goals(p.days_stale, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find strategic goals by date range
/// Expected JSON payload:
/// {
///   "start_date": "2024-01-01T00:00:00Z",
///   "end_date": "2024-12-31T23:59:59Z",
///   "pagination": { PaginationDto },
///   "include": [StrategicGoalIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_find_by_date_range(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            start_date: String,
            end_date: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<StrategicGoalIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<StrategicGoalInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_strategic_goal_service()?;
        let goals = block_on_async(svc.find_strategic_goals_by_date_range(&p.start_date, &p.end_date, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&goals)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Statistics Operations
// ---------------------------------------------------------------------------

/// Get status distribution
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_get_status_distribution(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_strategic_goal_service()?;
        
        let distribution = block_on_async(svc.get_status_distribution(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get value statistics
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_get_value_statistics(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_strategic_goal_service()?;
        
        let stats = block_on_async(svc.get_value_statistics(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stats)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get filtered strategic goal IDs for bulk selection
/// Expected JSON payload:
/// {
///   "filter": {
///     "status_ids": [1, 2],
///     "years": [2024, 2023],
///     "months": [1, 2, 3],
///     "responsible_teams": ["Team A", "Team B"],
///     "search_text": "optional search",
///     "user_role": {"user_id": "uuid", "role": "created|updated"},
///     "progress_range": [50.0, 100.0],
///     "target_value_range": [1000.0, 5000.0],
///     "actual_value_range": [500.0, 4000.0],
///     "date_range": ["2024-01-01T00:00:00Z", "2024-12-31T23:59:59Z"],
///     "days_stale": 30,
///     "exclude_deleted": true
///   },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_get_filtered_ids(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct UserRoleFilter {
            user_id: String,
            role: String,
        }
        
        #[derive(Deserialize)]
        struct FilterDto {
            status_ids: Option<Vec<i64>>,
            responsible_teams: Option<Vec<String>>,
            years: Option<Vec<i32>>,
            months: Option<Vec<i32>>,
            user_role: Option<UserRoleFilter>,
            sync_priorities: Option<Vec<String>>,
            search_text: Option<String>,
            progress_range: Option<(f64, f64)>,
            target_value_range: Option<(f64, f64)>,
            actual_value_range: Option<(f64, f64)>,
            date_range: Option<(String, String)>,
            days_stale: Option<u32>,
            exclude_deleted: Option<bool>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            filter: FilterDto,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert DTO to domain filter
        let user_role = if let Some(ur) = p.filter.user_role {
            let user_id = Uuid::parse_str(&ur.user_id)
                .map_err(|_| FFIError::invalid_argument("invalid user_id in user_role"))?;
            let role = match ur.role.as_str() {
                "created" => crate::domains::strategic_goal::types::UserGoalRole::Created,
                "updated" => crate::domains::strategic_goal::types::UserGoalRole::Updated,
                _ => return Err(FFIError::invalid_argument("invalid role in user_role")),
            };
            Some((user_id, role))
        } else {
            None
        };
        
        // Convert sync priority strings to enums
        let sync_priorities = if let Some(priorities) = p.filter.sync_priorities {
            let converted: Result<Vec<_>, _> = priorities
                .into_iter()
                .map(|s| crate::domains::sync::types::SyncPriority::from_str(&s))
                .collect();
            Some(converted.map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?)
        } else {
            None
        };
        
        let filter = crate::domains::strategic_goal::types::StrategicGoalFilter {
            status_ids: p.filter.status_ids,
            responsible_teams: p.filter.responsible_teams,
            years: p.filter.years,
            months: p.filter.months,
            user_role,
            sync_priorities,
            search_text: p.filter.search_text,
            progress_range: p.filter.progress_range,
            target_value_range: p.filter.target_value_range,
            actual_value_range: p.filter.actual_value_range,
            date_range: p.filter.date_range,
            days_stale: p.filter.days_stale,
            exclude_deleted: p.filter.exclude_deleted,
        };
        
        let svc = globals::get_strategic_goal_service()?;
        let filtered_ids = block_on_async(svc.get_filtered_goal_ids(filter, &auth))
            .map_err(FFIError::from_service_error)?;
        
        // Convert UUIDs to strings for JSON response
        let id_strings: Vec<String> = filtered_ids.into_iter().map(|id| id.to_string()).collect();
        
        let json_resp = serde_json::to_string(&id_strings)
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
/// MUST be called by Swift for every *mut c_char returned by strategic goal functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strategic_goal_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}