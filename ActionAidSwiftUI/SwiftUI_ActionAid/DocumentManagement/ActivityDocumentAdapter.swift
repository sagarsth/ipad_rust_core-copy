//
//  ActivityDocumentAdapter.swift
//  ActionAid SwiftUI
//
//  Adapter to make ActivityResponse work with the new generic document system
//

import Foundation

// MARK: - Activity Document Upload Adapter

/// Wrapper that provides document upload functionality for Activities
struct ActivityDocumentAdapter: DocumentUploadable {
    let activity: ActivityResponse
    private let documentHandler = DocumentFFIHandler()
    
    // MARK: - DocumentIntegratable Implementation
    
    var entityId: String {
        return activity.id
    }
    
    var entityTableName: String {
        return "activities"
    }
    
    var linkableFields: [(String, String)] {
        return [
            ("", "None"),
            ("description", "Description"),
            ("kpi", "KPI"),
            ("actual_value", "Actual Value"),
            ("photo_evidence", "Photo Evidence"),
            ("receipts", "Receipts"),
            ("signed_report", "Signed Report"),
            ("monitoring_data", "Monitoring Data"),
            ("output_verification", "Output Verification")
        ]
    }
    
    var entityTypeName: String {
        return "Activity"
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
        print("🚀 [ActivityDocumentAdapter] Using optimized path-based upload: \(filePath)")
        
        let authCtx = AuthCtxDto(
            userId: auth.user_id,
            role: auth.role,
            deviceId: auth.device_id,
            offlineMode: auth.offline_mode
        )
        
        return await documentHandler.uploadDocumentFromPath(
            filePath: filePath,
            originalFilename: originalFilename,
            title: title,
            documentTypeId: documentTypeId,
            relatedEntityId: activity.id,
            relatedEntityType: "activities",
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
        print("🚀 [ActivityDocumentAdapter] Using optimized bulk upload for \(files.count) files")
        
        let authCtx = AuthCtxDto(
            userId: auth.user_id,
            role: auth.role,
            deviceId: auth.device_id,
            offlineMode: auth.offline_mode
        )
        
        var results: [MediaDocumentResponse] = []
        var errors: [Error] = []
        
        for (fileData, filename) in files {
            let tempDir = FileManager.default.temporaryDirectory
            let tempPath = tempDir.appendingPathComponent(UUID().uuidString + "_" + filename)
            
            do {
                try fileData.write(to: tempPath)
                
                let result = await documentHandler.uploadDocumentFromPath(
                    filePath: tempPath.path,
                    originalFilename: filename,
                    title: title,
                    documentTypeId: documentTypeId,
                    relatedEntityId: activity.id,
                    relatedEntityType: "activities",
                    linkedField: nil,
                    syncPriority: syncPriority.rawValue,
                    compressionPriority: compressionPriority?.rawValue,
                    tempRelatedId: nil,
                    auth: authCtx
                )
                
                try? FileManager.default.removeItem(at: tempPath)
                
                switch result {
                case .success(let document):
                    results.append(document)
                case .failure(let error):
                    errors.append(error)
                }
                
            } catch {
                try? FileManager.default.removeItem(at: tempPath)
                errors.append(error)
            }
        }
        
        if !results.isEmpty {
            return .success(results)
        } else if let firstError = errors.first {
            return .failure(firstError)
        } else {
            return .failure(NSError(domain: "ActivityDocumentAdapter", code: 1, userInfo: [NSLocalizedDescriptionKey: "No files processed"]))
        }
    }
    
    func getDefaultDocumentTypeId(auth: AuthContextPayload) async -> String? {
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
                
                for item in items {
                    if let name = item["name"] as? String,
                       let docTypeId = item["id"] as? String,
                       name.lowercased() == "document" {
                        return docTypeId
                    }
                }
                
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
        
        return nil
    }
    
    // MARK: - Helper Methods
    
    private func encodeToJSON<T: Codable>(_ object: T) -> String? {
        guard let data = try? JSONEncoder().encode(object) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

// MARK: - Helper Extension

extension ActivityResponse {
    /// Convert to DocumentUploadable adapter
    func asDocumentUploadAdapter() -> ActivityDocumentAdapter {
        return ActivityDocumentAdapter(activity: self)
    }
}