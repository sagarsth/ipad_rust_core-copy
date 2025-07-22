//
//  ProjectModels.swift
//  SwiftUI_ActionAid
//
//  Project domain models and related types
//

import Foundation
import SwiftUI

// MARK: - Core Project Models

struct ProjectResponse: Codable, Identifiable {
    let id: String
    let name: String
    let objective: String?
    let outcome: String?
    let statusId: Int64?
    let status: StatusInfo?
    let timeline: String?
    let responsibleTeam: String?
    let strategicGoalId: String?
    let strategicGoal: StrategicGoalSummary?
    let createdAt: String
    let updatedAt: String
    let createdByUserId: String?
    let updatedByUserId: String?
    let createdBy: String?
    let createdByUsername: String?
    let updatedByUsername: String?
    let strategicGoalName: String?
    let activityCount: Int64?
    let workshopCount: Int64?
    let documentCount: Int64?
    let syncPriority: SyncPriority
    let documents: [MediaDocumentResponse]?
    
    enum CodingKeys: String, CodingKey {
        case id, name, objective, outcome, timeline, documents
        case statusId = "status_id"
        case status
        case responsibleTeam = "responsible_team"
        case strategicGoalId = "strategic_goal_id"
        case strategicGoal = "strategic_goal"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case createdByUserId = "created_by_user_id"
        case updatedByUserId = "updated_by_user_id"
        case createdBy = "created_by"
        case createdByUsername = "created_by_username"
        case updatedByUsername = "updated_by_username"
        case strategicGoalName = "strategic_goal_name"
        case activityCount = "activity_count"
        case workshopCount = "workshop_count"
        case documentCount = "document_count"
        case syncPriority = "sync_priority"
    }
    
    // Helper computed properties
    var statusName: String {
        switch statusId {
        case 1: return "On Track"
        case 2: return "At Risk"
        case 3: return "Delayed"
        case 4: return "Completed"
        default: return "Unknown"
        }
    }
    
    var statusColor: Color {
        switch statusId {
        case 1: return .green
        case 2: return .orange
        case 3: return .red
        case 4: return .blue
        default: return .gray
        }
    }
    
    /// Computed property to get strategic goal name from either direct field or nested object
    var effectiveStrategicGoalName: String? {
        // First try the direct field
        if let directName = strategicGoalName {
            return directName
        }
        // Fall back to nested strategic goal object
        return strategicGoal?.objectiveCode
    }
}

struct NewProject: Codable {
    let strategicGoalId: String?
    let name: String
    let objective: String?
    let outcome: String?
    let statusId: Int64?
    let timeline: String?
    let responsibleTeam: String?
    let syncPriority: SyncPriority
    let createdByUserId: String?
    
    enum CodingKeys: String, CodingKey {
        case name, objective, outcome, timeline, syncPriority
        case strategicGoalId = "strategic_goal_id"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case createdByUserId = "created_by_user_id"
    }
}

struct UpdateProject: Codable {
    let strategicGoalId: String?? // Optional<Optional<String>> for nullable updates
    let name: String?
    let objective: String?
    let outcome: String?
    let statusId: Int64?
    let timeline: String?
    let responsibleTeam: String?
    let syncPriority: SyncPriority?
    let updatedByUserId: String?
    
    enum CodingKeys: String, CodingKey {
        case name, objective, outcome, timeline, syncPriority
        case strategicGoalId = "strategic_goal_id"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case updatedByUserId = "updated_by_user_id"
    }
}

// MARK: - Project Include Options

enum ProjectInclude: String, Codable, CaseIterable {
    case strategicGoal = "strategic_goal"
    case status = "status"
    case createdBy = "created_by"
    case activityCount = "activity_count"
    case workshopCount = "workshop_count"
    case documents = "documents"
    case documentReferences = "document_references"
    case activityTimeline = "activity_timeline"
    case statusDetails = "status_details"
    case counts = "counts"
    case all = "all"
}

// MARK: - Filter Models

struct ProjectFilter: Codable {
    let statusIds: [Int64]?
    let strategicGoalIds: [String]?
    let responsibleTeams: [String]?
    let searchText: String?
    let dateRange: (String, String)?
    let excludeDeleted: Bool?
    
    private enum CodingKeys: String, CodingKey {
        case statusIds, strategicGoalIds, responsibleTeams, searchText, dateRange, excludeDeleted
    }
    
    init(statusIds: [Int64]? = nil,
         strategicGoalIds: [String]? = nil,
         responsibleTeams: [String]? = nil,
         searchText: String? = nil,
         dateRange: (String, String)? = nil,
         excludeDeleted: Bool? = nil) {
        self.statusIds = statusIds
        self.strategicGoalIds = strategicGoalIds
        self.responsibleTeams = responsibleTeams
        self.searchText = searchText
        self.dateRange = dateRange
        self.excludeDeleted = excludeDeleted
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        statusIds = try container.decodeIfPresent([Int64].self, forKey: .statusIds)
        strategicGoalIds = try container.decodeIfPresent([String].self, forKey: .strategicGoalIds)
        responsibleTeams = try container.decodeIfPresent([String].self, forKey: .responsibleTeams)
        searchText = try container.decodeIfPresent(String.self, forKey: .searchText)
        excludeDeleted = try container.decodeIfPresent(Bool.self, forKey: .excludeDeleted)
        
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
        try container.encodeIfPresent(statusIds, forKey: .statusIds)
        try container.encodeIfPresent(strategicGoalIds, forKey: .strategicGoalIds)
        try container.encodeIfPresent(responsibleTeams, forKey: .responsibleTeams)
        try container.encodeIfPresent(searchText, forKey: .searchText)
        try container.encodeIfPresent(excludeDeleted, forKey: .excludeDeleted)
        
        // Handle tuple encoding
        if let dateRange = dateRange {
            try container.encode([dateRange.0, dateRange.1], forKey: .dateRange)
        }
    }
}

// MARK: - Statistics and Analytics Models

struct ProjectStatistics: Codable {
    let totalProjects: Int64
    let byStatus: [String: Int64]
    let byStrategicGoal: [String: Int64]
    let byResponsibleTeam: [String: Int64]
    let documentCount: Int64
    
    enum CodingKeys: String, CodingKey {
        case totalProjects = "total_projects"
        case byStatus = "by_status"
        case byStrategicGoal = "by_strategic_goal"
        case byResponsibleTeam = "by_responsible_team"
        case documentCount = "document_count"
    }
}

struct ProjectStatusBreakdown: Codable {
    let statusId: Int64
    let statusName: String
    let count: Int64
    let percentage: Double
    
    enum CodingKeys: String, CodingKey {
        case statusId = "status_id"
        case statusName = "status_name"
        case count
        case percentage
    }
}

struct ProjectMetadataCounts: Codable {
    let projectsByTeam: [String: Int64]
    let projectsByStatus: [String: Int64]
    let projectsByGoal: [String: Int64]
}

struct ProjectWithDocumentTimeline: Codable {
    let project: ProjectResponse
    let documentsByType: [String: [MediaDocumentResponse]]
    let totalDocumentCount: UInt64
}

struct ProjectDocumentReference: Codable {
    let fieldName: String
    let displayName: String
    let documentId: String?
    let filename: String?
    let uploadDate: String? // ISO date string
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



struct ProjectActivityTimeline: Codable {
    let activeProjects: Int64
    let inactiveProjects: Int64
    let totalProjects: Int64
    let activityPercentage: Double
    let staleProjects: Int64
    let recentlyUpdatedProjects: Int64
    
    enum CodingKeys: String, CodingKey {
        case activeProjects = "active_projects"
        case inactiveProjects = "inactive_projects"
        case totalProjects = "total_projects"
        case activityPercentage = "activity_percentage"
        case staleProjects = "stale_projects"
        case recentlyUpdatedProjects = "recently_updated_projects"
    }
}

struct TeamWorkloadDistribution: Codable {
    let teamName: String
    let projectCount: Int64
    let activeProjectCount: Int64
    let completedProjectCount: Int64
    let totalWorkload: Double
}

struct StrategicGoalDistribution: Codable {
    let strategicGoalId: String
    let objectiveCode: String
    let outcome: String?
    let projectCount: Int64
    let percentage: Double
}

struct DocumentCoverageAnalysis: Codable {
    let totalProjects: Int64
    let projectsWithDocuments: Int64
    let projectsWithoutDocuments: Int64
    let averageDocumentsPerProject: Double
    let documentCoveragePercentage: Double
}

// MARK: - Shared Component Models

struct StrategicGoalSummary: Codable {
    let id: String
    let objectiveCode: String
    let outcome: String?
    
    enum CodingKeys: String, CodingKey {
        case id
        case objectiveCode = "objective_code"
        case outcome
    }
}

struct StatusInfo: Codable {
    let id: Int64
    let value: String
}

// MARK: - Document Upload Support
// Extension moved to ProjectDocumentAdapter.swift to avoid redeclaration

// MARK: - Conformances for Shared Components

extension ProjectResponse: MonthGroupable {
    // MonthGroupable conformance is already satisfied by createdAt and updatedAt properties
}

// Add Equatable conformance for shared components
extension ProjectResponse: Equatable {
    static func == (lhs: ProjectResponse, rhs: ProjectResponse) -> Bool {
        return lhs.id == rhs.id &&
               lhs.name == rhs.name &&
               lhs.objective == rhs.objective &&
               lhs.outcome == rhs.outcome &&
               lhs.statusId == rhs.statusId &&
               lhs.timeline == rhs.timeline &&
               lhs.responsibleTeam == rhs.responsibleTeam &&
               lhs.strategicGoalId == rhs.strategicGoalId &&
               lhs.updatedAt == rhs.updatedAt
    }
}

// MARK: - Document Upload Support
// TODO: DocumentUploadable conformance will be implemented later when document uploading is added

// MARK: - Table Configuration

struct ProjectTableConfig {
    static let columns: [TableColumn] = [
        TableColumn(
            key: "name",
            title: "Name",
            width: nil, // Remove fixed width to allow expansion
            alignment: .leading,
            isRequired: true
        ),
        TableColumn(
            key: "status",
            title: "Status",
            width: 120,
            alignment: .center,
            isRequired: true // Status should always be visible as it's core info
        ),
        TableColumn(
            key: "responsible_team",
            title: "Team",
            width: 150,
            alignment: .leading,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "strategic_goal",
            title: "Strategic Goal",
            width: 180,
            alignment: .leading,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "timeline",
            title: "Timeline",
            width: 140,
            alignment: .leading,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "documents",
            title: "Docs",
            width: 80,
            alignment: .center
        ),
        TableColumn(
            key: "updated_at",
            title: "Updated",
            width: 120,
            alignment: .center,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        )
    ]
}

// MARK: - Response Models for Project Operations

// Simple error type for Result handling
struct DocumentUploadError: Error, Codable {
    let message: String
    
    init(_ message: String) {
        self.message = message
    }
}

struct ProjectCreateWithDocumentsResponse: Codable {
    let project: ProjectResponse
    let documentResults: [Result<MediaDocumentResponse, DocumentUploadError>]
    
    // Custom decoding to handle Result types
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        project = try container.decode(ProjectResponse.self, forKey: .project)
        
        // For now, decode as a simple array - the Rust backend returns this structure
        // We'll adapt it to Result types for the UI
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
        try container.encode(project, forKey: .project)
        
        // Encode successful results only for now
        let successfulDocuments = documentResults.compactMap { result in
            if case .success(let doc) = result { return doc } else { return nil }
        }
        try container.encode(successfulDocuments, forKey: .documentResults)
    }
    
    private enum CodingKeys: String, CodingKey {
        case project, documentResults = "document_results"
    }
}

// MARK: - Export Models

struct ProjectExportOptions: Codable {
    let includeBlobs: Bool
    let targetPath: String?
    let filter: ProjectFilter
    let format: ExportFormat
    
    enum CodingKeys: String, CodingKey {
        case includeBlobs = "include_blobs"
        case targetPath = "target_path" 
        case filter
        case format
    }
}

struct ProjectExportByIdsOptions: Codable {
    let ids: [String]
    let includeBlobs: Bool?
    let targetPath: String?
    let format: ExportFormat
    
    enum CodingKeys: String, CodingKey {
        case ids
        case includeBlobs = "include_blobs"
        case targetPath = "target_path"
        case format
    }
}

// Note: ExportJobResponse and ExportJob are already defined in StrategicGoalModels.swift
// and are shared across all domains for consistency

 