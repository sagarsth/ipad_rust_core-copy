//
//  FFIParticipantDeclarations.swift
//  SwiftUI_ActionAid
//
//  FFI function declarations for the Participant domain
//

import Foundation

// MARK: - Basic CRUD Operations

@_silgen_name("participant_create")
func participant_create(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_create_with_documents")
func participant_create_with_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get")
func participant_get(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_list")
func participant_list(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_update")
func participant_update(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_delete")
func participant_delete(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Enterprise Advanced Filtering Operations

@_silgen_name("participant_find_ids_by_filter")
func participant_find_ids_by_filter(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_find_by_filter")
func participant_find_by_filter(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_bulk_update_sync_priority_by_filter")
func participant_bulk_update_sync_priority_by_filter(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_search_with_relationships")
func participant_search_with_relationships(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Enterprise Enrichment & Analytics

@_silgen_name("participant_get_with_enrichment")
func participant_get_with_enrichment(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_comprehensive_statistics")
func participant_get_comprehensive_statistics(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_document_references")
func participant_get_document_references(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Enterprise Bulk Operations & Performance

@_silgen_name("participant_bulk_update_streaming")
func participant_bulk_update_streaming(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_index_optimization_suggestions")
func participant_get_index_optimization_suggestions(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_find_ids_by_filter_optimized")
func participant_find_ids_by_filter_optimized(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Document Integration

@_silgen_name("participant_upload_document")
func participant_upload_document(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_bulk_upload_documents")
func participant_bulk_upload_documents(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Analytics and Statistics

@_silgen_name("participant_get_demographics")
func participant_get_demographics(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_gender_distribution")
func participant_get_gender_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_age_group_distribution")
func participant_get_age_group_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_location_distribution")
func participant_get_location_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_disability_distribution")
func participant_get_disability_distribution(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Filtered Queries

@_silgen_name("participant_find_by_gender")
func participant_find_by_gender(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_find_by_age_group")
func participant_find_by_age_group(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_find_by_location")
func participant_find_by_location(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_find_by_disability")
func participant_find_by_disability(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_workshop_participants")
func participant_get_workshop_participants(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Detailed Views

@_silgen_name("participant_get_with_workshops")
func participant_get_with_workshops(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_with_livelihoods")
func participant_get_with_livelihoods(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

@_silgen_name("participant_get_with_document_timeline")
func participant_get_with_document_timeline(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Duplicate Detection

@_silgen_name("participant_check_duplicates")
func participant_check_duplicates(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt

// MARK: - Memory Management

@_silgen_name("participant_free")
func participant_free(_ ptr: UnsafeMutablePointer<CChar>)