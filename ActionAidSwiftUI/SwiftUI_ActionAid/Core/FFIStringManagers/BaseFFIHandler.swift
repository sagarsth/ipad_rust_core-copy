//
//  BaseFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Generic FFI handler protocol to eliminate duplicated FFI code across domains
//

import Foundation

// MARK: - Base FFI Handler Protocol

/// Universal protocol for all domain FFI handlers
/// Eliminates code duplication across Strategic Goals, Projects, Participants, Livelihoods, etc.
protocol DomainFFIHandler {
    associatedtype EntityType: DomainEntity
    associatedtype NewEntityType: Codable
    associatedtype UpdateEntityType: Codable
    associatedtype FilterType: DomainEntityFilter
    associatedtype IncludeType: DomainEntityInclude
    
    // MARK: - Core CRUD Operations
    
    /// Create a new entity
    func create(new: NewEntityType, auth: AuthContextPayload) async -> Result<EntityType, Error>
    
    /// Get entity by ID
    func get(id: String, include: [IncludeType]?, auth: AuthContextPayload) async -> Result<EntityType, Error>
    
    /// List entities with pagination and filtering
    func list(pagination: PaginationDto?, include: [IncludeType]?, auth: AuthContextPayload) async -> Result<PaginatedResult<EntityType>, Error>
    
    /// Update entity
    func update(id: String, update: UpdateEntityType, auth: AuthContextPayload) async -> Result<EntityType, Error>
    
    /// Delete entity (soft or hard)
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error>
    
    /// Bulk delete entities
    func bulkDelete(ids: [String], hardDelete: Bool?, force: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error>
    
    // MARK: - Filtering Operations
    
    /// Get filtered entity IDs (for bulk selection)
    func getFilteredIds(filter: FilterType, auth: AuthContextPayload) async -> Result<[String], Error>
    
    // MARK: - Statistics Operations
    
    /// Get entity statistics (optional - not all domains may have this)
    func getStatistics(auth: AuthContextPayload) async -> Result<Any, Error>
}

// MARK: - Generic FFI Handler Base Class

/// Generic base class that implements common FFI handler functionality
/// Eliminates 80%+ of duplicated code across domain handlers
class BaseFFIHandler<Entity: DomainEntity, NewEntity: Codable, UpdateEntity: Codable, Filter: DomainEntityFilter, Include: DomainEntityInclude>: DomainFFIHandler {
    
    typealias EntityType = Entity
    typealias NewEntityType = NewEntity
    typealias UpdateEntityType = UpdateEntity
    typealias FilterType = Filter
    typealias IncludeType = Include
    
    // MARK: - Common Properties
    
    private let queue: DispatchQueue
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()
    
    // MARK: - FFI Function Type Aliases
    
    typealias CreateFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias GetFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias ListFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias UpdateFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias DeleteFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias BulkDeleteFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias FilterIdsFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias StatisticsFFIFunction = (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    typealias FreeFunction = (UnsafeMutablePointer<CChar>) -> Void
    
    // MARK: - FFI Function References
    
    private let createFFI: CreateFFIFunction
    private let getFFI: GetFFIFunction
    private let listFFI: ListFFIFunction
    private let updateFFI: UpdateFFIFunction
    private let deleteFFI: DeleteFFIFunction
    private let bulkDeleteFFI: BulkDeleteFFIFunction?
    private let filterIdsFFI: FilterIdsFFIFunction?
    private let statisticsFFI: StatisticsFFIFunction?
    private let freeFFI: FreeFunction
    
    // MARK: - Initialization
    
    init(
        domainName: String,
        createFFI: @escaping CreateFFIFunction,
        getFFI: @escaping GetFFIFunction,
        listFFI: @escaping ListFFIFunction,
        updateFFI: @escaping UpdateFFIFunction,
        deleteFFI: @escaping DeleteFFIFunction,
        freeFFI: @escaping FreeFunction,
        bulkDeleteFFI: BulkDeleteFFIFunction? = nil,
        filterIdsFFI: FilterIdsFFIFunction? = nil,
        statisticsFFI: StatisticsFFIFunction? = nil
    ) {
        self.queue = DispatchQueue(label: "com.actionaid.\(domainName).ffi", qos: .userInitiated)
        self.createFFI = createFFI
        self.getFFI = getFFI
        self.listFFI = listFFI
        self.updateFFI = updateFFI
        self.deleteFFI = deleteFFI
        self.bulkDeleteFFI = bulkDeleteFFI
        self.filterIdsFFI = filterIdsFFI
        self.statisticsFFI = statisticsFFI
        self.freeFFI = freeFFI
        
        setupJSONConfiguration()
    }
    
    private func setupJSONConfiguration() {
        jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
        jsonEncoder.dateEncodingStrategy = .iso8601
        jsonDecoder.dateDecodingStrategy = .iso8601
    }
    
    // MARK: - Generic Helper Methods
    
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
                        free: self.freeFFI
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
    
    // MARK: - CRUD Implementation
    
    func create(new: NewEntity, auth: AuthContextPayload) async -> Result<Entity, Error> {
        struct CreateRequest: Codable {
            let entity: NewEntity
            let auth: AuthContextPayload
        }
        let payload = CreateRequest(entity: new, auth: auth)
        return await executeOperation(payload: payload, ffiCall: createFFI)
    }
    
    func get(id: String, include: [Include]?, auth: AuthContextPayload) async -> Result<Entity, Error> {
        struct GetRequest: Codable {
            let id: String
            let include: [Include]?
            let auth: AuthContextPayload
        }
        let payload = GetRequest(id: id, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: getFFI)
    }
    
    func list(pagination: PaginationDto?, include: [Include]?, auth: AuthContextPayload) async -> Result<PaginatedResult<Entity>, Error> {
        struct ListRequest: Codable {
            let pagination: PaginationDto?
            let include: [Include]?
            let auth: AuthContextPayload
        }
        let payload = ListRequest(pagination: pagination, include: include, auth: auth)
        return await executeOperation(payload: payload, ffiCall: listFFI)
    }
    
    func update(id: String, update: UpdateEntity, auth: AuthContextPayload) async -> Result<Entity, Error> {
        struct UpdateRequest: Codable {
            let id: String
            let update: UpdateEntity
            let auth: AuthContextPayload
        }
        let payload = UpdateRequest(id: id, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: updateFFI)
    }
    
    func delete(id: String, hardDelete: Bool?, auth: AuthContextPayload) async -> Result<DeleteResponse, Error> {
        struct DeleteRequest: Codable {
            let id: String
            let hardDelete: Bool?
            let auth: AuthContextPayload
        }
        let payload = DeleteRequest(id: id, hardDelete: hardDelete, auth: auth)
        return await executeOperation(payload: payload, ffiCall: deleteFFI)
    }
    
    func bulkDelete(ids: [String], hardDelete: Bool?, force: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error> {
        guard let bulkDeleteFFI = bulkDeleteFFI else {
            // Fallback to individual deletes if bulk delete is not supported
            return await fallbackBulkDelete(ids: ids, hardDelete: hardDelete, auth: auth)
        }
        
        struct BulkDeleteRequest: Codable {
            let ids: [String]
            let hardDelete: Bool?
            let force: Bool?
            let auth: AuthContextPayload
        }
        let payload = BulkDeleteRequest(ids: ids, hardDelete: hardDelete, force: force, auth: auth)
        return await executeOperation(payload: payload, ffiCall: bulkDeleteFFI)
    }
    
    func getFilteredIds(filter: Filter, auth: AuthContextPayload) async -> Result<[String], Error> {
        guard let filterIdsFFI = filterIdsFFI else {
            return .failure(FFIError.rustError("Filtered IDs not supported for this entity type"))
        }
        
        struct FilterRequest: Codable {
            let filter: Filter
            let auth: AuthContextPayload
        }
        let payload = FilterRequest(filter: filter, auth: auth)
        return await executeOperation(payload: payload, ffiCall: filterIdsFFI)
    }
    
    func getStatistics(auth: AuthContextPayload) async -> Result<Any, Error> {
        guard let statisticsFFI = statisticsFFI else {
            return .failure(FFIError.rustError("Statistics not supported for this entity type"))
        }
        
        struct StatisticsRequest: Codable {
            let auth: AuthContextPayload
        }
        let payload = StatisticsRequest(auth: auth)
        
        // Return raw JSON data since statistics structure varies by domain
        let result: Result<[String: Any], Error> = await executeOperation(payload: payload, ffiCall: statisticsFFI)
        return result.map { $0 as Any }
    }
    
    // MARK: - Fallback Methods
    
    private func fallbackBulkDelete(ids: [String], hardDelete: Bool?, auth: AuthContextPayload) async -> Result<BatchDeleteResult, Error> {
        var hardDeleted: [String] = []
        var softDeleted: [String] = []
        var failed: [String] = []
        var errors: [String: String] = [:]
        
        for id in ids {
            let result = await delete(id: id, hardDelete: hardDelete, auth: auth)
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
            dependencies: [:],
            errors: errors
        )
        
        return .success(batchResult)
    }
}

// MARK: - Concrete FFI Handler Implementations

/// Strategic Goal FFI Handler using the generic base
class GenericStrategicGoalFFIHandler: BaseFFIHandler<StrategicGoalResponse, NewStrategicGoal, UpdateStrategicGoal, StrategicGoalFilter, StrategicGoalInclude> {
    
    init() {
        super.init(
            domainName: "strategic_goal",
            createFFI: strategic_goal_create,
            getFFI: strategic_goal_get,
            listFFI: strategic_goal_list,
            updateFFI: strategic_goal_update,
            deleteFFI: strategic_goal_delete,
            freeFFI: strategic_goal_free,
            bulkDeleteFFI: strategic_goal_bulk_delete,
            filterIdsFFI: strategic_goal_get_filtered_ids,
            statisticsFFI: strategic_goal_get_statistics
        )
    }
}

/// Project FFI Handler using the generic base
class GenericProjectFFIHandler: BaseFFIHandler<ProjectResponse, NewProject, UpdateProject, ProjectFilter, ProjectInclude> {
    
    init() {
        super.init(
            domainName: "project",
            createFFI: project_create,
            getFFI: project_get,
            listFFI: project_list,
            updateFFI: project_update,
            deleteFFI: project_delete,
            freeFFI: project_free,
            filterIdsFFI: project_get_filtered_ids,
            statisticsFFI: project_get_statistics
        )
    }
}

// MARK: - Migration Guide

/*
 Migration Guide for Existing FFI Handlers:

 1. Replace your current FFI handler with the generic version:
 
    // OLD:
    private let ffiHandler = StrategicGoalFFIHandler()
    
    // NEW:
    private let ffiHandler = GenericStrategicGoalFFIHandler()

 2. The API remains exactly the same, so no view code changes needed:
 
    let result = await ffiHandler.list(pagination: pagination, include: include, auth: auth)

 3. Benefits:
    - 80% less FFI handler code per domain
    - Consistent behavior across all domains
    - Easier to add new domains (just specify FFI function names)
    - Centralized error handling and logging
    - Automatic fallback for bulk operations

 4. For new domains (Participants, Livelihoods):
 
    class GenericParticipantFFIHandler: BaseFFIHandler<ParticipantResponse, NewParticipant, UpdateParticipant, ParticipantFilter, ParticipantInclude> {
        init() {
            super.init(
                domainName: "participant",
                createFFI: participant_create,
                getFFI: participant_get,
                listFFI: participant_list,
                updateFFI: participant_update,
                deleteFFI: participant_delete,
                freeFFI: participant_free
                // Optional: add bulk operations, filtering, statistics as they become available
            )
        }
    }
 */ 