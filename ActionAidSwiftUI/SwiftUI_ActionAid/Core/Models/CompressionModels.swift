//
//  CompressionModels.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

// MARK: - Enums

/// Represents the priority of a compression task.
enum CompressionPriority: String, Codable {
    case high = "HIGH"
    case normal = "NORMAL"
    case low = "LOW"
    case background = "BACKGROUND"
}

/// Represents the compression method to be used.
enum CompressionMethod: String, Codable {
    case lossless = "Lossless"
    case lossy = "Lossy"
    case pdfOptimize = "PdfOptimize"
    case officeOptimize = "OfficeOptimize"
    case none = "None"
}

/// Represents the status of a document's compression.
enum CompressionStatus: String, Codable {
    case pending
    case inProgress
    case completed
    case skipped
    case failed
}

// MARK: - Configuration

/// Configuration for a compression task.
struct CompressionConfig: Codable {
    let method: CompressionMethod
    let qualityLevel: Int
    let minSizeBytes: Int64

    enum CodingKeys: String, CodingKey {
        case method
        case qualityLevel = "quality_level"
        case minSizeBytes = "min_size_bytes"
    }
}

// MARK: - Request Payloads

struct CompressDocumentRequest: Codable {
    let documentId: String
    let config: CompressionConfig?

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
        case config
    }
}

struct QueueDocumentRequest: Codable {
    let documentId: String
    let priority: CompressionPriority

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
        case priority
    }
}

struct DocumentIdRequest: Codable {
    let documentId: String

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
    }
}

struct UpdatePriorityRequest: Codable {
    let documentId: String
    let priority: CompressionPriority

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
        case priority
    }
}

struct BulkUpdatePriorityRequest: Codable {
    let documentIds: [String]
    let priority: CompressionPriority

    enum CodingKeys: String, CodingKey {
        case documentIds = "document_ids"
        case priority
    }
}

struct GetQueueEntriesRequest: Codable {
    let status: String?
    let limit: Int?
    let offset: Int?
}

struct ValidateConfigRequest: Codable {
    let config: CompressionConfig
}

struct GetSupportedMethodsRequest: Codable {
    let mimeType: String
    let fileExtension: String?

    enum CodingKeys: String, CodingKey {
        case mimeType = "mime_type"
        case fileExtension = "file_extension"
    }
}

struct ResetStuckJobsRequest: Codable {
    let timeoutMinutes: Int
    let auth: AuthContextPayload

    enum CodingKeys: String, CodingKey {
        case timeoutMinutes = "timeout_minutes"
        case auth
    }
}

// MARK: - Response Payloads

struct CompressionResultResponse: Codable {
    let documentId: String
    let originalSize: Int64
    let compressedSize: Int64
    let compressedFilePath: String
    let spaceSavedBytes: Int64
    let spaceSavedPercentage: Double
    let methodUsed: CompressionMethod
    let qualityLevel: Int
    let durationMs: Int64

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
        case originalSize = "original_size"
        case compressedSize = "compressed_size"
        case compressedFilePath = "compressed_file_path"
        case spaceSavedBytes = "space_saved_bytes"
        case spaceSavedPercentage = "space_saved_percentage"
        case methodUsed = "method_used"
        case qualityLevel = "quality_level"
        case durationMs = "duration_ms"
    }
}

struct CompressionQueueStatusResponse: Codable {
    let pending: Int
    let inProgress: Int
    let completed: Int
    let failed: Int
    let total: Int
    let totalSizePending: Int64
    let totalSizeCompleted: Int64

    enum CodingKeys: String, CodingKey {
        case pending
        case inProgress = "in_progress"
        case completed
        case failed
        case total
        case totalSizePending = "total_size_pending"
        case totalSizeCompleted = "total_size_completed"
    }
}

struct CancelResponse: Codable {
    let cancelled: Bool
}

struct CompressionStatsResponse: Codable {
    let totalFilesCompressed: Int
    let totalBytesSaved: Int64
    let totalOriginalBytes: Int64
    let totalCompressedBytes: Int64
    let averageCompressionRatio: Double
    let averageDurationMs: Double

    enum CodingKeys: String, CodingKey {
        case totalFilesCompressed = "total_files_compressed"
        case totalBytesSaved = "total_bytes_saved"
        case totalOriginalBytes = "total_original_bytes"
        case totalCompressedBytes = "total_compressed_bytes"
        case averageCompressionRatio = "average_compression_ratio"
        case averageDurationMs = "average_duration_ms"
    }
}

struct UpdatePriorityResponse: Codable {
    let updated: Bool
}

struct BulkUpdatePriorityResponse: Codable {
    let updatedCount: Int

    enum CodingKeys: String, CodingKey {
        case updatedCount = "updated_count"
    }
}

struct IsDocumentInUseResponse: Codable {
    let inUse: Bool
    
    enum CodingKeys: String, CodingKey {
        case inUse = "in_use"
    }
}

struct ValidateConfigResponse: Codable {
    let valid: Bool
    let errors: [String]?
}

struct RetryFailedResponse: Codable {
    let queued: Bool
}

struct RetryAllFailedResponse: Codable {
    let queuedCount: Int

    enum CodingKeys: String, CodingKey {
        case queuedCount = "queued_count"
    }
}

struct SupportedMethod: Codable {
    let method: CompressionMethod
    let recommended: Bool
    let qualityRange: [Int]?
    let defaultQuality: Int?

    enum CodingKeys: String, CodingKey {
        case method
        case recommended
        case qualityRange = "quality_range"
        case defaultQuality = "default_quality"
    }
}

struct GetSupportedMethodsResponse: Codable {
    let mimeType: String
    let fileExtension: String?
    let methods: [SupportedMethod]

    enum CodingKeys: String, CodingKey {
        case mimeType = "mime_type"
        case fileExtension = "file_extension"
        case methods
    }
}

struct DocumentHistoryResponse: Codable {
    let documentId: String
    let currentStatus: String?
    let originalSize: Int64?
    let compressedSize: Int64?
    let spaceSaved: Int64?
    let compressedPath: String?
    let lastUpdated: String?
    let queueStatus: String?
    let attempts: Int?
    let errorMessage: String?
    let queuedAt: String?
    let queueUpdatedAt: String?

    enum CodingKeys: String, CodingKey {
        case documentId = "document_id"
        case currentStatus = "current_status"
        case originalSize = "original_size"
        case compressedSize = "compressed_size"
        case spaceSaved = "space_saved"
        case compressedPath = "compressed_path"
        case lastUpdated = "last_updated"
        case queueStatus = "queue_status"
        case attempts
        case errorMessage = "error_message"
        case queuedAt = "queued_at"
        case queueUpdatedAt = "queue_updated_at"
    }
}

struct ResetStuckJobsResponse: Codable {
    let resetCount: Int
    let status: String
    let message: String

    enum CodingKeys: String, CodingKey {
        case resetCount = "reset_count"
        case status
        case message
    }
}

struct ComprehensiveResetRequest: Codable {
    let timeoutMinutes: Int?
    let auth: AuthContextPayload
    
    enum CodingKeys: String, CodingKey {
        case timeoutMinutes = "timeout_minutes"
        case auth
    }
}

struct ComprehensiveResetResponse: Codable {
    let resetCount: Int
    let issuesFound: [String]
    let recommendations: [String]
    let status: String
    
    enum CodingKeys: String, CodingKey {
        case resetCount = "reset_count"
        case issuesFound = "issues_found"
        case recommendations
        case status
    }
} 