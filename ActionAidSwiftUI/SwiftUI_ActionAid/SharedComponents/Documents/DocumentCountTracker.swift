//
//  DocumentCountTracker.swift
//  ActionAid SwiftUI
//
//  Generic document count tracking and caching for any entity type
//

import SwiftUI
import Foundation

// MARK: - Document Count Cache Entry
struct DocumentCountCacheEntry {
    let count: Int
    let timestamp: Date
    let compressionInfo: CompressionInfo?
    
    struct CompressionInfo {
        let activeCompressions: Int
        let failedCompressions: Int
        let pendingCompressions: Int
    }
    
    var isStale: Bool {
        Date().timeIntervalSince(timestamp) > 30.0 // Cache for 30 seconds
    }
}

// MARK: - Document Count Configuration
struct DocumentCountConfig {
    let tableName: String
    let refreshInterval: TimeInterval
    let cacheTimeout: TimeInterval
    let enableCompressionTracking: Bool
    let debugLogging: Bool
    
    init(
        tableName: String,
        refreshInterval: TimeInterval = 30.0,
        cacheTimeout: TimeInterval = 30.0,
        enableCompressionTracking: Bool = true,
        debugLogging: Bool = false
    ) {
        self.tableName = tableName
        self.refreshInterval = refreshInterval
        self.cacheTimeout = cacheTimeout
        self.enableCompressionTracking = enableCompressionTracking
        self.debugLogging = debugLogging
    }
    
    static let strategicGoals = DocumentCountConfig(
        tableName: "strategic_goals",
        refreshInterval: 30.0,
        cacheTimeout: 30.0,
        enableCompressionTracking: true,
        debugLogging: true
    )
    
    static let projects = DocumentCountConfig(
        tableName: "projects",
        refreshInterval: 30.0,
        cacheTimeout: 30.0,
        enableCompressionTracking: true,
        debugLogging: false
    )
    
    static let users = DocumentCountConfig(
        tableName: "users",
        refreshInterval: 60.0, // Less frequent for users
        cacheTimeout: 60.0,
        enableCompressionTracking: false,
        debugLogging: false
    )
}

// MARK: - Document Count Tracker
@MainActor
class DocumentCountTracker: ObservableObject {
    // MARK: - Published State
    @Published var documentCounts: [String: Int] = [:]
    @Published var isLoading = false
    @Published var hasActiveCompressions = false
    @Published var lastRefreshTime = Date.distantPast
    
    // MARK: - Private State
    private var cache: [String: DocumentCountCacheEntry] = [:]
    private var refreshTimer: Timer?
    private var lastCompressionCount = 0
    private let documentHandler = DocumentFFIHandler()
    
    // MARK: - Configuration
    let config: DocumentCountConfig
    
    // MARK: - Initialization
    init(config: DocumentCountConfig) {
        self.config = config
    }
    
    deinit {
        // Use Task to properly handle MainActor isolation
        Task { @MainActor in
            self.stopRefreshTimer()
        }
    }
    
    // MARK: - Public Methods
    
    /// Load document counts for a list of entity IDs
    func loadDocumentCounts(
        for entityIds: [String],
        auth: AuthCtxDto,
        withDelay: Bool = false
    ) async {
        if config.debugLogging {
            print("📎 [DOCUMENT_COUNT_TRACKER] Loading counts for \(entityIds.count) entities (table: \(config.tableName))")
        }
        
        // Check cache first
        let cachedResults = getCachedCounts(for: entityIds)
        if !cachedResults.isEmpty {
            if config.debugLogging {
                print("📎 [DOCUMENT_COUNT_TRACKER] Using cached results for \(cachedResults.count) entities")
            }
            updateDocumentCounts(with: cachedResults)
        }
        
        // Determine which entities need fresh data
        let staleEntityIds = entityIds.filter { entityId in
            guard let cacheEntry = cache[entityId] else { return true }
            return cacheEntry.isStale
        }
        
        if staleEntityIds.isEmpty && !cachedResults.isEmpty {
            if config.debugLogging {
                print("📎 [DOCUMENT_COUNT_TRACKER] All data is fresh from cache")
            }
            return
        }
        
        if withDelay {
            // Add delay to ensure backend has processed uploads
            try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
        }
        
        isLoading = true
        
        let result = await documentHandler.getDocumentCountsByEntities(
            relatedEntityIds: staleEntityIds.isEmpty ? entityIds : staleEntityIds,
            relatedTable: config.tableName,
            auth: auth
        )
        
        isLoading = false
        lastRefreshTime = Date()
        
        switch result {
        case .success(let documentCounts):
            if config.debugLogging {
                print("📎 [DOCUMENT_COUNT_TRACKER] Backend returned \(documentCounts.count) count responses")
            }
            
            await processDocumentCountResults(documentCounts, for: entityIds)
            
        case .failure(let error):
            if config.debugLogging {
                print("❌ [DOCUMENT_COUNT_TRACKER] Backend function failed: \(error)")
            }
            
            // Fallback: Set all counts to 0 to prevent UI inconsistencies
            await fallbackToZeroCounts(for: entityIds)
        }
    }
    
    /// Get cached count for a specific entity
    func getCachedCount(for entityId: String) -> Int? {
        guard let cacheEntry = cache[entityId], !cacheEntry.isStale else {
            return nil
        }
        return cacheEntry.count
    }
    
    /// Force refresh all counts (ignoring cache)
    func forceRefresh(for entityIds: [String], auth: AuthCtxDto) async {
        cache.removeAll()
        await loadDocumentCounts(for: entityIds, auth: auth)
    }
    
    /// Start automatic refresh timer for active compressions
    func startRefreshTimer(for entityIds: [String], auth: AuthCtxDto) {
        guard refreshTimer == nil else { return }
        
        if config.debugLogging {
            print("📎 [DOCUMENT_COUNT_TRACKER] Starting refresh timer (interval: \(config.refreshInterval)s)")
        }
        
        refreshTimer = Timer.scheduledTimer(withTimeInterval: config.refreshInterval, repeats: true) { _ in
            Task { @MainActor in
                if self.hasActiveCompressions {
                    await self.loadDocumentCounts(for: entityIds, auth: auth)
                }
            }
        }
    }
    
    /// Stop automatic refresh timer
    func stopRefreshTimer() {
        refreshTimer?.invalidate()
        refreshTimer = nil
        
        if config.debugLogging {
            print("📎 [DOCUMENT_COUNT_TRACKER] Stopped refresh timer")
        }
    }
    
    /// Clear all cached data
    func clearCache() {
        cache.removeAll()
        documentCounts.removeAll()
        hasActiveCompressions = false
        
        if config.debugLogging {
            print("📎 [DOCUMENT_COUNT_TRACKER] Cleared all cached data")
        }
    }
    
    // MARK: - Private Methods
    
    private func getCachedCounts(for entityIds: [String]) -> [String: Int] {
        var results: [String: Int] = [:]
        
        for entityId in entityIds {
            if let cacheEntry = cache[entityId], !cacheEntry.isStale {
                results[entityId] = cacheEntry.count
            }
        }
        
        return results
    }
    
    private func updateDocumentCounts(with counts: [String: Int]) {
        for (entityId, count) in counts {
            documentCounts[entityId] = count
        }
    }
    
    private func processDocumentCountResults(
        _ countResponses: [DocumentCountResponse],
        for allEntityIds: [String]
    ) async {
        var newCounts: [String: Int] = [:]
        var activeCompressions = 0
        
        // Process backend responses
        for countResponse in countResponses {
            let count = Int(countResponse.documentCount)
            newCounts[countResponse.entityId] = count
            
            // Update cache with new data
            let compressionInfo = config.enableCompressionTracking ? 
                DocumentCountCacheEntry.CompressionInfo(
                    activeCompressions: 0, // TODO: Add compression status tracking
                    failedCompressions: 0,
                    pendingCompressions: 0
                ) : nil
            
            cache[countResponse.entityId] = DocumentCountCacheEntry(
                count: count,
                timestamp: Date(),
                compressionInfo: compressionInfo
            )
            
            if config.debugLogging && count > 0 {
                print("📎 [DOCUMENT_COUNT_TRACKER] Entity \(countResponse.entityId) has \(count) documents")
            }
        }
        
        // Ensure all requested entities have an entry (even if 0)
        for entityId in allEntityIds {
            if newCounts[entityId] == nil {
                newCounts[entityId] = 0
                
                cache[entityId] = DocumentCountCacheEntry(
                    count: 0,
                    timestamp: Date(),
                    compressionInfo: nil
                )
            }
        }
        
        // Update published state
        updateDocumentCounts(with: newCounts)
        updateCompressionStatus(activeCompressions: activeCompressions)
        
        if config.debugLogging {
            let entitiesWithDocs = newCounts.filter { $0.value > 0 }.count
            print("📎 [DOCUMENT_COUNT_TRACKER] ✅ Completed: \(entitiesWithDocs)/\(allEntityIds.count) entities have documents")
        }
    }
    
    private func updateCompressionStatus(activeCompressions: Int) {
        let newHasActiveCompressions = activeCompressions > 0
        let compressionsFinished = lastCompressionCount > activeCompressions && lastCompressionCount > 0
        
        if newHasActiveCompressions != hasActiveCompressions {
            hasActiveCompressions = newHasActiveCompressions
            
            if config.debugLogging {
                if hasActiveCompressions {
                    print("🔄 [DOCUMENT_COUNT_TRACKER] \(activeCompressions) documents are compressing")
                } else {
                    print("✅ [DOCUMENT_COUNT_TRACKER] All compressions completed")
                }
            }
        } else if compressionsFinished && config.debugLogging {
            print("⚡ [DOCUMENT_COUNT_TRACKER] \(lastCompressionCount - activeCompressions) compression(s) just finished")
        }
        
        lastCompressionCount = activeCompressions
    }
    
    private func fallbackToZeroCounts(for entityIds: [String]) async {
        var zeroCounts: [String: Int] = [:]
        
        for entityId in entityIds {
            zeroCounts[entityId] = 0
            
            cache[entityId] = DocumentCountCacheEntry(
                count: 0,
                timestamp: Date(),
                compressionInfo: nil
            )
        }
        
        updateDocumentCounts(with: zeroCounts)
        hasActiveCompressions = false
        
        if config.debugLogging {
            print("📎 [DOCUMENT_COUNT_TRACKER] Applied fallback zero counts for \(entityIds.count) entities")
        }
    }
}

// MARK: - Document Count Extensions
extension Array where Element: Identifiable {
    /// Get entity IDs as strings for document counting
    var entityIds: [String] {
        return self.map { String(describing: $0.id) }
    }
}

// MARK: - Document Count View Modifier
extension View {
    /// Apply document count tracking to any view
    func withDocumentCounting<Entity: Identifiable>(
        entities: [Entity],
        tracker: DocumentCountTracker,
        auth: AuthCtxDto
    ) -> some View {
        self
            .onAppear {
                Task {
                    await tracker.loadDocumentCounts(for: entities.entityIds, auth: auth)
                    tracker.startRefreshTimer(for: entities.entityIds, auth: auth)
                }
            }
            .onDisappear {
                tracker.stopRefreshTimer()
            }
            .onChange(of: entities.count) { oldCount, newCount in
                if newCount != oldCount {
                    Task {
                        await tracker.loadDocumentCounts(for: entities.entityIds, auth: auth)
                    }
                }
            }
    }
}

// MARK: - Entity Document Count Protocol
protocol DocumentCountable {
    var id: String { get }
    
    @MainActor func hasDocuments(in tracker: DocumentCountTracker) -> Bool
    @MainActor func documentCount(in tracker: DocumentCountTracker) -> Int
}

extension DocumentCountable {
    @MainActor func hasDocuments(in tracker: DocumentCountTracker) -> Bool {
        return documentCount(in: tracker) > 0
    }
    
    @MainActor func documentCount(in tracker: DocumentCountTracker) -> Int {
        return tracker.documentCounts[id] ?? 0
    }
}

// MARK: - Convenience Extensions
extension StrategicGoalResponse: DocumentCountable {
    // DocumentCountable protocol methods are automatically inherited
}

// Add similar extensions for other entity types as needed
// extension ProjectResponse: DocumentCountable { ... }
// extension UserResponse: DocumentCountable { ... } 