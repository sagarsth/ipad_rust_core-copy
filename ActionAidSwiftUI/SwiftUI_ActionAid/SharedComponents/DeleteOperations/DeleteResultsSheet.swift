//
//  DeleteResultsSheet.swift
//  ActionAid SwiftUI
//
//  Generic delete results display for bulk operations
//

import SwiftUI

/// Generic bulk delete results that can be used across domains
protocol BulkDeleteResultsProtocol {
    var hardDeleted: [String] { get }
    var softDeleted: [String] { get }
    var failed: [String] { get }
    var dependencies: [String: [String]] { get }
    var errors: [String: String] { get }
}

/// Extension to make BatchDeleteResult conform to the protocol
extension BatchDeleteResult: BulkDeleteResultsProtocol {
    // Already has all the required properties
}

/// Generic delete results sheet for any domain
struct DeleteResultsSheet<Results: BulkDeleteResultsProtocol>: View {
    let results: Results
    let entityName: String           // e.g., "Goal", "Project", "User"
    let entityNamePlural: String     // e.g., "Goals", "Projects", "Users"
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Summary
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Bulk Delete Summary")
                            .font(.headline)
                        
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("✅ Hard Deleted")
                                    .font(.caption)
                                    .foregroundColor(.green)
                                Text("\(results.hardDeleted.count)")
                                    .font(.title2)
                                    .fontWeight(.bold)
                                    .foregroundColor(.green)
                            }
                            
                            Spacer()
                            
                            VStack(alignment: .leading, spacing: 4) {
                                Text("📦 Archived")
                                    .font(.caption)
                                    .foregroundColor(.orange)
                                Text("\(results.softDeleted.count)")
                                    .font(.title2)
                                    .fontWeight(.bold)
                                    .foregroundColor(.orange)
                            }
                            
                            Spacer()
                            
                            VStack(alignment: .leading, spacing: 4) {
                                Text("❌ Failed")
                                    .font(.caption)
                                    .foregroundColor(.red)
                                Text("\(results.failed.count)")
                                    .font(.title2)
                                    .fontWeight(.bold)
                                    .foregroundColor(.red)
                            }
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Failed items with dependencies
                    if !results.failed.isEmpty {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Failed Deletions")
                                .font(.headline)
                            
                            ForEach(results.failed, id: \.self) { failedId in
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("\(entityName) ID: \(failedId)")
                                        .font(.subheadline)
                                        .fontWeight(.medium)
                                    
                                    if let deps = results.dependencies[failedId], !deps.isEmpty {
                                        Text("Dependencies: \(deps.joined(separator: ", "))")
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                    }
                                    
                                    if let error = results.errors[failedId] {
                                        Text("Error: \(error)")
                                            .font(.caption)
                                            .foregroundColor(.red)
                                    }
                                }
                                .padding()
                                .background(Color.red.opacity(0.1))
                                .cornerRadius(8)
                            }
                        }
                    }
                    
                    // Dependencies information
                    if !results.dependencies.isEmpty {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Dependencies Detected")
                                .font(.headline)
                            
                            Text("Some \(entityNamePlural.lowercased()) could not be hard deleted due to dependencies. They were archived instead to preserve data integrity.")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .padding()
                        .background(Color.orange.opacity(0.1))
                        .cornerRadius(12)
                    }
                    
                    // Success message if no failures
                    if results.failed.isEmpty && (!results.hardDeleted.isEmpty || !results.softDeleted.isEmpty) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundColor(.green)
                                    .font(.title2)
                                Text("Operation Completed Successfully")
                                    .font(.headline)
                                    .foregroundColor(.green)
                            }
                            
                            let totalProcessed = results.hardDeleted.count + results.softDeleted.count
                            Text("All \(totalProcessed) \(totalProcessed == 1 ? entityName.lowercased() : entityNamePlural.lowercased()) were processed successfully.")
                                .font(.subheadline)
                                .foregroundColor(.secondary)
                        }
                        .padding()
                        .background(Color.green.opacity(0.1))
                        .cornerRadius(12)
                    }
                }
                .padding()
            }
            .navigationTitle("Delete Results")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

/// Convenience wrapper for Strategic Goals
struct StrategicGoalDeleteResultsSheet: View {
    let results: BatchDeleteResult
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        DeleteResultsSheet(
            results: results,
            entityName: "Goal",
            entityNamePlural: "Goals"
        )
    }
}

#Preview {
    DeleteResultsSheet(
        results: BatchDeleteResult(
            hardDeleted: ["goal1", "goal2"],
            softDeleted: ["goal3"],
            failed: ["goal4"],
            dependencies: ["goal4": ["project1", "project2"]],
            errors: ["goal4": "Cannot delete goal with active projects"]
        ),
        entityName: "Goal",
        entityNamePlural: "Goals"
    )
} 