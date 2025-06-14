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

// MARK: - iOS-Specific Models

/// iOS thermal states matching ProcessInfo.ThermalState
enum IOSThermalState: Int, Codable {
    case nominal = 0
    case fair = 1
    case serious = 2
    case critical = 3
}

/// iOS app states
enum IOSAppState: String, Codable {
    case active = "active"
    case background = "background"
    case inactive = "inactive"
}

/// iOS device types for optimization
enum IOSDeviceType: String, Codable {
    case iPhone = "IPhone"
    case iPad = "IPad"
    case iPadPro = "IPadPro"
}

/// iOS device state payload
struct IOSStateUpdate: Codable {
    let batteryLevel: Float
    let isCharging: Bool
    let thermalState: Int
    let appState: String
    let availableMemoryMB: UInt64?
    
    enum CodingKeys: String, CodingKey {
        case batteryLevel = "battery_level"
        case isCharging = "is_charging"
        case thermalState = "thermal_state"
        case appState = "app_state"
        case availableMemoryMB = "available_memory_mb"
    }
}

/// Background task extension payload
struct BackgroundTaskExtension: Codable {
    let grantedSeconds: UInt32
    
    enum CodingKeys: String, CodingKey {
        case grantedSeconds = "granted_seconds"
    }
}

/// Content visibility payload
struct ContentVisibility: Codable {
    let isVisible: Bool
    
    enum CodingKeys: String, CodingKey {
        case isVisible = "is_visible"
    }
}

/// App lifecycle event payload
struct AppLifecycleEvent: Codable {
    let event: String
}

/// Enhanced memory warning payload
struct EnhancedMemoryWarning: Codable {
    let level: UInt8
    let availableMemoryMB: UInt64?
    let pressureTrend: String?
    
    enum CodingKeys: String, CodingKey {
        case level
        case availableMemoryMB = "available_memory_mb"
        case pressureTrend = "pressure_trend"
    }
}

/// iOS device capabilities response
struct IOSDeviceCapabilities: Codable {
    let deviceType: String
    let maxConcurrentJobs: Int
    let memoryLimitMB: Int
    let thermalThrottleThreshold: Float
    let batteryLevelThreshold: Float
    let safeConcurrency: Int
    
    enum CodingKeys: String, CodingKey {
        case deviceType = "device_type"
        case maxConcurrentJobs = "max_concurrent_jobs"
        case memoryLimitMB = "memory_limit_mb"
        case thermalThrottleThreshold = "thermal_throttle_threshold"
        case batteryLevelThreshold = "battery_level_threshold"
        case safeConcurrency = "safe_concurrency"
    }
}

/// iOS optimizations configuration
struct IOSOptimizations: Codable {
    let backgroundProcessingLimit: Int
    let minBatteryLevel: Float
    let maxMemoryUsageMB: UInt64
    let respectLowPowerMode: Bool
    let pauseOnCriticalThermal: Bool
    let reduceQualityOnThermal: Bool
    
    enum CodingKeys: String, CodingKey {
        case backgroundProcessingLimit = "background_processing_limit"
        case minBatteryLevel = "min_battery_level"
        case maxMemoryUsageMB = "max_memory_usage_mb"
        case respectLowPowerMode = "respect_low_power_mode"
        case pauseOnCriticalThermal = "pause_on_critical_thermal"
        case reduceQualityOnThermal = "reduce_quality_on_thermal"
    }
}

/// iOS device state
struct IOSDeviceState: Codable {
    let batteryLevel: Float
    let isCharging: Bool
    let thermalState: IOSThermalState
    let appState: IOSAppState
    let availableMemoryMB: UInt64?
    let lastUpdated: String
    
    enum CodingKeys: String, CodingKey {
        case batteryLevel = "battery_level"
        case isCharging = "is_charging"
        case thermalState = "thermal_state"
        case appState = "app_state"
        case availableMemoryMB = "available_memory_mb"
        case lastUpdated = "last_updated"
    }
}

/// iOS worker status response
struct IOSWorkerStatus: Codable {
    let activeJobs: Int
    let maxConcurrentJobs: Int
    let effectiveMaxJobs: Int
    let queuePollIntervalMs: UInt64
    let runningDocumentIds: [String]
    let iosState: IOSDeviceState
    let isThrottled: Bool
    let throttleReason: String?
    
    enum CodingKeys: String, CodingKey {
        case activeJobs = "active_jobs"
        case maxConcurrentJobs = "max_concurrent_jobs"
        case effectiveMaxJobs = "effective_max_jobs"
        case queuePollIntervalMs = "queue_poll_interval_ms"
        case runningDocumentIds = "running_document_ids"
        case iosState = "ios_state"
        case isThrottled = "is_throttled"
        case throttleReason = "throttle_reason"
    }
}

/// Comprehensive iOS status response
struct ComprehensiveIOSStatus: Codable {
    let iosWorkerStatus: IOSWorkerStatus
    let systemInfo: SystemInfo
    
    enum CodingKeys: String, CodingKey {
        case iosWorkerStatus = "ios_worker_status"
        case systemInfo = "system_info"
    }
    
    struct SystemInfo: Codable {
        let rustVersion: String
        let timestamp: String
        let featureFlags: FeatureFlags
        
        enum CodingKeys: String, CodingKey {
            case rustVersion = "rust_version"
            case timestamp
            case featureFlags = "feature_flags"
        }
        
        struct FeatureFlags: Codable {
            let iosIntegration: Bool
            let backgroundProcessing: Bool
            let memoryPressureHandling: Bool
            let thermalManagement: Bool
            let batteryOptimization: Bool
            let contentVisibilityTracking: Bool
            let appLifecycleHandling: Bool
            
            enum CodingKeys: String, CodingKey {
                case iosIntegration = "ios_integration"
                case backgroundProcessing = "background_processing"
                case memoryPressureHandling = "memory_pressure_handling"
                case thermalManagement = "thermal_management"
                case batteryOptimization = "battery_optimization"
                case contentVisibilityTracking = "content_visibility_tracking"
                case appLifecycleHandling = "app_lifecycle_handling"
            }
        }
    }
}

/// Device capability detection response
struct DeviceCapabilityDetectionResponse: Codable {
    let status: String
    let detectedCapabilities: IOSDeviceCapabilities
    let appliedOptimizations: IOSOptimizations
    let recommendations: [String]
    
    enum CodingKeys: String, CodingKey {
        case status
        case detectedCapabilities = "detected_capabilities"
        case appliedOptimizations = "applied_optimizations"
        case recommendations
    }
} 