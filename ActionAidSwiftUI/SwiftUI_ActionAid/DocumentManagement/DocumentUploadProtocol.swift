//
//  DocumentUploadProtocol.swift
//  ActionAid SwiftUI
//
//  Protocol for domain-specific document upload functionality
//
//  📋 CLEAN ARCHITECTURE PATTERN FOR DOCUMENT UPLOADS
//  ==================================================
//
//  ✅ GOOD PATTERN (Use This For All Future Domains):
//  --------------------------------------------------
//  
//  1. Domain FFI Handler (XxxFFIHandler) - Focus on CORE CRUD only:
//     - create, get, list, update, delete
//     - domain-specific queries and analytics
//     - NO document upload methods (those are legacy/slow)
//
//  2. Generic Document Handler (DocumentFFIHandler) - Optimized uploads:
//     - uploadDocumentFromPath() - fast path-based uploads
//     - All document-related operations
//     - Shared across all domains
//
//  3. Domain Document Adapter (XxxDocumentAdapter) - Bridge pattern:
//     - Implements DocumentUploadable protocol
//     - Single upload: calls DocumentFFIHandler.uploadDocumentFromPath()
//     - Bulk upload: individual path-based uploads (no base64)
//     - Handles domain-specific field mappings
//
//  📱 EXAMPLE IMPLEMENTATION FOR NEW DOMAIN:
//  ----------------------------------------
//  
//  struct WorkshopDocumentAdapter: DocumentUploadable {
//      let workshop: WorkshopResponse
//      private let documentHandler = DocumentFFIHandler()
//      
//      func uploadDocument(..., filePath: String, ...) -> Result<MediaDocumentResponse, Error> {
//          return await documentHandler.uploadDocumentFromPath(
//              filePath: filePath,                    // ✅ Path-based (fast)
//              relatedEntityId: workshop.id,
//              relatedEntityType: "workshops",
//              ...
//          )
//      }
//      
//      func bulkUploadDocuments(files: [(Data, String)], ...) -> Result<[MediaDocumentResponse], Error> {
//          // ✅ Individual optimized uploads (see ProjectDocumentAdapter for full implementation)
//          var results: [MediaDocumentResponse] = []
//          for (fileData, filename) in files {
//              // Create temp file and use path-based upload
//              ...
//          }
//          return .success(results)
//      }
//  }
//
//  ❌ BAD PATTERN (Don't Do This):
//  ------------------------------
//  
//  class WorkshopFFIHandler {
//      // ❌ DON'T add these slow base64 methods:
//      func uploadDocument(fileData: Data, ...) -> Result<...> {
//          let payload = WorkshopUploadRequest(
//              fileData: fileData.base64EncodedString()  // ❌ SLOW! Causes memory issues
//          )
//      }
//  }
//
//  🚀 PERFORMANCE BENEFITS:
//  -----------------------
//  - ✅ No base64 encoding (50%+ memory reduction)
//  - ✅ No loading entire files into memory
//  - ✅ Rust handles file I/O directly (faster)
//  - ✅ Better for large files and bulk uploads
//  - ✅ Consistent pattern across all domains
//

import Foundation

// MARK: - Document Upload Protocol

/// Protocol for entities that can upload documents using domain-specific FFI handlers
protocol DocumentUploadable: DocumentIntegratable {
    /// Upload a single document from file path (iOS optimized)
    func uploadDocument(
        filePath: String,
        originalFilename: String,
        title: String?,
        documentTypeId: String,
        linkedField: String?,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<MediaDocumentResponse, Error>
    
    /// Upload multiple documents in bulk
    func bulkUploadDocuments(
        files: [(Data, String)],
        title: String?,
        documentTypeId: String,
        syncPriority: SyncPriority,
        compressionPriority: CompressionPriority?,
        auth: AuthContextPayload
    ) async -> Result<[MediaDocumentResponse], Error>
    
    /// Get default document type ID for this entity type
    func getDefaultDocumentTypeId(auth: AuthContextPayload) async -> String?
    
    /// Detect specific document type ID based on file extension
    func detectDocumentTypeId(for filename: String, auth: AuthContextPayload) async -> String?
}

// MARK: - Default Implementation

extension DocumentUploadable {
    /// Default implementation for document type detection
    func detectDocumentTypeId(for filename: String, auth: AuthContextPayload) async -> String? {
        // Default implementation returns nil, will use default document type
        return nil
    }
} 