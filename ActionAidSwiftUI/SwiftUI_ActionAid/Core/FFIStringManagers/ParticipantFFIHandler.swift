//
//  ParticipantFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Participant domain FFI handler for communicating with Rust backend
//

import Foundation

class ParticipantFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.participant.ffi", qos: .userInitiated)
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
                    let jsonPayload = try self.encode(payload)
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonPayload.withCString { cJson in
                                ffiCall(cJson, resultPtr)
                            }
                        },
                        parse: { responseString in
                            guard let data = responseString.data(using: .utf8) else {
                                throw FFIError.stringConversionFailed
                            }
                            return try self.jsonDecoder.decode(R.self, from: data)
                        },
                        free: participant_free
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
    
    func create(newParticipant: NewParticipant, auth: AuthContextPayload) async -> Result<ParticipantResponse, Error> {
        let payload = ParticipantCreateRequest(participant: newParticipant, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_create)
    }
    
    func createWithDocuments(
        newParticipant: NewParticipant,
        documents: [DocumentData],
        documentTypeId: String,
        auth: AuthContextPayload
    ) async -> Result<ParticipantCreateWithDocumentsResponse, Error> {
        let payload = ParticipantCreateWithDocumentsRequest(
            participant: newParticipant,
            documents: documents,
            documentTypeId: documentTypeId,
            auth: auth
        )
        return await executeOperation(payload: payload, ffiCall: participant_create_with_documents)
    }
    
    func get(id: String, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<ParticipantResponse, Error> {
        let payload = ParticipantGetRequest(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get)
    }
    
    func list(pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantListRequest(pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_list)
    }
    
    func update(id: String, update: UpdateParticipant, auth: AuthContextPayload) async -> Result<ParticipantResponse, Error> {
        let payload = ParticipantUpdateRequest(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_update)
    }
    
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error> {
        let payload = ParticipantDeleteRequest(id: id, hardDelete: hardDelete, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_delete)
    }
    
    func checkDuplicates(name: String, auth: AuthContextPayload) async -> Result<[ParticipantDuplicateInfo], Error> {
        let payload = ParticipantCheckDuplicatesRequest(name: name, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_check_duplicates)
    }
    
    func bulkDelete(ids: [String], hardDelete: Bool?, force: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error> {
        // Since we don't have a specific bulk delete FFI method, we'll delete each participant individually
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

    // MARK: - Advanced Filtering Operations
    
    func findIdsByFilter(filter: ParticipantFilter, auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = ParticipantFilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_ids_by_filter)
    }
    
    func findByFilter(filter: ParticipantFilter, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantFindByFilterRequest(filter: filter, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_by_filter)
    }
    
    func bulkUpdateSyncPriorityByFilter(filter: ParticipantFilter, syncPriority: SyncPriority, auth: AuthContextPayload) async -> Result<BulkUpdateResponse, Error> {
        let payload = ParticipantBulkUpdateSyncPriorityRequest(filter: filter, syncPriority: syncPriority.rawValue, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_bulk_update_sync_priority_by_filter)
    }
    
    func searchWithRelationships(searchText: String, pagination: PaginationDto?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantSearchRequest(searchText: searchText, pagination: pagination, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_search_with_relationships)
    }

    // MARK: - Enrichment & Analytics
    
    func getWithEnrichment(id: String, auth: AuthContextPayload) async -> Result<ParticipantWithEnrichment, Error> {
        let payload = ParticipantDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_with_enrichment)
    }
    
    func getComprehensiveStatistics(auth: AuthContextPayload) async -> Result<ParticipantStatistics, Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_comprehensive_statistics)
    }
    
    func getDocumentReferences(participantId: String, auth: AuthContextPayload) async -> Result<[ParticipantDocumentReference], Error> {
        let payload = ParticipantDocumentReferencesRequest(participantId: participantId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_document_references)
    }

    // MARK: - Bulk Operations
    
    func bulkUpdateStreaming(updates: [(String, UpdateParticipant)], chunkSize: Int?, auth: AuthContextPayload) async -> Result<ParticipantBulkOperationResult, Error> {
        let updateRequests = updates.map { ParticipantBulkUpdateRequest(participantId: $0.0, update: $0.1) }
        let payload = ParticipantBulkUpdateStreamingRequest(updates: updateRequests, chunkSize: chunkSize, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_bulk_update_streaming)
    }
    
    func getIndexOptimizationSuggestions(auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_index_optimization_suggestions)
    }
    
    func findIdsByFilterOptimized(filter: ParticipantFilter, auth: AuthContextPayload) async -> Result<[String], Error> {
        let payload = ParticipantFilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_ids_by_filter_optimized)
    }

    // MARK: - Demographics and Statistics
    
    func getDemographics(auth: AuthContextPayload) async -> Result<ParticipantDemographics, Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_demographics)
    }
    
    func getGenderDistribution(auth: AuthContextPayload) async -> Result<[String: Int64], Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_gender_distribution)
    }
    
    func getAgeGroupDistribution(auth: AuthContextPayload) async -> Result<[String: Int64], Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_age_group_distribution)
    }
    
    func getLocationDistribution(auth: AuthContextPayload) async -> Result<[String: Int64], Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_location_distribution)
    }
    
    func getDisabilityDistribution(auth: AuthContextPayload) async -> Result<[String: Int64], Error> {
        let payload = ParticipantStatsRequest(auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_disability_distribution)
    }

    // MARK: - Filtered Queries
    
    func findByGender(gender: String, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantFindByGenderRequest(gender: gender, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_by_gender)
    }
    
    func findByAgeGroup(ageGroup: String, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantFindByAgeGroupRequest(ageGroup: ageGroup, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_by_age_group)
    }
    
    func findByLocation(location: String, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantFindByLocationRequest(location: location, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_by_location)
    }
    
    func findByDisability(hasDisability: Bool, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantFindByDisabilityRequest(hasDisability: hasDisability, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_find_by_disability)
    }
    
    func getWorkshopParticipants(workshopId: String, pagination: PaginationDto?, include: [ParticipantInclude]?, auth: AuthContextPayload) async -> Result<PaginatedResult<ParticipantResponse>, Error> {
        let payload = ParticipantGetWorkshopParticipantsRequest(workshopId: workshopId, pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_workshop_participants)
    }

    // MARK: - Detailed Views
    
    func getWithWorkshops(id: String, auth: AuthContextPayload) async -> Result<ParticipantWithWorkshops, Error> {
        let payload = ParticipantDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_with_workshops)
    }
    
    func getWithLivelihoods(id: String, auth: AuthContextPayload) async -> Result<ParticipantWithLivelihoods, Error> {
        let payload = ParticipantDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_with_livelihoods)
    }
    
    func getWithDocumentTimeline(id: String, auth: AuthContextPayload) async -> Result<ParticipantWithDocumentTimeline, Error> {
        let payload = ParticipantDetailRequest(id: id, auth: auth)
        return await executeOperation(payload: payload, ffiCall: participant_get_with_document_timeline)
    }
}

// MARK: - Request DTOs

struct ParticipantCreateRequest: Codable {
    let participant: NewParticipant
    let auth: AuthContextPayload
}

struct ParticipantStatsRequest: Codable {
    let auth: AuthContextPayload
}

struct ParticipantCreateWithDocumentsRequest: Codable {
    let participant: NewParticipant
    let documents: [DocumentData]
    let documentTypeId: String
    let auth: AuthContextPayload
}

struct ParticipantGetRequest: Codable {
    let id: String
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantListRequest: Codable {
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantUpdateRequest: Codable {
    let id: String
    let update: UpdateParticipant
    let auth: AuthContextPayload
}

struct ParticipantDeleteRequest: Codable {
    let id: String
    let hardDelete: Bool?
    let auth: AuthContextPayload
}

struct ParticipantFilterRequest: Codable {
    let filter: ParticipantFilter
    let auth: AuthContextPayload
}

struct ParticipantFindByFilterRequest: Codable {
    let filter: ParticipantFilter
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantBulkUpdateSyncPriorityRequest: Codable {
    let filter: ParticipantFilter
    let syncPriority: String
    let auth: AuthContextPayload
}

struct ParticipantSearchRequest: Codable {
    let searchText: String
    let pagination: PaginationDto?
    let auth: AuthContextPayload
}

struct ParticipantDetailRequest: Codable {
    let id: String
    let auth: AuthContextPayload
}

struct ParticipantDocumentReferencesRequest: Codable {
    let participantId: String
    let auth: AuthContextPayload
}

struct ParticipantBulkUpdateRequest: Codable {
    let participantId: String
    let update: UpdateParticipant
}

struct ParticipantBulkUpdateStreamingRequest: Codable {
    let updates: [ParticipantBulkUpdateRequest]
    let chunkSize: Int?
    let auth: AuthContextPayload
}

struct ParticipantFindByGenderRequest: Codable {
    let gender: String
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantFindByAgeGroupRequest: Codable {
    let ageGroup: String
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantFindByLocationRequest: Codable {
    let location: String
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantFindByDisabilityRequest: Codable {
    let hasDisability: Bool
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantGetWorkshopParticipantsRequest: Codable {
    let workshopId: String
    let pagination: PaginationDto?
    let include: [ParticipantInclude]?
    let auth: AuthContextPayload
}

struct ParticipantCheckDuplicatesRequest: Codable {
    let name: String
    let auth: AuthContextPayload
}