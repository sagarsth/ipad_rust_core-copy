//
//  DocumentIntegration.swift
//  ActionAid SwiftUI
//
//  Protocol for entities that can have documents attached
//  Makes document management reusable across all domains
//

import Foundation

// MARK: - Document Integration Protocol

/// Protocol for entities that support document attachments
protocol DocumentIntegratable {
    /// The unique identifier of the entity
    var entityId: String { get }
    
    /// The database table name for this entity type
    var entityTableName: String { get }
    
    /// Fields that can have documents linked to them
    /// Returns array of (fieldKey, displayName) tuples
    var linkableFields: [(String, String)] { get }
    
    /// Display name for this entity type (e.g. "Strategic Goal", "Project")
    var entityTypeName: String { get }
}

// MARK: - Document Count Tracking

/// Protocol for entities that track document counts
protocol DocumentCountTrackable {
    /// Check if this entity has documents based on document count tracking
    func hasDocuments(in documentCounts: [String: Int]) -> Bool
}

// MARK: - Default Implementation

extension DocumentCountTrackable where Self: DocumentIntegratable {
    func hasDocuments(in documentCounts: [String: Int]) -> Bool {
        return (documentCounts[self.entityId] ?? 0) > 0
    }
} 