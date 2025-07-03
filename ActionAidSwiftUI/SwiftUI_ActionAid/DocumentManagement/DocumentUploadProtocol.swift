//
//  DocumentUploadProtocol.swift
//  ActionAid SwiftUI
//
//  Protocol for domain-specific document upload functionality
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