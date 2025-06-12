//
//  FFICompressionDeclarations.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

// MARK: - Compression FFI Declarations

// This file contains the Swift declarations for the FFI functions
// defined in the Rust `compression` module (`src/ffi/compression.rs`).

// MARK: - Memory Management
@_silgen_name("compression_free")
func compression_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Compression Core Functions
@_silgen_name("compression_compress_document")
func compression_compress_document(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_get_queue_status")
func compression_get_queue_status(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("compression_queue_document")
func compression_queue_document(_ payload_json: UnsafePointer<CChar>) -> CInt

@_silgen_name("compression_cancel")
func compression_cancel(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_get_stats")
func compression_get_stats(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("compression_get_document_status")
func compression_get_document_status(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_update_priority")
func compression_update_priority(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_bulk_update_priority")
func compression_bulk_update_priority(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_is_document_in_use")
func compression_is_document_in_use(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Additional Utility Functions
@_silgen_name("compression_get_queue_entries")
func compression_get_queue_entries(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_get_default_config")
func compression_get_default_config(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("compression_validate_config")
func compression_validate_config(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_retry_failed")
func compression_retry_failed(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_retry_all_failed")
func compression_retry_all_failed(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("compression_process_queue_now")
func compression_process_queue_now() -> CInt

@_silgen_name("compression_get_supported_methods")
func compression_get_supported_methods(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_get_document_history")
func compression_get_document_history(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("compression_reset_stuck_comprehensive")
func compression_reset_stuck_comprehensive(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt 