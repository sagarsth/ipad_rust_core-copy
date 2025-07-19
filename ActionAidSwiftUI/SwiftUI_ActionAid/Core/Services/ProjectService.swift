import Foundation

class ProjectService {
    
    // MARK: - Singleton
    static let shared = ProjectService()
    private init() {}
    
    // MARK: - Filter Operations
    
    /// Get filtered project IDs for bulk selection
    func getFilteredProjectIds(filter: ProjectFilter, auth: AuthContextPayload) async throws -> [String] {
        return try await withCheckedThrowingContinuation { continuation in
            let request = ProjectFilterRequest(
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
                project_get_filtered_ids(requestCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                project_free(resultPtr) 
                
                do {
                    let ids = try JSONDecoder().decode([String].self, from: Data(resultString.utf8))
                    continuation.resume(returning: ids)
                } catch {
                    continuation.resume(throwing: FFIError.rustError("Failed to decode IDs: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    project_free(resultPtr)
                }
                continuation.resume(throwing: FFIError.rustError("Failed to get filtered IDs"))
            }
        }
    }
    
    // MARK: - Export Operations
    
    /// Export projects by IDs with format support
    func exportProjectsByIds(
        ids: [String],
        includeBlobs: Bool = false,
        format: ExportFormat = .default,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            let exportOptions = ProjectExportByIdsOptions(
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
            
            print("🚀 [PROJECT_EXPORT_SERVICE] Calling backend with format: \(format.displayName)")
            print("🚀 [PROJECT_EXPORT_SERVICE] Export options JSON: \(optionsString)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = optionsString.withCString { optionsCStr in
                token.withCString { tokenCStr in
                    export_projects_by_ids(optionsCStr, tokenCStr, &result)
                }
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                do {
                    let exportResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [PROJECT_EXPORT_SERVICE] Export job created: \(exportResponse.job.id)")
                    continuation.resume(returning: exportResponse)
                } catch {
                    print("❌ [PROJECT_EXPORT_SERVICE] Failed to decode export response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode export response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [PROJECT_EXPORT_SERVICE] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Export failed: \(errorString)"))
                } else {
                    print("❌ [PROJECT_EXPORT_SERVICE] Unknown export error")
                    continuation.resume(throwing: FFIError.rustError("Export failed: Unknown error"))
                }
            }
        }
    }
    
    /// Get export job status
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await withCheckedThrowingContinuation { continuation in
            print("🔄 [PROJECT_EXPORT_STATUS] Checking status for job: \(jobId)")
            
            var result: UnsafeMutablePointer<CChar>?
            
            let status = jobId.withCString { jobIdCStr in
                export_get_status(jobIdCStr, &result)
            }
            
            if status == 0, let resultPtr = result {
                let resultString = String(cString: resultPtr)
                export_free(resultPtr)
                
                print("🔄 [PROJECT_EXPORT_STATUS] Raw response: \(resultString)")
                
                do {
                    let statusResponse = try JSONDecoder().decode(ExportJobResponse.self, from: Data(resultString.utf8))
                    print("✅ [PROJECT_EXPORT_STATUS] Job status: \(statusResponse.job.status)")
                    continuation.resume(returning: statusResponse)
                } catch {
                    print("❌ [PROJECT_EXPORT_STATUS] Failed to decode status response: \(error)")
                    continuation.resume(throwing: FFIError.rustError("Failed to decode status response: \(error.localizedDescription)"))
                }
            } else {
                if let resultPtr = result {
                    let errorString = String(cString: resultPtr)
                    export_free(resultPtr)
                    print("❌ [PROJECT_EXPORT_STATUS] Backend error: \(errorString)")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: \(errorString)"))
                } else {
                    print("❌ [PROJECT_EXPORT_STATUS] Unknown status check error")
                    continuation.resume(throwing: FFIError.rustError("Status check failed: Unknown error"))
                }
            }
        }
    }
} 