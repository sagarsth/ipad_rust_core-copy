//
//  DocumentFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

/// A handler class that provides a Swift-friendly interface to the Rust `document` FFI functions.
///
/// This class encapsulates the complexity of FFI calls, providing a safe, modern, and asynchronous API
/// for all document and document type management. It handles JSON serialization, FFI memory management,
/// and converts Rust results into Swift `Result` types.
class DocumentFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.document.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()
    
    init() {
        jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
        
        // Set up date formatting to match backend RFC3339 format (only applies to actual Date types)
        jsonEncoder.dateEncodingStrategy = .iso8601
        
        // For decoding, all our models use String fields for dates, so this shouldn't interfere
        jsonDecoder.dateDecodingStrategy = .iso8601
    }

    // MARK: - Document Type Management

    func createDocumentType(newType: NewDocumentType, auth: AuthCtxDto) async -> Result<DocumentTypeResponse, Error> {
        struct Payload: Codable { let document_type: NewDocumentType, auth: AuthCtxDto }
        let payload = Payload(document_type: newType, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_type_create)
    }

    func getDocumentType(id: String, auth: AuthCtxDto) async -> Result<DocumentTypeResponse, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_type_get)
    }

    func listDocumentTypes(pagination: PaginationDto?, auth: AuthCtxDto) async -> Result<[DocumentTypeResponse], Error> {
        struct Payload: Codable { let pagination: PaginationDto?, auth: AuthCtxDto }
        let payload = Payload(pagination: pagination, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_type_list)
    }

    func updateDocumentType(id: String, update: UpdateDocumentType, auth: AuthCtxDto) async -> Result<DocumentTypeResponse, Error> {
        struct Payload: Codable { let id: String, update: UpdateDocumentType, auth: AuthCtxDto }
        let payload = Payload(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_type_update)
    }
    
    func deleteDocumentType(id: String, auth: AuthCtxDto) async -> Result<Void, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeVoidOperation(payload: payload, ffiCall: document_type_delete)
    }

    func findDocumentTypeByName(name: String, auth: AuthCtxDto) async -> Result<DocumentTypeResponse?, Error> {
        struct Payload: Codable { let name: String, auth: AuthCtxDto }
        let payload = Payload(name: name, auth: auth)
        // This call can return "null" for not found, which decodes to nil
        return await executeNullableOperation(payload: payload, ffiCall: document_type_find_by_name)
    }

    // MARK: - Media Document Management

    // ❌ REMOVED: Legacy slow base64 upload method
    // - uploadDocument(fileData: Data, ...)
    //
    // ✅ USE INSTEAD: uploadDocumentFromPath() below (50%+ faster, no memory issues)
    
    func getDocument(id: String, include: [DocumentIncludeDto]?, auth: AuthCtxDto) async -> Result<MediaDocumentResponse, Error> {
        struct Payload: Codable { let id: String, include: [DocumentIncludeDto]?, auth: AuthCtxDto }
        let payload = Payload(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_get)
    }
    
    func listDocumentsByEntity(relatedTable: String, relatedId: String, pagination: PaginationDto?, include: [DocumentIncludeDto]?, auth: AuthCtxDto) async -> Result<PaginatedResult<MediaDocumentResponse>, Error> {
        struct Payload: Codable {
            let related_table: String
            let related_id: String
            let pagination: PaginationDto?
            let include: [DocumentIncludeDto]?
            let auth: AuthCtxDto
        }
        
        print("🔍 [listDocumentsByEntity] Creating payload with:")
        print("  - relatedTable: \(relatedTable)")
        print("  - relatedId: \(relatedId)")
        print("  - pagination: \(String(describing: pagination))")
        print("  - include: \(String(describing: include))")
        print("  - auth.userId: \(auth.userId)")
        print("  - auth.deviceId: \(auth.deviceId)")
        
        let payload = Payload(related_table: relatedTable, related_id: relatedId, pagination: pagination, include: include, auth: auth)
        
        print("🔍 [listDocumentsByEntity] About to call executeOperation")
        return await executeOperation(payload: payload, ffiCall: document_list_by_entity)
    }
    
    func downloadDocument(id: String, auth: AuthCtxDto) async -> Result<DownloadResponse, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_download)
    }
    
    func openDocument(id: String, auth: AuthCtxDto) async -> Result<OpenResponse, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_open)
    }
    
    func isDocumentAvailable(id: String, auth: AuthCtxDto) async -> Result<AvailabilityResponse, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_is_available)
    }
    
    func deleteDocument(id: String, auth: AuthCtxDto) async -> Result<Void, Error> {
        struct Payload: Codable { let id: String, auth: AuthCtxDto }
        let payload = Payload(id: id, auth: auth)
        return await executeVoidOperation(payload: payload, ffiCall: document_delete)
    }
    
    func calculateSummary(relatedTable: String, relatedId: String, auth: AuthCtxDto) async -> Result<DocumentSummary, Error> {
        struct Payload: Codable { let related_table: String, related_id: String, auth: AuthCtxDto }
        let payload = Payload(related_table: relatedTable, related_id: relatedId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_calculate_summary)
    }

    func linkTempDocuments(tempRelatedId: String, finalRelatedTable: String, finalRelatedId: String, auth: AuthCtxDto) async -> Result<LinkResponse, Error> {
        struct Payload: Codable {
            let temp_related_id: String
            let final_related_table: String
            let final_related_id: String
            let auth: AuthCtxDto
        }
        let payload = Payload(temp_related_id: tempRelatedId, final_related_table: finalRelatedTable, final_related_id: finalRelatedId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_link_temp)
    }
    
    /// Get document counts for multiple entity IDs efficiently in a single call
    func getDocumentCountsByEntities(relatedEntityIds: [String], relatedTable: String, auth: AuthCtxDto) async -> Result<[DocumentCountResponse], Error> {
        struct Payload: Codable {
            let related_entity_ids: [String]
            let related_table: String
            let auth: AuthCtxDto
        }
        let payload = Payload(related_entity_ids: relatedEntityIds, related_table: relatedTable, auth: auth)
        return await executeOperation(payload: payload, ffiCall: document_get_counts_by_entities)
    }

    // MARK: - iOS Optimized Path-Based Upload Methods (NO BASE64!)
    
    /// Upload single document from file path (iOS optimized - eliminates Base64 overhead)
    func uploadDocumentFromPath(
        filePath: String,
        originalFilename: String,
        title: String?,
        documentTypeId: String,
        relatedEntityId: String,
        relatedEntityType: String,
        linkedField: String?,
        syncPriority: String,
        compressionPriority: String?,
        tempRelatedId: String?,
        auth: AuthCtxDto
    ) async -> Result<MediaDocumentResponse, Error> {
        struct Payload: Codable {
            let file_path: String              // Path instead of base64 data!
            let original_filename: String
            let title: String?
            let document_type_id: String
            let related_entity_id: String
            let related_entity_type: String
            let linked_field: String?
            let sync_priority: String
            let compression_priority: String?
            let temp_related_id: String?
            let auth: AuthCtxDto
        }
        let payload = Payload(
            file_path: filePath,                // Just the path, no Base64!
            original_filename: originalFilename,
            title: title,
            document_type_id: documentTypeId,
            related_entity_id: relatedEntityId,
            related_entity_type: relatedEntityType,
            linked_field: linkedField,
            sync_priority: syncPriority,
            compression_priority: compressionPriority,
            temp_related_id: tempRelatedId,
            auth: auth
        )
        
        print("🚀 [DocumentFFIHandler] Uploading from path: \(filePath)")
        return await executeOperation(payload: payload, ffiCall: document_upload_from_path)
    }

    // MARK: - Private Helper Functions

    private func executeOperation<P: Encodable, R: Decodable>(payload: P, ffiCall: @escaping (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt) async -> Result<R, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonString = try self.encode(payload)
                    
                    // DEBUG: Print the exact JSON being sent to Rust
                    print("🔍 [DocumentFFIHandler] Sending JSON to Rust:")
                    print(jsonString)
                    
                    // DEBUG: Check for any Date objects in the payload
                    if jsonString.contains("2025-06-25") {
                        print("❌ [DocumentFFIHandler] FOUND PROBLEMATIC DATE IN PAYLOAD!")
                        print("Full payload: \(jsonString)")
                    }
                    
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonString.withCString { cString in
                                ffiCall(cString, resultPtr)
                            }
                        },
                        parse: { responseString in
                            // DEBUG: Print response from Rust
                            print("🔍 [DocumentFFIHandler] Received from Rust:")
                            print(responseString)
                            
                            guard let data = responseString.data(using: .utf8) else { throw FFIError.stringConversionFailed }
                            return try self.jsonDecoder.decode(R.self, from: data)
                        },
                        free: document_free
                    )

                    if let value = ffiResult.value {
                        continuation.resume(returning: .success(value))
                    } else {
                        continuation.resume(returning: .failure(FFIError.rustError(ffiResult.error ?? "Unknown FFI error")))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }
    
    private func executeNullableOperation<P: Encodable, R: Decodable>(payload: P, ffiCall: @escaping (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt) async -> Result<R?, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonString = try self.encode(payload)
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonString.withCString { cString in
                                ffiCall(cString, resultPtr)
                            }
                        },
                        parse: { responseString -> R? in
                            if responseString == "null" {
                                return nil
                            }
                            guard let data = responseString.data(using: .utf8) else { throw FFIError.stringConversionFailed }
                            return try self.jsonDecoder.decode(R.self, from: data)
                        },
                        free: document_free
                    )

                    if ffiResult.isSuccess {
                        continuation.resume(returning: .success(ffiResult.value ?? nil))
                    } else {
                        continuation.resume(returning: .failure(FFIError.rustError(ffiResult.error ?? "Unknown FFI error")))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }

    private func executeVoidOperation<P: Encodable>(payload: P, ffiCall: @escaping (UnsafePointer<CChar>) -> CInt) async -> Result<Void, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonString = try self.encode(payload)
                    let status = jsonString.withCString { cString in
                        ffiCall(cString)
                    }
                    if status == 0 {
                        continuation.resume(returning: .success(()))
                    } else {
                        let error = FFIHelper.getLastError()
                        continuation.resume(returning: .failure(FFIError.rustError(error)))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }
    
    private func encode<T: Encodable>(_ value: T) throws -> String {
        // DEBUG: Print the type being encoded
        print("🔍 [DocumentFFIHandler] Encoding type: \(type(of: value))")
        
        let data = try jsonEncoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw FFIError.stringConversionFailed
        }
        
        // DEBUG: Check for problematic dates in the encoded string
        if string.contains("2025-06-25") || string.contains("updated_at") {
            print("❌ [DocumentFFIHandler] PROBLEMATIC JSON DETECTED:")
            print("Encoding: \(type(of: value))")
            print("JSON: \(string)")
        }
        
        return string
    }
} 