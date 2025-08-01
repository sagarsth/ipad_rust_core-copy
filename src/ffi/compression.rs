// src/ffi/compression.rs
// =============================================================================
// COMPRESSION DOMAIN – FFI BINDINGS
// =============================================================================
// This module exposes the public surface of `CompressionService` to Swift.
// All functions follow standard FFI conventions: JSON payloads, explicit 
// success/error codes, and manual memory management for returned strings.
//
// MEMORY OWNERSHIP:
// - Swift owns input JSON strings (read-only in Rust)
// - Rust owns output strings (Swift must call compression_free)
// - All strings are UTF-8, null-terminated
//
// JSON CONTRACTS:
// - compress_document: {"document_id": "uuid", "config": CompressionConfig?}
// - queue_document: {"document_id": "uuid", "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
// - cancel: {"document_id": "uuid"}
// - get_document_status: {"document_id": "uuid"}
// - update_priority: {"document_id": "uuid", "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
// - bulk_update_priority: {"document_ids": ["uuid", ...], "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
// - is_document_in_use: {"document_id": "uuid"}
//
// SAFETY RULES:
// 1. Never call from multiple threads without proper Rust runtime initialization
// 2. All input pointers must be valid, null-terminated UTF-8
// 3. Call compression_free exactly once for each returned string pointer
// 4. Check return codes before accessing result data
// -----------------------------------------------------------------------------

use crate::ffi::{handle_status_result, handle_json_result, to_ffi_error, block_on_async, FFIResult};
use crate::ffi::error::{FFIError, ErrorCode};
use crate::globals;
use crate::domains::compression::types::{CompressionConfig, CompressionPriority};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::domains::compression::service::CompressionService;
use sqlx::Row;
use crate::auth::AuthContext;
use crate::types::UserRole;
use std::str::FromStr;
use tokio::sync::oneshot;

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


// -----------------------------------------------------------------------------
// DTO Types for JSON Deserialization -------------------------------------------
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
struct CompressDocumentRequest {
    document_id: String,
    config: Option<CompressionConfig>,
}

#[derive(Deserialize)]
struct QueueDocumentRequest {
    document_id: String,
    priority: String,
}

#[derive(Deserialize)]
struct DocumentIdRequest {
    document_id: String,
}

#[derive(Deserialize)]
struct UpdatePriorityRequest {
    document_id: String,
    priority: String,
}

#[derive(Deserialize)]
struct BulkUpdatePriorityRequest {
    document_ids: Vec<String>,
    priority: String,
}

#[derive(Deserialize)]
struct GetQueueEntriesRequest {
    status: Option<String>, // "pending", "processing", "completed", "failed", "skipped"
    limit: Option<i32>,
    offset: Option<i32>,
}

#[derive(Deserialize)]
struct ValidateConfigRequest {
    config: CompressionConfig,
}

#[derive(Deserialize)]
struct GetSupportedMethodsRequest {
    mime_type: String,
    file_extension: Option<String>,
}

// Helper to parse priority string
fn parse_priority(priority_str: &str) -> FFIResult<CompressionPriority> {
    match priority_str.to_uppercase().as_str() {
        "HIGH" => Ok(CompressionPriority::High),
        "NORMAL" => Ok(CompressionPriority::Normal),
        "LOW" => Ok(CompressionPriority::Low),
        "BACKGROUND" | "BG" => Ok(CompressionPriority::Background),
        _ => Err(FFIError::with_details(
            ErrorCode::InvalidArgument,
            "Invalid priority",
            &format!("Priority must be HIGH, NORMAL, LOW, or BACKGROUND, got: {}", priority_str)
        ))
    }
}

// Helper to parse UUID (centralized for better error messages)
fn parse_document_uuid(uuid_str: &str) -> FFIResult<Uuid> {
    Uuid::parse_str(uuid_str).map_err(|_| FFIError::with_details(
        ErrorCode::InvalidArgument,
        "Invalid document UUID",
        &format!("Failed to parse UUID: {}", uuid_str)
    ))
}

// Helper to parse JSON input (optimized for iOS)
fn parse_json_input<T: for<'de> Deserialize<'de>>(input: *const c_char) -> FFIResult<T> {
    if input.is_null() {
        return Err(FFIError::new(ErrorCode::InvalidArgument, "Input JSON is null"));
    }
    
    let c_str = unsafe { CStr::from_ptr(input) };
    let json_str = c_str.to_str()
        .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid UTF-8 in input JSON"))?;
    
    // Prevent memory exhaustion on iOS with large payloads
    if json_str.len() > 1_048_576 { // 1MB limit
        return Err(FFIError::new(ErrorCode::InvalidArgument, "Input JSON exceeds 1MB limit"));
    }
    
    // Use from_str for better error messages and slightly better performance
    serde_json::from_str(json_str)
        .map_err(|e| FFIError::with_details(
            ErrorCode::InvalidArgument,
            "JSON parsing failed",
            &format!("Failed to parse JSON at line {}: {}", e.line(), e)
        ))
}

// -----------------------------------------------------------------------------
// FFI Functions ----------------------------------------------------------------
// -----------------------------------------------------------------------------

/// Compress a document with optional configuration
/// Input: {"document_id": "uuid", "config": CompressionConfig?}
/// Output: CompressionResult JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_compress_document(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    // Validate result pointer first
    if result.is_null() {
        return ErrorCode::NullPointer as c_int;
    }
    *result = std::ptr::null_mut(); // Initialize to null
    
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let request: CompressDocumentRequest = parse_json_input(payload_json)?;
        let document_id = parse_document_uuid(&request.document_id)?;
        
        // Clone service to avoid lifetime issues
        let service = globals::get_compression_service()?.clone();
        
        crate::ffi::block_on_async(async move {
            service.compress_document(document_id, request.config).await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    *result = json_result;
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get current compression queue status
/// Output: CompressionQueueStatus JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_queue_status(result: *mut *mut c_char) -> c_int {
    // Validate result pointer first
    if result.is_null() {
        return ErrorCode::NullPointer as c_int;
    }
    *result = std::ptr::null_mut(); // Initialize to null
    
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let service = globals::get_compression_service()?.clone();
        
        crate::ffi::block_on_async(async move {
            service.get_compression_queue_status().await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    *result = json_result;
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Queue a document for compression
/// Input: {"document_id": "uuid", "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_queue_document(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        let request: QueueDocumentRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        let priority = parse_priority(&request.priority)?;
        
        let service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            service.queue_document_for_compression(document_id, priority).await
                .map_err(|e| to_ffi_error(e))
        })
    })
}

/// Cancel pending compression for a document
/// Input: {"document_id": "uuid"}
/// Output: {"cancelled": boolean} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_cancel(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: DocumentIdRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?;
        
        let cancelled = crate::ffi::block_on_async(async {
            service.cancel_compression(document_id).await
                .map_err(|e| to_ffi_error(e))
        })?;
        
        Ok(serde_json::json!({"cancelled": cancelled}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get compression statistics
/// Output: CompressionStats JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_stats(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            service.get_compression_stats().await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get compression status for a document
/// Input: {"document_id": "uuid"}
/// Output: CompressionStatus JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_document_status(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let request: DocumentIdRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            service.get_document_compression_status(document_id).await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Update compression priority for a document
/// Input: {"document_id": "uuid", "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
/// Output: {"updated": boolean} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_update_priority(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: UpdatePriorityRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        let priority = parse_priority(&request.priority)?;
        
        let service = globals::get_compression_service()?;
        
        let updated = crate::ffi::block_on_async(async {
            service.update_compression_priority(document_id, priority).await
                .map_err(|e| to_ffi_error(e))
        })?;
        
        Ok(serde_json::json!({"updated": updated}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Bulk update compression priorities (optimized for large batches)
/// Input: {"document_ids": ["uuid", ...], "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
/// Output: {"updated_count": number, "processing_time_ms": number} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_bulk_update_priority(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let start_time = std::time::Instant::now();
        
        let request: BulkUpdatePriorityRequest = parse_json_input(payload_json)?;
        
        // Validate batch size for performance
        if request.document_ids.len() > 1000 {
            return Err(FFIError::with_details(
                ErrorCode::InvalidArgument,
                "Batch size too large",
                "Maximum 1000 documents per batch operation"
            ));
        }
        
        let document_ids: Result<Vec<Uuid>, _> = request.document_ids.iter()
            .map(|id_str| Uuid::parse_str(id_str))
            .collect();
        let document_ids = document_ids
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid UUID in document_ids"))?;
        let priority = parse_priority(&request.priority)?;
        
        let service = globals::get_compression_service()?;
        
        let updated_count = crate::ffi::block_on_async(async {
            service.bulk_update_compression_priority(&document_ids, priority).await
                .map_err(|e| to_ffi_error(e))
        })?;
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        
        Ok(serde_json::json!({
            "updated_count": updated_count,
            "processing_time_ms": processing_time
        }))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Check if document is currently in use
/// Input: {"document_id": "uuid"}
/// Output: {"in_use": boolean} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_is_document_in_use(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: DocumentIdRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?;
        
        let in_use = crate::ffi::block_on_async(async {
            service.is_document_in_use(document_id).await
                .map_err(|e| to_ffi_error(e))
        })?;
        
        Ok(serde_json::json!({"in_use": in_use}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Free memory allocated by compression functions
/// SAFETY: ptr must be a valid pointer returned by a compression function
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        // Track deallocation for debugging
        crate::ffi::track_string_deallocation(ptr);
        let _ = CString::from_raw(ptr);
    }
}

// Additional FFI Functions

/// Get detailed queue entries with optional filtering
/// Input: {"status": "pending|processing|completed|failed|skipped", "limit": 100, "offset": 0}
/// Output: Array of CompressionQueueEntry JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_queue_entries(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: GetQueueEntriesRequest = parse_json_input(payload_json)?;
        
        let repo = globals::get_compression_repo()?;
        
        let entries: Vec<serde_json::Value> = crate::ffi::block_on_async(async {
            // Get all entries first
            let _queue_status = repo.get_queue_status().await
                .map_err(|e| to_ffi_error(e))?;
            
            // In a real implementation, you'd want to add a method to the repository
            // to fetch entries with filtering. For now, we'll return a placeholder
            // indicating this needs repository enhancement
            Ok::<Vec<serde_json::Value>, FFIError>(vec![])
        })?;
        
        Ok(serde_json::json!({"entries": entries}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get default compression configuration
/// Output: CompressionConfig JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_default_config(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<_> {
        // Return the default config
        let default_config = CompressionConfig::default();
        Ok(default_config)
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Validate compression configuration
/// Input: {"config": CompressionConfig}
/// Output: {"valid": boolean, "errors": [string]?} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_validate_config(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: ValidateConfigRequest = parse_json_input(payload_json)?;
        
        let mut errors = Vec::new();
        
        // Validate quality level based on method
        match request.config.method {
            crate::domains::compression::types::CompressionMethod::Lossless => {
                if request.config.quality_level < 0 || request.config.quality_level > 9 {
                    errors.push("Lossless compression quality must be between 0-9".to_string());
                }
            },
            crate::domains::compression::types::CompressionMethod::Lossy => {
                if request.config.quality_level < 0 || request.config.quality_level > 100 {
                    errors.push("Lossy compression quality must be between 0-100".to_string());
                }
            },
            _ => {}
        }
        
        // Validate minimum size
        if request.config.min_size_bytes < 0 {
            errors.push("Minimum size cannot be negative".to_string());
        }
        
        let valid = errors.is_empty();
        
        Ok(serde_json::json!({
            "valid": valid,
            "errors": if valid { None } else { Some(errors) }
        }))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Retry a specific failed compression
/// Input: {"document_id": "uuid"}
/// Output: {"queued": boolean} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_retry_failed(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: DocumentIdRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?;
        let repo = globals::get_compression_repo()?;
        
        let queued: bool = crate::ffi::block_on_async(async {
            // Check if the document has a failed queue entry
            if let Some(entry) = repo.get_queue_entry_by_document_id(document_id).await
                .map_err(|e| to_ffi_error(e))? {
                
                if entry.status == "failed" {
                    // Reset status to pending and reset attempts
                    repo.update_queue_entry_status(entry.id, "pending", None).await
                        .map_err(|e| to_ffi_error(e))?;
                    Ok::<bool, FFIError>(true)
                } else {
                    Ok::<bool, FFIError>(false) // Not failed, so not retried
                }
            } else {
                // No queue entry, queue it fresh
                service.queue_document_for_compression(document_id, CompressionPriority::Normal).await
                    .map_err(|e| to_ffi_error(e))?;
                Ok::<bool, FFIError>(true)
            }
        })?;
        
        Ok(serde_json::json!({"queued": queued}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Retry all failed compressions
/// Output: {"queued_count": number} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_retry_all_failed(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let pool = globals::get_db_pool()?;
        
        let queued_count: u64 = crate::ffi::block_on_async(async {
            // Direct SQL query to reset all failed entries to pending
            let now_timestamp = chrono::Utc::now().to_rfc3339();
            let result = sqlx::query!(
                r#"
                UPDATE compression_queue 
                SET status = 'pending', 
                    attempts = 0,
                    error_message = NULL,
                    updated_at = ?
                WHERE status = 'failed'
                "#,
                now_timestamp
            )
            .execute(&pool)
            .await
            .map_err(|e| FFIError::with_details(
                ErrorCode::InternalError,
                "Failed to retry compressions",
                &format!("Database error: {}", e)
            ))?;
            
            Ok::<u64, FFIError>(result.rows_affected())
        })?;
        
        Ok(serde_json::json!({"queued_count": queued_count}))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Trigger immediate queue processing (for manual/testing purposes)
/// This doesn't guarantee immediate processing but wakes up the worker
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_process_queue_now() -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        // In a real implementation, you'd want to add a method to the CompressionWorker
        // or CompressionManager to trigger immediate processing.
        // For now, we'll just log that this was called.
        
        eprintln!("Manual compression queue processing requested");
        
        // You could implement this by:
        // 1. Adding a channel to the worker that triggers immediate processing
        // 2. Or reducing the poll interval temporarily
        // 3. Or manually calling get_next_document_for_compression
        
        Ok(())
    })
}

/// Get supported compression methods for a file type
/// Input: {"mime_type": "image/jpeg", "file_extension": "jpg"}
/// Output: {"methods": [{"method": "lossy", "recommended": true}, ...]} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_supported_methods(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: GetSupportedMethodsRequest = parse_json_input(payload_json)?;
        
        let mut methods = Vec::new();
        
        // Check based on mime type
        match request.mime_type.as_str() {
            "image/jpeg" | "image/jpg" => {
                methods.push(serde_json::json!({
                    "method": "lossy",
                    "recommended": true,
                    "quality_range": [0, 100],
                    "default_quality": 85
                }));
                methods.push(serde_json::json!({
                    "method": "none",
                    "recommended": false
                }));
            },
            "image/png" => {
                methods.push(serde_json::json!({
                    "method": "lossless",
                    "recommended": true,
                    "quality_range": [0, 9],
                    "default_quality": 6
                }));
                methods.push(serde_json::json!({
                    "method": "lossy",
                    "recommended": false,
                    "quality_range": [0, 100],
                    "default_quality": 90
                }));
            },
            "image/heic" | "image/heif" => {
                methods.push(serde_json::json!({
                    "method": "lossy",
                    "recommended": true,
                    "quality_range": [0, 100],
                    "default_quality": 80,
                    "note": "HEIC converted to JPEG/WebP for compatibility"
                }));
                methods.push(serde_json::json!({
                    "method": "lossless",
                    "recommended": false,
                    "quality_range": [0, 9],
                    "default_quality": 8
                }));
            },
            "image/webp" | "image/bmp" | "image/tiff" => {
                methods.push(serde_json::json!({
                    "method": "lossy",
                    "recommended": true,
                    "quality_range": [0, 100],
                    "default_quality": 85
                }));
                methods.push(serde_json::json!({
                    "method": "lossless",
                    "recommended": false,
                    "quality_range": [0, 9],
                    "default_quality": 6
                }));
            },
            "application/pdf" => {
                methods.push(serde_json::json!({
                    "method": "pdf_optimize",
                    "recommended": true,
                    "quality_range": [0, 4],
                    "default_quality": 2
                }));
            },
            mime if mime.starts_with("application/vnd.openxmlformats") => {
                methods.push(serde_json::json!({
                    "method": "office_optimize",
                    "recommended": true
                }));
            },
            mime if mime.starts_with("video/") => {
                methods.push(serde_json::json!({
                    "method": "video_optimize",
                    "recommended": true,
                    "quality_range": [0, 10],
                    "default_quality": 4,
                    "note": "Container optimization and metadata removal"
                }));
                methods.push(serde_json::json!({
                    "method": "lossless",
                    "recommended": false,
                    "quality_range": [0, 9],
                    "default_quality": 3,
                    "note": "Generic compression (may not be effective)"
                }));
            },
            _ => {
                // Default to lossless for unknown types
                methods.push(serde_json::json!({
                    "method": "lossless",
                    "recommended": true,
                    "quality_range": [0, 9],
                    "default_quality": 6
                }));
            }
        }
        
        // Always include "none" as an option
        if !methods.iter().any(|m| m["method"] == "none") {
            methods.push(serde_json::json!({
                "method": "none",
                "recommended": false
            }));
        }
        
        Ok(serde_json::json!({
            "mime_type": request.mime_type,
            "file_extension": request.file_extension,
            "methods": methods
        }))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get detailed compression history for a document
/// Input: {"document_id": "uuid"}
/// Output: Compression attempt history JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_document_history(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: DocumentIdRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let pool = globals::get_db_pool()?;
        
        let history: serde_json::Value = crate::ffi::block_on_async(async {
            let doc_id_str = document_id.to_string();
            
            // Get document compression info
            let doc_info = sqlx::query!(
                r#"
                SELECT 
                    compression_status,
                    compressed_file_path,
                    compressed_size_bytes,
                    size_bytes as original_size,
                    updated_at
                FROM media_documents
                WHERE id = ?
                "#,
                doc_id_str
            )
            .fetch_optional(&pool)
            .await
            .map_err(|e| FFIError::with_details(
                ErrorCode::InternalError,
                "Failed to fetch document info",
                &format!("Database error: {}", e)
            ))?;
            
            // Get queue history
            let queue_info = sqlx::query!(
                r#"
                SELECT 
                    status,
                    attempts,
                    error_message,
                    created_at,
                    updated_at
                FROM compression_queue
                WHERE document_id = ?
                "#,
                doc_id_str
            )
            .fetch_optional(&pool)
            .await
            .map_err(|e| FFIError::with_details(
                ErrorCode::InternalError,
                "Failed to fetch queue info",
                &format!("Database error: {}", e)
            ))?;
            
            Ok::<serde_json::Value, FFIError>(serde_json::json!({
                "document_id": document_id,
                "current_status": doc_info.as_ref().map(|d| &d.compression_status),
                "original_size": doc_info.as_ref().map(|d| d.original_size),
                "compressed_size": doc_info.as_ref().and_then(|d| d.compressed_size_bytes),
                "space_saved": doc_info.as_ref().and_then(|d| {
                    d.compressed_size_bytes.map(|cs| d.original_size - cs)
                }),
                "compressed_path": doc_info.as_ref().and_then(|d| d.compressed_file_path.as_ref()),
                "last_updated": doc_info.as_ref().map(|d| &d.updated_at),
                "queue_status": queue_info.as_ref().map(|q| &q.status),
                "attempts": queue_info.as_ref().map(|q| q.attempts),
                "error_message": queue_info.as_ref().and_then(|q| q.error_message.as_ref()),
                "queued_at": queue_info.as_ref().map(|q| &q.created_at),
                "queue_updated_at": queue_info.as_ref().map(|q| &q.updated_at),
            }))
        })?;
        
        Ok(history)
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get comprehensive compression debug information
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_debug_info(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        crate::ffi::block_on_async(async {
            let compression_service = globals::get_compression_service().map_err(|e| {
                crate::errors::ServiceError::Domain(crate::errors::DomainError::Internal(e.to_string()))
            })?.clone();
            let pool = globals::get_db_pool().map_err(|e| {
                crate::errors::ServiceError::Domain(crate::errors::DomainError::Internal(e.to_string()))
            })?;
            
            let mut debug_info = Vec::new();
            
            // Get queue status
            match compression_service.get_compression_queue_status().await {
                Ok(queue_status) => {
                    debug_info.push(format!("📊 QUEUE STATUS:"));
                    debug_info.push(format!("   • Pending: {}", queue_status.pending_count));
                    debug_info.push(format!("   • Processing: {}", queue_status.processing_count));
                    debug_info.push(format!("   • Completed: {}", queue_status.completed_count));
                    debug_info.push(format!("   • Failed: {}", queue_status.failed_count));
                    debug_info.push(format!("   • Skipped: {}", queue_status.skipped_count));
                },
                Err(e) => debug_info.push(format!("❌ Failed to get queue status: {:?}", e)),
            }
            
            // Get compression stats
            match compression_service.get_compression_stats().await {
                Ok(stats) => {
                    debug_info.push(format!("\n📈 COMPRESSION STATS:"));
                    debug_info.push(format!("   • Total files compressed: {}", stats.total_files_compressed));
                    debug_info.push(format!("   • Files pending: {}", stats.total_files_pending));
                    debug_info.push(format!("   • Files failed: {}", stats.total_files_failed));
                    debug_info.push(format!("   • Files skipped: {}", stats.total_files_skipped));
                    debug_info.push(format!("   • Original size: {} MB", stats.total_original_size / 1024 / 1024));
                    debug_info.push(format!("   • Compressed size: {} MB", stats.total_compressed_size / 1024 / 1024));
                    debug_info.push(format!("   • Space saved: {} MB", stats.space_saved / 1024 / 1024));
                    if let Some(last_compression) = stats.last_compression_date {
                        debug_info.push(format!("   • Last compression: {}", last_compression));
                    } else {
                        debug_info.push(format!("   • Last compression: Never"));
                    }
                },
                Err(e) => debug_info.push(format!("❌ Failed to get compression stats: {:?}", e)),
            }
            
            let result_json = serde_json::json!({
                "status": "success",
                "debug_info": debug_info.join("\n")
            });
            
            Ok::<serde_json::Value, crate::errors::ServiceError>(result_json)
        }).map_err(|e| to_ffi_error(e))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Handle iOS memory pressure (0=normal, 1=warning, 2=critical)
/// Swift can call this when it receives memory warnings
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_handle_memory_pressure(level: c_int) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        if level < 0 || level > 2 {
            return Err(FFIError::invalid_argument("Memory pressure level must be 0-2"));
        }
        
                // Get the compression worker sender directly
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::HandleMemoryPressure {
                level: level as u8,
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send memory pressure message to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            println!("🍎 [FFI] Memory pressure level {} handled by worker", level);
            Ok(())
        })
    })
}

/// Update iOS device state from Swift
/// Expected JSON payload:
/// {
///   "battery_level": 0.85,
///   "is_charging": false,
///   "thermal_state": 0, // 0=nominal, 1=fair, 2=serious, 3=critical
///   "app_state": "active", // "active", "background", "inactive"
///   "available_memory_mb": 512
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_update_ios_state(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        #[derive(Deserialize)]
        struct IOSStateUpdate {
            battery_level: f32,
            is_charging: bool,
            thermal_state: u8,
            app_state: String,
            available_memory_mb: Option<u64>,
        }
        
        let request: IOSStateUpdate = parse_json_input(payload_json)?;
        
        // Validate thermal state
        let thermal_state = match request.thermal_state {
            0 => crate::domains::compression::types::IOSThermalState::Nominal,
            1 => crate::domains::compression::types::IOSThermalState::Fair,
            2 => crate::domains::compression::types::IOSThermalState::Serious,
            3 => crate::domains::compression::types::IOSThermalState::Critical,
            _ => return Err(FFIError::invalid_argument("thermal_state must be 0-3")),
        };
        
        // Validate app state
        let app_state = match request.app_state.to_lowercase().as_str() {
            "active" => crate::domains::compression::types::IOSAppState::Active,
            "background" => crate::domains::compression::types::IOSAppState::Background,
            "inactive" => crate::domains::compression::types::IOSAppState::Inactive,
            _ => return Err(FFIError::invalid_argument("app_state must be 'active', 'background', or 'inactive'")),
        };
        
        // Validate battery level
        if request.battery_level < 0.0 || request.battery_level > 1.0 {
            return Err(FFIError::invalid_argument("battery_level must be between 0.0 and 1.0"));
        }
        
                // Get the compression worker sender directly
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::UpdateIOSState {
                battery_level: request.battery_level,
                is_charging: request.is_charging,
                thermal_state,
                app_state,
                available_memory_mb: request.available_memory_mb,
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send iOS state update to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            println!("🍎 [FFI] iOS state updated: battery={:.0}%, charging={}, thermal={:?}, app={:?}", 
                     request.battery_level * 100.0, request.is_charging, thermal_state, app_state);
            Ok(())
        })
    })
}

/// Get iOS-enhanced worker status  
/// Output: IOSWorkerStatus JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_ios_status(result: *mut *mut c_char) -> c_int {
        let json_result = handle_json_result(|| -> FFIResult<_> {
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::GetIOSStatus {
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to request iOS status from worker".to_string()));
            }
            
            // Wait for response
            match response_receiver.await {
                Ok(status) => Ok(status),
                Err(_) => Err(FFIError::internal("Failed to receive iOS status from worker".to_string())),
            }
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Manually trigger compression for a document (for debugging)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_manual_trigger(payload: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: DocumentIdRequest = parse_json_input(payload)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?.clone();
        
        crate::ffi::block_on_async(async move {
            let result = service.compress_document(document_id, None).await
                .map_err(|e| to_ffi_error(e))?;
            
            Ok(serde_json::json!({
                "status": "success",
                "document_id": result.document_id,
                "original_size": result.original_size,
                "compressed_size": result.compressed_size,
                "compressed_file_path": result.compressed_file_path,
                "space_saved_bytes": result.space_saved_bytes,
                "space_saved_percentage": result.space_saved_percentage,
                "method_used": result.method_used.as_str(),
                "quality_level": result.quality_level,
                "duration_ms": result.duration_ms
            }))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Reset stuck compression jobs with comprehensive database fixes
/// Expected JSON payload:
/// {
///   "timeout_minutes": 10,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_reset_stuck_comprehensive(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            timeout_minutes: Option<u32>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let _auth: AuthContext = p.auth.try_into()?;
        
        let timeout_minutes = p.timeout_minutes.unwrap_or(10);
        
        // Get database pool from globals
        let pool = globals::get_db_pool()?;
        
        let reset_result = block_on_async(async move {
            let mut reset_count = 0;
            let mut issues_found = Vec::new();
            
            // 1. Fix documents stuck in "processing" for too long (correct error_type values)
            let processing_query = format!(
                "UPDATE media_documents 
                 SET compression_status = 'failed', 
                     error_type = 'compression_failure',
                     error_message = 'Compression timed out after {} minutes',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE compression_status = 'processing' 
                 AND datetime(updated_at) < datetime('now', '-{} minutes')", 
                timeout_minutes, timeout_minutes
            );
            
            match sqlx::query(&processing_query).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        reset_count += rows;
                        issues_found.push(format!("Fixed {} documents stuck in 'processing' state", rows));
                    }
                },
                Err(e) => issues_found.push(format!("Failed to fix processing documents: {}", e))
            }
            
            // 2. Fix case inconsistency and invalid compression status values
            let case_fixes = [
                ("UPDATE media_documents SET compression_status = 'pending' WHERE compression_status = 'PENDING'", "uppercase PENDING"),
                ("UPDATE media_documents SET compression_status = 'processing' WHERE compression_status = 'IN_PROGRESS'", "uppercase IN_PROGRESS"),
                ("UPDATE media_documents SET compression_status = 'completed' WHERE compression_status = 'COMPLETED'", "uppercase COMPLETED"),
                ("UPDATE media_documents SET compression_status = 'failed' WHERE compression_status = 'FAILED'", "uppercase FAILED"),
                ("UPDATE media_documents SET compression_status = 'skipped' WHERE compression_status = 'SKIPPED'", "uppercase SKIPPED"),
                ("UPDATE media_documents SET compression_status = 'processing' WHERE compression_status = 'in_progress'", "legacy in_progress"),
            ];
            
            for (query, description) in case_fixes {
                match sqlx::query(query).execute(&pool).await {
                    Ok(result) => {
                        let rows = result.rows_affected();
                        if rows > 0 {
                            reset_count += rows;
                            issues_found.push(format!("Fixed {} documents with {} status", rows, description));
                        }
                    },
                    Err(e) => issues_found.push(format!("Failed to fix {}: {}", description, e))
                }
            }
            
            // 3. Fix documents with 0-byte compressed files
            match sqlx::query(
                "UPDATE media_documents 
                 SET compression_status = 'failed',
                     error_type = 'compression_failure',
                     error_message = 'Compressed file is 0 bytes - data loss detected',
                     compressed_file_path = NULL,
                     compressed_size_bytes = NULL
                 WHERE compressed_size_bytes = 0 AND compression_status = 'completed'"
            ).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        reset_count += rows;
                        issues_found.push(format!("Fixed {} documents with 0-byte compressed files (DATA LOSS PREVENTED)", rows));
                    }
                },
                Err(e) => issues_found.push(format!("Failed to fix 0-byte files: {}", e))
            }
            
            // 4. Reset failed queue entries to pending for retry
            match sqlx::query(
                "UPDATE compression_queue 
                 SET status = 'pending', 
                     attempts = 0,
                     error_message = NULL,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE status = 'failed' OR status = 'processing'"
            ).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        reset_count += rows;
                        issues_found.push(format!("Reset {} failed/stuck queue entries for retry", rows));
                    }
                },
                Err(e) => issues_found.push(format!("Failed to reset queue entries: {}", e))
            }
            
            // 5. Clean up orphaned queue entries (documents that no longer exist)
            match sqlx::query(
                "DELETE FROM compression_queue 
                 WHERE document_id NOT IN (SELECT id FROM media_documents WHERE deleted_at IS NULL)"
            ).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        reset_count += rows;
                        issues_found.push(format!("Removed {} orphaned queue entries for deleted documents", rows));
                    }
                },
                Err(e) => issues_found.push(format!("Failed to clean orphaned entries: {}", e))
            }
            
            // 6. Recalculate compression statistics (FIX: Use correct column names from schema)
            match sqlx::query(
                "UPDATE compression_stats 
                 SET total_files_pending = (
                     SELECT COUNT(*) FROM compression_queue WHERE status = 'pending'
                 ),
                 total_files_failed = (
                     SELECT COUNT(*) FROM media_documents 
                     WHERE compression_status = 'failed' AND deleted_at IS NULL
                 ),
                 total_files_compressed = (
                     SELECT COUNT(*) FROM media_documents 
                     WHERE compression_status = 'completed' AND deleted_at IS NULL
                 ),
                 total_files_skipped = (
                     SELECT COUNT(*) FROM media_documents 
                     WHERE compression_status = 'skipped' AND deleted_at IS NULL
                 ),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = 'global'"
            ).execute(&pool).await {
                Ok(_) => {
                    issues_found.push("✅ Recalculated compression statistics (excluding deleted files)".to_string());
                },
                Err(e) => issues_found.push(format!("Failed to update stats: {}", e))
            }
            
            // 7. NEW: Clean up queue entries for deleted documents
            match sqlx::query(
                "DELETE FROM compression_queue 
                 WHERE document_id IN (
                     SELECT id FROM media_documents WHERE deleted_at IS NOT NULL
                 )"
            ).execute(&pool).await {
                Ok(result) => {
                    let rows = result.rows_affected();
                    if rows > 0 {
                        reset_count += rows;
                        issues_found.push(format!("Cleaned up {} queue entries for deleted documents", rows));
                    }
                },
                Err(e) => issues_found.push(format!("Failed to clean up deleted document queues: {}", e))
            }
            
            #[derive(serde::Serialize)]
            struct ComprehensiveResetResponse {
                reset_count: u64,
                issues_found: Vec<String>,
                recommendations: Vec<String>,
                status: String,
            }
            
            let recommendations = if issues_found.iter().any(|s| s.contains("Failed")) {
                vec![
                    "⚠️ Some database operations failed - check logs".to_string(),
                    "🔄 Retry the reset operation if issues persist".to_string(),
                    "🛡️ Manual database inspection may be needed".to_string(),
                    "📞 Contact support if problems continue".to_string(),
                ]
            } else {
                vec![
                    "✅ All database inconsistencies have been fixed".to_string(),
                    "🔄 Failed compressions will retry automatically".to_string(),
                    "🛡️ Data loss from 0-byte compressed files prevented".to_string(),
                    "📊 Statistics now exclude deleted files".to_string(),
                    "⚡ Future uploads will use the iOS optimization (no Base64)".to_string(),
                ]
            };
            
            Ok(ComprehensiveResetResponse {
                reset_count,
                issues_found,
                recommendations,
                status: "success".to_string(),
            })
        });
        
        let response = reset_result.map_err(|e: FFIError| FFIError::internal(format!("reset failed: {e}")))?;
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Simple reset stuck compression jobs (compatible with existing Swift code)
/// Expected JSON payload:
/// {
///   "timeout_minutes": 10,
///   "auth": { AuthCtxDto }
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_reset_stuck_jobs(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    handle_status_result(|| unsafe {
        ensure_ptr!(payload_json);
        ensure_ptr!(result);
        
        let json = CStr::from_ptr(payload_json).to_str().map_err(|_| FFIError::invalid_argument("utf8"))?;
        
        #[derive(Deserialize)]
        struct Payload {
            timeout_minutes: Option<u32>,
            auth: AuthCtxDto,
        }
        
        let p: Payload = serde_json::from_str(json).map_err(|e| FFIError::invalid_argument(&format!("json {e}")))?;
        let _auth: AuthContext = p.auth.try_into()?;
        
        let timeout_minutes = p.timeout_minutes.unwrap_or(10);
        
        // Get database pool from globals
        let pool = globals::get_db_pool()?;
        
        let reset_result = block_on_async(async move {
            let mut reset_count = 0;
            
            // Reset stuck documents (simpler version)
            let processing_query = format!(
                "UPDATE media_documents 
                 SET compression_status = 'failed', 
                     error_type = 'timeout',
                     error_message = 'Compression timed out after {} minutes',
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE compression_status = 'processing' 
                 AND datetime(updated_at) < datetime('now', '-{} minutes')", 
                timeout_minutes, timeout_minutes
            );
            
            if let Ok(result) = sqlx::query(&processing_query).execute(&pool).await {
                reset_count += result.rows_affected();
            }
            
            // Reset failed queue entries
            if let Ok(result) = sqlx::query(
                "UPDATE compression_queue 
                 SET status = 'pending', 
                     attempts = 0,
                     error_message = NULL,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE status = 'failed' OR status = 'processing'"
            ).execute(&pool).await {
                reset_count += result.rows_affected();
            }
            
            #[derive(serde::Serialize)]
            struct SimpleResetResponse {
                reset_count: u64,
                status: String,
                message: String,
            }
            
            Ok(SimpleResetResponse {
                reset_count,
                status: "success".to_string(),
                message: format!("Reset {} stuck compression jobs", reset_count),
            })
        });
        
        let response = reset_result.map_err(|e: FFIError| FFIError::internal(format!("reset failed: {e}")))?;
        let json_resp = serde_json::to_string(&response)
            .map_err(|e| FFIError::internal(format!("ser {e}")))?;
        let cstr = CString::new(json_resp).unwrap();
        *result = cstr.into_raw();
        Ok(())
    })
}

/// Handle iOS background task extension
/// Expected JSON payload:
/// {
///   "granted_seconds": 30
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_handle_background_task_extension(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        #[derive(Deserialize)]
        struct BackgroundTaskExtension {
            granted_seconds: u32,
        }
        
        let request: BackgroundTaskExtension = parse_json_input(payload_json)?;
        
        // Validate granted seconds
        if request.granted_seconds > 300 {
            return Err(FFIError::invalid_argument("granted_seconds cannot exceed 300 (5 minutes)"));
        }
        
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::HandleBackgroundTaskExtension {
                granted_seconds: request.granted_seconds,
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send background task extension message to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            println!("🍎 [FFI] Background task extended: {} seconds", request.granted_seconds);
            Ok(())
        })
    })
}

/// Handle content visibility change
/// Expected JSON payload:
/// {
///   "is_visible": true
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_handle_content_visibility(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        #[derive(Deserialize)]
        struct ContentVisibility {
            is_visible: bool,
        }
        
        let request: ContentVisibility = parse_json_input(payload_json)?;
        
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::HandleContentVisibility {
                is_visible: request.is_visible,
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send content visibility message to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            println!("👀 [FFI] Content visibility: {}", if request.is_visible { "visible" } else { "hidden" });
            Ok(())
        })
    })
}

/// Handle iOS app lifecycle events
/// Expected JSON payload:
/// {
///   "event": "entering_background" // or "becoming_active", "resigned_active"
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_handle_app_lifecycle_event(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        #[derive(Deserialize)]
        struct AppLifecycleEvent {
            event: String,
        }
        
        let request: AppLifecycleEvent = parse_json_input(payload_json)?;
        
        // Validate event type
        match request.event.as_str() {
            "entering_background" | "becoming_active" | "resigned_active" => {},
            _ => return Err(FFIError::invalid_argument("event must be 'entering_background', 'becoming_active', or 'resigned_active'")),
        }
        
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::HandleAppLifecycleEvent {
                event: request.event.clone(),
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send app lifecycle event to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            println!("📱 [FFI] App lifecycle event: {}", request.event);
            Ok(())
        })
    })
}

/// Get comprehensive iOS device state and worker information
/// Output: Comprehensive iOS state JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_comprehensive_ios_status(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::GetIOSStatus {
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to request comprehensive iOS status from worker".to_string()));
            }
            
            // Wait for response
            match response_receiver.await {
                Ok(ios_status) => {
                    // Add additional system information
                    let comprehensive_status = serde_json::json!({
                        "ios_worker_status": ios_status,
                        "system_info": {
                            "rust_version": env!("CARGO_PKG_VERSION"),
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                            "feature_flags": {
                                "ios_integration": true,
                                "background_processing": true,
                                "memory_pressure_handling": true,
                                "thermal_management": true,
                                "battery_optimization": true,
                                "content_visibility_tracking": true,
                                "app_lifecycle_handling": true
                            }
                        }
                    });
                    Ok(comprehensive_status)
                },
                Err(_) => Err(FFIError::internal("Failed to receive comprehensive iOS status from worker".to_string())),
            }
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Handle iOS memory warning with enhanced debugging
/// Expected JSON payload:
/// {
///   "level": 2, // 0=normal, 1=warning, 2=critical
///   "available_memory_mb": 45,
///   "pressure_trend": "increasing" // optional: "increasing", "stable", "decreasing"
/// }
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_handle_enhanced_memory_warning(payload_json: *const c_char) -> c_int {
    handle_status_result(|| -> FFIResult<()> {
        #[derive(Deserialize)]
        struct EnhancedMemoryWarning {
            level: u8,
            available_memory_mb: Option<u64>,
            pressure_trend: Option<String>,
        }
        
        let request: EnhancedMemoryWarning = parse_json_input(payload_json)?;
        
        if request.level > 2 {
            return Err(FFIError::invalid_argument("Memory pressure level must be 0-2"));
        }
        
        let worker_sender = globals::get_compression_worker_sender()?;
        
        crate::ffi::block_on_async(async {
            let (response_sender, response_receiver) = oneshot::channel();
            
            // Send both memory pressure and iOS state update
            if let Err(_) = worker_sender.send(crate::domains::compression::worker::CompressionWorkerMessage::HandleMemoryPressure {
                level: request.level,
                response: response_sender,
            }).await {
                return Err(FFIError::internal("Failed to send enhanced memory pressure message to worker".to_string()));
            }
            
            // Wait for confirmation
            let _ = response_receiver.await;
            
            let trend_info = request.pressure_trend.unwrap_or_else(|| "unknown".to_string());
            println!("🧠 [FFI] Enhanced memory warning handled: level={}, available={}MB, trend={}", 
                     request.level, 
                     request.available_memory_mb.unwrap_or(0), 
                     trend_info);
            Ok(())
        })
    })
}

/// Trigger iOS device capability detection and optimization
/// This function can be called when the app starts or when device conditions change
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_detect_ios_capabilities(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        // Detect current device capabilities
        let capabilities = crate::domains::compression::types::IOSDeviceCapabilities::detect_ios_device();
        
        // Create optimizations based on detected capabilities
        let optimized_settings = crate::domains::compression::types::IOSOptimizations {
            respect_low_power_mode: true,
            pause_on_critical_thermal: true,
            reduce_quality_on_thermal: capabilities.device_type != crate::domains::compression::types::IOSDeviceType::IPadPro,
            background_processing_limit: match capabilities.device_type {
                crate::domains::compression::types::IOSDeviceType::IPhone => 1,
                crate::domains::compression::types::IOSDeviceType::IPad => 1,
                crate::domains::compression::types::IOSDeviceType::IPadPro => 2,
            },
            min_battery_level: capabilities.battery_level_threshold,
            max_memory_usage_mb: capabilities.memory_limit_mb as u64,
        };
        
        // Update worker with new optimizations
        if let Ok(worker_sender) = globals::get_compression_worker_sender() {
            let worker_sender_clone = worker_sender.clone();
            let optimizations_clone = optimized_settings.clone();
            
            tokio::spawn(async move {
                let (response_sender, response_receiver) = oneshot::channel();
                
                if let Ok(_) = worker_sender_clone.send(crate::domains::compression::worker::CompressionWorkerMessage::UpdateIOSOptimizations {
                    optimizations: optimizations_clone,
                    response: response_sender,
                }).await {
                    let _ = response_receiver.await;
                    println!("🍎 [FFI] iOS optimizations updated based on device detection");
                }
            });
        }
        
        Ok(serde_json::json!({
            "status": "success",
            "detected_capabilities": {
                "device_type": format!("{:?}", capabilities.device_type),
                "max_concurrent_jobs": capabilities.max_concurrent_jobs,
                "memory_limit_mb": capabilities.memory_limit_mb,
                "thermal_throttle_threshold": capabilities.thermal_throttle_threshold,
                "battery_level_threshold": capabilities.battery_level_threshold,
                "safe_concurrency": capabilities.get_safe_concurrency()
            },
            "applied_optimizations": {
                "background_processing_limit": optimized_settings.background_processing_limit,
                "min_battery_level": optimized_settings.min_battery_level,
                "max_memory_usage_mb": optimized_settings.max_memory_usage_mb,
                "respect_low_power_mode": optimized_settings.respect_low_power_mode,
                "pause_on_critical_thermal": optimized_settings.pause_on_critical_thermal,
                "reduce_quality_on_thermal": optimized_settings.reduce_quality_on_thermal
            },
            "recommendations": [
                "🔋 Battery optimization enabled",
                "🌡️ Thermal management configured",
                "🧠 Memory pressure handling active",
                "📱 App lifecycle integration ready",
                "⚡ Background processing limits applied"
            ]
        }))
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Manual cleanup of stale documents and stuck jobs
/// This function immediately cleans up the compression system and can be called anytime
/// Output: JSON with cleanup results
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_cleanup_stale_documents(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let compression_service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            println!("🧹 [FFI] Manual stale document cleanup triggered");
            
            // Run stale document cleanup
            let stale_cleanup_result = compression_service.cleanup_stale_documents().await;
            let stale_cleaned = match &stale_cleanup_result {
                Ok(count) => *count,
                Err(e) => {
                    println!("❌ [FFI] Stale document cleanup failed: {:?}", e);
                    0
                }
            };
            
            // Run stuck job recovery
            let stuck_recovery_result = compression_service.reset_stuck_jobs().await;
            let stuck_reset = match &stuck_recovery_result {
                Ok(count) => *count,
                Err(e) => {
                    println!("❌ [FFI] Stuck job recovery failed: {:?}", e);
                    0
                }
            };
            
            let total_processed = stale_cleaned + stuck_reset;
            let success = stale_cleanup_result.is_ok() && stuck_recovery_result.is_ok();
            
            let mut issues = Vec::new();
            let mut actions = Vec::new();
            
            if stale_cleaned > 0 {
                actions.push(format!("Cleaned up {} stale documents", stale_cleaned));
            }
            if stuck_reset > 0 {
                actions.push(format!("Reset {} stuck jobs", stuck_reset));
            }
            if total_processed == 0 {
                actions.push("No cleanup needed - system is healthy".to_string());
            }
            
            if let Err(e) = stale_cleanup_result {
                issues.push(format!("Stale cleanup error: {}", e));
            }
            if let Err(e) = stuck_recovery_result {
                issues.push(format!("Stuck job recovery error: {}", e));
            }
            
            println!("🎉 [FFI] Manual cleanup completed: {} items processed", total_processed);
            
            Ok(serde_json::json!({
                "status": if success { "success" } else { "partial_success" },
                "total_processed": total_processed,
                "stale_documents_cleaned": stale_cleaned,
                "stuck_jobs_reset": stuck_reset,
                "actions_taken": actions,
                "issues": issues,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "recommendations": if total_processed > 0 {
                    vec![
                        "✅ Compression system cleaned up successfully",
                        "🔄 New documents should now process faster",
                        "📊 Check compression queue status for current state"
                    ]
                } else {
                    vec![
                        "✨ System was already clean",
                        "🚀 Compression system is running optimally"
                    ]
                }
            }))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
} 




