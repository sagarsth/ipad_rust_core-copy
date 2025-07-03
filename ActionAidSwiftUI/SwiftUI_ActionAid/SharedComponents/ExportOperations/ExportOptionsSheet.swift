//
//  ExportOptionsSheet.swift
//  ActionAid SwiftUI
//
//  Generic export options sheet for all domains
//

import SwiftUI

/// Generic export options sheet that can be used across all domains
struct GenericExportOptionsSheet: View {
    let selectedItemCount: Int
    let entityName: String              // e.g., "Strategic Goal", "Project", "User"
    let entityNamePlural: String        // e.g., "Strategic Goals", "Projects", "Users"
    let onExport: (Bool, ExportFormat) -> Void
    @Binding var isExporting: Bool
    @Binding var exportError: String?
    @Environment(\.dismiss) var dismiss
    
    @State private var includeBlobs = false
    @State private var selectedFormat: ExportFormat = .default
    @State private var showFormatRecommendation = false
    
    private var exportDescription: String {
        if selectedItemCount == 1 {
            return "This will export the selected \(entityName.lowercased()) with your current filter settings."
        } else {
            return "This will export \(selectedItemCount) \(entityNamePlural.lowercased()) that match your current filter settings."
        }
    }
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 24) {
                    // Smart recommendation banner
                    if showFormatRecommendation {
                        recommendationBanner
                    }
                    
                    exportFormatSelection
                    exportOptionsSection
                    exportInfoSection
                    
                    if let error = exportError {
                        errorSection(error)
                    }
                }
                .padding()
            }
            .navigationTitle("Export \(selectedItemCount == 1 ? entityName : entityNamePlural)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Export") {
                        onExport(includeBlobs, selectedFormat)
                    }
                    .disabled(isExporting)
                }
            }
            .onAppear {
                // Set recommended format based on selected item count
                let recommendedFormat = ExportFormat.recommended(for: selectedItemCount)
                if selectedFormat.id == ExportFormat.default.id {
                    selectedFormat = recommendedFormat
                    showFormatRecommendation = true
                }
            }
        }
    }
    
    private var recommendationBanner: some View {
        HStack {
            Image(systemName: "lightbulb.fill")
                .foregroundColor(.yellow)
            VStack(alignment: .leading, spacing: 4) {
                Text("Smart Recommendation")
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundColor(.primary)
                Text("Based on \(selectedItemCount) items, we recommend \(selectedFormat.displayName) format.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
        .padding()
        .background(Color.yellow.opacity(0.1))
        .cornerRadius(12)
    }
    
    private var exportFormatSelection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Export Format")
                .font(.headline)
                .foregroundColor(.primary)
            
            ForEach(ExportFormat.allCases, id: \.id) { format in
                ExportFormatRow(
                    format: format,
                    isSelected: selectedFormat.id == format.id,
                    isRecommended: format.isRecommendedForLargeDatasets && selectedItemCount > 100,
                    onSelect: { selectedFormat = format }
                )
            }
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    private var exportOptionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Export Options")
                .font(.headline)
                .foregroundColor(.primary)
            
            HStack {
                Toggle("Include document attachments", isOn: $includeBlobs)
                    .font(.subheadline)
                Spacer()
            }
            
            if includeBlobs {
                Text("⚠️ Including documents will significantly increase export time and file size.")
                    .font(.caption)
                    .foregroundColor(.orange)
            }
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    private var exportInfoSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Export Information")
                .font(.headline)
                .foregroundColor(.primary)
            
            Text(exportDescription)
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            // Format-specific info
            formatInfoView
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    @ViewBuilder
    private var formatInfoView: some View {
        switch selectedFormat {
        case .jsonLines:
            Label("Best for data processing and APIs", systemImage: "gearshape.2")
                .font(.caption)
                .foregroundColor(.blue)
        case .csv:
            Label("Opens in Excel, Google Sheets, and most tools", systemImage: "tablecells")
                .font(.caption)
                .foregroundColor(.green)
        case .parquet:
            Label("Smallest file size, fastest for large datasets", systemImage: "speedometer")
                .font(.caption)
                .foregroundColor(.purple)
        }
    }
    
    private func errorSection(_ error: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.red)
                Text("Export Error")
                    .font(.headline)
                    .foregroundColor(.red)
            }
            Text(error)
                .font(.caption)
                .foregroundColor(.red)
        }
        .padding()
        .background(Color.red.opacity(0.1))
        .cornerRadius(12)
    }
}

/// Individual format selection row
private struct ExportFormatRow: View {
    let format: ExportFormat
    let isSelected: Bool
    let isRecommended: Bool
    let onSelect: () -> Void
    
    var body: some View {
        Button(action: onSelect) {
            HStack {
                Image(systemName: format.icon)
                    .foregroundColor(isSelected ? .blue : .secondary)
                    .frame(width: 20)
                
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(format.displayName)
                            .font(.subheadline)
                            .fontWeight(isSelected ? .medium : .regular)
                            .foregroundColor(.primary)
                        
                        if isRecommended {
                            Text("RECOMMENDED")
                                .font(.caption2)
                                .fontWeight(.bold)
                                .foregroundColor(.white)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.blue)
                                .cornerRadius(4)
                        }
                        
                        Spacer()
                    }
                    
                    Text(format.description)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.leading)
                }
                
                Spacer()
                
                if isSelected {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.blue)
                }
            }
            .padding(.vertical, 8)
        }
        .buttonStyle(PlainButtonStyle())
    }
}

/// Convenience wrappers for specific domains
struct StrategicGoalExportOptionsSheet: View {
    let selectedItemCount: Int
    let onExport: (Bool, ExportFormat) -> Void
    @Binding var isExporting: Bool
    @Binding var exportError: String?
    
    var body: some View {
        GenericExportOptionsSheet(
            selectedItemCount: selectedItemCount,
            entityName: "Strategic Goal",
            entityNamePlural: "Strategic Goals",
            onExport: onExport,
            isExporting: $isExporting,
            exportError: $exportError
        )
    }
}

struct ProjectExportOptionsSheet: View {
    let selectedItemCount: Int
    let onExport: (Bool, ExportFormat) -> Void
    @Binding var isExporting: Bool
    @Binding var exportError: String?
    
    var body: some View {
        GenericExportOptionsSheet(
            selectedItemCount: selectedItemCount,
            entityName: "Project",
            entityNamePlural: "Projects",
            onExport: onExport,
            isExporting: $isExporting,
            exportError: $exportError
        )
    }
}

struct UserExportOptionsSheet: View {
    let selectedItemCount: Int
    let onExport: (Bool, ExportFormat) -> Void
    @Binding var isExporting: Bool
    @Binding var exportError: String?
    
    var body: some View {
        GenericExportOptionsSheet(
            selectedItemCount: selectedItemCount,
            entityName: "User",
            entityNamePlural: "Users",
            onExport: onExport,
            isExporting: $isExporting,
            exportError: $exportError
        )
    }
}

#Preview {
    GenericExportOptionsSheet(
        selectedItemCount: 25,
        entityName: "Strategic Goal",
        entityNamePlural: "Strategic Goals",
        onExport: { _, _ in },
        isExporting: .constant(false),
        exportError: .constant(nil)
    )
} 