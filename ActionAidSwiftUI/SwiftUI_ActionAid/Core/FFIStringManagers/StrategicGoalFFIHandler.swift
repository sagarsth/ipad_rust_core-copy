//
//  StrategicGoalFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

class StrategicGoalFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.strategicgoal.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    init() {
        jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
    }

    private func encode<T: Encodable>(_ value: T) throws -> String {
        let data = try jsonEncoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw FFIError.stringConversionFailed
        }
        return string
    }

    private func executeOperation<P: Encodable, R: Decodable>(
        payload: P,
        ffiCall: @escaping (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    ) async -> Result<R, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonPayload = try self.encode(payload)
                    // DEBUG: Print payload for FFI create
                    print("[StrategicGoalFFIHandler] JSON Payload: \(jsonPayload)")
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonPayload.withCString { cJson in
                                ffiCall(cJson, resultPtr)
                            }
                        },
                        parse: { responseString in
                            // DEBUG: Print response from FFI
                            print("[StrategicGoalFFIHandler] FFI Response: \(responseString)")
                            guard let data = responseString.data(using: .utf8) else {
                                throw FFIError.stringConversionFailed
                            }
                            do {
                                return try self.jsonDecoder.decode(R.self, from: data)
                            } catch let decodingError as DecodingError {
                                var message: String
                                switch decodingError {
                                case .keyNotFound(let codingKey, let context):
                                    message = "Missing key '\(codingKey.stringValue)': \(context.debugDescription)"
                                case .typeMismatch(let type, let context):
                                    message = "Type mismatch for '\(type)': \(context.debugDescription)"
                                case .valueNotFound(let type, let context):
                                    message = "Value not found for '\(type)': \(context.debugDescription)"
                                case .dataCorrupted(let context):
                                    message = "Data corrupted: \(context.debugDescription)"
                                @unknown default:
                                    message = decodingError.localizedDescription
                                }
                                print("[StrategicGoalFFIHandler] JSON Decoding Detailed Error: \(message)")
                                throw FFIError.rustError(message)
                            } catch {
                                print("[StrategicGoalFFIHandler] JSON Decoding Unknown Error: \(error)")
                                throw error
                            }
                        },
                        free: strategic_goal_free
                    )
                    
                    if let value = ffiResult.value {
                        continuation.resume(returning: .success(value))
                    } else if let error = ffiResult.error {
                        continuation.resume(returning: .failure(FFIError.rustError(error)))
                    } else {
                        continuation.resume(returning: .failure(FFIError.unknown))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }

    // MARK: - CRUD
    
    func create(newGoal: NewStrategicGoal, auth: AuthContextPayload) async -> Result<StrategicGoalResponse, Error> {
        let payload = StrategicGoalCreateRequest(goal: newGoal, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_create)
    }
    
    func get(id: String, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<StrategicGoalResponse, Error> {
        let payload = StrategicGoalGetRequest(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_get)
    }
    
    func list(pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = StrategicGoalListRequest(pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_list)
    }
    
    func update(id: String, update: UpdateStrategicGoal, auth: AuthContextPayload) async -> Result<StrategicGoalResponse, Error> {
        let payload = StrategicGoalUpdateRequest(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_update)
    }
    
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error> {
        let payload = StrategicGoalDeleteRequest(id: id, hardDelete: hardDelete, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_delete)
    }

    func bulkDelete(ids: [String], hardDelete: Bool?, force: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error> {
        let payload = StrategicGoalBulkDeleteRequest(ids: ids, hardDelete: hardDelete, force: force, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_bulk_delete)
    }

    // MARK: - Document Integration
    
    func createWithDocuments(
        newGoal: NewStrategicGoal,
        documents: [DocumentData],
        documentTypeId: String,
        auth: AuthContextPayload
    ) async -> Result<CreateWithDocumentsResponse, Error> {
        let payload = StrategicGoalCreateWithDocumentsRequest(
            goal: newGoal,
            documents: documents,
            documentTypeId: documentTypeId,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: strategic_goal_create_with_documents)
    }

    func uploadDocument(
        goalId: String,
        fileData: Data,
        originalFilename: String,
        title: String?,
        documentTypeId: String,
        linkedField: String?,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<MediaDocumentResponse, Error> {
        let payload = UploadDocumentRequest(
            goalId: goalId,
            fileData: fileData.base64EncodedString(),
            originalFilename: originalFilename,
            title: title,
            documentTypeId: documentTypeId,
            linkedField: linkedField,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: strategic_goal_upload_document)
    }

    func bulkUploadDocuments(
        goalId: String,
        files: [(Data, String)],
        title: String?,
        documentTypeId: String,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<[MediaDocumentResponse], Error> {
        let filePayloads = files.map { BulkUploadDocumentsRequest.File(fileData: $0.base64EncodedString(), filename: $1) }
        let payload = BulkUploadDocumentsRequest(
            goalId: goalId,
            files: filePayloads,
            title: title,
            documentTypeId: documentTypeId,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: strategic_goal_bulk_upload_documents)
    }

    // MARK: - iOS Optimized Upload Methods (NO BASE64 ENCODING!)
    
    /// Upload single document from file path (iOS optimized - eliminates Base64 overhead)
    func uploadDocumentFromPath(
        goalId: String,
        filePath: String,
        originalFilename: String,
        title: String?,
        documentTypeId: String,
        linkedField: String?,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<MediaDocumentResponse, Error> {
        let payload = UploadDocumentFromPathRequest(
            goalId: goalId,
            filePath: filePath,                    // Just the path, no Base64!
            originalFilename: originalFilename,
            title: title,
            documentTypeId: documentTypeId,
            linkedField: linkedField,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority,
            auth: auth
        )
        
        print("🚀 [StrategicGoalFFIHandler] Uploading from path: \(filePath)")
        return await executeOperation(payload: payload, ffiCall: strategic_goal_upload_document_from_path)
    }
    
    /// Bulk upload documents from file paths (iOS optimized - eliminates Base64 overhead)
    func bulkUploadDocumentsFromPaths(
        goalId: String,
        filePaths: [(String, String)], // (path, filename)
        title: String?,
        documentTypeId: String,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<[MediaDocumentResponse], Error> {
        let filePathPayloads = filePaths.map { 
            BulkUploadDocumentsFromPathsRequest.FilePath(filePath: $0.0, filename: $0.1) 
        }
        let payload = BulkUploadDocumentsFromPathsRequest(
            goalId: goalId,
            filePaths: filePathPayloads,           // Array of paths, no Base64!
            title: title,
            documentTypeId: documentTypeId,
            syncPriority: syncPriority,
            compressionPriority: compressionPriority,
            auth: auth
        )
        
        print("🚀 [StrategicGoalFFIHandler] Bulk uploading from \(filePaths.count) paths")
        return await executeOperation(payload: payload, ffiCall: strategic_goal_bulk_upload_documents_from_paths)
    }

    // MARK: - Queries
    
    func findByStatus(statusId: Int, pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = FindByStatusRequest(statusId: statusId, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_find_by_status)
    }
    
    func findByTeam(teamName: String, pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = FindByTeamRequest(teamName: teamName, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_find_by_team)
    }

    func findByUserRole(userId: String, role: UserGoalRole, pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = FindByUserRoleRequest(userId: userId, role: role, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_find_by_user_role)
    }

    func findStale(daysStale: Int, pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = FindStaleRequest(daysStale: daysStale, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_find_stale)
    }

    func findByDateRange(startDate: String, endDate: String, pagination: PaginationDto?, include: [StrategicGoalInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<StrategicGoalResponse>, Error> {
        let payload = FindByDateRangeRequest(startDate: startDate, endDate: endDate, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_find_by_date_range)
    }

    // MARK: - Statistics
    
    func getStatusDistribution(auth: AuthContextPayload) async -> Result<StatusDistributionResponse, Error> {
        let payload = StatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_get_status_distribution)
    }

    func getValueStatistics(auth: AuthContextPayload) async -> Result<GoalValueSummaryResponse, Error> {
        let payload = StatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_get_value_statistics)
    }

    // MARK: - Bulk Selection Support
    
    /// Get filtered strategic goal IDs for bulk selection
    /// Supports complex AND/OR filter logic matching SwiftUI selection behavior
    func getFilteredIds(filter: StrategicGoalFilter, auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = StrategicGoalFilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: strategic_goal_get_filtered_ids)
    }
} 