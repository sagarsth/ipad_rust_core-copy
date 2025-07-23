//
//  CRUDSheetManager.swift
//  ActionAid SwiftUI
//
//  Generic CRUD sheet state management for any entity type
//

import SwiftUI
import Foundation

// MARK: - CRUD Operation Types
enum CRUDOperation {
    case create
    case edit
    case delete
}

// MARK: - CRUD Sheet Configuration
struct CRUDSheetConfig {
    let entityName: String
    let entityNamePlural: String
    let createTitle: String
    let editTitle: String
    let deleteTitle: String
    
    init(entityName: String, entityNamePlural: String? = nil) {
        self.entityName = entityName
        self.entityNamePlural = entityNamePlural ?? "\(entityName)s"
        self.createTitle = "Create \(entityName)"
        self.editTitle = "Edit \(entityName)"
        self.deleteTitle = "Delete \(entityName)"
    }
    
    static let strategicGoal = CRUDSheetConfig(
        entityName: "Strategic Goal",
        entityNamePlural: "Strategic Goals"
    )
    
    static let project = CRUDSheetConfig(
        entityName: "Project"
    )
    
    static let user = CRUDSheetConfig(
        entityName: "User"
    )

    static let activity = CRUDSheetConfig(
        entityName: "Activity",
        entityNamePlural: "Activities"
    )
    
    static let participant = CRUDSheetConfig(
        entityName: "Participant"
    )
    
    static let document = CRUDSheetConfig(
        entityName: "Document"
    )
}

// MARK: - CRUD Result
enum CRUDResult<Entity> {
    case success(Entity)
    case failure(Error)
    case cancelled
}

// MARK: - CRUD Sheet Manager
@MainActor
class CRUDSheetManager<Entity: Identifiable>: ObservableObject {
    // MARK: - Published State
    @Published var showCreateSheet = false
    @Published var showEditSheet = false
    @Published var showDeleteConfirmation = false
    @Published var showDeleteOptions = false
    @Published var selectedEntity: Entity?
    @Published var isPerformingOperation = false
    @Published var errorMessage: String?
    @Published var showErrorAlert = false
    
    // MARK: - Configuration
    let config: CRUDSheetConfig
    
    // MARK: - Callbacks
    var onEntityCreated: ((Entity) -> Void)?
    var onEntityUpdated: ((Entity) -> Void)?
    var onEntityDeleted: ((Entity) -> Void)?
    var onError: ((Error) -> Void)?
    
    // MARK: - Initialization
    init(config: CRUDSheetConfig) {
        self.config = config
    }
    
    // MARK: - Sheet Presentation Methods
    
    /// Present the create sheet
    func presentCreateSheet() {
        selectedEntity = nil
        errorMessage = nil
        showCreateSheet = true
    }
    
    /// Present the edit sheet for a specific entity
    func presentEditSheet(for entity: Entity) {
        selectedEntity = entity
        errorMessage = nil
        showEditSheet = true
    }
    
    /// Present delete confirmation for a specific entity
    func presentDeleteConfirmation(for entity: Entity) {
        selectedEntity = entity
        errorMessage = nil
        showDeleteConfirmation = true
    }
    
    /// Present delete options sheet for admin users
    func presentDeleteOptions(for entity: Entity) {
        selectedEntity = entity
        errorMessage = nil
        showDeleteOptions = true
    }
    
    // MARK: - Sheet Dismissal Methods
    
    /// Dismiss all sheets and clear state
    func dismissAllSheets() {
        showCreateSheet = false
        showEditSheet = false
        showDeleteConfirmation = false
        showDeleteOptions = false
        selectedEntity = nil
        errorMessage = nil
        isPerformingOperation = false
    }
    
    /// Dismiss sheet after successful operation
    func dismissAfterSuccess() {
        showCreateSheet = false
        showEditSheet = false
        showDeleteConfirmation = false
        showDeleteOptions = false
        isPerformingOperation = false
        // Keep selectedEntity for callbacks
    }
    
    // MARK: - Operation State Management
    
    /// Start a CRUD operation (shows loading state)
    func startOperation(_ operation: CRUDOperation) {
        isPerformingOperation = true
        errorMessage = nil
        showErrorAlert = false
    }
    
    /// Complete a CRUD operation with success
    func completeOperation(_ operation: CRUDOperation, result: Entity) {
        isPerformingOperation = false
        
        // Trigger appropriate callback
        switch operation {
        case .create:
            onEntityCreated?(result)
        case .edit:
            onEntityUpdated?(result)
        case .delete:
            onEntityDeleted?(result)
        }
        
        // Dismiss sheet after success
        dismissAfterSuccess()
    }
    
    /// Complete a CRUD operation with error
    func completeOperation(_ operation: CRUDOperation, error: Error) {
        isPerformingOperation = false
        errorMessage = error.localizedDescription
        showErrorAlert = true
        onError?(error)
    }
    
    // MARK: - Convenience Methods
    
    /// Handle create operation result
    func handleCreateResult(_ result: Result<Entity, Error>) {
        switch result {
        case .success(let entity):
            completeOperation(.create, result: entity)
        case .failure(let error):
            completeOperation(.create, error: error)
        }
    }
    
    /// Handle update operation result
    func handleUpdateResult(_ result: Result<Entity, Error>) {
        switch result {
        case .success(let entity):
            completeOperation(.edit, result: entity)
        case .failure(let error):
            completeOperation(.edit, error: error)
        }
    }
    
    /// Handle delete operation result
    func handleDeleteResult(_ result: Result<Entity, Error>) {
        switch result {
        case .success(let entity):
            completeOperation(.delete, result: entity)
        case .failure(let error):
            completeOperation(.delete, error: error)
        }
    }
    
    // MARK: - Validation Helpers
    
    /// Check if an operation can be performed (not currently loading)
    func canPerformOperation() -> Bool {
        return !isPerformingOperation
    }
    
    /// Reset error state
    func clearError() {
        errorMessage = nil
        showErrorAlert = false
    }
}

// MARK: - CRUD Sheet View Modifiers
extension View {
    /// Apply CRUD sheets to a view using the manager
    func withCRUDSheets<Entity: Identifiable, CreateView: View, EditView: View>(
        manager: CRUDSheetManager<Entity>,
        userRole: String?,
        @ViewBuilder createSheet: @escaping () -> CreateView,
        @ViewBuilder editSheet: @escaping (Entity) -> EditView,
        onDelete: @escaping (Entity, Bool, Bool) -> Void = { _, _, _ in }
    ) -> some View {
        self
            .sheet(isPresented: Binding(
                get: { manager.showCreateSheet },
                set: { if !$0 { manager.showCreateSheet = false } }
            )) {
                createSheet()
            }
            .sheet(isPresented: Binding(
                get: { manager.showEditSheet },
                set: { if !$0 { manager.showEditSheet = false } }
            )) {
                if let entity = manager.selectedEntity {
                    editSheet(entity)
                }
            }
            .alert(manager.config.deleteTitle, isPresented: Binding(
                get: { manager.showDeleteConfirmation },
                set: { if !$0 { manager.showDeleteConfirmation = false } }
            )) {
                Button("Cancel", role: .cancel) { 
                    manager.dismissAllSheets()
                }
                Button("Delete", role: .destructive) {
                    if let entity = manager.selectedEntity {
                        onDelete(entity, false, false) // Non-admin users get soft delete
                    }
                    manager.dismissAllSheets()
                }
            } message: {
                Text("Are you sure you want to delete this \(manager.config.entityName.lowercased())? It will be archived and can be restored later.")
            }
            .sheet(isPresented: Binding(
                get: { manager.showDeleteOptions },
                set: { if !$0 { manager.showDeleteOptions = false } }
            )) {
                if let entity = manager.selectedEntity {
                    EntityDeleteOptionsSheet(
                        config: DeleteConfiguration(
                            entityName: manager.config.entityName,
                            entityNamePlural: manager.config.entityNamePlural,
                            showForceDelete: true,
                            archiveDescription: "Archive the \(manager.config.entityName.lowercased()). It will be preserved and can be restored later.",
                            deleteDescription: "Permanently delete \(manager.config.entityName.lowercased()) if no dependencies exist.",
                            forceDeleteDescription: "⚠️ DANGER: Force delete \(manager.config.entityName.lowercased()) regardless of dependencies. This cannot be undone."
                        ),
                        selectedCount: 1,
                        userRole: userRole ?? "",
                        onDelete: { hardDelete, force in
                            onDelete(entity, hardDelete, force)
                            manager.dismissAllSheets()
                        }
                    )
                }
            }
            .alert("Error", isPresented: Binding(
                get: { manager.showErrorAlert },
                set: { if !$0 { manager.clearError() } }
            )) {
                Button("OK") { 
                    manager.clearError()
                }
            } message: {
                Text(manager.errorMessage ?? "An error occurred")
            }
            .overlay {
                if manager.isPerformingOperation {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Processing...")
                        .scaleEffect(1.2)
                }
            }
    }
}

// MARK: - CRUD Toolbar Helper
struct CRUDToolbarActions<Entity: Identifiable>: View {
    let manager: CRUDSheetManager<Entity>
    let userRole: String?
    let entity: Entity?
    let canEdit: Bool
    let canDelete: Bool
    
    init(
        manager: CRUDSheetManager<Entity>,
        userRole: String?,
        entity: Entity? = nil,
        canEdit: Bool = true,
        canDelete: Bool = true
    ) {
        self.manager = manager
        self.userRole = userRole
        self.entity = entity
        self.canEdit = canEdit
        self.canDelete = canDelete
    }
    
    var body: some View {
        Menu {
            if canEdit, let entity = entity {
                Button(action: { 
                    manager.presentEditSheet(for: entity)
                }) {
                    Label("Edit", systemImage: "pencil")
                }
            }
            
            if canDelete, let entity = entity {
                Divider()
                Button(role: .destructive, action: { 
                    // Check user role to determine delete options
                    if userRole?.lowercased() == "admin" {
                        manager.presentDeleteOptions(for: entity)
                    } else {
                        manager.presentDeleteConfirmation(for: entity)
                    }
                }) {
                    Label("Delete \(manager.config.entityName)", systemImage: "trash")
                }
            }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .disabled(manager.isPerformingOperation)
    }
} 