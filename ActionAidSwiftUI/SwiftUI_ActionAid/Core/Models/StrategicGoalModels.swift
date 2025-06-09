//
//  StrategicGoalModels.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

// MARK: - Enums

enum StrategicGoalInclude: String, Codable {
    case documents = "documents"
    case status = "status"
    case activities = "activities"
    case projects = "projects"
    case projectCount = "project_count"
    case participants = "participants"
    case documentCounts = "document_counts"
}

enum UserGoalRole: String, Codable {
    case created
    case updated
}

enum SyncPriority: String, Codable {
    case low = "Low"
    case normal = "Normal"
    case high = "High"
}


// MARK: - Core DTOs

struct NewStrategicGoal: Codable {
    var id: String?
    let objectiveCode: String
    let outcome: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let statusId: Int?
    let responsibleTeam: String?
    let syncPriority: SyncPriority
    let createdByUserId: String?

    enum CodingKeys: String, CodingKey {
        case id, kpi, outcome
        case objectiveCode = "objective_code"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case syncPriority = "sync_priority"
        case createdByUserId = "created_by_user_id"
    }
}

struct UpdateStrategicGoal: Codable {
    let objectiveCode: String?
    let outcome: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let statusId: Int?
    let responsibleTeam: String?
    let syncPriority: SyncPriority?

    enum CodingKeys: String, CodingKey {
        case kpi, outcome
        case objectiveCode = "objective_code"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case syncPriority = "sync_priority"
    }
}

// MARK: - Request Payloads

struct StrategicGoalCreateRequest: Codable {
    let goal: NewStrategicGoal
    let auth: AuthContextPayload
}

struct DocumentData: Codable {
    let fileData: String
    let filename: String
    let linkedField: String?
    
    enum CodingKeys: String, CodingKey {
        case filename
        case fileData = "file_data"
        case linkedField = "linked_field"
    }
}

struct StrategicGoalCreateWithDocumentsRequest: Codable {
    let goal: NewStrategicGoal
    let documents: [DocumentData]
    let documentTypeId: String
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case goal, documents, auth
        case documentTypeId = "document_type_id"
    }
}

struct StrategicGoalGetRequest: Codable {
    let id: String
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
}

struct StrategicGoalListRequest: Codable {
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
}

struct StrategicGoalUpdateRequest: Codable {
    let id: String
    let update: UpdateStrategicGoal
    let auth: AuthContextPayload
}

struct StrategicGoalDeleteRequest: Codable {
    let id: String
    let hardDelete: Bool?
    let auth: AuthContextPayload

    enum CodingKeys: String, CodingKey {
        case id, auth
        case hardDelete = "hard_delete"
    }
}

struct UploadDocumentRequest: Codable {
    let goalId: String
    let fileData: String
    let originalFilename: String
    let title: String?
    let documentTypeId: String
    let linkedField: String?
    let syncPriority: SyncPriority
    let compressionPriority: CompressionPriority?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case title, auth
        case goalId = "goal_id"
        case fileData = "file_data"
        case originalFilename = "original_filename"
        case documentTypeId = "document_type_id"
        case linkedField = "linked_field"
        case syncPriority = "sync_priority"
        case compressionPriority = "compression_priority"
    }
}

struct BulkUploadDocumentsRequest: Codable {
    struct File: Codable {
        let fileData: String
        let filename: String
        
        enum CodingKeys: String, CodingKey {
            case filename
            case fileData = "file_data"
        }
    }
    let goalId: String
    let files: [File]
    let title: String?
    let documentTypeId: String
    let syncPriority: SyncPriority
    let compressionPriority: CompressionPriority?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case files, title, auth
        case goalId = "goal_id"
        case documentTypeId = "document_type_id"
        case syncPriority = "sync_priority"
        case compressionPriority = "compression_priority"
    }
}

struct FindByStatusRequest: Codable {
    let statusId: Int
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case pagination, include, auth
        case statusId = "status_id"
    }
}

struct FindByTeamRequest: Codable {
    let teamName: String
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case pagination, include, auth
        case teamName = "team_name"
    }
}

struct FindByUserRoleRequest: Codable {
    let userId: String
    let role: UserGoalRole
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case role, pagination, include, auth
        case userId = "user_id"
    }
}

struct FindStaleRequest: Codable {
    let daysStale: Int
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case pagination, include, auth
        case daysStale = "days_stale"
    }
}

struct FindByDateRangeRequest: Codable {
    let startDate: String
    let endDate: String
    let pagination: PaginationDto?
    let include: [StrategicGoalInclude]?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case pagination, include, auth
        case startDate = "start_date"
        case endDate = "end_date"
    }
}

struct StatsRequest: Codable {
    let auth: AuthContextPayload
}

// MARK: - Response Payloads

struct StrategicGoalResponse: Codable, Identifiable {
    let id: String
    let objectiveCode: String
    let outcome: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let progressPercentage: Double?
    let statusId: Int?
    let responsibleTeam: String?
    let createdAt: String
    let updatedAt: String
    let syncPriority: SyncPriority
    
    enum CodingKeys: String, CodingKey {
        case id, outcome, kpi
        case objectiveCode = "objective_code"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case progressPercentage = "progress_percentage"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case syncPriority = "sync_priority"
    }
}

struct CreateWithDocumentsResponse: Codable {
    let goal: StrategicGoalResponse
    // Define a simplified document result for now
    // let documentResults: [Result<MediaDocumentResponse, String>]
}

struct DeleteResponse: Codable {
    let deleted: Bool
    let message: String
}

struct StatusDistributionResponse: Codable {
    let onTrack: Int
    let atRisk: Int
    let behind: Int
    let completed: Int
    
    enum CodingKeys: String, CodingKey {
        case onTrack = "On Track"
        case atRisk = "At Risk"
        case behind = "Behind"
        case completed = "Completed"
    }
}

struct GoalValueSummaryResponse: Codable {
    let avgTarget: Double?
    let avgActual: Double?
    let totalTarget: Double?
    let totalActual: Double?
    let count: Int
    let avgProgressPercentage: Double?
    
    enum CodingKeys: String, CodingKey {
        case count
        case avgTarget = "avg_target"
        case avgActual = "avg_actual"
        case totalTarget = "total_target"
        case totalActual = "total_actual"
        case avgProgressPercentage = "avg_progress_percentage"
    }
} 