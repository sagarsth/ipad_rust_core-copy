//
//  ExportManager.swift
//  ActionAid SwiftUI
//
//  Generic export manager for all domains
//

import Foundation
import SwiftUI

/// Protocol for domain-specific export services
protocol DomainExportService {
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse
    
    var domainName: String { get }
    var filePrefix: String { get }
}

/// Generic export manager that can be used across all domains
@MainActor
class ExportManager: ObservableObject {
    @Published var isExporting = false
    @Published var exportError: String?
    
    private let service: DomainExportService
    private let maxPollingAttempts = 30 // 30 seconds max
    
    init(service: DomainExportService) {
        self.service = service
    }
    
    /// Export selected items with automatic polling and completion handling
    func exportSelectedItems(
        ids: [String],
        includeBlobs: Bool = false,
        format: ExportFormat = .default,
        authToken: String,
        onClearSelection: @escaping () -> Void,
        onCompletion: @escaping (Bool) -> Void // true for success, false for failure
    ) async {
        guard !ids.isEmpty else { return }
        
        isExporting = true
        exportError = nil
        
        print("🔄 [EXPORT_MANAGER] Starting export for \(ids.count) \(service.domainName) items")
        print("🔄 [EXPORT_MANAGER] Format: \(format.displayName), Include blobs: \(includeBlobs)")
        
        do {
            // Create export directory in Documents (Files app accessible)
            let exportFolderURL = try createExportDirectory()
            
            // Create timestamped export file
            let targetPath = try createExportFilePath(
                folder: exportFolderURL,
                format: format
            )
            
            print("📁 [EXPORT_MANAGER] Export target path: \(targetPath)")
            
            // Start export
            let exportResponse = try await service.exportByIds(
                ids: ids,
                includeBlobs: includeBlobs,
                format: format,
                targetPath: targetPath,
                token: authToken
            )
            
            print("✅ [EXPORT_MANAGER] Export job created: \(exportResponse.job.status)")
            print("📊 [EXPORT_MANAGER] Export job ID: \(exportResponse.job.id)")
            
            // Check if export completed immediately or needs polling
            if exportResponse.job.status == "Completed" {
                print("🎉 [EXPORT_MANAGER] Export completed immediately")
                await handleExportCompletion(
                    exportResponse: exportResponse,
                    targetPath: targetPath,
                    onClearSelection: onClearSelection,
                    onCompletion: onCompletion
                )
            } else {
                print("⏳ [EXPORT_MANAGER] Export in progress, starting polling...")
                await pollExportCompletion(
                    jobId: exportResponse.job.id,
                    targetPath: targetPath,
                    onClearSelection: onClearSelection,
                    onCompletion: onCompletion
                )
            }
            
        } catch {
            print("❌ [EXPORT_MANAGER] Export failed: \(error)")
            exportError = "Export failed: \(error.localizedDescription)"
            isExporting = false
            onCompletion(false)
        }
    }
    
    /// Poll for export completion
    private func pollExportCompletion(
        jobId: String,
        targetPath: String,
        onClearSelection: @escaping () -> Void,
        onCompletion: @escaping (Bool) -> Void
    ) async {
        var attempts = 0
        
        while attempts < maxPollingAttempts {
            do {
                attempts += 1
                let statusResponse = try await service.getExportStatus(jobId: jobId)
                
                print("📊 [EXPORT_MANAGER] Export status poll \(attempts): \(statusResponse.job.status)")
                
                if statusResponse.job.status == "Completed" {
                    print("🎉 [EXPORT_MANAGER] Export completed after \(attempts) polls")
                    await handleExportCompletion(
                        exportResponse: statusResponse,
                        targetPath: targetPath,
                        onClearSelection: onClearSelection,
                        onCompletion: onCompletion
                    )
                    return
                } else if statusResponse.job.status == "Failed" {
                    print("❌ [EXPORT_MANAGER] Export failed: \(statusResponse.job.errorMessage ?? "Unknown error")")
                    exportError = statusResponse.job.errorMessage ?? "Export failed"
                    isExporting = false
                    onCompletion(false)
                    return
                }
                
                // Wait 1 second before next poll
                try await Task.sleep(nanoseconds: 1_000_000_000)
                
            } catch {
                print("❌ [EXPORT_MANAGER] Error polling export status: \(error)")
                exportError = "Failed to check export status: \(error.localizedDescription)"
                isExporting = false
                onCompletion(false)
                return
            }
        }
        
        // Timeout reached
        print("⏰ [EXPORT_MANAGER] Export polling timed out")
        exportError = "Export timed out - please check Files app later"
        isExporting = false
        onCompletion(false)
    }
    
    /// Handle successful export completion
    private func handleExportCompletion(
        exportResponse: ExportJobResponse,
        targetPath: String,
        onClearSelection: @escaping () -> Void,
        onCompletion: @escaping (Bool) -> Void
    ) async {
        // Clear selection and export state
        onClearSelection()
        isExporting = false
        
        // Show success message
        let entityCount = exportResponse.job.totalEntities ?? 0
        print("🎉 [EXPORT_MANAGER] Export successful: \(entityCount) records exported to \(targetPath)")
        
        if entityCount > 0 {
            // Open Files app to the export location
            if let url = URL(string: "shareddocuments://") {
                await UIApplication.shared.open(url)
            }
            onCompletion(true)
        } else {
            // Show error if no records were exported
            exportError = "No records were exported. The selected items may not exist or be accessible."
            onCompletion(false)
        }
    }
    
    /// Create export directory
    private func createExportDirectory() throws -> URL {
        let documentsURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        let exportFolderURL = documentsURL.appendingPathComponent("ActionAid_Exports")
        
        // Create directory if it doesn't exist
        try FileManager.default.createDirectory(at: exportFolderURL, withIntermediateDirectories: true)
        
        return exportFolderURL
    }
    
    /// Create timestamped export file path
    private func createExportFilePath(folder: URL, format: ExportFormat) throws -> String {
        let timestampFormatter = DateFormatter()
        timestampFormatter.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        let timestamp = timestampFormatter.string(from: Date())
        
        let exportFileName = "\(service.filePrefix)_selected_\(timestamp).\(format.fileExtension)"
        return folder.appendingPathComponent(exportFileName).path
    }
    
    /// Reset export state
    func resetExportState() {
        isExporting = false
        exportError = nil
    }
}

/// Strategic Goals export service implementation
struct StrategicGoalExportService: DomainExportService {
    var domainName: String = "Strategic Goals"
    var filePrefix: String = "strategic_goals"
    
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await StrategicGoalService.shared.exportStrategicGoalsByIds(
            ids: ids,
            includeBlobs: includeBlobs,
            format: format,
            targetPath: targetPath,
            token: token
        )
    }
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await StrategicGoalService.shared.getExportStatus(jobId: jobId)
    }
}

/// Project export service implementation
struct ProjectExportService: DomainExportService {
    var domainName: String = "Projects"
    var filePrefix: String = "projects"
    
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await ProjectService.shared.exportProjectsByIds(
            ids: ids,
            includeBlobs: includeBlobs,
            format: format,
            targetPath: targetPath,
            token: token
        )
    }
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await ProjectService.shared.getExportStatus(jobId: jobId)
    }
}

struct UserExportService: DomainExportService {
    var domainName: String = "Users"
    var filePrefix: String = "users"
    
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        // TODO: Implement when UserService is created
        throw NSError(domain: "NotImplemented", code: 0, userInfo: [NSLocalizedDescriptionKey: "User export not yet implemented"])
    }
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        // TODO: Implement when UserService is created
        throw NSError(domain: "NotImplemented", code: 0, userInfo: [NSLocalizedDescriptionKey: "User export status not yet implemented"])
    }
} 