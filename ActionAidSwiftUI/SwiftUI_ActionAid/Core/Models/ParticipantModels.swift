//
//  ParticipantModels.swift
//  SwiftUI_ActionAid
//
//  Participant domain models and related types
//

import Foundation
import SwiftUI

// MARK: - Core Enums

enum Gender: String, Codable, CaseIterable {
    case male = "male"
    case female = "female"
    case other = "other"
    case preferNotToSay = "prefer_not_to_say"
    
    var displayName: String {
        switch self {
        case .male: return "Male"
        case .female: return "Female"
        case .other: return "Other"
        case .preferNotToSay: return "Prefer not to say"
        }
    }
}

enum AgeGroup: String, Codable, CaseIterable {
    case child = "child"
    case youth = "youth"
    case adult = "adult"
    case elderly = "elderly"
    
    var displayName: String {
        switch self {
        case .child: return "Child"
        case .youth: return "Youth"
        case .adult: return "Adult"
        case .elderly: return "Elderly"
        }
    }
}

enum DisabilityType: String, Codable, CaseIterable {
    case visual = "visual"
    case hearing = "hearing"
    case physical = "physical"
    case intellectual = "intellectual"
    case psychosocial = "psychosocial"
    case multiple = "multiple"
    case other = "other"
    
    var displayName: String {
        switch self {
        case .visual: return "Visual"
        case .hearing: return "Hearing"
        case .physical: return "Physical"
        case .intellectual: return "Intellectual"
        case .psychosocial: return "Psychosocial"
        case .multiple: return "Multiple"
        case .other: return "Other"
        }
    }
}

// MARK: - Core Participant Models

struct ParticipantResponse: Codable, Identifiable {
    let id: String
    let name: String
    let gender: String?
    let disability: Bool
    let disabilityType: String?
    let ageGroup: String?
    let location: String?
    let createdAt: String
    let updatedAt: String
    
    // Enriched fields
    let documents: [MediaDocumentResponse]?
    let workshopCount: Int64?
    let livelihoodCount: Int64?
    let documentCount: Int64?
    let activeLivelihoodCount: Int64?
    let completedWorkshopCount: Int64?
    let upcomingWorkshopCount: Int64?
    let workshops: [WorkshopSummary]?
    let livelihoods: [LivelihoodSummary]?
    let documentCountsByType: [String: Int64]?
    
    enum CodingKeys: String, CodingKey {
        case id, name, gender, disability, location, documents, workshops, livelihoods
        case disabilityType = "disability_type"
        case ageGroup = "age_group"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case workshopCount = "workshop_count"
        case livelihoodCount = "livelihood_count"
        case documentCount = "document_count"
        case activeLivelihoodCount = "active_livelihood_count"
        case completedWorkshopCount = "completed_workshop_count"
        case upcomingWorkshopCount = "upcoming_workshop_count"
        case documentCountsByType = "document_counts_by_type"
    }
    
    // Helper computed properties
    var parsedGender: Gender? {
        guard let gender = gender else { return nil }
        return Gender(rawValue: gender)
    }
    
    var parsedAgeGroup: AgeGroup? {
        guard let ageGroup = ageGroup else { return nil }
        return AgeGroup(rawValue: ageGroup)
    }
    
    var genderDisplayName: String {
        parsedGender?.displayName ?? "Not specified"
    }
    
    var ageGroupDisplayName: String {
        parsedAgeGroup?.displayName ?? "Not specified"
    }
    
    var disabilityDescription: String {
        if disability {
            if let typeStr = disabilityType, let type = DisabilityType(rawValue: typeStr) {
                return type.displayName
            } else {
                return disabilityType ?? "Yes"
            }
        }
        return "No"
    }
}

struct NewParticipant: Codable {
    let name: String
    let gender: String?
    let disability: Bool?
    let disabilityType: String?
    let ageGroup: String?
    let location: String?
    let createdByUserId: String?
    let syncPriority: SyncPriority?
    
    enum CodingKeys: String, CodingKey {
        case name, gender, disability, location
        case disabilityType = "disability_type"
        case ageGroup = "age_group"
        case createdByUserId = "created_by_user_id"
        case syncPriority = "sync_priority"
    }
}

struct UpdateParticipant: Codable {
    let name: String?
    let gender: String?
    let disability: Bool?
    let disabilityType: String?
    let ageGroup: String?
    let location: String?
    let updatedByUserId: String
    let syncPriority: SyncPriority?
    
    enum CodingKeys: String, CodingKey {
        case name, gender, disability, location
        case disabilityType = "disability_type"
        case ageGroup = "age_group"
        case updatedByUserId = "updated_by_user_id"
        case syncPriority = "sync_priority"
    }
}

// MARK: - Participant Include Options

enum ParticipantInclude: String, Codable, CaseIterable {
    // FFI-supported options (these work with current backend)
    case documents = "documents"
    case workshops = "workshops"
    case livelihoods = "livelihoods"
    case all = "all"
    
    // Backend-supported but not FFI-enabled (these cause JSON errors)
    // TODO: Enable these in FFI layer (src/ffi/participant.rs ParticipantIncludeDto)
    case workshopCount = "workshop_count"
    case livelihoodCount = "livelihood_count"
    case activeLivelihoodCount = "active_livelihood_count"
    case completedWorkshopCount = "completed_workshop_count"
    case upcomingWorkshopCount = "upcoming_workshop_count"
    case documentCount = "document_count"
    case documentCountsByType = "document_counts_by_type"
    case allCounts = "all_counts"
}

// MARK: - Filter Models

struct ParticipantFilter: Codable {
    let genders: [String]?
    let ageGroups: [String]?
    let locations: [String]?
    let disability: Bool?
    let disabilityTypes: [String]?
    let searchText: String?
    let dateRange: (String, String)?
    let createdByUserIds: [String]?
    let workshopIds: [String]?
    let hasDocuments: Bool?
    let documentLinkedFields: [String]?
    let excludeDeleted: Bool
    
    private enum CodingKeys: String, CodingKey {
        case genders, locations, disability, excludeDeleted
        case ageGroups = "age_groups"
        case disabilityTypes = "disability_types"
        case searchText = "search_text"
        case dateRange = "date_range"
        case createdByUserIds = "created_by_user_ids"
        case workshopIds = "workshop_ids"
        case hasDocuments = "has_documents"
        case documentLinkedFields = "document_linked_fields"
    }
    
    init(genders: [String]? = nil,
         ageGroups: [String]? = nil,
         locations: [String]? = nil,
         disability: Bool? = nil,
         disabilityTypes: [String]? = nil,
         searchText: String? = nil,
         dateRange: (String, String)? = nil,
         createdByUserIds: [String]? = nil,
         workshopIds: [String]? = nil,
         hasDocuments: Bool? = nil,
         documentLinkedFields: [String]? = nil,
         excludeDeleted: Bool = true) {
        self.genders = genders
        self.ageGroups = ageGroups
        self.locations = locations
        self.disability = disability
        self.disabilityTypes = disabilityTypes
        self.searchText = searchText
        self.dateRange = dateRange
        self.createdByUserIds = createdByUserIds
        self.workshopIds = workshopIds
        self.hasDocuments = hasDocuments
        self.documentLinkedFields = documentLinkedFields
        self.excludeDeleted = excludeDeleted
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        genders = try container.decodeIfPresent([String].self, forKey: .genders)
        ageGroups = try container.decodeIfPresent([String].self, forKey: .ageGroups)
        locations = try container.decodeIfPresent([String].self, forKey: .locations)
        disability = try container.decodeIfPresent(Bool.self, forKey: .disability)
        disabilityTypes = try container.decodeIfPresent([String].self, forKey: .disabilityTypes)
        searchText = try container.decodeIfPresent(String.self, forKey: .searchText)
        createdByUserIds = try container.decodeIfPresent([String].self, forKey: .createdByUserIds)
        workshopIds = try container.decodeIfPresent([String].self, forKey: .workshopIds)
        hasDocuments = try container.decodeIfPresent(Bool.self, forKey: .hasDocuments)
        documentLinkedFields = try container.decodeIfPresent([String].self, forKey: .documentLinkedFields)
        excludeDeleted = try container.decodeIfPresent(Bool.self, forKey: .excludeDeleted) ?? true
        
        // Handle tuple encoding/decoding
        if let dateRangeArray = try container.decodeIfPresent([String].self, forKey: .dateRange),
           dateRangeArray.count == 2 {
            dateRange = (dateRangeArray[0], dateRangeArray[1])
        } else {
            dateRange = nil
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(genders, forKey: .genders)
        try container.encodeIfPresent(ageGroups, forKey: .ageGroups)
        try container.encodeIfPresent(locations, forKey: .locations)
        try container.encodeIfPresent(disability, forKey: .disability)
        try container.encodeIfPresent(disabilityTypes, forKey: .disabilityTypes)
        try container.encodeIfPresent(searchText, forKey: .searchText)
        try container.encodeIfPresent(createdByUserIds, forKey: .createdByUserIds)
        try container.encodeIfPresent(workshopIds, forKey: .workshopIds)
        try container.encodeIfPresent(hasDocuments, forKey: .hasDocuments)
        try container.encodeIfPresent(documentLinkedFields, forKey: .documentLinkedFields)
        try container.encode(excludeDeleted, forKey: .excludeDeleted)
        
        // Handle tuple encoding
        if let dateRange = dateRange {
            try container.encode([dateRange.0, dateRange.1], forKey: .dateRange)
        }
    }
}

// MARK: - Statistics and Analytics Models

struct ParticipantDemographics: Codable {
    let totalParticipants: Int64
    let activeParticipants: Int64
    let deletedParticipants: Int64
    let byGender: [String: Int64]
    let byAgeGroup: [String: Int64]
    let byLocation: [String: Int64]
    let byDisability: [String: Int64]
    let byDisabilityType: [String: Int64]
    let participantsWithWorkshops: Int64
    let participantsWithLivelihoods: Int64
    let participantsWithDocuments: Int64
    let participantsWithNoEngagement: Int64
    let avgWorkshopsPerParticipant: Double
    let maxWorkshopsPerParticipant: Int64
    let participantsByWorkshopCount: [String: Int64]
    let avgLivelihoodsPerParticipant: Double
    let maxLivelihoodsPerParticipant: Int64
    let participantsByLivelihoodCount: [String: Int64]
    let avgDocumentsPerParticipant: Double
    let maxDocumentsPerParticipant: Int64
    let participantsByDocumentCount: [String: Int64]
    let documentTypesUsage: [String: Int64]
    let participantsAddedThisMonth: Int64
    let participantsAddedThisYear: Int64
    let monthlyRegistrationTrend: [String: Int64]
    let participantsMissingGender: Int64
    let participantsMissingAgeGroup: Int64
    let participantsMissingLocation: Int64
    let dataCompletenessPercentage: Double
    let generatedAt: String
    
    enum CodingKeys: String, CodingKey {
        case totalParticipants = "total_participants"
        case activeParticipants = "active_participants"
        case deletedParticipants = "deleted_participants"
        case byGender = "by_gender"
        case byAgeGroup = "by_age_group"
        case byLocation = "by_location"
        case byDisability = "by_disability"
        case byDisabilityType = "by_disability_type"
        case participantsWithWorkshops = "participants_with_workshops"
        case participantsWithLivelihoods = "participants_with_livelihoods"
        case participantsWithDocuments = "participants_with_documents"
        case participantsWithNoEngagement = "participants_with_no_engagement"
        case avgWorkshopsPerParticipant = "avg_workshops_per_participant"
        case maxWorkshopsPerParticipant = "max_workshops_per_participant"
        case participantsByWorkshopCount = "participants_by_workshop_count"
        case avgLivelihoodsPerParticipant = "avg_livelihoods_per_participant"
        case maxLivelihoodsPerParticipant = "max_livelihoods_per_participant"
        case participantsByLivelihoodCount = "participants_by_livelihood_count"
        case avgDocumentsPerParticipant = "avg_documents_per_participant"
        case maxDocumentsPerParticipant = "max_documents_per_participant"
        case participantsByDocumentCount = "participants_by_document_count"
        case documentTypesUsage = "document_types_usage"
        case participantsAddedThisMonth = "participants_added_this_month"
        case participantsAddedThisYear = "participants_added_this_year"
        case monthlyRegistrationTrend = "monthly_registration_trend"
        case participantsMissingGender = "participants_missing_gender"
        case participantsMissingAgeGroup = "participants_missing_age_group"
        case participantsMissingLocation = "participants_missing_location"
        case dataCompletenessPercentage = "data_completeness_percentage"
        case generatedAt = "generated_at"
    }
}

struct ParticipantStatistics: Codable {
    let totalParticipants: Int64
    let activeParticipants: Int64
    let participantsWithDisabilities: Int64
    let byGender: [String: Int64]
    let byAgeGroup: [String: Int64]
    let byLocation: [String: Int64]
    let byDisabilityType: [String: Int64]
    let engagementDistribution: [String: Int64]
    let monthlyRegistrationTrends: [String: Int64]
    let dataCompleteness: Double
    
    enum CodingKeys: String, CodingKey {
        case totalParticipants = "total_participants"
        case activeParticipants = "active_participants"
        case participantsWithDisabilities = "participants_with_disabilities"
        case byGender = "by_gender"
        case byAgeGroup = "by_age_group"
        case byLocation = "by_location"
        case byDisabilityType = "by_disability_type"
        case engagementDistribution = "engagement_distribution"
        case monthlyRegistrationTrends = "monthly_registration_trends"
        case dataCompleteness = "data_completeness"
    }
}

// MARK: - Relationship Models

struct WorkshopSummary: Codable {
    let id: String
    let name: String
    let date: String?
    let location: String?
    let hasCompleted: Bool
    let preEvaluation: String?
    let postEvaluation: String?
    
    enum CodingKeys: String, CodingKey {
        case id, name, date, location
        case hasCompleted = "has_completed"
        case preEvaluation = "pre_evaluation"
        case postEvaluation = "post_evaluation"
    }
}

struct LivelihoodSummary: Codable {
    let id: String
    let name: String
    let type: String?
    let status: String?
    let startDate: String?
    
    enum CodingKeys: String, CodingKey {
        case id, name, status
        case type = "type_"
        case startDate = "start_date"
    }
}

struct ParticipantWithWorkshops: Codable {
    let participant: ParticipantResponse
    let workshops: [WorkshopSummary]
    let totalWorkshops: Int64
    let completedWorkshops: Int64
    let upcomingWorkshops: Int64
    
    enum CodingKeys: String, CodingKey {
        case participant, workshops
        case totalWorkshops = "total_workshops"
        case completedWorkshops = "completed_workshops"
        case upcomingWorkshops = "upcoming_workshops"
    }
}

struct ParticipantWithLivelihoods: Codable {
    let participant: ParticipantResponse
    let livelihoods: [LivelihoodSummary]
    let totalLivelihoods: Int64
    let activeLivelihoods: Int64
    
    enum CodingKeys: String, CodingKey {
        case participant, livelihoods
        case totalLivelihoods = "total_livelihoods"
        case activeLivelihoods = "active_livelihoods"
    }
}

struct ParticipantWithDocumentTimeline: Codable {
    let participant: ParticipantResponse
    let documentsByMonth: [String: [MediaDocumentResponse]]
    let totalDocumentCount: UInt64
    
    enum CodingKeys: String, CodingKey {
        case participant
        case documentsByMonth = "documents_by_month"
        case totalDocumentCount = "total_document_count"
    }
}

// MARK: - Advanced Types

struct ParticipantDocumentReference: Codable {
    let fieldName: String
    let displayName: String
    let documentId: String?
    let filename: String?
    let uploadDate: String?
    let fileSize: UInt64?
    
    enum CodingKeys: String, CodingKey {
        case fieldName = "field_name"
        case displayName = "display_name"
        case documentId = "document_id"
        case filename
        case uploadDate = "upload_date"
        case fileSize = "file_size"
    }
}

struct ParticipantWithEnrichment: Codable {
    let participant: ParticipantResponse  // Note: This expects the full participant object
    let workshopCount: Int64
    let livelihoodCount: Int64
    let activeLivelihoodCount: Int64
    let documentCount: Int64
    let recentDocumentCount: Int64
    
    enum CodingKeys: String, CodingKey {
        case participant
        case workshopCount = "workshop_count"
        case livelihoodCount = "livelihood_count"
        case activeLivelihoodCount = "active_livelihood_count"
        case documentCount = "document_count"
        case recentDocumentCount = "recent_document_count"
    }
}

struct ParticipantBulkOperationResult: Codable {
    let totalRequested: Int
    let successful: Int
    let failed: Int
    let skipped: Int
    let errorDetails: [String: String] // Changed from [(String, String)] to [String: String] for Codable conformance
    let operationDurationMs: UInt64
    
    enum CodingKeys: String, CodingKey {
        case totalRequested = "total_requested"
        case successful
        case failed
        case skipped
        case errorDetails = "error_details"
        case operationDurationMs = "operation_duration_ms"
    }
}

// MARK: - Response Models

struct ParticipantCreateWithDocumentsResponse: Codable {
    let participant: ParticipantResponse
    let documentResults: [Result<MediaDocumentResponse, DocumentUploadError>]
    
    // Custom decoding to handle Result types
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        participant = try container.decode(ParticipantResponse.self, forKey: .participant)
        
        // For now, decode as a simple array - adapt to Result types for UI
        if let documentsArray = try? container.decode([MediaDocumentResponse].self, forKey: .documentResults) {
            documentResults = documentsArray.map { Result.success($0) }
        } else if let errorsArray = try? container.decode([String].self, forKey: .documentResults) {
            documentResults = errorsArray.map { Result.failure(DocumentUploadError($0)) }
        } else {
            documentResults = []
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(participant, forKey: .participant)
        
        // Encode successful results only for now
        let successfulDocuments = documentResults.compactMap { result in
            if case .success(let doc) = result { return doc } else { return nil }
        }
        try container.encode(successfulDocuments, forKey: .documentResults)
    }
    
    private enum CodingKeys: String, CodingKey {
        case participant, documentResults = "document_results"
    }
}

// MARK: - Table Configuration

struct ParticipantTableConfig {
    static let columns: [TableColumn] = [
        TableColumn(
            key: "name",
            title: "Name",
            width: nil, // Allow expansion
            alignment: .leading,
            isRequired: true
        ),
        TableColumn(
            key: "gender",
            title: "Gender",
            width: 100,
            alignment: .center
        ),
        TableColumn(
            key: "age_group",
            title: "Age Group",
            width: 100,
            alignment: .center
        ),
        TableColumn(
            key: "location",
            title: "Location",
            width: 150,
            alignment: .leading
        ),
        TableColumn(
            key: "disability",
            title: "Disability",
            width: 120,
            alignment: .center
        ),
        TableColumn(
            key: "workshops",
            title: "Workshops",
            width: 100,
            alignment: .center
        ),
        TableColumn(
            key: "documents",
            title: "Docs",
            width: 60,
            alignment: .center
        ),
        TableColumn(
            key: "updated_at",
            title: "Updated",
            width: 120,
            alignment: .center
        )
    ]
}

// MARK: - Duplicate Detection Models

struct ParticipantDuplicateInfo: Codable, Identifiable {
    let id: String
    let name: String
    let gender: String?
    let ageGroup: String?
    let location: String?
    let disability: Bool
    let disabilityType: String?
    let createdAt: String
    let updatedAt: String
    
    // Document information for duplicate detection
    let profilePhotoUrl: String?
    let identificationDocuments: [DuplicateDocumentInfo]
    let otherDocuments: [DuplicateDocumentInfo]
    let totalDocumentCount: Int64
    
    // Activity summary
    let workshopCount: Int64
    let livelihoodCount: Int64
    
    enum CodingKeys: String, CodingKey {
        case id, name, gender, location, disability, workshopCount, livelihoodCount
        case ageGroup = "age_group"
        case disabilityType = "disability_type"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case profilePhotoUrl = "profile_photo_url"
        case identificationDocuments = "identification_documents"
        case otherDocuments = "other_documents"
        case totalDocumentCount = "total_document_count"
    }
    
    // Helper computed properties
    var genderDisplayName: String {
        guard let gender = gender, let parsedGender = Gender(rawValue: gender) else { return "Not specified" }
        return parsedGender.displayName
    }
    
    var ageGroupDisplayName: String {
        guard let ageGroup = ageGroup, let parsedAgeGroup = AgeGroup(rawValue: ageGroup) else { return "Not specified" }
        return parsedAgeGroup.displayName
    }
    
    var disabilityDescription: String {
        if disability {
            if let typeStr = disabilityType, let type = DisabilityType(rawValue: typeStr) {
                return type.displayName
            } else {
                return disabilityType ?? "Yes"
            }
        }
        return "No"
    }
    
    var hasDocuments: Bool {
        return totalDocumentCount > 0
    }
    
    var hasProfilePhoto: Bool {
        return profilePhotoUrl != nil
    }
}

struct DuplicateDocumentInfo: Codable, Identifiable {
    let id: String
    let originalFilename: String
    let filePath: String
    let linkedField: String?
    let documentTypeName: String?
    let uploadedAt: String
    
    enum CodingKeys: String, CodingKey {
        case id
        case originalFilename = "original_filename"
        case filePath = "file_path"
        case linkedField = "linked_field"
        case documentTypeName = "document_type_name"
        case uploadedAt = "uploaded_at"
    }
    
    var isIdentificationDocument: Bool {
        guard let field = linkedField else { return false }
        return field.contains("identification") || field.contains("id") || field.contains("identity")
    }
    
    var isProfilePhoto: Bool {
        guard let field = linkedField else { return false }
        return field.contains("profile") || field.contains("photo")
    }
}

// MARK: - Conformances

extension ParticipantResponse: MonthGroupable {
    // MonthGroupable conformance is satisfied by createdAt and updatedAt properties
}

extension ParticipantResponse: Equatable {
    static func == (lhs: ParticipantResponse, rhs: ParticipantResponse) -> Bool {
        return lhs.id == rhs.id &&
               lhs.name == rhs.name &&
               lhs.gender == rhs.gender &&
               lhs.disability == rhs.disability &&
               lhs.disabilityType == rhs.disabilityType &&
               lhs.ageGroup == rhs.ageGroup &&
               lhs.location == rhs.location &&
               lhs.updatedAt == rhs.updatedAt
    }
}