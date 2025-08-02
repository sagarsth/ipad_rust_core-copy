//
//  ParticipantService.swift
//  SwiftUI_ActionAid
//
//  Participant domain service layer
//

import Foundation

class ParticipantService {
    
    // MARK: - Singleton
    static let shared = ParticipantService()
    private init() {}
    
    // MARK: - Filter Operations
    
    /// Get filtered participant IDs for bulk selection
    func getFilteredParticipantIds(filter: ParticipantFilter, auth: AuthContextPayload) async throws -> [String] {
        let handler = ParticipantFFIHandler()
        let result = await handler.findIdsByFilter(filter: filter, auth: auth)
        
        switch result {
        case .success(let ids):
            return ids
        case .failure(let error):
            throw error
        }
    }
    
    /// Get filtered participant IDs with query optimization
    func getFilteredParticipantIdsOptimized(filter: ParticipantFilter, auth: AuthContextPayload) async throws -> [String] {
        let handler = ParticipantFFIHandler()
        let result = await handler.findIdsByFilterOptimized(filter: filter, auth: auth)
        
        switch result {
        case .success(let ids):
            return ids
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Bulk Operations
    
    /// Bulk update sync priority for participants matching filter
    func bulkUpdateSyncPriority(filter: ParticipantFilter, syncPriority: SyncPriority, auth: AuthContextPayload) async throws -> BulkUpdateResponse {
        let handler = ParticipantFFIHandler()
        let result = await handler.bulkUpdateSyncPriorityByFilter(filter: filter, syncPriority: syncPriority, auth: auth)
        
        switch result {
        case .success(let response):
            return response
        case .failure(let error):
            throw error
        }
    }
    
    /// Bulk update participants with streaming
    func bulkUpdateParticipants(updates: [(String, UpdateParticipant)], chunkSize: Int = 100, auth: AuthContextPayload) async throws -> ParticipantBulkOperationResult {
        let handler = ParticipantFFIHandler()
        let result = await handler.bulkUpdateStreaming(updates: updates, chunkSize: chunkSize, auth: auth)
        
        switch result {
        case .success(let operationResult):
            return operationResult
        case .failure(let error):
            throw error
        }
    }
    
    /// Bulk delete participants
    func bulkDeleteParticipants(ids: [String], hardDelete: Bool = false, auth: AuthContextPayload) async throws -> BatchDeleteResult {
        let handler = ParticipantFFIHandler()
        var hardDeleted: [String] = []
        var softDeleted: [String] = []
        var failed: [String] = []
        var errors: [String: String] = [:]
        
        for id in ids {
            let result = await handler.delete(id: id, hardDelete: hardDelete, auth: auth)
            
            switch result {
            case .success(let deleteResponse):
                if deleteResponse.isHardDeleted {
                    hardDeleted.append(id)
                } else if deleteResponse.wasDeleted {
                    softDeleted.append(id)
                } else {
                    failed.append(id)
                    errors[id] = deleteResponse.displayMessage
                }
            case .failure(let error):
                failed.append(id)
                errors[id] = error.localizedDescription
            }
        }
        
        return BatchDeleteResult(
            hardDeleted: hardDeleted,
            softDeleted: softDeleted,
            failed: failed,
            dependencies: [:],
            errors: errors
        )
    }
    
    // MARK: - Search Operations
    
    /// Search participants with relationships (workshops and livelihoods)
    func searchParticipantsWithRelationships(searchText: String, pagination: PaginationDto? = nil, auth: AuthContextPayload) async throws -> PaginatedResult<ParticipantResponse> {
        let handler = ParticipantFFIHandler()
        let result = await handler.searchWithRelationships(searchText: searchText, pagination: pagination, auth: auth)
        
        switch result {
        case .success(let participants):
            return participants
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Enrichment Operations
    
    /// Get participant with comprehensive enrichment data
    func getParticipantWithEnrichment(id: String, auth: AuthContextPayload) async throws -> ParticipantWithEnrichment {
        let handler = ParticipantFFIHandler()
        let result = await handler.getWithEnrichment(id: id, auth: auth)
        
        switch result {
        case .success(let enrichedParticipant):
            return enrichedParticipant
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Analytics Operations
    
    /// Get comprehensive participant statistics
    func getComprehensiveStatistics(auth: AuthContextPayload) async throws -> ParticipantStatistics {
        let handler = ParticipantFFIHandler()
        let result = await handler.getComprehensiveStatistics(auth: auth)
        
        switch result {
        case .success(let statistics):
            return statistics
        case .failure(let error):
            throw error
        }
    }
    
    /// Get participant demographics
    func getParticipantDemographics(auth: AuthContextPayload) async throws -> ParticipantDemographics {
        let handler = ParticipantFFIHandler()
        let result = await handler.getDemographics(auth: auth)
        
        switch result {
        case .success(let demographics):
            return demographics
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Distribution Analytics
    
    /// Get gender distribution
    func getGenderDistribution(auth: AuthContextPayload) async throws -> [String: Int64] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getGenderDistribution(auth: auth)
        
        switch result {
        case .success(let distribution):
            return distribution
        case .failure(let error):
            throw error
        }
    }
    
    /// Get age group distribution
    func getAgeGroupDistribution(auth: AuthContextPayload) async throws -> [String: Int64] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getAgeGroupDistribution(auth: auth)
        
        switch result {
        case .success(let distribution):
            return distribution
        case .failure(let error):
            throw error
        }
    }
    
    /// Get location distribution
    func getLocationDistribution(auth: AuthContextPayload) async throws -> [String: Int64] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getLocationDistribution(auth: auth)
        
        switch result {
        case .success(let distribution):
            return distribution
        case .failure(let error):
            throw error
        }
    }
    
    /// Get disability distribution
    func getDisabilityDistribution(auth: AuthContextPayload) async throws -> [String: Int64] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getDisabilityDistribution(auth: auth)
        
        switch result {
        case .success(let distribution):
            return distribution
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Document Reference Operations
    
    /// Get document references for a participant
    func getParticipantDocumentReferences(participantId: String, auth: AuthContextPayload) async throws -> [ParticipantDocumentReference] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getDocumentReferences(participantId: participantId, auth: auth)
        
        switch result {
        case .success(let references):
            return references
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Performance Optimization
    
    /// Get database index optimization suggestions
    func getIndexOptimizationSuggestions(auth: AuthContextPayload) async throws -> [String] {
        let handler = ParticipantFFIHandler()
        let result = await handler.getIndexOptimizationSuggestions(auth: auth)
        
        switch result {
        case .success(let suggestions):
            return suggestions
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Detailed Views
    
    /// Get participant with workshop details
    func getParticipantWithWorkshops(id: String, auth: AuthContextPayload) async throws -> ParticipantWithWorkshops {
        let handler = ParticipantFFIHandler()
        let result = await handler.getWithWorkshops(id: id, auth: auth)
        
        switch result {
        case .success(let participantWithWorkshops):
            return participantWithWorkshops
        case .failure(let error):
            throw error
        }
    }
    
    /// Get participant with livelihood details
    func getParticipantWithLivelihoods(id: String, auth: AuthContextPayload) async throws -> ParticipantWithLivelihoods {
        let handler = ParticipantFFIHandler()
        let result = await handler.getWithLivelihoods(id: id, auth: auth)
        
        switch result {
        case .success(let participantWithLivelihoods):
            return participantWithLivelihoods
        case .failure(let error):
            throw error
        }
    }
    
    /// Get participant with document timeline
    func getParticipantWithDocumentTimeline(id: String, auth: AuthContextPayload) async throws -> ParticipantWithDocumentTimeline {
        let handler = ParticipantFFIHandler()
        let result = await handler.getWithDocumentTimeline(id: id, auth: auth)
        
        switch result {
        case .success(let participantTimeline):
            return participantTimeline
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Export Operations
    
    /// Export participants by IDs with format support
    func exportParticipantsByIds(
        ids: [String],
        includeBlobs: Bool = false,
        format: ExportFormat = .default,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            let exportOptions = ParticipantExportByIdsOptions(
                ids: ids,
                includeBlobs: includeBlobs,
                targetPath: targetPath,
                format: format
            )
            
            guard let optionsData = try? JSONEncoder().encode(exportOptions),
                  let optionsString = String(data: optionsData, encoding: .utf8) else {
                continuation.resume(throwing: FFIError.stringConversionFailed)
                return
            }
            
            print("🚀 [PARTICIPANT_EXPORT_SERVICE] Calling backend with format: \(format.displayName)")
            print("🚀 [PARTICIPANT_EXPORT_SERVICE] Export options JSON: \(optionsString)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = optionsString.withCString { optionsCStr in
                token.withCString { tokenCStr in
                    export_participants_by_ids(optionsCStr, tokenCStr, &result)
                }
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [PARTICIPANT_EXPORT_SERVICE] Export job created: \(exportResponse.job.id)")
                    continuation.resume(returning: exportResponse)
                } catch {
                    print("❌ [PARTICIPANT_EXPORT_SERVICE] Failed to decode export response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [PARTICIPANT_EXPORT_SERVICE] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Export failed: \(errorString)"))
                } else {
                    print("❌ [PARTICIPANT_EXPORT_SERVICE] Unknown export error")
                    continuation.resume(throwing: FFIError.rustError("Export failed: Unknown error"))
                }
            }
        }
    }
    
    /// Get export job status
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            print("🔄 [PARTICIPANT_EXPORT_STATUS] Checking status for job: \(jobId)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = jobId.withCString { jobIdCStr in
                export_get_status(jobIdCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [PARTICIPANT_EXPORT_STATUS] Status retrieved: \(exportResponse.job.status)")
                    continuation.resume(returning: exportResponse)
                } catch {
                    print("❌ [PARTICIPANT_EXPORT_STATUS] Failed to decode status response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode status response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [PARTICIPANT_EXPORT_STATUS] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: \(errorString)"))
                } else {
                    print("❌ [PARTICIPANT_EXPORT_STATUS] Unknown status error")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: Unknown error"))
                }
            }
        }
    }
    
    // MARK: - Helper Methods
    
    /// Create a participant filter for common scenarios
    static func createFilterForDisability(hasDisability: Bool) -> ParticipantFilter {
        return ParticipantFilter(disability: hasDisability)
    }
    
    /// Create a participant filter for specific disability types
    static func createFilterForDisabilityTypes(_ types: [String]) -> ParticipantFilter {
        return ParticipantFilter(disabilityTypes: types)
    }
    
    /// Create a participant filter for demographics
    static func createDemographicFilter(
        genders: [String]? = nil,
        ageGroups: [String]? = nil,
        locations: [String]? = nil
    ) -> ParticipantFilter {
        return ParticipantFilter(
            genders: genders,
            ageGroups: ageGroups,
            locations: locations
        )
    }
    
    /// Create a participant filter for workshop participants
    static func createWorkshopFilter(workshopIds: [String]) -> ParticipantFilter {
        return ParticipantFilter(workshopIds: workshopIds)
    }
    
    /// Create a participant filter for document status
    static func createDocumentFilter(hasDocuments: Bool, fields: [String]? = nil) -> ParticipantFilter {
        return ParticipantFilter(
            hasDocuments: hasDocuments,
            documentLinkedFields: fields
        )
    }
}