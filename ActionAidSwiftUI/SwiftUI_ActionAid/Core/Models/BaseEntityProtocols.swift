//
//  BaseEntityProtocols.swift
//  SwiftUI_ActionAid
//
//  Universal protocols for all domain entities to enable generic components
//

import Foundation
import SwiftUI

// MARK: - Core Entity Protocol

/// Universal protocol that all domain entities must conform to
/// Enables generic CRUD operations, filtering, and UI components
protocol DomainEntity: Identifiable, Codable, Equatable {
    /// Unique identifier for the entity
    var id: String { get }
    
    /// Primary display name/title for the entity
    var displayName: String { get }
    
    /// Secondary description or subtitle
    var displaySubtitle: String? { get }
    
    /// Creation timestamp (ISO 8601 string)
    var createdAt: String { get }
    
    /// Last update timestamp (ISO 8601 string)
    var updatedAt: String { get }
    
    /// Sync priority level
    var syncPriority: SyncPriority { get }
    
    /// Created by user ID
    var createdByUserId: String? { get }
    
    /// Updated by user ID
    var updatedByUserId: String? { get }
    
    /// Created by username (for display)
    var createdByUsername: String? { get }
    
    /// Updated by username (for display)
    var updatedByUsername: String? { get }
    
    /// Document count for this entity
    var documentCount: Int64? { get }
    
    /// Entity type name for display purposes
    static var entityTypeName: String { get }
    
    /// Entity type name plural
    static var entityTypeNamePlural: String { get }
    
    /// Table name in database (for document linking)
    static var databaseTableName: String { get }
}

// MARK: - Status-Based Entity Protocol

/// Protocol for entities that have status fields
protocol StatusBasedEntity: DomainEntity {
    /// Status ID (typically 1-4 for On Track, At Risk, Delayed, Completed)
    var statusId: Int64? { get }
    
    /// Status display name
    var statusName: String { get }
    
    /// Status color for UI
    var statusColor: Color { get }
}

// MARK: - Team-Managed Entity Protocol

/// Protocol for entities managed by teams
protocol TeamManagedEntity: DomainEntity {
    /// Responsible team name
    var responsibleTeam: String? { get }
}

// MARK: - Searchable Entity Protocol

/// Protocol for entities that support text search
protocol SearchableEntity: DomainEntity {
    /// Returns all searchable text content for this entity
    var searchableContent: [String] { get }
    
    /// Check if entity matches search query
    func matchesSearch(_ query: String) -> Bool
}

// MARK: - Filterable Entity Protocol

/// Protocol for entities that support filtering
protocol FilterableEntity: DomainEntity {
    associatedtype FilterType: DomainEntityFilter
    
    /// Check if entity matches the given filter
    func matchesFilter(_ filter: FilterType) -> Bool
}

// MARK: - Progress-Based Entity Protocol

/// Protocol for entities with progress/completion tracking
protocol ProgressBasedEntity: DomainEntity {
    /// Progress percentage (0-100)
    var progressPercentage: Double? { get }
    
    /// Target value
    var targetValue: Double? { get }
    
    /// Actual value
    var actualValue: Double? { get }
    
    /// Computed safe progress (0-100, handles NaN/infinite)
    var safeProgress: Double { get }
}

// MARK: - Document Integration Protocols

/// Protocol for entities that support document uploads
protocol DocumentSupportingEntity: DomainEntity {
    /// Available fields for document linking
    var linkableFields: [(String, String)] { get }
    
    /// Convert to DocumentUploadable adapter
    func asDocumentUploadable() -> any DocumentUploadable
}

// MARK: - Base Filter Protocol

/// Universal protocol for entity filters
protocol DomainEntityFilter: Codable {
    /// Search text filter
    var searchText: String? { get set }
    
    /// Exclude deleted entities
    var excludeDeleted: Bool { get set }
    
    /// Date range filter (start, end)
    var dateRange: (String, String)? { get set }
    
    /// Check if filter is empty (no constraints)
    var isEmpty: Bool { get }
    
    /// Create an empty filter
    static func empty() -> Self
    
    /// Create a filter that matches all entities
    static func all() -> Self
}

// MARK: - Base Include Options Protocol

/// Universal protocol for include options
protocol DomainEntityInclude: Codable, CaseIterable, RawRepresentable where RawValue == String {
    /// Include document count
    static var documentCount: Self { get }
    
    /// Include full document list
    static var documents: Self { get }
    
    /// Include all available data
    static var all: Self { get }
}

// MARK: - CRUD Request Protocols

/// Base protocol for FFI create requests
protocol CreateEntityRequest: Codable {
    associatedtype EntityType: DomainEntity
    associatedtype NewEntityType: Codable
    
    var entity: NewEntityType { get }
    var auth: AuthContextPayload { get }
}

/// Base protocol for FFI update requests
protocol UpdateEntityRequest: Codable {
    associatedtype EntityType: DomainEntity
    associatedtype UpdateEntityType: Codable
    
    var id: String { get }
    var update: UpdateEntityType { get }
    var auth: AuthContextPayload { get }
}

/// Base protocol for FFI list requests
protocol ListEntityRequest: Codable {
    associatedtype IncludeType: DomainEntityInclude
    
    var pagination: PaginationDto? { get }
    var include: [IncludeType]? { get }
    var auth: AuthContextPayload { get }
}

// MARK: - Default Implementations

extension SearchableEntity {
    func matchesSearch(_ query: String) -> Bool {
        guard !query.isEmpty else { return true }
        let lowercaseQuery = query.lowercased()
        return searchableContent.contains { content in
            content.lowercased().contains(lowercaseQuery)
        }
    }
}

extension ProgressBasedEntity {
    var safeProgress: Double {
        let rawProgress = progressPercentage ?? 0.0
        if rawProgress.isNaN || rawProgress.isInfinite {
            return 0.0
        }
        return max(0.0, min(100.0, rawProgress))
    }
}

extension DomainEntity {
    /// Default subtitle is creation date
    var displaySubtitle: String? {
        let formatter = ISO8601DateFormatter()
        if let date = formatter.date(from: createdAt) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            return displayFormatter.string(from: date)
        }
        return createdAt
    }
}

// MARK: - Concrete Entity Conformances

// Strategic Goals
extension StrategicGoalResponse: StatusBasedEntity, TeamManagedEntity, SearchableEntity, ProgressBasedEntity, DocumentSupportingEntity {
    var displayName: String { objectiveCode }
    var displaySubtitle: String? { outcome }
    
    static var entityTypeName: String { "Strategic Goal" }
    static var entityTypeNamePlural: String { "Strategic Goals" }
    static var databaseTableName: String { "strategic_goals" }
    
    var searchableContent: [String] {
        [objectiveCode, outcome, kpi, responsibleTeam].compactMap { $0 }
    }
    
    var linkableFields: [(String, String)] {
        [
            ("", "None"),
            ("outcome", "Outcome"),
            ("kpi", "KPI"),
            ("actual_value", "Actual Value"),
            ("supporting_documentation", "Supporting Documentation"),
            ("impact_assessment", "Impact Assessment"),
            ("theory_of_change", "Theory of Change"),
            ("baseline_data", "Baseline Data")
        ]
    }
    
    func asDocumentUploadable() -> any DocumentUploadable {
        return StrategicGoalDocumentAdapter(goal: self)
    }
}

// Projects
extension ProjectResponse: StatusBasedEntity, TeamManagedEntity, SearchableEntity, DocumentSupportingEntity {
    var displayName: String { name }
    var displaySubtitle: String? { objective }
    
    static var entityTypeName: String { "Project" }
    static var entityTypeNamePlural: String { "Projects" }
    static var databaseTableName: String { "projects" }
    
    var progressPercentage: Double? { nil } // Projects don't have progress percentage
    var targetValue: Double? { nil }
    var actualValue: Double? { nil }
    
    var searchableContent: [String] {
        [name, objective, outcome, timeline, responsibleTeam, effectiveStrategicGoalName].compactMap { $0 }
    }
    
    var linkableFields: [(String, String)] {
        [
            ("", "None"),
            ("objective", "Objective"),
            ("outcome", "Outcome"),
            ("timeline", "Timeline"),
            ("project_plan", "Project Plan"),
            ("budget", "Budget"),
            ("reports", "Reports"),
            ("documentation", "Documentation")
        ]
    }
    
    func asDocumentUploadable() -> any DocumentUploadable {
        return ProjectDocumentAdapter(project: self)
    }
}

// MARK: - Filter Type Conformances

extension StrategicGoalFilter: DomainEntityFilter {
    static func empty() -> StrategicGoalFilter {
        StrategicGoalFilter(
            statusIds: nil, responsibleTeams: nil, years: nil, months: nil,
            userRole: nil, syncPriorities: nil, searchText: nil,
            progressRange: nil, targetValueRange: nil, actualValueRange: nil,
            dateRange: nil, daysStale: nil, excludeDeleted: true
        )
    }
    
    static func all() -> StrategicGoalFilter {
        empty()
    }
}

extension ProjectFilter: DomainEntityFilter {
    static func empty() -> ProjectFilter {
        ProjectFilter(excludeDeleted: true)
    }
    
    static func all() -> ProjectFilter {
        empty()
    }
}

// MARK: - Include Type Conformances

extension StrategicGoalInclude: DomainEntityInclude {
    static var documentCount: StrategicGoalInclude { .documentCounts }
    static var documents: StrategicGoalInclude { .documents }
    static var all: StrategicGoalInclude { .documents } // Closest to "all"
}

extension ProjectInclude: DomainEntityInclude {
    static var documentCount: ProjectInclude { .counts }
    static var documents: ProjectInclude { .documents }
    static var all: ProjectInclude { .all }
} 