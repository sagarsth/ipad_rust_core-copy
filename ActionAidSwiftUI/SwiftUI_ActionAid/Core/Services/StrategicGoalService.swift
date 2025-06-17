import Foundation

class StrategicGoalService {
    
    // MARK: - Singleton
    static let shared = StrategicGoalService()
    private init() {}
    
    // MARK: - Filter Operations
    
    /// Get filtered strategic goal IDs for bulk selection
    func getFilteredGoalIds(filter: StrategicGoalFilter, auth: AuthContextPayload) async throws -> [String] {
        return try await withCheckedThrowingContinuation { continuation in
            let request = StrategicGoalFilterRequest(
                filter: filter,
                auth: auth
            )
            
            guard let requestData = try? JSONEncoder().encode(request),
                  let requestString = String(data: requestData, encoding: .utf8) else {
                continuation.resume(throwing: FFIError.stringConversionFailed)
                return
            }
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = requestString.withCString { requestCStr in
                strategic_goal_get_filtered_ids(requestCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                strategic_goal_free(resultPtr)
                
                do {
                    let ids = try JSONDecoder().decode([String].self, from: Data(resultString.utf8))
                    continuation.resume(returning: ids)
                } catch {
                    continuation.resume(throwing: FFIError.rustError("Failed to decode IDs: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    strategic_goal_free(resultPtr)
                }
                continuation.resume(throwing: FFIError.rustError("Failed to get filtered IDs"))
            }
        }
    }
    
    // MARK: - Export Operations
    
    /// Export strategic goals using complex filter (matches UI filtering logic)
    func exportStrategicGoalsByFilter(
        filter: StrategicGoalFilter,
        includeBlobs: Bool = false,
        targetPath: String? = nil,
        token: String
    ) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            let exportOptions = StrategicGoalExportOptions(
                includeBlobs: includeBlobs,
                targetPath: targetPath,
                filter: filter
            )
            
            guard let optionsData = try? JSONEncoder().encode(exportOptions),
                  let optionsString = String(data: optionsData, encoding: .utf8) else {
                continuation.resume(throwing: FFIError.stringConversionFailed)
                return
            }
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = optionsString.withCString { optionsCStr in
                token.withCString { tokenCStr in
                    export_strategic_goals_by_filter(optionsCStr, tokenCStr, &result)
                }
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    continuation.resume(returning: exportResponse)
                } catch {
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    export_free(resultPtr)
                }
                continuation.resume(throwing: FFIError.rustError("Export operation failed"))
            }
        }
    }
    
    /// Export strategic goals by specific IDs
    func exportStrategicGoalsByIds(
        ids: [String],
        includeBlobs: Bool = false,
        targetPath: String? = nil,
        token: String
    ) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            let exportOptions = StrategicGoalExportByIdsOptions(
                ids: ids,
                includeBlobs: includeBlobs,
                targetPath: targetPath
            )
            
            guard let optionsData = try? JSONEncoder().encode(exportOptions),
                  let optionsString = String(data: optionsData, encoding: .utf8) else {
                continuation.resume(throwing: FFIError.stringConversionFailed)
                return
            }
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = optionsString.withCString { optionsCStr in
                token.withCString { tokenCStr in
                    export_strategic_goals_by_ids(optionsCStr, tokenCStr, &result)
                }
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    continuation.resume(returning: exportResponse)
                } catch {
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    export_free(resultPtr)
                }
                continuation.resume(throwing: FFIError.rustError("Export operation failed"))
            }
        }
    }
    
    /// Get export job status
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            var result: UnsafeMutablePointer<CChar>?
            
            let status = jobId.withCString { jobCStr in
                export_get_status(jobCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    continuation.resume(returning: exportResponse)
                } catch {
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export status: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    export_free(resultPtr)
                }
                continuation.resume(throwing: FFIError.rustError("Failed to get export status"))
            }
        }
    }
} 