// src/ffi/project.rs
// ============================================================================
// FFI bindings for the `ProjectService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `project_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `project_free` more than
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
use crate::domains::project::types::{
    NewProject, UpdateProject, ProjectResponse, ProjectInclude, ProjectSummary,
    ProjectStatistics, ProjectStatusBreakdown, ProjectMetadataCounts,
    ProjectWithDocumentTimeline, ProjectDocumentReference
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
fn parse_includes(includes: Option<Vec<ProjectIncludeDto>>) -> Option<Vec<ProjectInclude>> {
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

/// DTO for project includes
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectIncludeDto {
    StrategicGoal,
    Status,
    CreatedBy,
    ActivityCount,
    WorkshopCount,
    Documents,
    DocumentReferences,
    ActivityTimeline,
    StatusDetails,
    Counts,
    All,
}

impl From<ProjectIncludeDto> for ProjectInclude {
    fn from(dto: ProjectIncludeDto) -> Self {
        match dto {
            ProjectIncludeDto::StrategicGoal => ProjectInclude::StrategicGoal,
            ProjectIncludeDto::Status => ProjectInclude::Status,
            ProjectIncludeDto::CreatedBy => ProjectInclude::CreatedBy,
            ProjectIncludeDto::ActivityCount => ProjectInclude::ActivityCount,
            ProjectIncludeDto::WorkshopCount => ProjectInclude::WorkshopCount,
            ProjectIncludeDto::Documents => ProjectInclude::Documents,
            ProjectIncludeDto::DocumentReferences => ProjectInclude::DocumentReferences,
            ProjectIncludeDto::ActivityTimeline => ProjectInclude::ActivityTimeline,
            ProjectIncludeDto::StatusDetails => ProjectInclude::StatusDetails,
            ProjectIncludeDto::Counts => ProjectInclude::Counts,
            ProjectIncludeDto::All => ProjectInclude::All,
        }
    }
}

// ---------------------------------------------------------------------------
// Basic CRUD Operations
// ---------------------------------------------------------------------------

/// Create a new project
/// Expected JSON payload:
/// {
///   "project": { NewProjectDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct NewProjectDto {
            name: String,
            objective: Option<String>,
            outcome: Option<String>,
            status_id: Option<i64>,
            timeline: Option<String>,
            responsible_team: Option<String>,
            strategic_goal_id: Option<String>,
            sync_priority: String,
            created_by_user_id: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            project: NewProjectDto,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert DTO to domain struct with UUID parsing
        let strategic_goal_id = p.project.strategic_goal_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid strategic_goal_id"))?;
        
        let created_by_user_id = p.project.created_by_user_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid created_by_user_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.project.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        
        let new_project = NewProject {
            name: p.project.name,
            objective: p.project.objective,
            outcome: p.project.outcome,
            status_id: p.project.status_id,
            timeline: p.project.timeline,
            responsible_team: p.project.responsible_team,
            strategic_goal_id,
            sync_priority,
            created_by_user_id,
        };
        
        let svc = globals::get_project_service()?;
        let project = block_on_async(svc.create_project(new_project, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&project)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Create a new project with documents
/// Expected JSON payload:
/// {
///   "project": { NewProject },
///   "documents": [{"file_data": "base64", "filename": "string", "linked_field": "optional_string"}, ...],
///   "document_type_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_create_with_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
        struct NewProjectDto {
            name: String,
            objective: Option<String>,
            outcome: Option<String>,
            status_id: Option<i64>,
            timeline: Option<String>,
            responsible_team: Option<String>,
            strategic_goal_id: Option<String>,
            sync_priority: String,
            created_by_user_id: Option<String>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            project: NewProjectDto,
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
        let strategic_goal_id = p.project.strategic_goal_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid strategic_goal_id"))?;
        
        let created_by_user_id = p.project.created_by_user_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid created_by_user_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.project.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        
        let new_project = NewProject {
            name: p.project.name,
            objective: p.project.objective,
            outcome: p.project.outcome,
            status_id: p.project.status_id,
            timeline: p.project.timeline,
            responsible_team: p.project.responsible_team,
            strategic_goal_id,
            sync_priority,
            created_by_user_id,
        };
        
        let svc = globals::get_project_service()?;
        let (project, doc_results) = block_on_async(svc.create_project_with_documents(
            new_project, documents, document_type_id, &auth
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct CreateWithDocsResponse {
            project: ProjectResponse,
            document_results: Vec<Result<crate::domains::document::types::MediaDocumentResponse, String>>,
        }
        
        let response = CreateWithDocsResponse {
            project,
            document_results: doc_results.into_iter().map(|r| r.map_err(|e| e.to_string())).collect(),
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get project by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let project = block_on_async(svc.get_project_by_id(id, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&project)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List projects with pagination and includes
/// Expected JSON payload:
/// {
///   "pagination": { PaginationDto },
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let projects = block_on_async(svc.list_projects(params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Update project
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "update": { UpdateProject },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateProject,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        // Debug: Print the deserialized UpdateProject struct
        println!("🔧 [FFI_UPDATE] Deserialized UpdateProject:");
        println!("   • strategic_goal_id: {:?}", p.update.strategic_goal_id);
        println!("   • Raw JSON strategic_goal_id: {:?}", 
            serde_json::from_str::<serde_json::Value>(json)
                .and_then(|v| Ok(v.get("update").and_then(|u| u.get("strategic_goal_id")).cloned()))
                .unwrap_or(Some(serde_json::Value::String("PARSE_ERROR".to_string())))
        );
        
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let project = block_on_async(svc.update_project(id, p.update, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&project)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete project (soft or hard delete)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "hard_delete": bool,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_delete(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
        let svc = globals::get_project_service()?;
        
        let delete_result = block_on_async(svc.delete_project(id, p.hard_delete.unwrap_or(false), &auth))
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

/// Upload a single document for a project
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
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
pub unsafe extern "C" fn project_upload_document(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            project_id: String,
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
        
        let project_id = Uuid::parse_str(&p.project_id)
            .map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let document = block_on_async(svc.upload_document_for_project(
            project_id,
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

/// Bulk upload multiple documents for a project
/// Expected JSON payload:
/// {
///   "project_id": "uuid",
///   "files": [{"file_data": "base64", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_bulk_upload_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            project_id: String,
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
        
        let project_id = Uuid::parse_str(&p.project_id)
            .map_err(|_| FFIError::invalid_argument("invalid project_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let documents = block_on_async(svc.bulk_upload_documents_for_project(
            project_id,
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

/// Get project statistics for dashboard
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_statistics(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let stats = block_on_async(svc.get_project_statistics(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stats)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get project status breakdown
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_status_breakdown(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let breakdown = block_on_async(svc.get_project_status_breakdown(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&breakdown)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get project metadata counts
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_metadata_counts(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let counts = block_on_async(svc.get_project_metadata_counts(&auth))
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

/// Find projects by status
/// Expected JSON payload:
/// {
///   "status_id": i64,
///   "pagination": { PaginationDto },
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_find_by_status(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            status_id: i64,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let projects = block_on_async(svc.find_projects_by_status(p.status_id, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find projects by responsible team
/// Expected JSON payload:
/// {
///   "team_name": "string",
///   "pagination": { PaginationDto },
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_find_by_responsible_team(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            team_name: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let projects = block_on_async(svc.find_projects_by_responsible_team(&p.team_name, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find projects by date range
/// Expected JSON payload:
/// {
///   "start_date": "2024-01-01T00:00:00Z",
///   "end_date": "2024-12-31T23:59:59Z",
///   "pagination": { PaginationDto },
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_find_by_date_range(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            start_date: String,
            end_date: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let projects = block_on_async(svc.find_projects_by_date_range(&p.start_date, &p.end_date, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Search projects by text
/// Expected JSON payload:
/// {
///   "query": "string",
///   "search_fields": ["name", "objective", "outcome"],
///   "pagination": { PaginationDto },
///   "include": [ProjectIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_search(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            query: String,
            // search_fields: Option<Vec<String>>, // Currently unused - could be used for field-specific search
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = parse_pagination(p.pagination);
        let auth: AuthContext = p.auth.try_into()?;
        
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_project_service()?;
        let projects = block_on_async(svc.search_projects(&p.query, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Detailed Views
// ---------------------------------------------------------------------------

/// Get project with document timeline
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_with_document_timeline(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let project_timeline = block_on_async(svc.get_project_with_document_timeline(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&project_timeline)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get project document references
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_document_references(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let doc_refs = block_on_async(svc.get_project_document_references(id, &auth))
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

/// Get filtered project IDs for bulk operations
/// Expected JSON payload:
/// {
///   "filter": {
///     "status_ids": [1, 2, 3],
///     "strategic_goal_ids": ["uuid1", "uuid2"],
///     "responsible_teams": ["Team A", "Team B"],
///     "search_text": "optional search text",
///     "date_range": ["2024-01-01T00:00:00Z", "2024-12-31T23:59:59Z"],
///     "exclude_deleted": true
///   },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_filtered_ids(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct FilterDto {
            status_ids: Option<Vec<i64>>,
            strategic_goal_ids: Option<Vec<String>>,
            responsible_teams: Option<Vec<String>>,
            search_text: Option<String>,
            date_range: Option<(String, String)>,
            exclude_deleted: Option<bool>,
        }
        
        #[derive(Deserialize)]
        struct Payload {
            filter: FilterDto,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        // Convert FilterDto to ProjectFilter
        let filter = crate::domains::project::types::ProjectFilter {
            status_ids: p.filter.status_ids,
            strategic_goal_ids: p.filter.strategic_goal_ids.map(|ids| {
                ids.into_iter()
                    .filter_map(|id| Uuid::parse_str(&id).ok())
                    .collect()
            }),
            responsible_teams: p.filter.responsible_teams,
            search_text: p.filter.search_text,
            date_range: p.filter.date_range,
            exclude_deleted: p.filter.exclude_deleted,
        };
        
        let svc = globals::get_project_service()?;
        let ids = block_on_async(svc.get_filtered_project_ids(filter, &auth))
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

// ---------------------------------------------------------------------------
// Advanced Dashboard Aggregations
// ---------------------------------------------------------------------------

/// Get team workload distribution for dashboard widgets
/// Expected JSON payload: { "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_team_workload_distribution(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let distribution = block_on_async(svc.get_team_workload_distribution(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get projects by strategic goal distribution for dashboard widgets
/// Expected JSON payload: { "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_strategic_goal_distribution(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let distribution = block_on_async(svc.get_projects_by_strategic_goal_distribution(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find stale projects for dashboard widgets
/// Expected JSON payload:
/// {
///   "days_stale": 30,
///   "pagination": { "page": 1, "per_page": 20 },
///   "include": ["Documents", "CreatedBy"],
///   "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_find_stale(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            days_stale: u32,
            pagination: Option<PaginationDto>,
            include: Option<Vec<ProjectIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let params = parse_pagination(p.pagination);
        let include = parse_includes(p.include);
        let include_slice = include.as_ref().map(|v| v.as_slice());
        let svc = globals::get_project_service()?;
        
        let stale_projects = block_on_async(svc.find_stale_projects(p.days_stale, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stale_projects)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get document coverage analysis for dashboard widgets
/// Expected JSON payload: { "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false } }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_document_coverage_analysis(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let analysis = block_on_async(svc.get_document_coverage_analysis(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&analysis)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get project activity timeline for dashboard widgets
/// Expected JSON payload:
/// {
///   "days_active": 14,
///   "auth": { "user_id": "uuid", "role": "admin", "device_id": "device_uuid", "offline_mode": false }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_get_activity_timeline(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            days_active: u32,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_project_service()?;
        
        let timeline = block_on_async(svc.get_project_activity_timeline(p.days_active, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&timeline)
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
/// MUST be called by Swift for every *mut c_char returned by project functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn project_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
} 