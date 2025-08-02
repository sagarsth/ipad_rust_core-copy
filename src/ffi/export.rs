// src/ffi/export.rs
// ============================================================================
// FFI bindings for the `ExportService`.
// All heavy–lifting logic lives in the domain/service layer. These wrappers
// simply (1) decode C-strings coming from Swift, (2) forward the request to the
// relevant async service method using a temporary Tokio runtime, (3) encode the
// result into JSON, and (4) return the string back across the FFI boundary.
//
// IMPORTANT – memory ownership rules:
//   •  Any *mut c_char returned from Rust must be freed by Swift by calling
//      the `export_free` function exported below. Internally we create the
//      CString with `into_raw()` which transfers ownership to the caller.
//
// ============================================================================

use crate::ffi::{handle_status_result, error::FFIError};
use crate::auth::AuthContext;
use crate::domains::export::types::{ExportRequest, ExportSummary, EntityFilter, ExportStatus, ExportFormat};
// Removed redundant v1 service import
use crate::domains::export::service_v2::{ExportServiceV2, ExportProgress};
use crate::domains::export::repository::SqliteExportJobRepository;
use crate::domains::export::repository_v2::SqliteStreamingRepository;
use crate::globals;
use tokio::runtime::Runtime;
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;
use uuid::Uuid;
use serde_json::json;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::path::PathBuf;
use serde::Deserialize;
use crate::types::UserRole;
use std::str::FromStr;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper function to run async code in a blocking way for FFI
fn block_on_async<F, T, E>(future: F) -> Result<T, E> 
where 
    F: std::future::Future<Output = Result<T, E>>,
{
            crate::ffi::block_on_async(future)
}

/// Helper to create auth context from token
fn create_auth_context_from_token(token: &str) -> Result<AuthContext, FFIError> {
    let auth_service = crate::globals::get_auth_service()
        .map_err(|e| FFIError::internal(format!("Failed to get auth service: {}", e)))?;
    block_on_async(auth_service.verify_token(token))
        .map_err(|e| FFIError::internal(format!("Token verification failed: {}", e)))
}

/// Helper to parse JSON payload
fn parse_json_payload<T: serde::de::DeserializeOwned>(json_str: &str) -> Result<T, FFIError> {
    serde_json::from_str(json_str)
        .map_err(|e| FFIError::invalid_argument(&format!("Invalid JSON payload: {}", e)))
}

/// Helper to create JSON response
fn create_json_response<T: serde::Serialize>(data: T) -> Result<*mut c_char, FFIError> {
    let json_string = serde_json::to_string(&data)
        .map_err(|e| FFIError::internal(format!("JSON serialization failed: {}", e)))?;
    
    let c_string = CString::new(json_string)
        .map_err(|e| FFIError::internal(format!("CString creation failed: {}", e)))?;
    
    Ok(c_string.into_raw())
}

// Removed redundant v1 service builder - use V2 only

/// Helper to build modern V2 export service with streaming and iOS optimizations
fn build_export_service_v2() -> Result<ExportServiceV2, FFIError> {
    let pool = crate::globals::get_db_pool()
        .map_err(|e| FFIError::internal(format!("Failed to get DB pool: {}", e)))?;
    let file_storage = crate::globals::get_file_storage_service()
        .map_err(|e| FFIError::internal(format!("Failed to get file storage service: {}", e)))?;
    let job_repo = Arc::new(SqliteExportJobRepository::new(pool.clone()));
    let streaming_repo = Arc::new(SqliteStreamingRepository::new(pool));
    let service = ExportServiceV2::new(job_repo, streaming_repo, file_storage);
    Ok(Arc::try_unwrap(service).unwrap())
}

/// Helper to format export job response
fn format_export_job_response(summary: ExportSummary) -> serde_json::Value {
    json!({
        "job": {
            "id": summary.job.id.to_string(),
            "requested_by_user_id": summary.job.requested_by_user_id.map(|id| id.to_string()),
            "requested_at": summary.job.requested_at.to_rfc3339(),
            "include_blobs": summary.job.include_blobs,
            "status": format!("{:?}", summary.job.status),
            "local_path": summary.job.local_path,
            "total_entities": summary.job.total_entities,
            "total_bytes": summary.job.total_bytes,
            "error_message": summary.job.error_message,
        }
    })
}

/// Helper to parse date from string
fn parse_date(date_str: &str) -> Result<DateTime<Utc>, FFIError> {
    DateTime::parse_from_rfc3339(date_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| FFIError::invalid_argument(&format!("Invalid date format: {}", date_str)))
}

/// C-friendly representation of AuthContext coming from Swift (for legacy compatibility)
#[derive(Deserialize)]
struct AuthCtxDto {
    user_id: String,
    role: String,
    device_id: String,
    offline_mode: bool,
}

fn dto_to_auth(dto: AuthCtxDto) -> Result<AuthContext, FFIError> {
    Ok(AuthContext::new(
        Uuid::parse_str(&dto.user_id).map_err(|_| FFIError::invalid_argument("Invalid user_id UUID"))?,
        UserRole::from_str(&dto.role).ok_or_else(|| FFIError::invalid_argument("Invalid role"))?,
        dto.device_id,
        dto.offline_mode,
    ))
}

// ============================================================================
// CORE EXPORT FUNCTIONS
// ============================================================================

/// Create a new export job
/// 
/// # Arguments
/// * `export_request_json` - JSON containing export request data
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary with job ID and status
/// 
/// # Safety
/// This function should only be called with valid, null-terminated C strings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_create_export(
    export_request_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_request_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_request_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export request JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let export_request: ExportRequest = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        // Always use V2 service for modern streaming, CSV/Parquet support with iOS optimizations
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("V2 Export creation failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Get export job status
/// 
/// # Arguments
/// * `job_id` - UUID of the export job
/// 
/// # Returns
/// JSON containing export job summary with current status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_get_status(
    job_id: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if job_id.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let id_str = CStr::from_ptr(job_id).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid job ID string"))?;
        
        let id = Uuid::parse_str(id_str)
            .map_err(|_| FFIError::invalid_argument("Invalid UUID format"))?;
        
        // Use V2 service for all operations
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.get_export_status(id))
            .map_err(|e| FFIError::internal(format!("Failed to get export status: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for strategic goals by specific IDs
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options and IDs
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary with job ID and status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_strategic_goals_by_ids(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        #[derive(Deserialize)]
        struct ExportByIdsOptions {
            ids: Vec<String>,
            include_blobs: Option<bool>,
            target_path: Option<String>,
            format: Option<ExportFormat>,
        }
        
        let options: ExportByIdsOptions = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        // Parse IDs to UUIDs with debugging
        log::info!("Received IDs for export: {:?}", options.ids);
        let ids: Result<Vec<Uuid>, _> = options.ids.iter()
            .map(|id_str| {
                log::info!("Attempting to parse UUID: '{}'", id_str);
                let result = Uuid::parse_str(id_str);
                if let Err(ref e) = result {
                    log::error!("Failed to parse UUID '{}': {}", id_str, e);
                }
                result
            })
            .collect();
        let ids = ids.map_err(|e| FFIError::invalid_argument(&format!("Invalid UUID format in IDs: {}", e)))?;
        
        let filters = vec![EntityFilter::StrategicGoalsByIds { ids }];
        let include_blobs = options.include_blobs.unwrap_or(false);
        let target_path = options.target_path.map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters,
            include_blobs,
            target_path,
            format: options.format.or_else(|| Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            })),
            use_compression: false,
            use_background: false,
        };
        
        // Use V2 service for proper CSV/Parquet streaming with iOS optimizations
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Export creation failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for all strategic goals
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path, status_id)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_strategic_goals_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        let status_id = options["status_id"].as_i64();
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::StrategicGoals { status_id }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Strategic goals export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

// ============================================================================
// INDIVIDUAL DOMAIN EXPORTS
// ============================================================================

/// Create export for projects by specific IDs
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options and IDs
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary with job ID and status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_projects_by_ids(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        #[derive(Deserialize)]
        struct ExportByIdsOptions {
            ids: Vec<String>,
            include_blobs: Option<bool>,
            target_path: Option<String>,
            format: Option<ExportFormat>,
        }
        
        let options: ExportByIdsOptions = parse_json_payload(json_str)?;
        log::info!("[PROJECT_EXPORT_FFI] Parsed export options: include_blobs={}, target_path={:?}, format={:?}", 
                   options.include_blobs.unwrap_or(false), options.target_path, options.format);
        
        let auth_context = create_auth_context_from_token(token_str)?;
        log::info!("[PROJECT_EXPORT_FFI] Created auth context for user: {}", auth_context.user_id);
        
        // Parse IDs to UUIDs with debugging
        log::info!("[PROJECT_EXPORT_FFI] Received project IDs for export: {:?}", options.ids);
        let ids: Result<Vec<Uuid>, _> = options.ids.iter()
            .enumerate()
            .map(|(idx, id_str)| {
                log::info!("[PROJECT_EXPORT_FFI] Attempting to parse project UUID {}: '{}'", idx, id_str);
                let result = Uuid::parse_str(id_str);
                if let Err(ref e) = result {
                    log::error!("[PROJECT_EXPORT_FFI] Failed to parse project UUID '{}': {}", id_str, e);
                } else {
                    log::info!("[PROJECT_EXPORT_FFI] Successfully parsed project UUID {}: {}", idx, result.as_ref().unwrap());
                }
                result
            })
            .collect();
        let ids = ids.map_err(|e| {
            log::error!("[PROJECT_EXPORT_FFI] UUID parsing failed: {}", e);
            FFIError::invalid_argument(&format!("Invalid UUID format in project IDs: {}", e))
        })?;
        
        let filters = vec![EntityFilter::ProjectsByIds { ids: ids.clone() }];
        let include_blobs = options.include_blobs.unwrap_or(false);
        let target_path = options.target_path.map(PathBuf::from);
        
        log::info!("[PROJECT_EXPORT_FFI] Creating filter: ProjectsByIds with {} IDs", ids.len());
        log::info!("[PROJECT_EXPORT_FFI] Export settings: include_blobs={}, target_path={:?}", include_blobs, target_path);
        
        let export_request = ExportRequest {
            filters,
            include_blobs,
            target_path,
            format: options.format.or_else(|| Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            })),
            use_compression: false,
            use_background: false,
        };
        
        log::info!("[PROJECT_EXPORT_FFI] Created export request with format: {:?}", export_request.format);
        
        // Use V2 service for proper CSV/Parquet streaming with iOS optimizations
        log::info!("[PROJECT_EXPORT_FFI] Building export service V2");
        let export_service_v2 = build_export_service_v2()?;
        
        log::info!("[PROJECT_EXPORT_FFI] Calling export_streaming on service V2");
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| {
                log::error!("[PROJECT_EXPORT_FFI] Export streaming failed: {}", e);
                FFIError::internal(format!("Project export creation failed: {}", e))
            })?;
        
        log::info!("[PROJECT_EXPORT_FFI] Export streaming completed successfully, job_id: {}", summary.job.id);
        
        let response = format_export_job_response(summary);
        log::info!("[PROJECT_EXPORT_FFI] Formatted export job response");
        
        *result = create_json_response(response)?;
        log::info!("[PROJECT_EXPORT_FFI] Created JSON response and returning success");
        Ok(())
    })
}

/// Create export for all projects
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_projects_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::ProjectsAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Projects export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for participants by specific IDs
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options and IDs
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary with job ID and status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_participants_by_ids(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        #[derive(Deserialize)]
        struct ExportByIdsOptions {
            ids: Vec<String>,
            include_blobs: Option<bool>,
            target_path: Option<String>,
            format: Option<ExportFormat>,
        }
        
        let options: ExportByIdsOptions = parse_json_payload(json_str)?;
        log::info!("[PARTICIPANT_EXPORT_FFI] Parsed export options: include_blobs={}, target_path={:?}, format={:?}", 
                   options.include_blobs.unwrap_or(false), options.target_path, options.format);
        
        let auth_context = create_auth_context_from_token(token_str)?;
        log::info!("[PARTICIPANT_EXPORT_FFI] Created auth context for user: {}", auth_context.user_id);
        
        // Parse IDs to UUIDs with debugging
        log::info!("[PARTICIPANT_EXPORT_FFI] Received participant IDs for export: {:?}", options.ids);
        let ids: Result<Vec<Uuid>, _> = options.ids.iter()
            .enumerate()
            .map(|(idx, id_str)| {
                log::info!("[PARTICIPANT_EXPORT_FFI] Attempting to parse participant UUID {}: '{}'", idx, id_str);
                let result = Uuid::parse_str(id_str);
                if let Err(ref e) = result {
                    log::error!("[PARTICIPANT_EXPORT_FFI] Failed to parse participant UUID '{}': {}", id_str, e);
                } else {
                    log::info!("[PARTICIPANT_EXPORT_FFI] Successfully parsed participant UUID {}: {}", idx, result.as_ref().unwrap());
                }
                result
            })
            .collect();
        let ids = ids.map_err(|e| {
            log::error!("[PARTICIPANT_EXPORT_FFI] UUID parsing failed: {}", e);
            FFIError::invalid_argument(&format!("Invalid UUID format in participant IDs: {}", e))
        })?;
        
        let filters = vec![EntityFilter::ParticipantsByIds { ids: ids.clone() }];
        let include_blobs = options.include_blobs.unwrap_or(false);
        let target_path = options.target_path.map(PathBuf::from);
        
        log::info!("[PARTICIPANT_EXPORT_FFI] Creating filter: ParticipantsByIds with {} IDs", ids.len());
        log::info!("[PARTICIPANT_EXPORT_FFI] Export settings: include_blobs={}, target_path={:?}", include_blobs, target_path);
        
        let export_request = ExportRequest {
            filters,
            include_blobs,
            target_path,
            format: options.format.or_else(|| Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            })),
            use_compression: false,
            use_background: false,
        };
        
        log::info!("[PARTICIPANT_EXPORT_FFI] Created export request with format: {:?}", export_request.format);
        
        // Use V2 service for proper CSV/Parquet streaming with iOS optimizations
        log::info!("[PARTICIPANT_EXPORT_FFI] Building export service V2");
        let export_service_v2 = build_export_service_v2()?;
        
        log::info!("[PARTICIPANT_EXPORT_FFI] Calling export_streaming on service V2");
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| {
                log::error!("[PARTICIPANT_EXPORT_FFI] Export streaming failed: {}", e);
                FFIError::internal(format!("Participant export creation failed: {}", e))
            })?;
        
        log::info!("[PARTICIPANT_EXPORT_FFI] Export streaming completed successfully, job_id: {}", summary.job.id);
        
        let response = format_export_job_response(summary);
        log::info!("[PARTICIPANT_EXPORT_FFI] Formatted export job response");
        
        *result = create_json_response(response)?;
        log::info!("[PARTICIPANT_EXPORT_FFI] Created JSON response and returning success");
        Ok(())
    })
}

/// Create export for all participants
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_participants_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::ParticipantsAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Participants export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for all activities
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_activities_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::ActivitiesAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Activities export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for activities by specific IDs
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options and IDs
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary with job ID and status
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_activities_by_ids(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        #[derive(Deserialize)]
        struct ExportByIdsOptions {
            ids: Vec<String>,
            include_blobs: Option<bool>,
            target_path: Option<String>,
            format: Option<ExportFormat>,
        }
        
        let options: ExportByIdsOptions = parse_json_payload(json_str)?;
        log::info!("[ACTIVITY_EXPORT_FFI] Parsed export options: include_blobs={}, target_path={:?}, format={:?}", 
                   options.include_blobs.unwrap_or(false), options.target_path, options.format);
        
        let auth_context = create_auth_context_from_token(token_str)?;
        log::info!("[ACTIVITY_EXPORT_FFI] Created auth context for user: {}", auth_context.user_id);
        
        // Parse IDs to UUIDs with debugging
        log::info!("[ACTIVITY_EXPORT_FFI] Received activity IDs for export: {:?}", options.ids);
        let ids: Result<Vec<Uuid>, _> = options.ids.iter()
            .enumerate()
            .map(|(idx, id_str)| {
                log::info!("[ACTIVITY_EXPORT_FFI] Attempting to parse activity UUID {}: '{}'", idx, id_str);
                let result = Uuid::parse_str(id_str);
                if let Err(ref e) = result {
                    log::error!("[ACTIVITY_EXPORT_FFI] Failed to parse activity UUID '{}': {}", id_str, e);
                } else {
                    log::info!("[ACTIVITY_EXPORT_FFI] Successfully parsed activity UUID {}: {}", idx, result.as_ref().unwrap());
                }
                result
            })
            .collect();
        let ids = ids.map_err(|e| {
            log::error!("[ACTIVITY_EXPORT_FFI] UUID parsing failed: {}", e);
            FFIError::invalid_argument(&format!("Invalid UUID format in activity IDs: {}", e))
        })?;
        
        let filters = vec![EntityFilter::ActivitiesByIds { ids: ids.clone() }];
        let include_blobs = options.include_blobs.unwrap_or(false);
        let target_path = options.target_path.map(PathBuf::from);
        
        log::info!("[ACTIVITY_EXPORT_FFI] Creating filter: ActivitiesByIds with {} IDs", ids.len());
        log::info!("[ACTIVITY_EXPORT_FFI] Export settings: include_blobs={}, target_path={:?}", include_blobs, target_path);
        
        let export_request = ExportRequest {
            filters,
            include_blobs,
            target_path,
            format: options.format.or_else(|| Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            })),
            use_compression: false,
            use_background: false,
        };
        
        log::info!("[ACTIVITY_EXPORT_FFI] Created export request with format: {:?}", export_request.format);

        
        // Use V2 service for proper CSV/Parquet streaming with iOS optimizations
        log::info!("[ACTIVITY_EXPORT_FFI] Building export service V2");
        let export_service_v2 = build_export_service_v2()?;
        
        log::info!("[ACTIVITY_EXPORT_FFI] Calling export_streaming on service V2");
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| {
                log::error!("[ACTIVITY_EXPORT_FFI] Export streaming failed: {}", e);
                FFIError::internal(format!("Activity export creation failed: {}", e))
            })?;
        
        log::info!("[ACTIVITY_EXPORT_FFI] Export streaming completed successfully, job_id: {}", summary.job.id);
        
        let response = format_export_job_response(summary);
        log::info!("[ACTIVITY_EXPORT_FFI] Formatted export job response");
        
        *result = create_json_response(response)?;
        log::info!("[ACTIVITY_EXPORT_FFI] Created JSON response and returning success");
        Ok(())
    })
}

/// Create export for all donors
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_donors_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::DonorsAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Donors export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for all funding records
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_funding_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::FundingAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Funding export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for all livelihoods
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_livelihoods_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::LivelihoodsAll],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Livelihoods export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for all workshops
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_workshops_all(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::WorkshopsAll { include_participants: true }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Workshops export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create unified export for all domains
/// 
/// # Arguments
/// * `unified_export_json` - JSON containing export options (include_blobs, target_path, include_type_tags)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_unified_all_domains(
    unified_export_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if unified_export_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(unified_export_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid unified export JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        let include_type_tags = options["include_type_tags"].as_bool().unwrap_or(true);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::UnifiedAllDomains { include_type_tags }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Unified export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

// ============================================================================
// DATE RANGE EXPORTS
// ============================================================================

/// Create export for strategic goals within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path, status_id)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_strategic_goals_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        let status_id = options["status_id"].as_i64();
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::StrategicGoalsByDateRange { start_date, end_date, status_id }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Strategic goals date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for strategic goals using complex filter (matches UI filtering logic)
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options with complex filter
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_strategic_goals_by_filter(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        #[derive(Deserialize)]
        struct ExportWithFilterOptions {
            include_blobs: Option<bool>,
            target_path: Option<String>,
            filter: serde_json::Value, // We'll parse this as the complex filter
        }
        
        let options: ExportWithFilterOptions = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        // Parse the complex filter
        let strategic_goal_filter: crate::domains::strategic_goal::types::StrategicGoalFilter = 
            serde_json::from_value(options.filter)
                .map_err(|e| FFIError::invalid_argument(&format!("Invalid filter format: {}", e)))?;
        
        let include_blobs = options.include_blobs.unwrap_or(false);
        let target_path = options.target_path.map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::StrategicGoalsByFilter { filter: strategic_goal_filter }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Strategic goals filter export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for projects within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_projects_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::ProjectsByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Projects date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for activities within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_activities_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::ActivitiesByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Activities date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for donors within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_donors_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::DonorsByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Donors date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for funding within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_funding_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::FundingByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Funding date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for livelihoods within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_livelihoods_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::LivelihoodsByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Livelihoods date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for workshops within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_workshops_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::WorkshopsByDateRange { start_date, end_date, include_participants: true }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Workshops date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create export for media documents within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_media_documents_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::MediaDocumentsByDateRange { start_date, end_date }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Media documents date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Create unified export for all domains within date range
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (start_date, end_date, include_blobs, target_path, include_type_tags)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_unified_by_date_range(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let start_date = parse_date(options["start_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing start_date"))?)?;
        let end_date = parse_date(options["end_date"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing end_date"))?)?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        let include_type_tags = options["include_type_tags"].as_bool().unwrap_or(true);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::UnifiedByDateRange { start_date, end_date, include_type_tags }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service = build_export_service_v2()?;
        let summary = block_on_async(export_service.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Unified date range export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

// ============================================================================
// MEDIA DOCUMENT EXPORTS
// ============================================================================

/// Create export for media documents by related entity
/// 
/// # Arguments
/// * `export_options_json` - JSON containing export options (related_table, related_id, include_blobs, target_path)
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_media_documents_by_entity(
    export_options_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_options_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_options_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export options JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let options: serde_json::Value = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let related_table = options["related_table"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing related_table"))?
            .to_string();
        let related_id = Uuid::parse_str(options["related_id"].as_str()
            .ok_or_else(|| FFIError::invalid_argument("Missing related_id"))?)
            .map_err(|_| FFIError::invalid_argument("Invalid related_id UUID"))?;
        let include_blobs = options["include_blobs"].as_bool().unwrap_or(false);
        let target_path = options["target_path"].as_str().map(PathBuf::from);
        
        let export_request = ExportRequest {
            filters: vec![EntityFilter::MediaDocumentsByRelatedEntity { related_table, related_id }],
            include_blobs,
            target_path,
            format: Some(ExportFormat::Csv { 
                delimiter: b',', 
                quote_char: b'"', 
                escape_char: None, 
                compress: false 
            }),
            use_compression: false,
            use_background: false,
        };
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Media documents by entity export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

// ============================================================================
// CUSTOM AND ADVANCED EXPORTS
// ============================================================================

/// Create custom export with multiple filters
/// 
/// # Arguments
/// * `filters_json` - JSON containing array of filters and export options
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing export job summary
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_create_custom(
    filters_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if filters_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(filters_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid filters JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        let export_request: ExportRequest = parse_json_payload(json_str)?;
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(export_request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Custom export failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Validate export request before creating
/// 
/// # Arguments
/// * `export_request_json` - JSON containing export request data
/// * `token` - Access token for authentication
/// 
/// # Returns
/// JSON containing validation result
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_validate_request(
    export_request_json: *const c_char,
    token: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if export_request_json.is_null() || token.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(export_request_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid export request JSON string"))?;
        
        let token_str = CStr::from_ptr(token).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid token string"))?;
        
        // Validate JSON parsing
        let export_request: Result<ExportRequest, _> = serde_json::from_str(json_str);
        let auth_context = create_auth_context_from_token(token_str)?;
        
        let validation_result = match export_request {
            Ok(request) => {
                // Check permissions
                match auth_context.authorize(crate::types::Permission::ExportData) {
                    Ok(_) => json!({
                        "valid": true,
                        "message": "Export request is valid",
                        "filters_count": request.filters.len(),
                        "include_blobs": request.include_blobs
                    }),
                    Err(e) => json!({
                        "valid": false,
                        "message": format!("Permission denied: {}", e),
                        "error_type": "permission_denied"
                    })
                }
            },
            Err(e) => json!({
                "valid": false,
                "message": format!("Invalid request format: {}", e),
                "error_type": "invalid_format"
            })
        };
        
        *result = create_json_response(validation_result)?;
        Ok(())
    })
}

// ============================================================================
// MEMORY MANAGEMENT
// ============================================================================

/// Free a string allocated by the export FFI functions
/// 
/// # Safety
/// This function should only be called with pointers returned from export FFI functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); }
    }
}

// ============================================================================
// LEGACY COMPATIBILITY (keeping old function names)
// ============================================================================

/// Legacy export_create function (redirects to export_create_export)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn export_create(
    request_json: *const c_char,
    result: *mut *mut c_char,
) -> c_int {
    handle_status_result(|| unsafe {
        if request_json.is_null() || result.is_null() {
            return Err(FFIError::invalid_argument("Null pointer(s) provided"));
        }
        
        let json_str = CStr::from_ptr(request_json).to_str()
            .map_err(|_| FFIError::invalid_argument("Invalid request JSON string"))?;
        
        // Parse the old payload format that included auth context
        #[derive(Deserialize)]
        struct LegacyPayload {
            request: ExportRequest,
            auth: AuthCtxDto,
        }
        
        let payload: LegacyPayload = parse_json_payload(json_str)?;
        let auth_context = dto_to_auth(payload.auth)?;
        
        let export_service_v2 = build_export_service_v2()?;
        let summary = block_on_async(export_service_v2.export_streaming(payload.request, &auth_context))
            .map_err(|e| FFIError::internal(format!("Legacy export creation failed: {}", e)))?;
        
        let response = format_export_job_response(summary);
        *result = create_json_response(response)?;
        Ok(())
    })
}

/// Simple ping function to test FFI.
#[unsafe(no_mangle)]
pub extern "C" fn ping(message: *const c_char) -> *mut c_char {
    let message_str = unsafe { CStr::from_ptr(message).to_str().unwrap() };
    let response = format!("pong: {}", message_str);
    CString::new(response).unwrap().into_raw()
} 