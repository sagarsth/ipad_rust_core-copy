//
//  FFIActivityDeclarations.swift
//  SwiftUI_ActionAid
//
//  FFI function declarations for the Activity domain
//

import Foundation

// MARK: - Basic CRUD Operations

@_silgen_name("activity_create")
func activity_create(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_create_with_documents")
func activity_create_with_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_get")
func activity_get(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_list")
func activity_list(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_update")
func activity_update(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_delete")
func activity_delete(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Document Integration

@_silgen_name("activity_upload_document")
func activity_upload_document(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_bulk_upload_documents")
func activity_bulk_upload_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Analytics and Statistics

@_silgen_name("activity_get_statistics")
func activity_get_statistics(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_get_status_breakdown")
func activity_get_status_breakdown(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_get_metadata_counts")
func activity_get_metadata_counts(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Query Operations

@_silgen_name("activity_find_by_status")
func activity_find_by_status(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_find_by_date_range")
func activity_find_by_date_range(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_search")
func activity_search(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Detailed Views

@_silgen_name("activity_get_document_references")
func activity_get_document_references(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Filtering and Bulk Operations

@_silgen_name("activity_get_filtered_ids")
func activity_get_filtered_ids(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_bulk_update_status")
func activity_bulk_update_status(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Advanced Dashboard Aggregations

@_silgen_name("activity_get_workload_by_project")
func activity_get_workload_by_project(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_find_stale")
func activity_find_stale(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("activity_get_progress_analysis")
func activity_get_progress_analysis(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Memory Management

@_silgen_name("activity_free")
func activity_free(_ ptr: UnsafeMutablePointer<CChar>)