//
//  EntityDeleteOptionsSheet.swift
//  ActionAid SwiftUI
//
//  Generic delete options sheet for single and bulk operations
//

import SwiftUI

/// Configuration for delete operations
struct DeleteConfiguration {
    let entityName: String           // e.g., "Strategic Goal", "Project", "User"
    let entityNamePlural: String     // e.g., "Strategic Goals", "Projects", "Users"
    let showForceDelete: Bool        // Whether to show force delete option
    let archiveDescription: String   // Custom description for archive option
    let deleteDescription: String    // Custom description for delete option
    let forceDeleteDescription: String // Custom description for force delete option
    
    static func strategicGoal(isPlural: Bool = false) -> DeleteConfiguration {
        DeleteConfiguration(
            entityName: "Strategic Goal",
            entityNamePlural: "Strategic Goals",
            showForceDelete: true,
            archiveDescription: isPlural ? 
                "Move the goals to archive. They can be restored later. Associated documents will be preserved. Projects will remain linked." :
                "Move the goal to archive. It can be restored later. Associated documents will be preserved. Projects will remain linked.",
            deleteDescription: isPlural ?
                "Permanently delete goals if no dependencies exist. Goals with projects will be archived instead." :
                "Permanently delete goal if no dependencies exist. Goals with projects will be archived instead.",
            forceDeleteDescription: isPlural ?
                "⚠️ DANGER: Force delete all goals regardless of dependencies. Projects will lose their strategic goal link. This cannot be undone." :
                "⚠️ DANGER: Force delete goal regardless of dependencies. Projects will lose their strategic goal link. This cannot be undone."
        )
    }
    
    static func project(isPlural: Bool = false) -> DeleteConfiguration {
        DeleteConfiguration(
            entityName: "Project",
            entityNamePlural: "Projects", 
            showForceDelete: true,
            archiveDescription: isPlural ?
                "Move the projects to archive. They can be restored later. Associated documents and activities will be preserved." :
                "Move the project to archive. It can be restored later. Associated documents and activities will be preserved.",
            deleteDescription: isPlural ?
                "Permanently delete projects if no dependencies exist. Projects with activities will be archived instead." :
                "Permanently delete project if no dependencies exist. Projects with activities will be archived instead.",
            forceDeleteDescription: isPlural ?
                "⚠️ DANGER: Force delete all projects regardless of dependencies. Activities will be orphaned. This cannot be undone." :
                "⚠️ DANGER: Force delete project regardless of dependencies. Activities will be orphaned. This cannot be undone."
        )
    }
    
    static func user(isPlural: Bool = false) -> DeleteConfiguration {
        DeleteConfiguration(
            entityName: "User",
            entityNamePlural: "Users",
            showForceDelete: false, // Users typically shouldn't have force delete
            archiveDescription: isPlural ?
                "Deactivate the users. Their data will be preserved and they can be reactivated later." :
                "Deactivate the user. Their data will be preserved and they can be reactivated later.",
            deleteDescription: isPlural ?
                "Permanently delete users if they have no associated data." :
                "Permanently delete user if they have no associated data.",
            forceDeleteDescription: "" // Not used for users
        )
    }
}

/// Generic delete options sheet for entities
struct EntityDeleteOptionsSheet: View {
    let config: DeleteConfiguration
    let selectedCount: Int? // nil for single entity, count for bulk
    let userRole: String
    let onDelete: (Bool, Bool) -> Void  // (hardDelete, force)
    @Environment(\.dismiss) var dismiss
    
    private var isPlural: Bool {
        selectedCount != nil && selectedCount! > 1
    }
    
    private var entityDisplayName: String {
        if let count = selectedCount {
            return count == 1 ? config.entityName.lowercased() : config.entityNamePlural.lowercased()
        }
        return config.entityName.lowercased()
    }
    
    private var titleText: String {
        if let count = selectedCount {
            return count == 1 ? 
                "How would you like to delete this \(entityDisplayName)?" :
                "Delete \(count) \(entityDisplayName)?"
        }
        return "How would you like to delete this \(entityDisplayName)?"
    }
    
    private var navigationTitle: String {
        selectedCount != nil ? "Bulk Delete" : "Delete \(config.entityName)"
    }
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Text(titleText)
                    .font(.headline)
                    .multilineTextAlignment(.center)
                    .padding(.top)
                
                VStack(spacing: 16) {
                    archiveButton
                    if userRole.lowercased() == "admin" {
                        deleteButton
                        if config.showForceDelete {
                            forceDeleteButton
                        }
                    }
                }
                .padding(.horizontal)
                
                Spacer()
            }
            .navigationTitle(navigationTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
            }
        }
    }
    
    private var archiveButton: some View {
        Button(action: {
            onDelete(false, false)
            dismiss()
        }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "archivebox")
                        .foregroundColor(.orange)
                        .font(.title2)
                    Text(isPlural ? "Archive \(config.entityNamePlural)" : "Archive \(config.entityName)")
                        .font(.headline)
                        .foregroundColor(.primary)
                    Spacer()
                }
                
                Text(config.archiveDescription)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.leading)
            }
            .padding()
            .background(Color(.systemGray6))
            .cornerRadius(12)
        }
        .buttonStyle(PlainButtonStyle())
    }
    
    private var deleteButton: some View {
        Button(action: {
            onDelete(true, false)
            dismiss()
        }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "trash.fill")
                        .foregroundColor(.red)
                        .font(.title2)
                    Text(isPlural ? "Delete \(config.entityNamePlural)" : "Delete \(config.entityName)")
                        .font(.headline)
                        .foregroundColor(.red)
                    Spacer()
                }
                
                Text(config.deleteDescription)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.leading)
            }
            .padding()
            .background(Color.red.opacity(0.1))
            .cornerRadius(12)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color.red.opacity(0.3), lineWidth: 1)
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
    
    private var forceDeleteButton: some View {
        Button(action: {
            onDelete(true, true)
            dismiss()
        }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.red)
                        .font(.title2)
                    Text("Force Delete")
                        .font(.headline)
                        .foregroundColor(.red)
                    Spacer()
                }
                
                Text(config.forceDeleteDescription)
                    .font(.caption)
                    .foregroundColor(.red)
                    .multilineTextAlignment(.leading)
            }
            .padding()
            .background(Color.red.opacity(0.2))
            .cornerRadius(12)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color.red, lineWidth: 2)
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

#Preview("Single Entity") {
    EntityDeleteOptionsSheet(
        config: .strategicGoal(),
        selectedCount: nil,
        userRole: "admin",
        onDelete: { _, _ in }
    )
}

#Preview("Bulk Delete") {
    EntityDeleteOptionsSheet(
        config: .strategicGoal(isPlural: true),
        selectedCount: 5,
        userRole: "admin",
        onDelete: { _, _ in }
    )
} 