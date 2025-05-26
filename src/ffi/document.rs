// src/ffi/document.rs
// ============================================================================
// FFI bindings for the `DocumentService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `document_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `document_free` more than
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
use crate::domains::document::types::{
    NewDocumentType, UpdateDocumentType, DocumentTypeResponse, MediaDocumentResponse,
    DocumentSummary
};
use crate::domains::document::service::DocumentInclude as ServiceDocumentInclude;
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

/// Ensure pointer is not null
macro_rules! ensure_ptr {
    ($ptr:expr) => {
        if $ptr.is_null() {
            return Err(FFIError::invalid_argument("null pointer"));
        }
    };
}

/// DTO mirroring the subset of `AuthContext` that we expect to receive from
/// Swift. We purposefully keep this separate so that the public JSON contract
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

/// DTO for document includes
#[derive(Deserialize)]
#[serde(tag = "type")]
enum DocumentIncludeDto {
    DocumentType,
    Versions,
    AccessLogs { pagination: Option<PaginationDto> },
}

impl From<DocumentIncludeDto> for ServiceDocumentInclude {
    fn from(dto: DocumentIncludeDto) -> Self {
        match dto {
            DocumentIncludeDto::DocumentType => ServiceDocumentInclude::DocumentType,
            DocumentIncludeDto::Versions => ServiceDocumentInclude::Versions,
            DocumentIncludeDto::AccessLogs { pagination } => {
                let params = pagination.map(|p| p.into()).unwrap_or_default();
                ServiceDocumentInclude::AccessLogs(params)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Document Type FFI Functions
// ---------------------------------------------------------------------------

/// Create a new document type
/// Expected JSON payload:
/// {
///   "document_type": { NewDocumentType },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_type_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            document_type: NewDocumentType,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let doc_type = block_on_async(svc.create_document_type(&auth, p.document_type))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&doc_type)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get document type by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_type_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let doc_type = block_on_async(svc.get_document_type_by_id(id))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&doc_type)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List document types with pagination
/// Expected JSON payload:
/// {
///   "pagination": { PaginationDto },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_type_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { 
            pagination: Option<PaginationDto>,
            auth: AuthCtxDto 
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let doc_types = block_on_async(svc.list_document_types(params))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&doc_types)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Update document type
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "update": { UpdateDocumentType },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_type_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateDocumentType,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let doc_type = block_on_async(svc.update_document_type(&auth, id, p.update))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&doc_type)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete document type (hard delete, admin only)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_type_delete(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct Payload { id: String, auth: AuthCtxDto }
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        block_on_async(svc.delete_document_type(&auth, id)).map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Media Document FFI Functions
// ---------------------------------------------------------------------------

/// Upload a single document
/// Expected JSON payload:
/// {
///   "file_data": "base64_encoded_file_data",
///   "original_filename": "string",
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "related_entity_id": "uuid",
///   "related_entity_type": "string",
///   "linked_field": "optional_string",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "temp_related_id": "optional_uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_upload(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            file_data: String, // base64 encoded
            original_filename: String,
            title: Option<String>,
            document_type_id: String,
            related_entity_id: String,
            related_entity_type: String,
            linked_field: Option<String>,
            sync_priority: String,
            compression_priority: Option<String>,
            temp_related_id: Option<String>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        
        // Decode base64 file data
        let file_data = base64::decode(&p.file_data)
            .map_err(|_| FFIError::invalid_argument("invalid base64 file data"))?;
        
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        let related_entity_id = Uuid::parse_str(&p.related_entity_id)
            .map_err(|_| FFIError::invalid_argument("invalid related_entity_id"))?;
        let temp_related_id = p.temp_related_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid temp_related_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let document = block_on_async(svc.upload_document(
            &auth,
            file_data,
            p.original_filename,
            p.title,
            document_type_id,
            related_entity_id,
            p.related_entity_type,
            p.linked_field,
            sync_priority,
            compression_priority,
            temp_related_id,
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&document)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Bulk upload multiple documents
/// Expected JSON payload:
/// {
///   "files": [{"file_data": "base64", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "related_entity_id": "uuid",
///   "related_entity_type": "string",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "temp_related_id": "optional_uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_bulk_upload(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            files: Vec<FileData>,
            title: Option<String>,
            document_type_id: String,
            related_entity_id: String,
            related_entity_type: String,
            sync_priority: String,
            compression_priority: Option<String>,
            temp_related_id: Option<String>,
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
        
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        let related_entity_id = Uuid::parse_str(&p.related_entity_id)
            .map_err(|_| FFIError::invalid_argument("invalid related_entity_id"))?;
        let temp_related_id = p.temp_related_id.as_ref()
            .map(|s| Uuid::parse_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid temp_related_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let documents = block_on_async(svc.bulk_upload_documents(
            &auth,
            files,
            p.title,
            document_type_id,
            related_entity_id,
            p.related_entity_type,
            sync_priority,
            compression_priority,
            temp_related_id,
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&documents)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get media document by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "include": [DocumentIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            include: Option<Vec<DocumentIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<ServiceDocumentInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_document_service()?;
        let document = block_on_async(svc.get_media_document_by_id(&auth, id, include_slice))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&document)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List media documents by related entity
/// Expected JSON payload:
/// {
///   "related_table": "string",
///   "related_id": "uuid",
///   "pagination": { PaginationDto },
///   "include": [DocumentIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_list_by_entity(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            related_table: String,
            related_id: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<DocumentIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let related_id = Uuid::parse_str(&p.related_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<ServiceDocumentInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_document_service()?;
        let documents = block_on_async(svc.list_media_documents_by_related_entity(
            &auth, &p.related_table, related_id, params, include_slice
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&documents)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Download document (returns filename and base64 data if available locally)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_download(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let (filename, data) = block_on_async(svc.download_document(&auth, id))
            .map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct DownloadResponse {
            filename: String,
            data: Option<String>, // base64 encoded if available
        }
        
        let response = DownloadResponse {
            filename,
            data: data.map(|d| base64::encode(d)),
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Open document (get local file path if available)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_open(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let file_path = block_on_async(svc.open_document(&auth, id))
            .map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct OpenResponse {
            file_path: Option<String>,
        }
        
        let response = OpenResponse { file_path };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Check if document is available on device
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_is_available(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let is_available = block_on_async(svc.is_document_available_on_device(id))
            .map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct AvailabilityResponse {
            is_available: bool,
        }
        
        let response = AvailabilityResponse { is_available };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete media document (hard delete, admin only)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_delete(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        #[derive(Deserialize)] struct Payload { id: String, auth: AuthCtxDto }
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        block_on_async(svc.delete_media_document(&auth, id)).map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

/// Calculate document summary by linked fields
/// Expected JSON payload:
/// {
///   "related_table": "string",
///   "related_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_calculate_summary(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            related_table: String,
            related_id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let related_id = Uuid::parse_str(&p.related_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let summary = block_on_async(svc.calculate_document_summary_by_linked_fields(
            &auth, &p.related_table, related_id
        )).map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&summary)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Link temporary documents to final entity
/// Expected JSON payload:
/// {
///   "temp_related_id": "uuid",
///   "final_related_table": "string",
///   "final_related_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_link_temp(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            temp_related_id: String,
            final_related_table: String,
            final_related_id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let temp_related_id = Uuid::parse_str(&p.temp_related_id)
            .map_err(|_| FFIError::invalid_argument("invalid temp_related_id"))?;
        let final_related_id = Uuid::parse_str(&p.final_related_id)
            .map_err(|_| FFIError::invalid_argument("invalid final_related_id"))?;
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        let count = block_on_async(svc.link_temp_documents(
            temp_related_id, &p.final_related_table, final_related_id
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct LinkResponse {
            linked_count: u64,
        }
        
        let response = LinkResponse { linked_count: count };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Register document in use
/// Expected JSON payload:
/// {
///   "document_id": "uuid",
///   "user_id": "uuid",
///   "device_id": "uuid",
///   "use_type": "view|edit",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_register_in_use(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            document_id: String,
            user_id: String,
            device_id: String,
            use_type: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let document_id = Uuid::parse_str(&p.document_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let user_id = Uuid::parse_str(&p.user_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let device_id = Uuid::parse_str(&p.device_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        block_on_async(svc.register_document_in_use(document_id, user_id, device_id, &p.use_type))
            .map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

/// Unregister document in use
/// Expected JSON payload:
/// {
///   "document_id": "uuid",
///   "user_id": "uuid",
///   "device_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_unregister_in_use(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            document_id: String,
            user_id: String,
            device_id: String,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let document_id = Uuid::parse_str(&p.document_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let user_id = Uuid::parse_str(&p.user_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let device_id = Uuid::parse_str(&p.device_id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let _auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_document_service()?;
        
        block_on_async(svc.unregister_document_in_use(document_id, user_id, device_id))
            .map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Memory Management
// ---------------------------------------------------------------------------

/// Free memory allocated by Rust for C strings
/// MUST be called by Swift for every *mut c_char returned by document functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn document_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}
