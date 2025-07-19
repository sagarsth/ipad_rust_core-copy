//
//  FFIProjectDeclarations.swift
//  SwiftUI_ActionAid
//
//  FFI function declarations for the Project domain
//

import Foundation

// MARK: - Basic CRUD Operations

@_silgen_name("project_create")
func project_create(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_create_with_documents")
func project_create_with_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get")
func project_get(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_list")
func project_list(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_update")
func project_update(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_delete")
func project_delete(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Document Integration

@_silgen_name("project_upload_document")
func project_upload_document(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_bulk_upload_documents")
func project_bulk_upload_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Analytics and Statistics

@_silgen_name("project_get_statistics")
func project_get_statistics(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_status_breakdown")
func project_get_status_breakdown(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_metadata_counts")
func project_get_metadata_counts(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Query Operations

@_silgen_name("project_find_by_status")
func project_find_by_status(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_find_by_responsible_team")
func project_find_by_responsible_team(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_find_by_date_range")
func project_find_by_date_range(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_search")
func project_search(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Detailed Views

@_silgen_name("project_get_with_document_timeline")
func project_get_with_document_timeline(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_document_references")
func project_get_document_references(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Filtering and Bulk Operations

@_silgen_name("project_get_filtered_ids")
func project_get_filtered_ids(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Advanced Dashboard Aggregations

@_silgen_name("project_get_team_workload_distribution")
func project_get_team_workload_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_strategic_goal_distribution")
func project_get_strategic_goal_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_find_stale")
func project_find_stale(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_document_coverage_analysis")
func project_get_document_coverage_analysis(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("project_get_activity_timeline")
func project_get_activity_timeline(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Export Operations

@_silgen_name("export_projects_by_ids")
func export_projects_by_ids(_ options: UnsafePointer<CChar>, _ token: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("export_projects_all")
func export_projects_all(_ options: UnsafePointer<CChar>, _ token: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("export_projects_by_date_range")
func export_projects_by_date_range(_ options: UnsafePointer<CChar>, _ token: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Memory Management

@_silgen_name("project_free")
func project_free(_ ptr: UnsafeMutablePointer<CChar>) 