//
//  ProjectFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Project domain FFI handler for communicating with Rust backend
//

import Foundation

class ProjectFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.project.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    init() {
        jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
        
        // Set up date formatting to match backend RFC3339 format (only applies to actual Date types)
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        jsonEncoder.dateEncodingStrategy = .iso8601
        
        // For decoding, all our models use String fields for dates, so this shouldn't interfere
        jsonDecoder.dateDecodingStrategy = .iso8601
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
                    // DEBUG: Print payload for FFI calls
                    print("[ProjectFFIHandler] JSON Payload: \(jsonPayload)")
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonPayload.withCString { cJson in
                                ffiCall(cJson, resultPtr)
                            }
                        },
                        parse: { responseString in
                            // DEBUG: Print response from FFI
                            print("[ProjectFFIHandler] FFI Response: \(responseString)")
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
                                print("[ProjectFFIHandler] JSON Decoding Detailed Error: \(message)")
                                throw FFIError.rustError(message)
                            } catch {
                                print("[ProjectFFIHandler] JSON Decoding Unknown Error: \(error)")
                                throw error
                            }
                        },
                        free: project_free
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

    // MARK: - CRUD Operations
    
    func create(newProject: NewProject, auth: AuthContextPayload) async -> Result<ProjectResponse, Error> {
        let payload = ProjectCreateRequest(project: newProject, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_create)
    }
    
    func createWithDocuments(
        newProject: NewProject,
        documents: [DocumentData],
        documentTypeId: String,
        auth: AuthContextPayload
    ) async -> Result<ProjectCreateWithDocumentsResponse, Error> {
        let payload = ProjectCreateWithDocumentsRequest(
            project: newProject,
            documents: documents,
            documentTypeId: documentTypeId,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: project_create_with_documents)
    }
    
    func get(id: String, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<ProjectResponse, Error> {
        let payload = ProjectGetRequest(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get)
    }
    
    func list(pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectListRequest(pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_list)
    }
    
    func update(id: String, update: UpdateProject, auth: AuthContextPayload) async -> Result<ProjectResponse, Error> {
        let payload = ProjectUpdateRequest(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_update)
    }
    
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error> {
        let payload = ProjectDeleteRequest(id: id, hardDelete: hardDelete, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_delete)
    }
    
    func bulkDelete(ids: [String], hardDelete: Bool?, force: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error> {
        // Since we don't have a specific bulk delete FFI method, we'll delete each project individually
        var results: [String: Result<DeleteResponse, Error>] = [:]
        
        for id in ids {
            let result = await delete(id: id, hardDelete: hardDelete, auth: auth)
            results[id] = result
        }
        
        // Process results into BatchDeleteResult format
        var hardDeleted: [String] = []
        var softDeleted: [String] = []
        var failed: [String] = []
        var errors: [String: String] = [:]
        
        for (id, result) in results {
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
        
        let batchResult = BatchDeleteResult(
            hardDeleted: hardDeleted,
            softDeleted: softDeleted,
            failed: failed,
            dependencies: [:], // No dependency checking for now
            errors: errors
        )
        
        return .success(batchResult)
    }

    // MARK: - Document Integration
    
    // MARK: - Document Upload Methods
    // NOTE: Document uploads for Projects now use the optimized generic DocumentFFIHandler.uploadDocumentFromPath()
    // This eliminates base64 encoding overhead and improves performance significantly.
    // See ProjectDocumentAdapter for the proper implementation pattern.
    
    // ❌ REMOVED: Legacy slow base64 upload methods
    // - uploadDocument(fileData: Data, ...) 
    // - bulkUploadDocuments(files: [(Data, String)], ...)
    //
    // ✅ USE INSTEAD: DocumentFFIHandler.uploadDocumentFromPath() via ProjectDocumentAdapter
    // 
    // This establishes the clean pattern for ALL domains:
    // 1. Domain FFI handlers focus on core CRUD operations only
    // 2. Document uploads use the generic optimized DocumentFFIHandler
    // 3. Domain adapters bridge the gap using DocumentUploadable protocol

    // MARK: - Analytics and Statistics
    
    func getStatistics(auth: AuthContextPayload) async -> Result<ProjectStatistics, Error> {
        print("🔄 [ProjectFFIHandler] getStatistics called")
        let payload = ProjectStatsRequest(auth: auth)
        let result: Result<ProjectStatistics, Error> = await executeOperation(payload: payload, ffiCall: project_get_statistics)
        
        switch result {
        case .success(let stats):
            print("✅ [ProjectFFIHandler] getStatistics success: \(stats)")
        case .failure(let error):
            print("❌ [ProjectFFIHandler] getStatistics failed: \(error)")
        }
        
        return result
    }

    func getStatusBreakdown(auth: AuthContextPayload) async -> Result<[ProjectStatusBreakdown], Error> {
        print("🔄 [ProjectFFIHandler] getStatusBreakdown called")
        let payload = ProjectStatsRequest(auth: auth)
        let result: Result<[ProjectStatusBreakdown], Error> = await executeOperation(payload: payload, ffiCall: project_get_status_breakdown)
        
        switch result {
        case .success(let breakdown):
            print("✅ [ProjectFFIHandler] getStatusBreakdown success: \(breakdown)")
        case .failure(let error):
            print("❌ [ProjectFFIHandler] getStatusBreakdown failed: \(error)")
        }
        
        return result
    }

    func getMetadataCounts(auth: AuthContextPayload) async -> Result<ProjectMetadataCounts, Error> {
        let payload = ProjectStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_metadata_counts)
    }

    // MARK: - Query Operations
    
    func findByStatus(statusId: Int64, pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectFindByStatusRequest(statusId: statusId, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_find_by_status)
    }
    
    func findByResponsibleTeam(teamName: String, pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectFindByTeamRequest(teamName: teamName, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_find_by_responsible_team)
    }

    func findByDateRange(startDate: String, endDate: String, pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectFindByDateRangeRequest(startDate: startDate, endDate: endDate, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_find_by_date_range)
    }

    func search(query: String, pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectSearchRequest(query: query, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_search)
    }

    // MARK: - Detailed Views
    
    func getWithDocuments(id: String, auth: AuthContextPayload) async -> Result<ProjectWithDocumentTimeline, Error> {
        let payload = ProjectDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_with_document_timeline)
    }
    
    func getDocumentReferences(id: String, auth: AuthContextPayload) async -> Result<[ProjectDocumentReference], Error> {
        let payload = ProjectDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_document_references)
    }

    // MARK: - Filtering and Bulk Operations
    
    func getFilteredIds(filter: ProjectFilter, auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = ProjectFilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_filtered_ids)
    }

    // MARK: - Advanced Analytics (Existing FFI functions only)
    
    func getActivityTimeline(daysActive: UInt32, auth: AuthContextPayload) async -> Result<ProjectActivityTimeline, Error> {
        let payload = ProjectActivityTimelineRequest(daysActive: daysActive, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_activity_timeline)
    }
    
    func getTeamWorkloadDistribution(auth: AuthContextPayload) async -> Result<[TeamWorkloadDistribution], Error> {
        let payload = ProjectStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_team_workload_distribution)
    }
    
    func getStrategicGoalDistribution(auth: AuthContextPayload) async -> Result<[StrategicGoalDistribution], Error> {
        let payload = ProjectStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_strategic_goal_distribution)
    }
    
    func findStale(daysStale: UInt32, pagination: PaginationDto?, include: [ProjectInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ProjectResponse>, Error> {
        let payload = ProjectFindStaleRequest(daysStale: daysStale, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_find_stale)
    }
    
    func getDocumentCoverageAnalysis(auth: AuthContextPayload) async -> Result<DocumentCoverageAnalysis, Error> {
        let payload = ProjectStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: project_get_document_coverage_analysis)
    }
}

// MARK: - Request DTOs

struct ProjectCreateRequest: Codable {
    let project: NewProject
    let auth: AuthContextPayload
}

struct ProjectStatsRequest: Codable {
    let auth: AuthContextPayload
}

struct ProjectCreateWithDocumentsRequest: Codable {
    let project: NewProject
    let documents: [DocumentData]
    let documentTypeId: String
    let auth: AuthContextPayload
}

struct ProjectGetRequest: Codable {
    let id: String
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectListRequest: Codable {
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectUpdateRequest: Codable {
    let id: String
    let update: UpdateProject
    let auth: AuthContextPayload
}

struct ProjectDeleteRequest: Codable {
    let id: String
    let hardDelete: Bool?
    let auth: AuthContextPayload
}

// ❌ REMOVED: Legacy base64 upload request structs
// - ProjectUploadDocumentRequest
// - ProjectBulkUploadDocumentsRequest
// 
// These are no longer needed since projects now use the optimized 
// DocumentFFIHandler.uploadDocumentFromPath() approach

struct ProjectFindByStatusRequest: Codable {
    let statusId: Int64
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectFindByTeamRequest: Codable {
    let teamName: String
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectFindByDateRangeRequest: Codable {
    let startDate: String
    let endDate: String
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectSearchRequest: Codable {
    let query: String
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}

struct ProjectDetailRequest: Codable {
    let id: String
    let auth: AuthContextPayload
}

struct ProjectFilterRequest: Codable {
    let filter: ProjectFilter
    let auth: AuthContextPayload
}



struct ProjectActivityTimelineRequest: Codable {
    let daysActive: UInt32
    let auth: AuthContextPayload
}

struct ProjectFindStaleRequest: Codable {
    let daysStale: UInt32
    let pagination: PaginationDto?
    let include: [ProjectInclude]?
    let auth: AuthContextPayload
}



 