//
//  ActivityFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Activity domain FFI handler for communicating with Rust backend
//

import Foundation

class ActivityFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.activity.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    init() {
        jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
        
        // Set up date formatting to match backend RFC3339 format
        let dateFormatter = ISO8601DateFormatter()
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        jsonEncoder.dateEncodingStrategy = .iso8601
        
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
                    print("🔧 [ACTIVITY_FFI] executeOperation: Encoding payload...")
                    let jsonPayload = try self.encode(payload)
                    print("🔧 [ACTIVITY_FFI] executeOperation: JSON payload: \(jsonPayload)")
                    
                    print("🔧 [ACTIVITY_FFI] executeOperation: Calling FFI...")
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonPayload.withCString { cJson in
                                ffiCall(cJson, resultPtr)
                            }
                        },
                        parse: { responseString in
                            print("🔧 [ACTIVITY_FFI] executeOperation: Received response: \(responseString)")
                            guard let data = responseString.data(using: .utf8) else {
                                throw FFIError.stringConversionFailed
                            }
                            return try self.jsonDecoder.decode(R.self, from: data)
                        },
                        free: activity_free
                    )
                    
                    print("🔧 [ACTIVITY_FFI] executeOperation: FFI call completed")
                    
                    if let value = ffiResult.value {
                        print("🔧 [ACTIVITY_FFI] executeOperation: SUCCESS - Got value: \(value)")
                        continuation.resume(returning: .success(value))
                    } else if let error = ffiResult.error {
                        print("🔧 [ACTIVITY_FFI] executeOperation: ERROR - \(error)")
                        continuation.resume(returning: .failure(FFIError.rustError(error)))
                    } else {
                        print("🔧 [ACTIVITY_FFI] executeOperation: UNKNOWN ERROR")
                        continuation.resume(returning: .failure(FFIError.unknown))
                    }
                } catch {
                    print("🔧 [ACTIVITY_FFI] executeOperation: EXCEPTION - \(error)")
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }

    // MARK: - CRUD Operations
    
    func create(newActivity: NewActivity, auth: AuthContextPayload) async -> Result<ActivityResponse, Error> {
        print("🔧 [ACTIVITY_FFI] create() called")
        print("🔧 [ACTIVITY_FFI] newActivity: \(newActivity)")
        print("🔧 [ACTIVITY_FFI] auth.user_id: \(auth.user_id)")
        
        let payload = ActivityCreateRequest(activity: newActivity, auth: auth)
        print("🔧 [ACTIVITY_FFI] ActivityCreateRequest payload created")
        
        let result: Result<ActivityResponse, Error> = await executeOperation(payload: payload, ffiCall: activity_create)
        print("🔧 [ACTIVITY_FFI] executeOperation returned: \(result)")
        
        return result
    }
    
    func createWithDocuments(
        newActivity: NewActivity,
        documents: [DocumentData],
        documentTypeId: String,
        auth: AuthContextPayload
    ) async -> Result<ActivityCreateWithDocumentsResponse, Error> {
        let payload = ActivityCreateWithDocumentsRequest(
            activity: newActivity,
            documents: documents,
            documentTypeId: documentTypeId,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: activity_create_with_documents)
    }
    
    func get(id: String, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<ActivityResponse, Error> {
        let payload = ActivityGetRequest(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get)
    }
    
    func list(pagination: PaginationDto?, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ActivityResponse>, Error> {
        let payload = ActivityListRequest(pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_list)
    }
    
    func update(id: String, update: UpdateActivity, auth: AuthContextPayload) async -> Result<ActivityResponse, Error> {
        let payload = ActivityUpdateRequest(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_update)
    }
    
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error> {
        let payload = ActivityDeleteRequest(id: id, hardDelete: hardDelete, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_delete)
    }

    // MARK: - Analytics and Statistics
    
    func getStatistics(auth: AuthContextPayload) async -> Result<ActivityStatistics, Error> {
        let payload = ActivityStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_statistics)
    }

    func getStatusBreakdown(auth: AuthContextPayload) async -> Result<[ActivityStatusBreakdown], Error> {
        let payload = ActivityStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_status_breakdown)
    }

    func getMetadataCounts(auth: AuthContextPayload) async -> Result<ActivityMetadataCounts, Error> {
        let payload = ActivityStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_metadata_counts)
    }

    // MARK: - Query Operations
    
    func findByStatus(statusId: Int64, pagination: PaginationDto?, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ActivityResponse>, Error> {
        let payload = ActivityFindByStatusRequest(statusId: statusId, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_find_by_status)
    }

    func findByDateRange(startDate: String, endDate: String, pagination: PaginationDto?, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ActivityResponse>, Error> {
        let payload = ActivityFindByDateRangeRequest(startDate: startDate, endDate: endDate, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_find_by_date_range)
    }

    func search(query: String, pagination: PaginationDto?, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ActivityResponse>, Error> {
        let payload = ActivitySearchRequest(query: query, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_search)
    }

    // MARK: - Detailed Views
    
    func getDocumentReferences(id: String, auth: AuthContextPayload) async -> Result<[ActivityDocumentReference], Error> {
        let payload = ActivityDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_document_references)
    }

    // MARK: - Filtering and Bulk Operations
    
    func getFilteredIds(filter: ActivityFilter, auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = ActivityFilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_filtered_ids)
    }
    
    func bulkUpdateStatus(activityIds: [String], statusId: Int64, auth: AuthContextPayload) async -> Result<BulkUpdateStatusResponse, Error> {
        let payload = ActivityBulkUpdateStatusRequest(activityIds: activityIds, statusId: statusId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_bulk_update_status)
    }

    // MARK: - Advanced Analytics
    
    func getWorkloadByProject(auth: AuthContextPayload) async -> Result<[ActivityWorkloadByProject], Error> {
        let payload = ActivityStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_workload_by_project)
    }
    
    func findStale(daysStale: UInt32, pagination: PaginationDto?, include: [ActivityInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ActivityResponse>, Error> {
        let payload = ActivityFindStaleRequest(daysStale: daysStale, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_find_stale)
    }
    
    func getProgressAnalysis(auth: AuthContextPayload) async -> Result<ActivityProgressAnalysis, Error> {
        let payload = ActivityStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: activity_get_progress_analysis)
    }
}

// MARK: - Request DTOs

struct ActivityCreateRequest: Codable {
    let activity: NewActivity
    let auth: AuthContextPayload
}

struct ActivityStatsRequest: Codable {
    let auth: AuthContextPayload
}

struct ActivityCreateWithDocumentsRequest: Codable {
    let activity: NewActivity
    let documents: [DocumentData]
    let documentTypeId: String
    let auth: AuthContextPayload
}

struct ActivityGetRequest: Codable {
    let id: String
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}

struct ActivityListRequest: Codable {
    let pagination: PaginationDto?
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}

struct ActivityUpdateRequest: Codable {
    let id: String
    let update: UpdateActivity
    let auth: AuthContextPayload
}

struct ActivityDeleteRequest: Codable {
    let id: String
    let hardDelete: Bool?
    let auth: AuthContextPayload
}

struct ActivityFindByStatusRequest: Codable {
    let statusId: Int64
    let pagination: PaginationDto?
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}

struct ActivityFindByDateRangeRequest: Codable {
    let startDate: String
    let endDate: String
    let pagination: PaginationDto?
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}

struct ActivitySearchRequest: Codable {
    let query: String
    let pagination: PaginationDto?
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}

struct ActivityDetailRequest: Codable {
    let id: String
    let auth: AuthContextPayload
}

struct ActivityFilterRequest: Codable {
    let filter: ActivityFilter
    let auth: AuthContextPayload
}

struct ActivityBulkUpdateStatusRequest: Codable {
    let activityIds: [String]
    let statusId: Int64
    let auth: AuthContextPayload
}

struct ActivityFindStaleRequest: Codable {
    let daysStale: UInt32
    let pagination: PaginationDto?
    let include: [ActivityInclude]?
    let auth: AuthContextPayload
}