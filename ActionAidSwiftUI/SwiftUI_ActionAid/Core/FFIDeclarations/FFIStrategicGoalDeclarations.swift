//
//  FFIStrategicGoalDeclarations.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

// MARK: - Strategic Goal FFI Declarations

@_silgen_name("strategic_goal_free")
func strategic_goal_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Basic CRUD
@_silgen_name("strategic_goal_create")
func strategic_goal_create(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_get")
func strategic_goal_get(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_list")
func strategic_goal_list(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_update")
func strategic_goal_update(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_delete")
func strategic_goal_delete(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Document Integration
@_silgen_name("strategic_goal_create_with_documents")
func strategic_goal_create_with_documents(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_upload_document")
func strategic_goal_upload_document(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_bulk_upload_documents")
func strategic_goal_bulk_upload_documents(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - iOS Optimized Path-Based Upload (NO BASE64!)
@_silgen_name("strategic_goal_upload_document_from_path")
func strategic_goal_upload_document_from_path(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_bulk_upload_documents_from_paths")
func strategic_goal_bulk_upload_documents_from_paths(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Query Operations
@_silgen_name("strategic_goal_find_by_status")
func strategic_goal_find_by_status(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_find_by_team")
func strategic_goal_find_by_team(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_find_by_user_role")
func strategic_goal_find_by_user_role(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_find_stale")
func strategic_goal_find_stale(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_find_by_date_range")
func strategic_goal_find_by_date_range(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Statistics
@_silgen_name("strategic_goal_get_status_distribution")
func strategic_goal_get_status_distribution(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("strategic_goal_get_value_statistics")
func strategic_goal_get_value_statistics(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Bulk Selection Support
@_silgen_name("strategic_goal_get_filtered_ids")
func strategic_goal_get_filtered_ids(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt 