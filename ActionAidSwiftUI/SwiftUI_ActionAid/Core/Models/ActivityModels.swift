//
//  ActivityModels.swift
//  SwiftUI_ActionAid
//
//  Activity domain models and related types
//

import Foundation
import SwiftUI

// MARK: - Core Activity Models

struct ActivityResponse: Codable, Identifiable {
    let id: String
    let projectId: String?
    let syncPriority: SyncPriority
    let description: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let progressPercentage: Double?
    let statusId: Int64?
    let status: StatusInfo?
    let project: ProjectSummary?
    let createdAt: String
    let updatedAt: String
    let createdByUserId: String
    let updatedByUserId: String
    let deletedAt: String?
    let deletedByUserId: String?
    
    // Enrichment fields
    let createdByUsername: String?
    let updatedByUsername: String?
    let projectName: String?
    let statusName: String?
    let documentCount: Int64?
    let documents: [MediaDocumentResponse]?
    
    enum CodingKeys: String, CodingKey {
        case id, description, kpi, documents
        case projectId = "project_id"
        case syncPriority = "sync_priority"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case progressPercentage = "progress_percentage"
        case statusId = "status_id"
        case status, project
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case createdByUserId = "created_by_user_id"
        case updatedByUserId = "updated_by_user_id"
        case deletedAt = "deleted_at"
        case deletedByUserId = "deleted_by_user_id"
        case createdByUsername = "created_by_username"
        case updatedByUsername = "updated_by_username"
        case projectName = "project_name"
        case statusName = "status_name"
        case documentCount = "document_count"
    }
    
    // Helper computed properties
    var statusColor: Color {
        switch statusId {
        case 1: return .green    // Completed
        case 2: return .blue     // In Progress
        case 3: return .orange   // Pending
        case 4: return .red      // Blocked
        default: return .gray
        }
    }
    
    var progressColor: Color {
        guard let progress = progressPercentage else { return .gray }
        if progress >= 80 { return .green }
        else if progress >= 50 { return .blue }
        else if progress > 0 { return .orange }
        else { return .red }
    }
    
    var formattedProgress: String {
        guard let progress = progressPercentage else { return "N/A" }
        return String(format: "%.1f%%", progress)
    }
    
    var isDeleted: Bool {
        deletedAt != nil
    }
}

// MARK: - Project Summary for Activities
struct ProjectSummary: Codable {
    let id: String
    let name: String
    let statusId: Int64?
    
    enum CodingKeys: String, CodingKey {
        case id, name
        case statusId = "status_id"
    }
}

struct NewActivity: Codable {
    let projectId: String?
    let description: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let statusId: Int64?
    let syncPriority: SyncPriority
    let createdByUserId: String?
    
    enum CodingKeys: String, CodingKey {
        case description, kpi
        case projectId = "project_id"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case statusId = "status_id"
        case syncPriority = "sync_priority"
        case createdByUserId = "created_by_user_id"
    }
}

struct UpdateActivity: Codable {
    let projectId: String??  // Double optional for nullable updates
    let description: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let statusId: Int64?
    let syncPriority: SyncPriority?
    let updatedByUserId: String
    
    enum CodingKeys: String, CodingKey {
        case description, kpi, syncPriority
        case projectId = "project_id"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case statusId = "status_id"
        case updatedByUserId = "updated_by_user_id"
    }
}

// MARK: - Activity Include Options

enum ActivityInclude: String, Codable, CaseIterable {
    case project = "project"
    case status = "status"
    case createdBy = "created_by"
    case updatedBy = "updated_by"
    case documents = "documents"
    case all = "all"
}

// MARK: - Filter Models

struct ActivityFilter: Codable {
    let statusIds: [Int64]?
    let projectIds: [String]?
    let searchText: String?
    let dateRange: (String, String)?
    let targetValueRange: (Double, Double)?
    let actualValueRange: (Double, Double)?
    let excludeDeleted: Bool?
    
    private enum CodingKeys: String, CodingKey {
        case statusIds = "status_ids"
        case projectIds = "project_ids"
        case searchText = "search_text"
        case dateRange = "date_range"
        case targetValueRange = "target_value_range"
        case actualValueRange = "actual_value_range"
        case excludeDeleted = "exclude_deleted"
    }
    
    // Custom memberwise initializer
    init(statusIds: [Int64]? = nil,
         projectIds: [String]? = nil,
         searchText: String? = nil,
         dateRange: (String, String)? = nil,
         targetValueRange: (Double, Double)? = nil,
         actualValueRange: (Double, Double)? = nil,
         excludeDeleted: Bool? = nil) {
        self.statusIds = statusIds
        self.projectIds = projectIds
        self.searchText = searchText
        self.dateRange = dateRange
        self.targetValueRange = targetValueRange
        self.actualValueRange = actualValueRange
        self.excludeDeleted = excludeDeleted
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        statusIds = try container.decodeIfPresent([Int64].self, forKey: .statusIds)
        projectIds = try container.decodeIfPresent([String].self, forKey: .projectIds)
        searchText = try container.decodeIfPresent(String.self, forKey: .searchText)
        excludeDeleted = try container.decodeIfPresent(Bool.self, forKey: .excludeDeleted)
        
        // Handle tuple decoding
        if let dateArray = try container.decodeIfPresent([String].self, forKey: .dateRange), dateArray.count == 2 {
            dateRange = (dateArray[0], dateArray[1])
        } else {
            dateRange = nil
        }
        
        if let targetArray = try container.decodeIfPresent([Double].self, forKey: .targetValueRange), targetArray.count == 2 {
            targetValueRange = (targetArray[0], targetArray[1])
        } else {
            targetValueRange = nil
        }
        
        if let actualArray = try container.decodeIfPresent([Double].self, forKey: .actualValueRange), actualArray.count == 2 {
            actualValueRange = (actualArray[0], actualArray[1])
        } else {
            actualValueRange = nil
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(statusIds, forKey: .statusIds)
        try container.encodeIfPresent(projectIds, forKey: .projectIds)
        try container.encodeIfPresent(searchText, forKey: .searchText)
        try container.encodeIfPresent(excludeDeleted, forKey: .excludeDeleted)
        
        // Handle tuple encoding
        if let dateRange = dateRange {
            try container.encode([dateRange.0, dateRange.1], forKey: .dateRange)
        }
        
        if let targetRange = targetValueRange {
            try container.encode([targetRange.0, targetRange.1], forKey: .targetValueRange)
        }
        
        if let actualRange = actualValueRange {
            try container.encode([actualRange.0, actualRange.1], forKey: .actualValueRange)
        }
    }
}

// MARK: - Statistics and Analytics Models

struct ActivityStatistics: Codable {
    let totalActivities: Int64
    let byStatus: [String: Int64]
    let byProject: [String: Int64]
    let completionRate: Double
    let averageProgress: Double
    let documentCount: Int64
    
    enum CodingKeys: String, CodingKey {
        case totalActivities = "total_activities"
        case byStatus = "by_status"
        case byProject = "by_project"
        case completionRate = "completion_rate"
        case averageProgress = "average_progress"
        case documentCount = "document_count"
    }
}

struct ActivityStatusBreakdown: Codable {
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

struct ActivityMetadataCounts: Codable {
    let activitiesByProject: [String: Int64]
    let activitiesByStatus: [String: Int64]
    let activitiesWithTargets: Int64
    let activitiesWithActuals: Int64
    let activitiesWithDocuments: Int64
    
    enum CodingKeys: String, CodingKey {
        case activitiesByProject = "activities_by_project"
        case activitiesByStatus = "activities_by_status"
        case activitiesWithTargets = "activities_with_targets"
        case activitiesWithActuals = "activities_with_actuals"
        case activitiesWithDocuments = "activities_with_documents"
    }
}

struct ActivityProgressAnalysis: Codable {
    let activitiesOnTrack: Int64
    let activitiesBehind: Int64
    let activitiesAtRisk: Int64
    let activitiesNoProgress: Int64
    let averageProgressPercentage: Double
    let completionRate: Double
    let activitiesWithTargets: Int64
    let activitiesWithoutTargets: Int64
    
    enum CodingKeys: String, CodingKey {
        case activitiesOnTrack = "activities_on_track"
        case activitiesBehind = "activities_behind"
        case activitiesAtRisk = "activities_at_risk"
        case activitiesNoProgress = "activities_no_progress"
        case averageProgressPercentage = "average_progress_percentage"
        case completionRate = "completion_rate"
        case activitiesWithTargets = "activities_with_targets"
        case activitiesWithoutTargets = "activities_without_targets"
    }
}

// MARK: - Document Reference

struct ActivityDocumentReference: Codable {
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

// MARK: - Response Models

struct ActivityCreateWithDocumentsResponse: Codable {
    let activity: ActivityResponse
    let documentResults: [Result<MediaDocumentResponse, DocumentUploadError>]
    
    // Custom decoding to handle Result types
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        activity = try container.decode(ActivityResponse.self, forKey: .activity)
        
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
        try container.encode(activity, forKey: .activity)
        
        // Encode successful results only for now
        let successfulDocuments = documentResults.compactMap { result in
            if case .success(let doc) = result { return doc } else { return nil }
        }
        try container.encode(successfulDocuments, forKey: .documentResults)
    }
    
    private enum CodingKeys: String, CodingKey {
        case activity, documentResults = "document_results"
    }
}

// MARK: - Workload Distribution

struct ActivityWorkloadByProject: Codable {
    let projectId: String
    let projectName: String
    let activityCount: Int64
    let completedCount: Int64
    let inProgressCount: Int64
    let averageProgress: Double
    
    enum CodingKeys: String, CodingKey {
        case projectId = "project_id"
        case projectName = "project_name"
        case activityCount = "activity_count"
        case completedCount = "completed_count"
        case inProgressCount = "in_progress_count"
        case averageProgress = "average_progress"
    }
}

// MARK: - Table Configuration

struct ActivityTableConfig {
    static let columns: [TableColumn] = [
        TableColumn(
            key: "description",
            title: "Description",
            width: nil, // Allow expansion
            alignment: .leading,
            isRequired: true
        ),
        TableColumn(
            key: "kpi",
            title: "KPI",
            width: 150,
            alignment: .leading
        ),
        TableColumn(
            key: "progress",
            title: "Progress",
            width: 100,
            alignment: .center,
            isRequired: true
        ),
        TableColumn(
            key: "target",
            title: "Target",
            width: 80,
            alignment: .trailing
        ),
        TableColumn(
            key: "actual",
            title: "Actual",
            width: 80,
            alignment: .trailing
        ),
        TableColumn(
            key: "status",
            title: "Status",
            width: 100,
            alignment: .center
        ),
        TableColumn(
            key: "project",
            title: "Project",
            width: 150,
            alignment: .leading
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

// MARK: - Conformances

extension ActivityResponse: MonthGroupable {
    // MonthGroupable conformance is satisfied by createdAt and updatedAt properties
}

extension ActivityResponse: Equatable {
    static func == (lhs: ActivityResponse, rhs: ActivityResponse) -> Bool {
        return lhs.id == rhs.id &&
               lhs.projectId == rhs.projectId &&
               lhs.description == rhs.description &&
               lhs.kpi == rhs.kpi &&
               lhs.targetValue == rhs.targetValue &&
               lhs.actualValue == rhs.actualValue &&
               lhs.statusId == rhs.statusId &&
               lhs.updatedAt == rhs.updatedAt
    }
}

// MARK: - Export Models

struct ActivityExportOptions: Codable {
    let includeBlobs: Bool
    let targetPath: String?
    let filter: ActivityFilter
    let format: ExportFormat
    
    enum CodingKeys: String, CodingKey {
        case includeBlobs = "include_blobs"
        case targetPath = "target_path"
        case filter
        case format
    }
}

struct ActivityExportByIdsOptions: Codable {
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

// MARK: - Bulk Update Response

struct BulkUpdateStatusResponse: Codable {
    let updatedCount: UInt64
    let statusId: Int64
    
    enum CodingKeys: String, CodingKey {
        case updatedCount = "updated_count"
        case statusId = "status_id"
    }
}