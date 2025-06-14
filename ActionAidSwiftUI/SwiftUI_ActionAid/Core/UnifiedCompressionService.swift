//
//  UnifiedCompressionService.swift
//  SwiftUI_ActionAid
//
//  Unified compression service for all domains - handles document compression
//  across Strategic Goals, Users, Donors, Projects, Activities, and any future domains
//

import Foundation
import UIKit
import Combine

/// Unified compression service that works across all domains
@MainActor
class UnifiedCompressionService: ObservableObject {
    
    // MARK: - Published Properties
    @Published var isActive: Bool = false
    @Published var currentStatus: ComprehensiveIOSStatus?
    @Published var queueStatus: CompressionQueueStatusResponse?
    @Published var compressionStats: CompressionStatsResponse?
    @Published var deviceCapabilities: IOSDeviceCapabilities?
    @Published var isThrottled: Bool = false
    @Published var throttleReason: String?
    
    // MARK: - Private Properties
    nonisolated(unsafe) private var deviceMonitorTimer: Timer?
    private var backgroundTaskID: UIBackgroundTaskIdentifier = .invalid
    private var cancellables = Set<AnyCancellable>()
    
    // MARK: - Singleton
    static let shared = UnifiedCompressionService()
    
    private init() {
        setupNotificationObservers()
        detectDeviceCapabilities()
    }
    
    deinit {
        // Stop device monitoring (safe to call since method and property are nonisolated)
        stopDeviceMonitoring()
        NotificationCenter.default.removeObserver(self)
        
        print("🗑️ [UnifiedCompressionService] Service deallocated")
    }
    
    // MARK: - Public Interface
    
    /// Start the compression service with iOS integration
    func start() {
        guard !isActive else { return }
        
        print("🚀 [UnifiedCompressionService] Starting compression service")
        isActive = true
        
        // Detect device capabilities and optimize
        detectDeviceCapabilities()
        
        // Start device monitoring
        startDeviceMonitoring()
        
        // Initial status update
        updateStatus()
        
        print("✅ [UnifiedCompressionService] Compression service started")
    }
    
    /// Stop the compression service
    func stop() {
        guard isActive else { return }
        
        print("🛑 [UnifiedCompressionService] Stopping compression service")
        isActive = false
        
        stopDeviceMonitoring()
        endBackgroundTask()
        
        print("✅ [UnifiedCompressionService] Compression service stopped")
    }
    
    /// Queue a document for compression (domain-agnostic) with improved error handling
    func queueDocument(
        documentId: String, 
        priority: CompressionPriority = .normal,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        // Validate input parameters
        guard !documentId.isEmpty else {
            let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                NSLocalizedDescriptionKey: "Document ID cannot be empty"
            ])
            completion(.failure(error))
            return
        }
        
        let request = QueueDocumentRequest(documentId: documentId, priority: priority)
        
        do {
            let jsonData = try JSONEncoder().encode(request)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "Failed to encode request"
                ])
                completion(.failure(error))
                return
            }
            
            let result = jsonString.withCString { cString in
                compression_queue_document(cString)
            }
            
            if result == 0 {
                print("📄 [UnifiedCompressionService] Queued document \(documentId) with priority \(priority.rawValue)")
                completion(.success(()))
                updateQueueStatus()
            } else if result == 3 {
                // Document already queued - update priority instead
                print("🔄 [UnifiedCompressionService] Document \(documentId) already queued, updating priority to \(priority.rawValue)")
                updateDocumentPriority(documentId: documentId, priority: priority, completion: completion)
            } else {
                let errorMessage: String
                switch result {
                case 1: errorMessage = "Invalid document ID format"
                case 2: errorMessage = "Document not found"
                default: errorMessage = "Failed to queue document for compression"
                }
                
                let error = NSError(domain: "CompressionError", code: Int(result), userInfo: [
                    NSLocalizedDescriptionKey: errorMessage
                ])
                completion(.failure(error))
            }
        } catch {
            let wrappedError = NSError(domain: "CompressionError", code: -1, userInfo: [
                NSLocalizedDescriptionKey: "Failed to prepare compression request",
                NSUnderlyingErrorKey: error
            ])
            completion(.failure(wrappedError))
        }
    }
    
    /// Update priority for an already queued document
    private func updateDocumentPriority(
        documentId: String,
        priority: CompressionPriority,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        let updateRequest = UpdatePriorityRequest(documentId: documentId, priority: priority)
        
        do {
            let jsonData = try JSONEncoder().encode(updateRequest)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "Failed to encode priority update request"
                ])
                completion(.failure(error))
                return
            }
            
            let result = FFIHelper.execute(
                call: { resultPtr in
                    jsonString.withCString { cString in
                        compression_update_priority(cString, resultPtr)
                    }
                },
                parse: { response in
                    try JSONDecoder().decode(UpdatePriorityResponse.self, from: response.data(using: .utf8)!)
                },
                free: compression_free
            )
            
            if let updateResult = result.value, updateResult.updated {
                print("✅ [UnifiedCompressionService] Updated priority for document \(documentId) to \(priority.rawValue)")
                completion(.success(()))
                updateQueueStatus()
            } else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: result.error ?? "Failed to update document priority"
                ])
                completion(.failure(error))
            }
        } catch {
            let wrappedError = NSError(domain: "CompressionError", code: -1, userInfo: [
                NSLocalizedDescriptionKey: "Failed to update document priority",
                NSUnderlyingErrorKey: error
            ])
            completion(.failure(wrappedError))
        }
    }
    
    /// Compress a document immediately (domain-agnostic) with enhanced safety
    func compressDocument(
        documentId: String,
        config: CompressionConfig? = nil,
        completion: @escaping (Result<CompressionResultResponse, Error>) -> Void
    ) {
        // Validate input parameters
        guard !documentId.isEmpty else {
            let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                NSLocalizedDescriptionKey: "Document ID cannot be empty"
            ])
            completion(.failure(error))
            return
        }
        
        let request = CompressDocumentRequest(documentId: documentId, config: config)
        
        do {
            let jsonData = try JSONEncoder().encode(request)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "Failed to encode compression request"
                ])
                completion(.failure(error))
                return
            }
            
            // Ensure string length is reasonable to prevent memory issues
            guard jsonString.count < 50000 else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: "Request too large"
                ])
                completion(.failure(error))
                return
            }
            
            let result = FFIHelper.execute(
                call: { resultPtr in
                    jsonString.withCString { cString in
                        compression_compress_document(cString, resultPtr)
                    }
                },
                parse: { response in
                    guard let data = response.data(using: .utf8) else {
                        throw NSError(domain: "CompressionError", code: -1, userInfo: [
                            NSLocalizedDescriptionKey: "Invalid response format"
                        ])
                    }
                    return try JSONDecoder().decode(CompressionResultResponse.self, from: data)
                },
                free: compression_free
            )
            
            if let compressionResult = result.value {
                print("✅ [UnifiedCompressionService] Compressed document \(documentId)")
                print("   📊 Saved \(compressionResult.spaceSavedPercentage)% (\(compressionResult.spaceSavedBytes) bytes)")
                completion(.success(compressionResult))
                updateStats()
            } else {
                let errorMessage = result.error ?? "Compression operation failed"
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: errorMessage
                ])
                completion(.failure(error))
            }
        } catch {
            let wrappedError = NSError(domain: "CompressionError", code: -1, userInfo: [
                NSLocalizedDescriptionKey: "Failed to process compression request",
                NSUnderlyingErrorKey: error
            ])
            completion(.failure(wrappedError))
        }
    }
    
    /// Cancel compression for a document
    func cancelCompression(
        documentId: String,
        completion: @escaping (Result<Bool, Error>) -> Void
    ) {
        let request = DocumentIdRequest(documentId: documentId)
        
        do {
            let jsonData = try JSONEncoder().encode(request)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            let result = FFIHelper.execute(
                call: { resultPtr in
                    jsonString.withCString { cString in
                        compression_cancel(cString, resultPtr)
                    }
                },
                parse: { response in
                    try JSONDecoder().decode(CancelResponse.self, from: response.data(using: .utf8)!)
                },
                free: compression_free
            )
            
            if let cancelResult = result.value {
                completion(.success(cancelResult.cancelled))
            } else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: result.error ?? "Failed to cancel compression"
                ])
                completion(.failure(error))
            }
        } catch {
            completion(.failure(error))
        }
    }
    
    /// Get compression status for a document
    func getDocumentStatus(
        documentId: String,
        completion: @escaping (Result<DocumentHistoryResponse, Error>) -> Void
    ) {
        let request = DocumentIdRequest(documentId: documentId)
        
        do {
            let jsonData = try JSONEncoder().encode(request)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            let result = FFIHelper.execute(
                call: { resultPtr in
                    jsonString.withCString { cString in
                        compression_get_document_status(cString, resultPtr)
                    }
                },
                parse: { response in
                    try JSONDecoder().decode(DocumentHistoryResponse.self, from: response.data(using: .utf8)!)
                },
                free: compression_free
            )
            
            if let status = result.value {
                completion(.success(status))
            } else {
                let error = NSError(domain: "CompressionError", code: -1, userInfo: [
                    NSLocalizedDescriptionKey: result.error ?? "Failed to get document status"
                ])
                completion(.failure(error))
            }
        } catch {
            completion(.failure(error))
        }
    }
    
    // MARK: - iOS Device Integration
    
    /// Detect device capabilities and apply optimizations
    private func detectDeviceCapabilities() {
        let result = FFIHelper.execute(
            call: { resultPtr in
                compression_detect_ios_capabilities(resultPtr)
            },
            parse: { response in
                try JSONDecoder().decode(DeviceCapabilityDetectionResponse.self, from: response.data(using: .utf8)!)
            },
            free: compression_free
        )
        
        if let detection = result.value {
            self.deviceCapabilities = detection.detectedCapabilities
            print("📱 [UnifiedCompressionService] Device detected: \(detection.detectedCapabilities.deviceType)")
            print("   ⚡ Safe concurrency: \(detection.detectedCapabilities.safeConcurrency)")
            print("   🧠 Memory limit: \(detection.detectedCapabilities.memoryLimitMB)MB")
            
            for recommendation in detection.recommendations {
                print("   💡 \(recommendation)")
            }
        } else {
            print("⚠️ [UnifiedCompressionService] Failed to detect device capabilities: \(result.error ?? "Unknown error")")
        }
    }
    
    /// Start monitoring device state
    private func startDeviceMonitoring() {
        guard deviceMonitorTimer == nil else { return }
        
        // Enable battery monitoring
        UIDevice.current.isBatteryMonitoringEnabled = true
        
        // Start timer for regular updates
        deviceMonitorTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.updateDeviceState()
            }
        }
        
        // Initial update
        updateDeviceState()
        
        print("📊 [UnifiedCompressionService] Device monitoring started")
    }
    
    /// Stop monitoring device state
    nonisolated private func stopDeviceMonitoring() {
        deviceMonitorTimer?.invalidate()
        deviceMonitorTimer = nil
        UIDevice.current.isBatteryMonitoringEnabled = false
        
        print("📊 [UnifiedCompressionService] Device monitoring stopped")
    }
    
    /// Update device state to Rust
    private func updateDeviceState() {
        let batteryLevel = UIDevice.current.batteryLevel
        let isCharging = UIDevice.current.batteryState == .charging || UIDevice.current.batteryState == .full
        let thermalState = ProcessInfo.processInfo.thermalState
        let appState: String = {
            switch UIApplication.shared.applicationState {
            case .active: return "active"
            case .background: return "background"
            case .inactive: return "inactive"
            @unknown default: return "unknown"
            }
        }()
        
        let availableMemory = getAvailableMemoryMB()
        
        let stateUpdate = IOSStateUpdate(
            batteryLevel: batteryLevel,
            isCharging: isCharging,
            thermalState: thermalState.rawValue,
            appState: appState,
            availableMemoryMB: availableMemory
        )
        
        do {
            let jsonData = try JSONEncoder().encode(stateUpdate)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            let result = jsonString.withCString { cString in
                compression_update_ios_state(cString)
            }
            
            if result == 0 {
                // Update local state
                updateStatus()
            } else {
                print("⚠️ [UnifiedCompressionService] Failed to update device state")
            }
        } catch {
            print("❌ [UnifiedCompressionService] Error encoding device state: \(error)")
        }
    }
    
    /// Get available memory in MB
    private func getAvailableMemoryMB() -> UInt64 {
        var info = mach_task_basic_info()
        var count = mach_msg_type_number_t(MemoryLayout<mach_task_basic_info>.size)/4
        
        let kerr: kern_return_t = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: 1) {
                task_info(mach_task_self_,
                         task_flavor_t(MACH_TASK_BASIC_INFO),
                         $0,
                         &count)
            }
        }
        
        if kerr == KERN_SUCCESS {
            let usedMemory = info.resident_size
            let totalMemory = ProcessInfo.processInfo.physicalMemory
            let availableMemory = totalMemory - usedMemory
            return availableMemory / (1024 * 1024) // Convert to MB
        }
        
        return 0
    }
    
    // MARK: - Background Task Management
    
    /// Handle app entering background
    private func handleEnteringBackground() {
        print("📱 [UnifiedCompressionService] App entering background")
        
        // Start background task
        beginBackgroundTask()
        
        // Notify Rust
        let event = AppLifecycleEvent(event: "entering_background")
        do {
            let jsonData = try JSONEncoder().encode(event)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            jsonString.withCString { cString in
                compression_handle_app_lifecycle_event(cString)
            }
        } catch {
            print("❌ [UnifiedCompressionService] Error handling background event: \(error)")
        }
    }
    
    /// Handle app becoming active
    private func handleBecomingActive() {
        print("📱 [UnifiedCompressionService] App becoming active")
        
        // End background task
        endBackgroundTask()
        
        // Notify Rust
        let event = AppLifecycleEvent(event: "becoming_active")
        do {
            let jsonData = try JSONEncoder().encode(event)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            jsonString.withCString { cString in
                compression_handle_app_lifecycle_event(cString)
            }
        } catch {
            print("❌ [UnifiedCompressionService] Error handling active event: \(error)")
        }
        
        // Update device state
        updateDeviceState()
    }
    
    /// Begin background task
    private func beginBackgroundTask() {
        guard backgroundTaskID == .invalid else { return }
        
        backgroundTaskID = UIApplication.shared.beginBackgroundTask { [weak self] in
            // Background task expired
            self?.handleBackgroundTaskExpired()
        }
        
        if backgroundTaskID != .invalid {
            // Notify Rust of granted time (iOS typically gives 30 seconds)
            let backgroundTaskExtension = BackgroundTaskExtension(grantedSeconds: 30)
            do {
                let jsonData = try JSONEncoder().encode(backgroundTaskExtension)
                let jsonString = String(data: jsonData, encoding: .utf8)!
                
                jsonString.withCString { cString in
                    compression_handle_background_task_extension(cString)
                }
                
                print("🍎 [UnifiedCompressionService] Background task started (30 seconds)")
            } catch {
                print("❌ [UnifiedCompressionService] Error handling background task extension: \(error)")
            }
        }
    }
    
    /// End background task
    private func endBackgroundTask() {
        guard backgroundTaskID != .invalid else { return }
        
        UIApplication.shared.endBackgroundTask(backgroundTaskID)
        backgroundTaskID = .invalid
        
        // Notify Rust that background task ended
        let backgroundTaskExtension = BackgroundTaskExtension(grantedSeconds: 0)
        do {
            let jsonData = try JSONEncoder().encode(backgroundTaskExtension)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            jsonString.withCString { cString in
                compression_handle_background_task_extension(cString)
            }
            
            print("🍎 [UnifiedCompressionService] Background task ended")
        } catch {
            print("❌ [UnifiedCompressionService] Error handling background task end: \(error)")
        }
    }
    
    /// Handle background task expiration
    private func handleBackgroundTaskExpired() {
        print("⏰ [UnifiedCompressionService] Background task expired")
        endBackgroundTask()
    }
    
    // MARK: - Memory Pressure Handling
    
    /// Handle memory warning
    private func handleMemoryWarning() {
        print("🧠 [UnifiedCompressionService] Memory warning received")
        
        let availableMemory = getAvailableMemoryMB()
        let memoryWarning = EnhancedMemoryWarning(
            level: 2, // Critical
            availableMemoryMB: availableMemory,
            pressureTrend: "increasing"
        )
        
        do {
            let jsonData = try JSONEncoder().encode(memoryWarning)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            jsonString.withCString { cString in
                compression_handle_enhanced_memory_warning(cString)
            }
        } catch {
            print("❌ [UnifiedCompressionService] Error handling memory warning: \(error)")
        }
    }
    
    // MARK: - Content Visibility
    
    /// Handle content visibility change
    func handleContentVisibility(isVisible: Bool) {
        let visibility = ContentVisibility(isVisible: isVisible)
        
        do {
            let jsonData = try JSONEncoder().encode(visibility)
            let jsonString = String(data: jsonData, encoding: .utf8)!
            
            jsonString.withCString { cString in
                compression_handle_content_visibility(cString)
            }
            
            print("👀 [UnifiedCompressionService] Content visibility: \(isVisible ? "visible" : "hidden")")
        } catch {
            print("❌ [UnifiedCompressionService] Error handling content visibility: \(error)")
        }
    }
    
    // MARK: - Status Updates
    
    /// Update comprehensive status
    private func updateStatus() {
        let result = FFIHelper.execute(
            call: { resultPtr in
                compression_get_comprehensive_ios_status(resultPtr)
            },
            parse: { response in
                try JSONDecoder().decode(ComprehensiveIOSStatus.self, from: response.data(using: .utf8)!)
            },
            free: compression_free
        )
        
        if let status = result.value {
            self.currentStatus = status
            self.isThrottled = status.iosWorkerStatus.isThrottled
            self.throttleReason = status.iosWorkerStatus.throttleReason
        }
    }
    
    /// Update queue status
    private func updateQueueStatus() {
        let result = FFIHelper.execute(
            call: { resultPtr in
                compression_get_queue_status(resultPtr)
            },
            parse: { response in
                try JSONDecoder().decode(CompressionQueueStatusResponse.self, from: response.data(using: .utf8)!)
            },
            free: compression_free
        )
        
        if let status = result.value {
            self.queueStatus = status
        }
    }
    
    /// Update compression stats
    private func updateStats() {
        let result = FFIHelper.execute(
            call: { resultPtr in
                compression_get_stats(resultPtr)
            },
            parse: { response in
                try JSONDecoder().decode(CompressionStatsResponse.self, from: response.data(using: .utf8)!)
            },
            free: compression_free
        )
        
        if let stats = result.value {
            self.compressionStats = stats
        }
    }
    
    // MARK: - Notification Setup
    
    private func setupNotificationObservers() {
        // App lifecycle notifications
        NotificationCenter.default.addObserver(
            forName: UIApplication.didEnterBackgroundNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleEnteringBackground()
        }
        
        NotificationCenter.default.addObserver(
            forName: UIApplication.didBecomeActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleBecomingActive()
        }
        
        // Memory warning notifications
        NotificationCenter.default.addObserver(
            forName: UIApplication.didReceiveMemoryWarningNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.handleMemoryWarning()
        }
        
        // Thermal state notifications
        NotificationCenter.default.addObserver(
            forName: ProcessInfo.thermalStateDidChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.updateDeviceState()
        }
    }
}

// MARK: - Domain-Specific Extensions

extension UnifiedCompressionService {
    
    /// Queue document for Strategic Goals domain
    func queueStrategicGoalDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [StrategicGoals] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [StrategicGoals] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for Users domain
    func queueUserDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [Users] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [Users] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for Donors domain
    func queueDonorDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [Donors] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [Donors] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for Projects domain
    func queueProjectDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [Projects] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [Projects] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for Activities domain
    func queueActivityDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [Activities] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [Activities] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for Livelihoods domain
    func queueLivelihoodDocument(documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [Livelihoods] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [Livelihoods] Failed to queue document: \(error)")
            }
        }
    }
    
    /// Queue document for any future domain
    func queueDomainDocument(domain: String, documentId: String, priority: CompressionPriority = .normal) {
        queueDocument(documentId: documentId, priority: priority) { result in
            switch result {
            case .success():
                print("📄 [\(domain)] Document \(documentId) queued for compression")
            case .failure(let error):
                print("❌ [\(domain)] Failed to queue document: \(error)")
            }
        }
    }
} 