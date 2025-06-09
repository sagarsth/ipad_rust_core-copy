//
//  DocumentModels.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

// MARK: - General Payloads & DTOs

/// A generic authentication context DTO to be included in FFI request payloads.
struct AuthCtxDto: Codable {
    let userId: String
    let role: String
    let deviceId: String
    let offlineMode: Bool
    
    enum CodingKeys: String, CodingKey {
        case userId = "user_id"
        case role
        case deviceId = "device_id"
        case offlineMode = "offline_mode"
    }
}

/// DTO for pagination parameters.
struct PaginationDto: Codable {
    let page: UInt32?
    let perPage: UInt32?
    
    enum CodingKeys: String, CodingKey {
        case page
        case perPage = "per_page"
    }
}

/// Generic paginated result wrapper containing data and pagination metadata.
struct PaginatedResult<T: Codable>: Codable {
    let data: [T]
    let pagination: PaginationInfo?
    
    struct PaginationInfo: Codable {
        let currentPage: UInt32
        let perPage: UInt32
        let totalPages: UInt32?
        let totalCount: UInt64?
        
        enum CodingKeys: String, CodingKey {
            case currentPage = "current_page"
            case perPage = "per_page" 
            case totalPages = "total_pages"
            case totalCount = "total_count"
        }
    }
}

// MARK: - Document Type Models

/// DTO for creating a new document type.
struct NewDocumentType: Codable {
    let name: String
    let description: String?
    let defaultPriority: String
    
    enum CodingKeys: String, CodingKey {
        case name, description
        case defaultPriority = "default_priority"
    }
}

/// DTO for updating an existing document type.
struct UpdateDocumentType: Codable {
    let name: String?
    let description: String?
    let defaultPriority: String?
    
    enum CodingKeys: String, CodingKey {
        case name, description
        case defaultPriority = "default_priority"
    }
}

/// A struct representing the response for a document type.
struct DocumentTypeResponse: Codable, Identifiable {
    let id: String
    let name: String
    let description: String?
    let defaultPriority: String
    let createdAt: String
    let updatedAt: String
    
    enum CodingKeys: String, CodingKey {
        case id, name, description
        case defaultPriority = "default_priority"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}


// MARK: - Media Document Models

/// A comprehensive response object for a media document.
struct MediaDocumentResponse: Codable, Identifiable {
    let id: String
    let relatedTable: String
    let relatedId: String?
    let typeId: String
    var typeName: String?
    let originalFilename: String
    let title: String?
    let mimeType: String
    let sizeBytes: Int64
    let filePath: String
    let compressionStatus: String
    let blobStatus: String
    let blobKey: String?
    let compressedFilePath: String?
    let compressedSizeBytes: Int64?
    let createdAt: String
    let updatedAt: String
    var isAvailableLocally: Bool?
    var hasError: Bool { filePath == "ERROR" }
    
    // Optional included data
    var versions: [DocumentVersion]?
    var access_logs: PaginatedResult<DocumentAccessLog>?

    enum CodingKeys: String, CodingKey {
        case id
        case relatedTable = "related_table"
        case relatedId = "related_id"
        case typeId = "type_id"
        case typeName = "type_name"
        case originalFilename = "original_filename"
        case title
        case mimeType = "mime_type"
        case sizeBytes = "size_bytes"
        case filePath = "file_path"
        case compressionStatus = "compression_status"
        case blobStatus = "blob_status"
        case blobKey = "blob_key"
        case compressedFilePath = "compressed_file_path"
        case compressedSizeBytes = "compressed_size_bytes"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case isAvailableLocally = "is_available_locally"
        case versions
        case access_logs
    }
}

/// Represents a single version of a document.
struct DocumentVersion: Codable, Identifiable {
    let id: String
    let documentId: String
    let versionNumber: Int32
    let filePath: String
    let sizeBytes: Int64
    let createdAt: String
    
    enum CodingKeys: String, CodingKey {
        case id
        case documentId = "document_id"
        case versionNumber = "version_number"
        case filePath = "file_path"
        case sizeBytes = "size_bytes"
        case createdAt = "created_at"
    }
}

/// Represents a log of access to a document.
struct DocumentAccessLog: Codable, Identifiable {
    let id: String
    let documentId: String
    let userId: String
    let accessType: String
    let accessDate: String
    let details: String?
    
    enum CodingKeys: String, CodingKey {
        case id
        case documentId = "document_id"
        case userId = "user_id"
        case accessType = "access_type"
        case accessDate = "access_date"
        case details
    }
}

/// Used to specify which related data to include in a document query.
enum DocumentIncludeDto: Codable {
    case documentType
    case versions
    case accessLogs(pagination: PaginationDto?)

    enum CodingKeys: String, CodingKey {
        case type, pagination
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .documentType:
            try container.encode("DocumentType", forKey: .type)
        case .versions:
            try container.encode("Versions", forKey: .type)
        case .accessLogs(let pagination):
            try container.encode("AccessLogs", forKey: .type)
            if let pagination = pagination {
                try container.encode(pagination, forKey: .pagination)
            }
        }
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "DocumentType":
            self = .documentType
        case "Versions":
            self = .versions
        case "AccessLogs":
            let pagination = try container.decodeIfPresent(PaginationDto.self, forKey: .pagination)
            self = .accessLogs(pagination: pagination)
        default:
            throw DecodingError.dataCorruptedError(forKey: .type, in: container, debugDescription: "Invalid document include type")
        }
    }
}

// MARK: - Response Models for Specific Operations

/// Response for a document download operation.
struct DownloadResponse: Codable {
    let filename: String
    let data: String? // Base64 encoded data
}

/// Response for a document open operation.
struct OpenResponse: Codable {
    let filePath: String?
    
    enum CodingKeys: String, CodingKey {
        case filePath = "file_path"
    }
}

/// Response for a document availability check.
struct AvailabilityResponse: Codable {
    let isAvailable: Bool
    
    enum CodingKeys: String, CodingKey {
        case isAvailable = "is_available"
    }
}

/// Response for a temporary document linking operation.
struct LinkResponse: Codable {
    let linkedCount: UInt64
    
    enum CodingKeys: String, CodingKey {
        case linkedCount = "linked_count"
    }
}

/// Response for a document count query.
struct CountResponse: Codable {
    let entityId: String
    let documentCount: Int64
    
    enum CodingKeys: String, CodingKey {
        case entityId = "entity_id"
        case documentCount = "document_count"
    }
}

/// Response for a bulk priority update operation.
struct BulkUpdateResponse: Codable {
    let updatedCount: UInt64
    
    enum CodingKeys: String, CodingKey {
        case updatedCount = "updated_count"
    }
}

/// Response for a document summary calculation.
struct DocumentSummary: Codable {
    let totalCount: Int64
    let unlinkedCount: Int64
    let linkedFields: [String: Int64]
    
    enum CodingKeys: String, CodingKey {
        case totalCount = "total_count"
        case unlinkedCount = "unlinked_count"
        case linkedFields = "linked_fields"
    }
} 