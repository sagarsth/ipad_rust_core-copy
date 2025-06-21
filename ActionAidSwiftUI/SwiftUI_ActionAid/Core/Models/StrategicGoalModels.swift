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
    case low = "low"
    case normal = "normal"
    case high = "high"
}


// MARK: - Core DTOs

struct NewStrategicGoal: Codable {
    var id: UUID?
    let objectiveCode: String
    let outcome: String?
    let kpi: String?
    let targetValue: Double?
    let actualValue: Double?
    let statusId: Int64?
    let responsibleTeam: String?
    let syncPriority: SyncPriority
    let createdByUserId: UUID?

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
}

// MARK: - Bulk Delete Models

struct StrategicGoalBulkDeleteRequest: Codable {
    let ids: [String]
    let hardDelete: Bool?
    let force: Bool?
    let auth: AuthContextPayload
}

struct BatchDeleteResult: Codable {
    let hardDeleted: [String]
    let softDeleted: [String]
    let failed: [String]
    let dependencies: [String: [String]]
    let errors: [String: String]
}

// MARK: - Failed Delete Detail

struct FailedDeleteDetail: Codable {
    let id: String
    let entityData: StrategicGoalResponse?
    let entityType: String
    let reason: FailureReason
    let dependencies: [String]
}

enum FailureReason: String, Codable, CaseIterable {
    case dependenciesPrevented = "DependenciesPrevented"
    case softDeletedDueToDependencies = "SoftDeletedDueToDependencies"
    case notFound = "NotFound"
    case authorizationFailed = "AuthorizationFailed"
    case databaseError = "DatabaseError"
    case unknown = "Unknown"
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

// MARK: - iOS Optimized Path-Based Upload Requests (NO BASE64!)

struct UploadDocumentFromPathRequest: Codable {
    let goalId: String
    let filePath: String               // NO BASE64! Just the file path
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
        case filePath = "file_path"      // Changed from file_data to file_path
        case originalFilename = "original_filename"
        case documentTypeId = "document_type_id"
        case linkedField = "linked_field"
        case syncPriority = "sync_priority"
        case compressionPriority = "compression_priority"
    }
}

struct BulkUploadDocumentsFromPathsRequest: Codable {
    struct FilePath: Codable {
        let filePath: String             // NO BASE64! Just the file path
        let filename: String
        
        enum CodingKeys: String, CodingKey {
            case filename
            case filePath = "file_path"  // Changed from file_data to file_path
        }
    }
    let goalId: String
    let filePaths: [FilePath]            // Array of paths instead of data
    let title: String?
    let documentTypeId: String
    let syncPriority: SyncPriority
    let compressionPriority: CompressionPriority?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case title, auth
        case goalId = "goal_id"
        case filePaths = "file_paths"    // Changed from files to file_paths
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

struct StrategicGoalResponse: Codable, Identifiable, MonthGroupable, Equatable {
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
    let createdByUserId: String?
    let updatedByUserId: String?
    let lastSyncedAt: String?
    let createdByUsername: String?
    let updatedByUsername: String?
    let documentUploadErrors: [String]?
    let projectCount: Int?
    let documentCounts: Int?
    
    enum CodingKeys: String, CodingKey {
        case id, kpi, outcome
        case objectiveCode = "objective_code"
        case targetValue = "target_value"
        case actualValue = "actual_value"
        case progressPercentage = "progress_percentage"
        case statusId = "status_id"
        case responsibleTeam = "responsible_team"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case syncPriority = "sync_priority"
        case createdByUserId = "created_by_user_id"
        case updatedByUserId = "updated_by_user_id"
        case lastSyncedAt = "last_synced_at"
        case createdByUsername = "created_by_username"
        case updatedByUsername = "updated_by_username"
        case documentUploadErrors = "document_upload_errors"
        case projectCount = "project_count"
        case documentCounts = "document_counts"
    }

    var displayLastSyncedAt: String {
        guard let syncedAt = lastSyncedAt, let date = ISO8601DateFormatter().date(from: syncedAt) else {
            return "Not synced yet"
        }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
    
    var hasDocuments: Bool {
        return (documentCounts ?? 0) > 0
    }
}

struct CreateWithDocumentsResponse: Codable {
    let goal: StrategicGoalResponse
    // Define a simplified document result for now
    // let documentResults: [Result<MediaDocumentResponse, String>]
}

// Updated to handle actual Rust DeleteResult enum
enum DeleteResponse: Codable {
    case hardDeleted
    case softDeleted(dependencies: [String])
    case dependenciesPrevented(dependencies: [String])
    
    enum CodingKeys: String, CodingKey {
        case hardDeleted = "HardDeleted"
        case softDeleted = "SoftDeleted"
        case dependenciesPrevented = "DependenciesPrevented"
    }
    
    init(from decoder: Decoder) throws {
        // First try to decode as a simple string (for HardDeleted)
        if let stringValue = try? decoder.singleValueContainer().decode(String.self) {
            switch stringValue {
            case "HardDeleted":
                self = .hardDeleted
                return
            default:
                throw DecodingError.dataCorruptedError(
                    in: try decoder.singleValueContainer(), 
                    debugDescription: "Unknown simple DeleteResult: \(stringValue)"
                )
            }
        }
        
        // Then try to decode as a keyed container (for SoftDeleted/DependenciesPrevented)
        let container = try decoder.container(keyedBy: CodingKeys.self)
        
        if let softDeletedData = try? container.decode([String: [String]].self, forKey: .softDeleted) {
            self = .softDeleted(dependencies: softDeletedData["dependencies"] ?? [])
        } else if let preventedData = try? container.decode([String: [String]].self, forKey: .dependenciesPrevented) {
            self = .dependenciesPrevented(dependencies: preventedData["dependencies"] ?? [])
        } else {
            throw DecodingError.dataCorruptedError(
                forKey: .hardDeleted, 
                in: container, 
                debugDescription: "Unable to decode DeleteResult"
            )
        }
    }
    
    func encode(to encoder: Encoder) throws {
        switch self {
        case .hardDeleted:
            var container = encoder.singleValueContainer()
            try container.encode("HardDeleted")
        case .softDeleted(let dependencies):
            var container = encoder.container(keyedBy: CodingKeys.self)
            try container.encode(["dependencies": dependencies], forKey: .softDeleted)
        case .dependenciesPrevented(let dependencies):
            var container = encoder.container(keyedBy: CodingKeys.self)
            try container.encode(["dependencies": dependencies], forKey: .dependenciesPrevented)
        }
    }
    
    // Helper properties for UI
    var wasDeleted: Bool {
        switch self {
        case .hardDeleted, .softDeleted: return true
        case .dependenciesPrevented: return false
        }
    }
    
    var isHardDeleted: Bool {
        if case .hardDeleted = self { return true }
        return false
    }
    
    var displayMessage: String {
        switch self {
        case .hardDeleted:
            return "Goal permanently deleted successfully"
        case .softDeleted(let dependencies):
            if dependencies.isEmpty {
                return "Goal archived successfully"
            } else {
                return "Goal archived due to dependencies: \(dependencies.joined(separator: ", "))"
            }
        case .dependenciesPrevented(let dependencies):
            return "Could not delete goal due to dependencies: \(dependencies.joined(separator: ", "))"
        }
    }
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

struct ValueStatisticsResponse: Codable {
    let avgTarget: Double?
    let avgActual: Double?
    let totalTarget: Double?
    let totalActual: Double?
    let count: Int64
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

// MARK: - Filter Models for Bulk Selection

struct StrategicGoalFilter: Codable {
    let statusIds: [Int64]?
    let responsibleTeams: [String]?
    let years: [Int32]?
    let months: [Int32]? // 1-12
    let userRole: UserRoleFilter?
    let syncPriorities: [String]?
    let searchText: String?
    let progressRange: [Double]? // [min, max]
    let targetValueRange: [Double]?
    let actualValueRange: [Double]?
    let dateRange: [String]? // [start_rfc3339, end_rfc3339]
    let daysStale: UInt32?
    let excludeDeleted: Bool?
    
    enum CodingKeys: String, CodingKey {
        case statusIds = "status_ids"
        case responsibleTeams = "responsible_teams"
        case years
        case months
        case userRole = "user_role"
        case syncPriorities = "sync_priorities"
        case searchText = "search_text"
        case progressRange = "progress_range"
        case targetValueRange = "target_value_range"
        case actualValueRange = "actual_value_range"
        case dateRange = "date_range"
        case daysStale = "days_stale"
        case excludeDeleted = "exclude_deleted"
    }
    
    // Convenience initializers
    static func all() -> StrategicGoalFilter {
        StrategicGoalFilter(
            statusIds: nil,
            responsibleTeams: nil,
            years: nil,
            months: nil,
            userRole: nil,
            syncPriorities: nil,
            searchText: nil,
            progressRange: nil,
            targetValueRange: nil,
            actualValueRange: nil,
            dateRange: nil,
            daysStale: nil,
            excludeDeleted: true
        )
    }
    
    static func byStatus(_ statusIds: [Int64]) -> StrategicGoalFilter {
        let filter = StrategicGoalFilter.all()
        return StrategicGoalFilter(
            statusIds: statusIds,
            responsibleTeams: filter.responsibleTeams,
            years: filter.years,
            months: filter.months,
            userRole: filter.userRole,
            syncPriorities: filter.syncPriorities,
            searchText: filter.searchText,
            progressRange: filter.progressRange,
            targetValueRange: filter.targetValueRange,
            actualValueRange: filter.actualValueRange,
            dateRange: filter.dateRange,
            daysStale: filter.daysStale,
            excludeDeleted: filter.excludeDeleted
        )
    }
    
    static func byDateParts(years: [Int32]?, months: [Int32]?) -> StrategicGoalFilter {
        let filter = StrategicGoalFilter.all()
        return StrategicGoalFilter(
            statusIds: filter.statusIds,
            responsibleTeams: filter.responsibleTeams,
            years: years,
            months: months,
            userRole: filter.userRole,
            syncPriorities: filter.syncPriorities,
            searchText: filter.searchText,
            progressRange: filter.progressRange,
            targetValueRange: filter.targetValueRange,
            actualValueRange: filter.actualValueRange,
            dateRange: filter.dateRange,
            daysStale: filter.daysStale,
            excludeDeleted: filter.excludeDeleted
        )
    }
    
    // Combine multiple filters with AND logic
    func combined(with other: StrategicGoalFilter) -> StrategicGoalFilter {
        StrategicGoalFilter(
            statusIds: other.statusIds ?? self.statusIds,
            responsibleTeams: other.responsibleTeams ?? self.responsibleTeams,
            years: other.years ?? self.years,
            months: other.months ?? self.months,
            userRole: other.userRole ?? self.userRole,
            syncPriorities: other.syncPriorities ?? self.syncPriorities,
            searchText: other.searchText ?? self.searchText,
            progressRange: other.progressRange ?? self.progressRange,
            targetValueRange: other.targetValueRange ?? self.targetValueRange,
            actualValueRange: other.actualValueRange ?? self.actualValueRange,
            dateRange: other.dateRange ?? self.dateRange,
            daysStale: other.daysStale ?? self.daysStale,
            excludeDeleted: other.excludeDeleted ?? self.excludeDeleted
        )
    }
    
    // Check if filter has any constraints
    var isEmpty: Bool {
        statusIds == nil &&
        responsibleTeams == nil &&
        years == nil &&
        months == nil &&
        userRole == nil &&
        syncPriorities == nil &&
        (searchText?.isEmpty ?? true) &&
        progressRange == nil &&
        targetValueRange == nil &&
        actualValueRange == nil &&
        dateRange == nil &&
        daysStale == nil
    }
}

struct UserRoleFilter: Codable {
    let userId: String
    let role: String // "created" or "updated"
    
    enum CodingKeys: String, CodingKey {
        case userId = "user_id"
        case role
    }
}

// MARK: - Filter Request Payloads

struct StrategicGoalFilterRequest: Codable {
    let filter: StrategicGoalFilter
    let auth: AuthContextPayload
}

// MARK: - Export Models

struct StrategicGoalExportOptions: Codable {
    let includeBlobs: Bool
    let targetPath: String?
    let filter: StrategicGoalFilter
    let format: ExportFormat
    
    enum CodingKeys: String, CodingKey {
        case includeBlobs = "include_blobs"
        case targetPath = "target_path" 
        case filter
        case format
    }
}

struct StrategicGoalExportByIdsOptions: Codable {
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

struct ExportJobResponse: Codable {
    let job: ExportJob
}

struct ExportJob: Codable {
    let id: String
    let requestedByUserId: String?
    let requestedAt: String
    let includeBlobs: Bool
    let status: String
    let localPath: String?
    let totalEntities: Int64?
    let totalBytes: Int64?
    let errorMessage: String?
    
    enum CodingKeys: String, CodingKey {
        case id
        case requestedByUserId = "requested_by_user_id"
        case requestedAt = "requested_at"
        case includeBlobs = "include_blobs"
        case status
        case localPath = "local_path"
        case totalEntities = "total_entities"
        case totalBytes = "total_bytes"
        case errorMessage = "error_message"
    }
}

enum ExportStatus: String, Codable, CaseIterable {
    case pending = "Pending"
    case running = "Running" 
    case completed = "Completed"
    case failed = "Failed"
    
    var displayName: String {
        switch self {
        case .pending: return "Pending"
        case .running: return "Running"
        case .completed: return "Completed"
        case .failed: return "Failed"
        }
    }
    
    var isCompleted: Bool {
        self == .completed
    }
    
    var isFailed: Bool {
        self == .failed
    }
    
    var isInProgress: Bool {
        self == .pending || self == .running
    }
}

// MARK: - Export Format Support

/// Export formats supported by the system
enum ExportFormat: Codable, CaseIterable, Identifiable {
    case jsonLines
    case csv(CsvOptions)
    case parquet(ParquetOptions)
    
    var id: String {
        switch self {
        case .jsonLines: return "jsonl"
        case .csv: return "csv" 
        case .parquet: return "parquet"
        }
    }
    
    var displayName: String {
        switch self {
        case .jsonLines: return "JSON Lines"
        case .csv: return "CSV"
        case .parquet: return "Parquet"
        }
    }
    
    var fileExtension: String {
        switch self {
        case .jsonLines: return "jsonl"
        case .csv(let options): return options.compress ? "csv.gz" : "csv"
        case .parquet: return "parquet"
        }
    }
    
    var description: String {
        switch self {
        case .jsonLines: return "Lightweight format, best for data processing"
        case .csv(let options): return options.compress ? "Compressed CSV (smaller file, requires decompression)" : "Universal CSV format, opens in Excel/Sheets"
        case .parquet: return "Compressed format, optimal for large datasets"
        }
    }
    
    var icon: String {
        switch self {
        case .jsonLines: return "curlybraces"
        case .csv: return "tablecells"
        case .parquet: return "arrow.down.circle.fill"
        }
    }
    
    var isRecommendedForLargeDatasets: Bool {
        switch self {
        case .parquet: return true
        case .csv(let options): return options.compress
        case .jsonLines: return false
        }
    }
    
    // Static list for CaseIterable conformance
    static var allCases: [ExportFormat] {
        [
            .jsonLines,
            .csv(CsvOptions.default),
            .parquet(ParquetOptions.default)
        ]
    }
    
    // Default format
    static var `default`: ExportFormat {
        .jsonLines
    }
    
    // Recommended format based on selection size
    static func recommended(for itemCount: Int) -> ExportFormat {
        if itemCount > 1000 {
            return .parquet(ParquetOptions.optimized)
        } else if itemCount > 50 {
            return .csv(CsvOptions.default) // Use uncompressed CSV for better compatibility
        } else {
            return .jsonLines
        }
    }
}

/// CSV export options
struct CsvOptions: Codable {
    let delimiter: UInt8
    let quoteChar: UInt8
    let escapeChar: UInt8?
    let compress: Bool
    
    static let `default` = CsvOptions(
        delimiter: 44, // comma
        quoteChar: 34, // double quote
        escapeChar: nil,
        compress: false
    )
    
    static let compressed = CsvOptions(
        delimiter: 44,
        quoteChar: 34,
        escapeChar: nil,
        compress: true
    )
    
    enum CodingKeys: String, CodingKey {
        case delimiter, quoteChar = "quote_char", escapeChar = "escape_char", compress
    }
}

/// Parquet compression types
enum ParquetCompression: String, Codable, CaseIterable {
    case none = "None"
    case snappy = "Snappy"
    case gzip = "Gzip"
    case lzo = "Lzo"
    case brotli = "Brotli"
    case lz4 = "Lz4"
    case zstd = "Zstd"
    
    var displayName: String {
        switch self {
        case .none: return "No Compression"
        case .snappy: return "Snappy (Fast)"
        case .gzip: return "GZip (Balanced)"
        case .lzo: return "LZO (Fast)"
        case .brotli: return "Brotli (Best Compression)"
        case .lz4: return "LZ4 (Fastest)"
        case .zstd: return "ZStandard (Modern)"
        }
    }
    
    var isRecommended: Bool {
        switch self {
        case .snappy, .zstd: return true
        default: return false
        }
    }
}

/// Parquet export options
struct ParquetOptions: Codable {
    let compression: ParquetCompression
    let rowGroupSize: Int
    let enableStatistics: Bool
    
    static let `default` = ParquetOptions(
        compression: .snappy,
        rowGroupSize: 10000,
        enableStatistics: true
    )
    
    static let optimized = ParquetOptions(
        compression: .zstd,
        rowGroupSize: 50000,
        enableStatistics: true
    )
    
    static let fast = ParquetOptions(
        compression: .lz4,
        rowGroupSize: 5000,
        enableStatistics: false
    )
    
    enum CodingKeys: String, CodingKey {
        case compression, rowGroupSize = "row_group_size", enableStatistics = "enable_statistics"
    }
} 