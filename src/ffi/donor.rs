// src/ffi/donor.rs
// ============================================================================
// FFI bindings for the `DonorService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `donor_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//   •  Never pass a pointer obtained from Swift back into `donor_free` more than
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
use crate::domains::donor::types::{
    NewDonor, UpdateDonor, DonorResponse, DonorInclude, DonorSummary,
    DonorDashboardStats, DonorWithFundingDetails, DonorWithDocumentTimeline
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

/// DTO for donor includes
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum DonorIncludeDto {
    FundingStats,
    Documents,
    FundingDetails,
    All,
}

impl From<DonorIncludeDto> for DonorInclude {
    fn from(dto: DonorIncludeDto) -> Self {
        match dto {
            DonorIncludeDto::FundingStats => DonorInclude::FundingStats,
            DonorIncludeDto::Documents => DonorInclude::Documents,
            DonorIncludeDto::FundingDetails => DonorInclude::FundingDetails,
            DonorIncludeDto::All => DonorInclude::All,
        }
    }
}

// ---------------------------------------------------------------------------
// Basic CRUD Operations
// ---------------------------------------------------------------------------

/// Create a new donor
/// Expected JSON payload:
/// {
///   "donor": { NewDonor },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_create(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            donor: NewDonor,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let donor = block_on_async(svc.create_donor(p.donor, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donor)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Create a new donor with documents
/// Expected JSON payload:
/// {
///   "donor": { NewDonor },
///   "documents": [{"file_data": "base64", "filename": "string", "linked_field": "optional_string"}, ...],
///   "document_type_id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_create_with_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            donor: NewDonor,
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
        let svc = globals::get_donor_service()?;
        
        let (donor, doc_results) = block_on_async(svc.create_donor_with_documents(
            p.donor, documents, document_type_id, &auth
        )).map_err(FFIError::from_service_error)?;
        
        #[derive(Serialize)]
        struct CreateWithDocsResponse {
            donor: DonorResponse,
            document_results: Vec<Result<crate::domains::document::types::MediaDocumentResponse, String>>,
        }
        
        let response = CreateWithDocsResponse {
            donor,
            document_results: doc_results.into_iter().map(|r| r.map_err(|e| e.to_string())).collect(),
        };
        
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get donor by ID
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donor = block_on_async(svc.get_donor_by_id(id, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donor)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// List donors with pagination and includes
/// Expected JSON payload:
/// {
///   "pagination": { PaginationDto },
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_list(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            pagination: Option<PaginationDto>,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donors = block_on_async(svc.list_donors(params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donors)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Update donor
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "update": { UpdateDonor },
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_update(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            id: String,
            update: UpdateDonor,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let donor = block_on_async(svc.update_donor(id, p.update, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donor)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Delete donor (soft or hard delete)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "hard_delete": bool,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_delete(payload_json: *const c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
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
        let svc = globals::get_donor_service()?;
        
        block_on_async(svc.delete_donor(id, p.hard_delete.unwrap_or(false), &auth))
            .map_err(FFIError::from_service_error)?;
        Ok(())
    })
}

/// Get donor summary (lightweight version)
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_summary(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let summary = block_on_async(svc.get_donor_summary(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&summary)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Document Integration
// ---------------------------------------------------------------------------

/// Upload a single document for a donor
/// Expected JSON payload:
/// {
///   "donor_id": "uuid",
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
pub unsafe extern "C" fn donor_upload_document(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            donor_id: String,
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
        
        let donor_id = Uuid::parse_str(&p.donor_id)
            .map_err(|_| FFIError::invalid_argument("invalid donor_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let document = block_on_async(svc.upload_document_for_donor(
            donor_id,
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

/// Bulk upload multiple documents for a donor
/// Expected JSON payload:
/// {
///   "donor_id": "uuid",
///   "files": [{"file_data": "base64", "filename": "string"}, ...],
///   "title": "optional_string",
///   "document_type_id": "uuid",
///   "sync_priority": "HIGH|NORMAL|LOW",
///   "compression_priority": "HIGH|NORMAL|LOW",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_bulk_upload_documents(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
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
            donor_id: String,
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
        
        let donor_id = Uuid::parse_str(&p.donor_id)
            .map_err(|_| FFIError::invalid_argument("invalid donor_id"))?;
        let document_type_id = Uuid::parse_str(&p.document_type_id)
            .map_err(|_| FFIError::invalid_argument("invalid document_type_id"))?;
        
        let sync_priority = SyncPriority::from_str(&p.sync_priority)
            .map_err(|_| FFIError::invalid_argument("invalid sync_priority"))?;
        let compression_priority = p.compression_priority.as_ref()
            .map(|s| CompressionPriority::from_str(s))
            .transpose()
            .map_err(|_| FFIError::invalid_argument("invalid compression_priority"))?;
        
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let documents = block_on_async(svc.bulk_upload_documents_for_donor(
            donor_id,
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

/// Get donor statistics for dashboard
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_statistics(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let stats = block_on_async(svc.get_donor_statistics(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&stats)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get donor type distribution
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_type_distribution(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let distribution = block_on_async(svc.get_type_distribution(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get donor country distribution
/// Expected JSON payload:
/// {
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_country_distribution(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let distribution = block_on_async(svc.get_country_distribution(&auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&distribution)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Filtered Queries
// ---------------------------------------------------------------------------

/// Find donors by type
/// Expected JSON payload:
/// {
///   "donor_type": "string",
///   "pagination": { PaginationDto },
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_find_by_type(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            donor_type: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donors = block_on_async(svc.find_donors_by_type(&p.donor_type, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donors)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find donors by country
/// Expected JSON payload:
/// {
///   "country": "string",
///   "pagination": { PaginationDto },
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_find_by_country(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            country: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donors = block_on_async(svc.find_donors_by_country(&p.country, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donors)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find donors with recent donations
/// Expected JSON payload:
/// {
///   "days_ago": u32,
///   "pagination": { PaginationDto },
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_find_with_recent_donations(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            days_ago: u32,
            pagination: Option<PaginationDto>,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donors = block_on_async(svc.find_donors_with_recent_donations(p.days_ago, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donors)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Find donors by date range
/// Expected JSON payload:
/// {
///   "start_date": "2024-01-01T00:00:00Z",
///   "end_date": "2024-12-31T23:59:59Z",
///   "pagination": { PaginationDto },
///   "include": [DonorIncludeDto, ...],
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_find_by_date_range(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            start_date: String,
            end_date: String,
            pagination: Option<PaginationDto>,
            include: Option<Vec<DonorIncludeDto>>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let params = p.pagination.map(|p| p.into()).unwrap_or_default();
        let auth: AuthContext = p.auth.try_into()?;
        
        let include: Option<Vec<DonorInclude>> = p.include.map(|inc| 
            inc.into_iter().map(|i| i.into()).collect()
        );
        let include_slice = include.as_ref().map(|v| v.as_slice());
        
        let svc = globals::get_donor_service()?;
        let donors = block_on_async(svc.find_donors_by_date_range(&p.start_date, &p.end_date, params, include_slice, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donors)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Detailed Views
// ---------------------------------------------------------------------------

/// Get donor with funding details
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_with_funding_details(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let donor_details = block_on_async(svc.get_donor_with_funding_details(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donor_details)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Get donor with document timeline
/// Expected JSON payload:
/// {
///   "id": "uuid",
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_get_with_document_timeline(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload { id: String, auth: AuthCtxDto }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let id = Uuid::parse_str(&p.id).map_err(|_| FFIError::invalid_argument("uuid"))?;
        let auth: AuthContext = p.auth.try_into()?;
        let svc = globals::get_donor_service()?;
        
        let donor_timeline = block_on_async(svc.get_donor_with_document_timeline(id, &auth))
            .map_err(FFIError::from_service_error)?;
        
        let json_resp = serde_json::to_string(&donor_timeline)
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
/// MUST be called by Swift for every *mut c_char returned by donor functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn donor_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
} 