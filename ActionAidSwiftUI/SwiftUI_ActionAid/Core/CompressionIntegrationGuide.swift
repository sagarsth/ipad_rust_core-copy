//
//  CompressionIntegrationGuide.swift
//  SwiftUI_ActionAid
//
//  Integration guide for using UnifiedCompressionService across all domains
//  This file provides examples and best practices for domain-agnostic compression
//

import Foundation
import SwiftUI

// MARK: - Integration Examples

/// Example integration for Strategic Goals domain
class StrategicGoalCompressionIntegration {
    
    /// Call this after uploading a document in Strategic Goals
    static func handleDocumentUpload(documentId: String, isHighPriority: Bool = false) {
        let priority: CompressionPriority = isHighPriority ? .high : .normal
        
        // Queue for compression automatically - wrap in Task for @MainActor
        Task { @MainActor in
            UnifiedCompressionService.shared.queueStrategicGoalDocument(
                documentId: documentId, 
                priority: priority
            )
        }
        
        print("📄 [StrategicGoals] Document \(documentId) queued for compression")
    }
    
    /// Async version - Call this after uploading a document in Strategic Goals
    @MainActor
    static func handleDocumentUploadAsync(documentId: String, isHighPriority: Bool = false) async {
        let priority: CompressionPriority = isHighPriority ? .high : .normal
        
        UnifiedCompressionService.shared.queueStrategicGoalDocument(
            documentId: documentId, 
            priority: priority
        )
        
        print("📄 [StrategicGoals] Document \(documentId) queued for compression")
    }
    
    /// Check compression status for a strategic goal document
    static func checkCompressionStatus(documentId: String, completion: @escaping (String) -> Void) {
        Task { @MainActor in
            UnifiedCompressionService.shared.getDocumentStatus(documentId: documentId) { result in
                switch result {
                case .success(let status):
                    completion(status.currentStatus ?? "unknown")
                case .failure(let error):
                    print("❌ [StrategicGoals] Failed to get compression status: \(error)")
                    completion("error")
                }
            }
        }
    }
}

/// Example integration for Users domain
class UserCompressionIntegration {
    
    /// Call this after uploading a user profile document
    static func handleUserDocumentUpload(documentId: String) {
        // User documents typically get normal priority - wrap in Task for @MainActor
        Task { @MainActor in
            UnifiedCompressionService.shared.queueUserDocument(
                documentId: documentId, 
                priority: .normal
            )
        }
        
        print("📄 [Users] User document \(documentId) queued for compression")
    }
    
    /// Handle profile picture upload (higher priority for better UX)
    static func handleProfilePictureUpload(documentId: String) {
        Task { @MainActor in
            UnifiedCompressionService.shared.queueUserDocument(
                documentId: documentId, 
                priority: .high // Profile pictures should compress quickly
            )
        }
        
        print("📸 [Users] Profile picture \(documentId) queued for high-priority compression")
    }
}

/// Example integration for Donors domain
class DonorCompressionIntegration {
    
    /// Call this after uploading donor documentation
    static func handleDonorDocumentUpload(documentId: String, isLegalDocument: Bool = false) {
        let priority: CompressionPriority = isLegalDocument ? .high : .normal
        
        Task { @MainActor in
            UnifiedCompressionService.shared.queueDonorDocument(
                documentId: documentId, 
                priority: priority
            )
        }
        
        print("📄 [Donors] Donor document \(documentId) queued with priority \(priority.rawValue)")
    }
}

/// Example integration for Projects domain
class ProjectCompressionIntegration {
    
    /// Call this after uploading project documentation
    static func handleProjectDocumentUpload(documentId: String, isReportDocument: Bool = false) {
        let priority: CompressionPriority = isReportDocument ? .high : .normal
        
        Task { @MainActor in
            UnifiedCompressionService.shared.queueProjectDocument(
                documentId: documentId, 
                priority: priority
            )
        }
        
        print("📄 [Projects] Project document \(documentId) queued for compression")
    }
    
    /// Handle bulk document upload for projects
    static func handleBulkDocumentUpload(documentIds: [String]) {
        Task { @MainActor in
            for documentId in documentIds {
                UnifiedCompressionService.shared.queueProjectDocument(
                    documentId: documentId, 
                    priority: .background // Bulk uploads use background priority
                )
            }
        }
        
        print("📦 [Projects] \(documentIds.count) documents queued for background compression")
    }
}

/// Example integration for Activities domain
class ActivityCompressionIntegration {
    
    /// Call this after uploading activity documentation
    static func handleActivityDocumentUpload(documentId: String) {
        Task { @MainActor in
            UnifiedCompressionService.shared.queueActivityDocument(
                documentId: documentId, 
                priority: .normal
            )
        }
        
        print("📄 [Activities] Activity document \(documentId) queued for compression")
    }
    
    /// Handle activity photo uploads
    static func handleActivityPhotoUpload(documentId: String) {
        Task { @MainActor in
            UnifiedCompressionService.shared.queueActivityDocument(
                documentId: documentId, 
                priority: .high // Photos should compress quickly for better UX
            )
        }
        
        print("📸 [Activities] Activity photo \(documentId) queued for high-priority compression")
    }
}

/// Example integration for Livelihoods domain
class LivelihoodCompressionIntegration {
    
    /// Call this after uploading livelihood documentation
    static func handleLivelihoodDocumentUpload(documentId: String) {
        Task { @MainActor in
            UnifiedCompressionService.shared.queueLivelihoodDocument(
                documentId: documentId, 
                priority: .normal
            )
        }
        
        print("📄 [Livelihoods] Livelihood document \(documentId) queued for compression")
    }
}

/// Generic integration for future domains
class GenericDomainCompressionIntegration {
    
    /// Call this for any new domain that supports document uploads
    static func handleDocumentUpload(
        domain: String, 
        documentId: String, 
        priority: CompressionPriority = .normal
    ) {
        Task { @MainActor in
            UnifiedCompressionService.shared.queueDomainDocument(
                domain: domain,
                documentId: documentId, 
                priority: priority
            )
        }
        
        print("📄 [\(domain)] Document \(documentId) queued for compression")
    }
}

// MARK: - SwiftUI Integration Examples

/// SwiftUI view that shows compression status - Integration example
struct CompressionIntegrationStatusView: View {
    @StateObject private var compressionService = UnifiedCompressionService.shared
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Compression Service")
                .font(.headline)
            
            HStack {
                Circle()
                    .fill(compressionService.isActive ? .green : .red)
                    .frame(width: 8, height: 8)
                
                Text(compressionService.isActive ? "Active" : "Inactive")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            if let status = compressionService.currentStatus {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Active Jobs: \(status.iosWorkerStatus.activeJobs)")
                        .font(.caption)
                    
                    Text("Device: \(status.iosWorkerStatus.iosState.batteryLevel * 100, specifier: "%.0f")% battery")
                        .font(.caption)
                    
                    if compressionService.isThrottled {
                        Text("⚠️ Throttled: \(compressionService.throttleReason ?? "Unknown")")
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                }
            }
            
            if let queueStatus = compressionService.queueStatus {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Queue Status:")
                        .font(.caption)
                        .fontWeight(.medium)
                    
                    Text("Pending: \(queueStatus.pending)")
                        .font(.caption)
                    
                    Text("Processing: \(queueStatus.inProgress)")
                        .font(.caption)
                    
                    Text("Completed: \(queueStatus.completed)")
                        .font(.caption)
                }
            }
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(8)
        .onAppear {
            compressionService.start()
        }
    }
}

// MARK: - Best Practices

/*
 COMPRESSION INTEGRATION BEST PRACTICES:
 
 1. **Start the Service Early**
    - Call UnifiedCompressionService.shared.start() in your App delegate or main view
    - The service handles iOS lifecycle automatically
 
 2. **Queue Documents Immediately After Upload**
    - Call the appropriate domain method right after successful upload
    - Don't wait for user interaction
 
 3. **Use Appropriate Priorities**
    - .high: User-facing content (profile pictures, immediate reports)
    - .normal: Regular documents
    - .low: Non-urgent content
    - .background: Bulk uploads, archived content
 
 4. **Handle All Domains Uniformly**
    - Strategic Goals: queueStrategicGoalDocument()
    - Users: queueUserDocument()
    - Donors: queueDonorDocument()
    - Projects: queueProjectDocument()
    - Activities: queueActivityDocument()
    - Livelihoods: queueLivelihoodDocument()
    - Future domains: queueDomainDocument()
 
 5. **Monitor Status (Optional)**
    - Use @StateObject to observe compression service in SwiftUI
    - Show compression progress for better UX
    - Handle throttling gracefully
 
 6. **Error Handling**
    - Compression failures are logged automatically
    - Failed jobs will retry automatically
    - No manual intervention needed
 
 7. **iOS Integration is Automatic**
    - Battery monitoring: ✅ Automatic
    - Memory pressure: ✅ Automatic
    - Thermal management: ✅ Automatic
    - Background tasks: ✅ Automatic
    - App lifecycle: ✅ Automatic
 
 8. **Swift Concurrency Support**
    - Use static methods for fire-and-forget queuing
    - Use async methods when you need to await completion
    - All @MainActor requirements handled automatically
 
 EXAMPLE USAGE IN YOUR DOMAIN:
 
 ```swift
 // Option 1: Fire-and-forget (non-async context)
 func handleDocumentUploadSuccess(documentId: String) {
     StrategicGoalCompressionIntegration.handleDocumentUpload(
         documentId: documentId,
         isHighPriority: false
     )
     // Continues immediately, compression queued in background
 }
 
 // Option 2: Async context
 func handleDocumentUploadSuccessAsync(documentId: String) async {
     await StrategicGoalCompressionIntegration.handleDocumentUploadAsync(
         documentId: documentId,
         isHighPriority: false
     )
     // Compression is queued before continuing
 }
 ```
 
 INTEGRATION CHECKLIST:
 
 ✅ Add compression call after document upload
 ✅ Choose appropriate priority level
 ✅ Start compression service in app initialization
 ✅ Optional: Add compression status UI
 ✅ Test on device (not simulator) for full iOS integration
 
 That's it! The service handles everything else automatically.
 */ 