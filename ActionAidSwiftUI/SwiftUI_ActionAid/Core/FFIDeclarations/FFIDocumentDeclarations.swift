//
//  FFIDocumentDeclarations.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

// MARK: - Document FFI Declarations

// This file contains the Swift declarations for the FFI functions
// defined in the Rust `document` module (`src/ffi/document.rs`).

// MARK: - Memory Management
@_silgen_name("document_free")
func document_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Document Type Functions
@_silgen_name("document_type_create")
func document_type_create(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_type_get")
func document_type_get(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_type_list")
func document_type_list(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_type_update")
func document_type_update(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_type_delete")
func document_type_delete(_ payload_json: UnsafePointer<CChar>) -> CInt

@_silgen_name("document_type_find_by_name")
func document_type_find_by_name(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Media Document Functions
@_silgen_name("document_upload")
func document_upload(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_bulk_upload")
func document_bulk_upload(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_get")
func document_get(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_list_by_entity")
func document_list_by_entity(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_download")
func document_download(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_open")
func document_open(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_is_available")
func document_is_available(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_delete")
func document_delete(_ payload_json: UnsafePointer<CChar>) -> CInt

@_silgen_name("document_calculate_summary")
func document_calculate_summary(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_link_temp")
func document_link_temp(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_find_by_date_range")
func document_find_by_date_range(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_get_counts_by_entities")
func document_get_counts_by_entities(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_bulk_update_sync_priority")
func document_bulk_update_sync_priority(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_get_versions")
func document_get_versions(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_get_access_logs")
func document_get_access_logs(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Usage Tracking
@_silgen_name("document_register_in_use")
func document_register_in_use(_ payload_json: UnsafePointer<CChar>) -> CInt

@_silgen_name("document_unregister_in_use")
func document_unregister_in_use(_ payload_json: UnsafePointer<CChar>) -> CInt

// MARK: - iOS Optimized Path-Based Upload Functions (NO BASE64!)
@_silgen_name("document_upload_from_path")
func document_upload_from_path(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("document_bulk_upload_from_paths")
func document_bulk_upload_from_paths(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt 