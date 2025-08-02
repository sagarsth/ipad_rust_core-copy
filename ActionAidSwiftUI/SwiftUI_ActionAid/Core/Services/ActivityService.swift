//
//  ActivityService.swift
//  SwiftUI_ActionAid
//
//  Activity domain service layer
//

import Foundation

class ActivityService {
    
    // MARK: - Singleton
    static let shared = ActivityService()
    private init() {}
    
    // MARK: - Filter Operations
    
    /// Get filtered activity IDs for bulk selection
    func getFilteredActivityIds(filter: ActivityFilter, auth: AuthContextPayload) async throws -> [String] {
        let handler = ActivityFFIHandler()
        let result = await handler.getFilteredIds(filter: filter, auth: auth)
        
        switch result {
        case .success(let ids):
            return ids
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Bulk Operations
    
    /// Bulk update activity status
    func bulkUpdateStatus(activityIds: [String], statusId: Int64, auth: AuthContextPayload) async throws -> BulkUpdateStatusResponse {
        let handler = ActivityFFIHandler()
        let result = await handler.bulkUpdateStatus(activityIds: activityIds, statusId: statusId, auth: auth)
        
        switch result {
        case .success(let response):
            return response
        case .failure(let error):
            throw error
        }
    }
    
    /// Bulk delete activities
    func bulkDeleteActivities(ids: [String], hardDelete: Bool = false, auth: AuthContextPayload) async throws -> BatchDeleteResult {
        let handler = ActivityFFIHandler()
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
    
    // MARK: - Export Operations
    
    /// Export activities by IDs with format support
    func exportActivitiesByIds(
        ids: [String],
        includeBlobs: Bool = false,
        format: ExportFormat = .default,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            let exportOptions = ActivityExportByIdsOptions(
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
            
            print("🚀 [ACTIVITY_EXPORT_SERVICE] Calling backend with format: \(format.displayName)")
            print("🚀 [ACTIVITY_EXPORT_SERVICE] Export options JSON: \(optionsString)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = optionsString.withCString { optionsCStr in
                token.withCString { tokenCStr in
                    export_activities_by_ids(optionsCStr, tokenCStr, &result)
                }
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [ACTIVITY_EXPORT_SERVICE] Export job created: \(exportResponse.job.id)")
                    continuation.resume(returning: exportResponse)
                } catch {
                    print("❌ [ACTIVITY_EXPORT_SERVICE] Failed to decode export response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [ACTIVITY_EXPORT_SERVICE] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Export failed: \(errorString)"))
                } else {
                    print("❌ [ACTIVITY_EXPORT_SERVICE] Unknown export error")
                    continuation.resume(throwing: FFIError.rustError("Export failed: Unknown error"))
                }
            }
        }
    }
    
    /// Get export job status
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            print("🔄 [ACTIVITY_EXPORT_STATUS] Checking status for job: \(jobId)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = jobId.withCString { jobIdCStr in
                export_get_status(jobIdCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [ACTIVITY_EXPORT_STATUS] Status retrieved: \(exportResponse.job.status)")
                    continuation.resume(returning: exportResponse)
                } catch {
                    print("❌ [ACTIVITY_EXPORT_STATUS] Failed to decode status response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode status response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [ACTIVITY_EXPORT_STATUS] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: \(errorString)"))
                } else {
                    print("❌ [ACTIVITY_EXPORT_STATUS] Unknown status error")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: Unknown error"))
                }
            }
        }
    }
    
    // MARK: - Analytics Operations
    
    /// Get comprehensive activity statistics
    func getActivityStatistics(auth: AuthContextPayload) async throws -> ActivityStatistics {
        let handler = ActivityFFIHandler()
        let result = await handler.getStatistics(auth: auth)
        
        switch result {
        case .success(let stats):
            return stats
        case .failure(let error):
            throw error
        }
    }
    
    /// Get activity progress analysis
    func getProgressAnalysis(auth: AuthContextPayload) async throws -> ActivityProgressAnalysis {
        let handler = ActivityFFIHandler()
        let result = await handler.getProgressAnalysis(auth: auth)
        
        switch result {
        case .success(let analysis):
            return analysis
        case .failure(let error):
            throw error
        }
    }
    
    /// Get workload distribution by project
    func getWorkloadByProject(auth: AuthContextPayload) async throws -> [ActivityWorkloadByProject] {
        let handler = ActivityFFIHandler()
        let result = await handler.getWorkloadByProject(auth: auth)
        
        switch result {
        case .success(let distribution):
            return distribution
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Stale Activity Detection
    
    /// Find activities that haven't been updated in specified days
    func findStaleActivities(
        daysStale: UInt32,
        pagination: PaginationDto? = nil,
        include: [ActivityInclude]? = nil,
        auth: AuthContextPayload
    ) async throws -> PaginatedResult<ActivityResponse> {
        let handler = ActivityFFIHandler()
        let result = await handler.findStale(
            daysStale: daysStale,
            pagination: pagination,
            include: include,
            auth: auth
        )
        
        switch result {
        case .success(let activities):
            return activities
        case .failure(let error):
            throw error
        }
    }
    
    // MARK: - Document Reference Operations
    
    /// Get document references for an activity
    func getActivityDocumentReferences(activityId: String, auth: AuthContextPayload) async throws -> [ActivityDocumentReference] {
        let handler = ActivityFFIHandler()
        let result = await handler.getDocumentReferences(id: activityId, auth: auth)
        
        switch result {
        case .success(let references):
            return references
        case .failure(let error):
            throw error
        }
    }
}