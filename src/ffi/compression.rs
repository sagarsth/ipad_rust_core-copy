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

use crate::ffi::{handle_status_result, handle_json_result, to_ffi_error, FFIResult};
use crate::ffi::error::{FFIError, ErrorCode};
use crate::globals;
use crate::domains::compression::types::{CompressionConfig, CompressionPriority};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

// Helper to parse JSON input
fn parse_json_input<T: for<'de> Deserialize<'de>>(input: *const c_char) -> FFIResult<T> {
    if input.is_null() {
        return Err(FFIError::new(ErrorCode::InvalidArgument, "Input JSON is null"));
    }
    
    let c_str = unsafe { CStr::from_ptr(input) };
    let json_str = c_str.to_str()
        .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid UTF-8 in input JSON"))?;
    
    serde_json::from_str(json_str)
        .map_err(|e| FFIError::with_details(
            ErrorCode::InvalidArgument,
            "JSON parsing failed",
            &format!("Failed to parse JSON: {}", e)
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
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let request: CompressDocumentRequest = parse_json_input(payload_json)?;
        let document_id = Uuid::parse_str(&request.document_id)
            .map_err(|_| FFIError::new(ErrorCode::InvalidArgument, "Invalid document_id UUID"))?;
        
        let service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            service.compress_document(document_id, request.config).await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
    if json_result.is_null() { ErrorCode::InternalError as c_int } else { ErrorCode::Success as c_int }
}

/// Get current compression queue status
/// Output: CompressionQueueStatus JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_get_queue_status(result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<_> {
        let service = globals::get_compression_service()?;
        
        crate::ffi::block_on_async(async {
            service.get_compression_queue_status().await
                .map_err(|e| to_ffi_error(e))
        })
    });
    
    if !result.is_null() {
        *result = json_result;
    }
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

/// Bulk update compression priorities
/// Input: {"document_ids": ["uuid", ...], "priority": "HIGH|NORMAL|LOW|BACKGROUND"}
/// Output: {"updated_count": number} JSON
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_bulk_update_priority(payload_json: *const c_char, result: *mut *mut c_char) -> c_int {
    let json_result = handle_json_result(|| -> FFIResult<serde_json::Value> {
        let request: BulkUpdatePriorityRequest = parse_json_input(payload_json)?;
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
        
        Ok(serde_json::json!({"updated_count": updated_count}))
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




