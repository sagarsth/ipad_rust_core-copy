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
use tokio::runtime::Runtime;

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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        let cancelled = rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        let updated = rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        let updated_count = rt.block_on(async {
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
        let rt = Runtime::new()
            .map_err(|e| FFIError::with_details(ErrorCode::InternalError, "Failed to create async runtime", &e.to_string()))?;
        
        let in_use = rt.block_on(async {
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

// -----------------------------------------------------------------------------
// Memory management -----------------------------------------------------------
// -----------------------------------------------------------------------------

/// Free string memory allocated by Rust
/// SAFETY: Must be called exactly once for each string returned from compression FFI functions
#[unsafe(no_mangle)]
pub unsafe extern "C" fn compression_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { let _ = CString::from_raw(ptr); }
    }
} 