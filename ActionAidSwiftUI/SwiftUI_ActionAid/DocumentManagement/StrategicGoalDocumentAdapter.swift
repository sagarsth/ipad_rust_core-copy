//
//  StrategicGoalDocumentAdapter.swift
//  ActionAid SwiftUI
//
//  Adapter to make StrategicGoalResponse work with the new generic document system
//

import Foundation

// MARK: - Strategic Goal Document Upload Adapter

/// Wrapper that provides document upload functionality for Strategic Goals
struct StrategicGoalDocumentAdapter: DocumentUploadable {
    let goal: StrategicGoalResponse
    private let ffiHandler = StrategicGoalFFIHandler()
    
    // MARK: - DocumentIntegratable Implementation
    
    var entityId: String {
        return goal.id
    }
    
    var entityTableName: String {
        return "strategic_goals"
    }
    
    var linkableFields: [(String, String)] {
        return [
            ("", "None"),
            ("outcome", "Outcome"),
            ("kpi", "KPI"),
            ("actual_value", "Actual Value"),
            ("supporting_documentation", "Supporting Documentation"),
            ("impact_assessment", "Impact Assessment"),
            ("theory_of_change", "Theory of Change"),
            ("baseline_data", "Baseline Data")
        ]
    }
    
    var entityTypeName: String {
        return "Strategic Goal"
    }
    
    // MARK: - DocumentUploadable Implementation
    
    func uploadDocument(
        filePath: String,
        originalFilename: String,
        title: String?,
        documentTypeId: String,
        linkedField: String?,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<MediaDocumentResponse, Error> {
        return await ffiHandler.uploadDocumentFromPath(
            goalId: goal.id,
            filePath: filePath,
            originalFilename: originalFilename,
            title: title,
            documentTypeId: documentTypeId,
            linkedField: linkedField,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority ?? .normal,
            auth: auth
        )
    }
    
    func bulkUploadDocuments(
        files: [(Data, String)],
        title: String?,
        documentTypeId: String,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<[MediaDocumentResponse], Error> {
        
        // ✅ OPTIMIZED APPROACH: Use individual path-based uploads instead of base64 bulk upload
        // This matches the same pattern as ProjectDocumentAdapter
        
        print("🚀 [StrategicGoalDocumentAdapter] Using optimized bulk upload for \(files.count) files")
        
        var results: [MediaDocumentResponse] = []
        var errors: [Error] = []
        
        // Process each file individually using the optimized path-based approach
        for (fileData, filename) in files {
            // Create temporary file for path-based upload
            let tempDir = FileManager.default.temporaryDirectory
            let tempPath = tempDir.appendingPathComponent(UUID().uuidString + "_" + filename)
            
            do {
                // Write data to temp file
                try fileData.write(to: tempPath)
                
                // Use domain-specific optimized path-based upload
                let result = await ffiHandler.uploadDocumentFromPath(
                    goalId: goal.id,
                    filePath: tempPath.path,
                    originalFilename: filename,
                    title: title,
                    documentTypeId: documentTypeId,
                    linkedField: nil, // Bulk uploads don't support field linking
                    syncPriority: syncPriority,
                    compressionPriority: compressionPriority ?? .normal,
                    auth: auth
                )
                
                // Clean up temp file
                try? FileManager.default.removeItem(at: tempPath)
                
                switch result {
                case .success(let document):
                    results.append(document)
                case .failure(let error):
                    errors.append(error)
                }
                
            } catch {
                // Clean up temp file on error
                try? FileManager.default.removeItem(at: tempPath)
                errors.append(error)
            }
        }
        
        // Return success if we got at least some results, otherwise return first error
        if !results.isEmpty {
            return .success(results)
        } else if let firstError = errors.first {
            return .failure(firstError)
        } else {
            return .failure(NSError(domain: "StrategicGoalDocumentAdapter", code: 1, userInfo: [NSLocalizedDescriptionKey: "No files processed"]))
        }
    }
    
    func getDefaultDocumentTypeId(auth: AuthContextPayload) async -> String? {
        // Use the existing FFI function to get document types
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(auth) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find "Document" type as default
                for item in items {
                    if let name = item["name"] as? String,
                       let docTypeId = item["id"] as? String,
                       name.lowercased() == "document" {
                        return docTypeId
                    }
                }
                
                // If no "Document" type found, use the first one
                if let firstItem = items.first,
                   let docTypeId = firstItem["id"] as? String {
                    return docTypeId
                }
            }
        }
        
        return nil
    }
    
    func detectDocumentTypeId(for filename: String, auth: AuthContextPayload) async -> String? {
        let fileExtension = (filename as NSString).pathExtension.lowercased()
        
        // Use the existing FFI function to get document types
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(auth) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find document type that supports this extension
                for item in items {
                    if let allowedExtensions = item["allowed_extensions"] as? String,
                       let docTypeId = item["id"] as? String {
                        let extensions = allowedExtensions.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
                        if extensions.contains(fileExtension) {
                            return docTypeId
                        }
                    }
                }
            }
        }
        
        return nil // Will use default document type
    }
    
    // MARK: - Helper Methods
    
    private func encodeToJSON<T: Codable>(_ object: T) -> String? {
        guard let data = try? JSONEncoder().encode(object) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

// MARK: - Note: asDocumentUploadable() extension is defined in StrategicGoalModels.swift 