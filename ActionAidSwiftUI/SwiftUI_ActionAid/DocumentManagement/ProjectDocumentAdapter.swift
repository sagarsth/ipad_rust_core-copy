//
//  ProjectDocumentAdapter.swift
//  ActionAid SwiftUI
//
//  Adapter to make ProjectResponse work with the new generic document system
//

import Foundation

// MARK: - Project Document Upload Adapter

/// Wrapper that provides document upload functionality for Projects
struct ProjectDocumentAdapter: DocumentUploadable {
    let project: ProjectResponse
    private let ffiHandler = ProjectFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    
    // MARK: - DocumentIntegratable Implementation
    
    var entityId: String {
        return project.id
    }
    
    var entityTableName: String {
        return "projects"
    }
    
    var linkableFields: [(String, String)] {
        return [
            ("", "None"),
            ("objective", "Objective"),
            ("outcome", "Outcome"),
            ("timeline", "Timeline"),
            ("proposal_document", "Proposal Document"),
            ("budget_document", "Budget Document"),
            ("logical_framework", "Logical Framework"),
            ("final_report", "Final Report"),
            ("monitoring_plan", "Monitoring Plan")
        ]
    }
    
    var entityTypeName: String {
        return "Project"
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
        // Use the optimized generic document upload function instead of project-specific base64 upload
        // This eliminates the need to load entire files into memory and convert to base64
        
        print("🚀 [ProjectDocumentAdapter] Using optimized path-based upload: \(filePath)")
        
        // Convert AuthContextPayload to AuthCtxDto for the generic document handler
        let authCtx = AuthCtxDto(
            userId: auth.user_id,
            role: auth.role,
            deviceId: auth.device_id,
            offlineMode: auth.offline_mode
        )
        
        // Use the generic optimized document upload function
        return await documentHandler.uploadDocumentFromPath(
            filePath: filePath,
            originalFilename: originalFilename,
            title: title,
            documentTypeId: documentTypeId,
            relatedEntityId: project.id,
            relatedEntityType: "projects",
            linkedField: linkedField,
            syncPriority: syncPriority.rawValue,
            compressionPriority: compressionPriority?.rawValue,
            tempRelatedId: nil,
            auth: authCtx
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
        // This avoids loading all files into memory simultaneously and eliminates base64 encoding overhead
        
        print("🚀 [ProjectDocumentAdapter] Using optimized bulk upload for \(files.count) files")
        
        // Convert AuthContextPayload to AuthCtxDto for the generic document handler
        let authCtx = AuthCtxDto(
            userId: auth.user_id,
            role: auth.role,
            deviceId: auth.device_id,
            offlineMode: auth.offline_mode
        )
        
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
                
                // Use optimized path-based upload
                let result = await documentHandler.uploadDocumentFromPath(
                    filePath: tempPath.path,
                    originalFilename: filename,
                    title: title,
                    documentTypeId: documentTypeId,
                    relatedEntityId: project.id,
                    relatedEntityType: "projects",
                    linkedField: nil, // Bulk uploads don't support field linking
                    syncPriority: syncPriority.rawValue,
                    compressionPriority: compressionPriority?.rawValue,
                    tempRelatedId: nil,
                    auth: authCtx
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
            return .failure(NSError(domain: "ProjectDocumentAdapter", code: 1, userInfo: [NSLocalizedDescriptionKey: "No files processed"]))
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



// MARK: - Helper Extension

extension ProjectResponse {
    /// Convert to DocumentUploadable adapter
    func asDocumentUploadAdapter() -> ProjectDocumentAdapter {
        return ProjectDocumentAdapter(project: self)
    }
} 