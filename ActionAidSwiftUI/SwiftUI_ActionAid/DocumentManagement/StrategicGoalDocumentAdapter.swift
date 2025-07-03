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
        return goal.entityId
    }
    
    var entityTableName: String {
        return goal.entityTableName
    }
    
    var linkableFields: [(String, String)] {
        return goal.linkableFields
    }
    
    var entityTypeName: String {
        return goal.entityTypeName
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
        return await ffiHandler.bulkUploadDocuments(
            goalId: goal.id,
            files: files,
            title: title,
            documentTypeId: documentTypeId,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority ?? .normal,
            auth: auth
        )
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