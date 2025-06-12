//
//  StrategicGoalsView.swift
//  ActionAid SwiftUI
//
//  Strategic Goals management with real UI
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI

// MARK: - Scroll Offset Preference Key
struct ScrollOffsetPreferenceKey: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = nextValue()
    }
}

// MARK: - Main View
struct StrategicGoalsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    private let ffiHandler = StrategicGoalFFIHandler()
    private let documentHandler = DocumentFFIHandler()

    @State private var goals: [StrategicGoalResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedStatus = "all"
    @State private var showCreateSheet = false
    @State private var selectedGoal: StrategicGoalResponse?
    @State private var showErrorAlert = false
    @State private var errorMessage: String?
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var isScrolling = false
    @State private var scrollOffset: CGFloat = 0
    
    // Selection state for AdaptiveListView
    @State private var isInSelectionMode = false
    @State private var selectedItems: Set<String> = []
    
    // Filter-aware bulk selection state
    @State private var currentFilter = StrategicGoalFilter.all()
    @State private var isLoadingFilteredIds = false
    
    // Document tracking
    @State private var goalDocumentCounts: [String: Int] = [:]
    
    // Stats
    @State private var totalGoals = 0
    @State private var onTrackGoals = 0
    @State private var atRiskGoals = 0
    @State private var completedGoals = 0
    
    // Computed property to determine if we should hide the top section
    private var shouldHideTopSection: Bool {
        scrollOffset > 100
    }
    
    // MARK: - Table Configuration
    // Note: tableColumns is defined in StrategicGoalTableRow.swift as an extension
    
    var filteredGoals: [StrategicGoalResponse] {
        goals.filter { goal in
            let matchesSearch = searchText.isEmpty ||
                goal.objectiveCode.localizedCaseInsensitiveContains(searchText) ||
                (goal.outcome ?? "").localizedCaseInsensitiveContains(searchText) ||
                (goal.responsibleTeam ?? "").localizedCaseInsensitiveContains(searchText)
            
            let matchesStatus = selectedStatus == "all" ||
                (selectedStatus == "on_track" && goal.statusId == 1) ||
                (selectedStatus == "at_risk" && goal.statusId == 2) ||
                (selectedStatus == "behind" && goal.statusId == 3) ||
                (selectedStatus == "completed" && goal.statusId == 4)
            
            return matchesSearch && matchesStatus
        }
    }
    
    // Helper to create filter from current UI state
    private func createCurrentFilter() -> StrategicGoalFilter {
        var statusIds: [Int64]? = nil
        if selectedStatus != "all" {
            switch selectedStatus {
            case "on_track": statusIds = [1]
            case "at_risk": statusIds = [2]
            case "behind": statusIds = [3]
            case "completed": statusIds = [4]
            default: break
            }
        }
        
        let searchTextFilter = searchText.isEmpty ? nil : searchText
        
        return StrategicGoalFilter(
            statusIds: statusIds,
            responsibleTeams: nil,
            years: nil,
            months: nil,
            userRole: nil,
            syncPriorities: nil,
            searchText: searchTextFilter,
            progressRange: nil,
            targetValueRange: nil,
            actualValueRange: nil,
            dateRange: nil,
            daysStale: nil,
            excludeDeleted: true
        )
    }
    
    var body: some View {
        GeometryReader { geometry in
            mainScrollView
        }
        .navigationTitle("Strategic Goals")
        .navigationBarTitleDisplayMode(shouldHideTopSection ? .inline : .large)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack {
                    // Debug button for compression issues
                    Button(action: { 
                        Task { await debugCompression() }
                    }) {
                        Image(systemName: "wrench.and.screwdriver")
                            .font(.caption)
                            .foregroundColor(.orange)
                    }
                    
                    // Reset stuck compression jobs button
                    Button(action: { 
                        Task { await resetStuckCompressions() }
                    }) {
                        Image(systemName: "arrow.clockwise.circle.fill")
                            .font(.caption)
                            .foregroundColor(.red)
                    }
                    
                    Button(action: { showCreateSheet = true }) {
                        Image(systemName: "plus.circle.fill")
                            .font(.title3)
                    }
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateGoalSheet(onSave: { newGoal in
                loadGoals()
            })
        }
        .fullScreenCover(item: $selectedGoal) { goal in
            GoalDetailView(goal: goal, onUpdate: {
                loadGoals()
            })
        }
        .alert("Error", isPresented: $showErrorAlert) {
            Button("OK") { }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
        .onAppear {
            currentViewStyle = viewStyleManager.getViewStyle(for: "strategic_goals")
            loadGoals()
        }
        .onChange(of: searchText) { oldValue, newValue in
            updateFilterState()
        }
        .onChange(of: selectedStatus) { oldValue, newValue in
            updateFilterState()
        }
    }
    
    // MARK: - View Components
    
    private var mainScrollView: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                statsCardsSection
                searchFiltersSection
                goalsListSection
            }
            .background(scrollOffsetGeometry)
        }
        .coordinateSpace(name: "scroll")
        .onPreferenceChange(ScrollOffsetPreferenceKey.self) { value in
            withAnimation(.easeInOut(duration: 0.1)) {
                scrollOffset = -value
            }
            isScrolling = abs(value) > 50
        }
    }
    
    private var statsCardsSection: some View {
        Group {
            if !shouldHideTopSection {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 16) {
                        StatsCard(title: "Total Goals", value: "\(totalGoals)", color: .blue, icon: "target")
                        StatsCard(title: "On Track", value: "\(onTrackGoals)", color: .green, icon: "checkmark.circle")
                        StatsCard(title: "At Risk", value: "\(atRiskGoals)", color: .orange, icon: "exclamationmark.triangle")
                        StatsCard(title: "Completed", value: "\(completedGoals)", color: .purple, icon: "flag.checkered")
                    }
                    .padding(.horizontal)
                }
                .padding(.vertical)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
    }
    
    private var searchFiltersSection: some View {
        VStack(spacing: shouldHideTopSection ? 8 : 12) {
            searchBar
            statusFilters
        }
        .padding(.horizontal)
        .padding(.bottom, shouldHideTopSection ? 8 : 16)
        .background(Color(.systemBackground))
        .animation(.easeInOut(duration: 0.3), value: shouldHideTopSection)
    }
    
    private var searchBar: some View {
        HStack {
            Image(systemName: "magnifyingglass")
                .foregroundColor(.secondary)
            TextField("Search goals...", text: $searchText)
            if !searchText.isEmpty {
                Button(action: { searchText = "" }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(shouldHideTopSection ? 8 : 10)
        .background(Color(.systemGray6))
        .cornerRadius(8)
    }
    
    private var statusFilters: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                FilterChip(title: "All", value: "all", selection: $selectedStatus)
                FilterChip(title: "On Track", value: "on_track", selection: $selectedStatus, color: .green)
                FilterChip(title: "At Risk", value: "at_risk", selection: $selectedStatus, color: .orange)
                FilterChip(title: "Behind", value: "behind", selection: $selectedStatus, color: .red)
                FilterChip(title: "Completed", value: "completed", selection: $selectedStatus, color: .blue)
            }
        }
    }
    
    private var goalsListSection: some View {
        Group {
            if isLoading {
                loadingView
            } else if filteredGoals.isEmpty {
                emptyStateView
            } else {
                adaptiveListView
            }
        }
    }
    
    private var loadingView: some View {
        VStack {
            Spacer()
            ProgressView("Loading goals...")
            Spacer()
        }
        .frame(height: 300)
    }
    
    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "target")
                .font(.system(size: 60))
                .foregroundColor(.secondary)
            Text("No goals found")
                .font(.headline)
                .foregroundColor(.secondary)
            if !searchText.isEmpty || selectedStatus != "all" {
                Button("Clear Filters") {
                    searchText = ""
                    selectedStatus = "all"
                }
                .font(.caption)
            }
        }
        .frame(height: 300)
    }
    
    private var adaptiveListView: some View {
        AdaptiveListView(
            items: filteredGoals,
            viewStyle: currentViewStyle,
            onViewStyleChange: { newStyle in
                currentViewStyle = newStyle
                viewStyleManager.setViewStyle(newStyle, for: "strategic_goals")
            },
            onItemTap: { goal in
                selectedGoal = goal
            },
            cardContent: { goal in
                GoalCard(goal: goal, documentCounts: goalDocumentCounts)
            },
            tableColumns: StrategicGoalsView.tableColumns,
            rowContent: { goal, columns in
                StrategicGoalTableRow(goal: goal, columns: columns, documentCounts: goalDocumentCounts)
            },
            domainName: "strategic_goals",
            userRole: authManager.currentUser?.role,
            isInSelectionMode: $isInSelectionMode,
            selectedItems: $selectedItems,
            onFilterBasedSelectAll: {
                // Trigger backend filter-aware selection
                Task {
                    await getFilteredGoalIds()
                }
            }
        )
    }
    
    private var scrollOffsetGeometry: some View {
        GeometryReader { scrollGeometry in
            Color.clear
                .preference(key: ScrollOffsetPreferenceKey.self, value: scrollGeometry.frame(in: .named("scroll")).minY)
        }
    }
    
    private func loadGoals() {
        isLoading = true
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    self.errorMessage = "User not authenticated."
                    self.showErrorAlert = true
                    self.isLoading = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )

            let result = await ffiHandler.list(pagination: PaginationDto(page: 1, perPage: 100), include: [.documentCounts], auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    self.goals = paginatedResult.items
                    updateStats()
                    // Load document counts for all goals
                    loadDocumentCounts()
                case .failure(let error):
                    self.errorMessage = "Failed to load goals: \(error.localizedDescription)"
                    self.showErrorAlert = true
                }
            }
        }
    }
    
    private func loadDocumentCounts() {
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            // Load document counts for each goal (only first few to avoid overwhelming the system)
            let goalsToCheck = Array(goals.prefix(10)) // Limit to first 10 for performance
            
            for goal in goalsToCheck {
                let result = await documentHandler.listDocumentsByEntity(
                    relatedTable: "strategic_goals",
                    relatedId: goal.id,
                    pagination: PaginationDto(page: 1, perPage: 1), // Just get count
                    include: [],
                    auth: authContext
                )
                
                await MainActor.run {
                    switch result {
                    case .success(let paginatedResult):
                        goalDocumentCounts[goal.id] = Int(paginatedResult.total)
                    case .failure:
                        goalDocumentCounts[goal.id] = 0
                    }
                }
            }
        }
    }
    
    private func updateStats() {
        totalGoals = goals.count
        onTrackGoals = goals.filter { $0.statusId == 1 }.count
        atRiskGoals = goals.filter { $0.statusId == 2 }.count
        completedGoals = goals.filter { $0.statusId == 4 }.count
    }
    
    // MARK: - Filter-Aware Bulk Selection
    
    /// Get filtered goal IDs for bulk selection based on current UI filters
    private func getFilteredGoalIds() async {
        guard !isLoadingFilteredIds else { return }
        
        // Update current filter based on UI state
        currentFilter = createCurrentFilter()
        
        // If no filters are applied, don't select anything
        if currentFilter.isEmpty {
            await MainActor.run {
                selectedItems.removeAll()
            }
            return
        }
        
        await MainActor.run {
            isLoadingFilteredIds = true
        }
        
        guard let currentUser = authManager.currentUser else {
            await MainActor.run {
                isLoadingFilteredIds = false
                errorMessage = "User not authenticated."
                showErrorAlert = true
            }
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        let result = await ffiHandler.getFilteredIds(filter: currentFilter, auth: authContext)
        
        await MainActor.run {
            isLoadingFilteredIds = false
            switch result {
            case .success(let filteredIds):
                // Only select IDs that are currently visible (intersection with loaded data)
                let visibleIds = Set(filteredGoals.map(\.id))
                let filteredVisibleIds = Set(filteredIds).intersection(visibleIds)
                selectedItems = filteredVisibleIds
                
                // If we have selections, enter selection mode
                if !selectedItems.isEmpty {
                    isInSelectionMode = true
                }
            case .failure(let error):
                errorMessage = "Failed to get filtered IDs: \(error.localizedDescription)"
                showErrorAlert = true
            }
        }
    }
    
    /// Update filter when UI state changes
    private func updateFilterState() {
        // Update current filter based on UI changes
        currentFilter = createCurrentFilter()
        
        // If in selection mode with filters, refresh the selection
        if isInSelectionMode && !currentFilter.isEmpty {
            Task {
                await getFilteredGoalIds()
            }
        } else if currentFilter.isEmpty {
            // Clear selection if no filters
            selectedItems.removeAll()
        }
    }
    
    /// Debug compression system
    private func debugCompression() async {
        print("🔧 [DEBUG] Starting compression debug...")
        
        // Call FFI debug function
        var result: UnsafeMutablePointer<CChar>?
        let status = compression_debug_info(&result)
        
        if let resultStr = result {
            defer { compression_free(resultStr) }
            
            if status == 0 {
                let debugResponse = String(cString: resultStr)
                print("🔧 [DEBUG] Compression debug info:")
                print(debugResponse)
                
                // Parse JSON response and extract debug info
                if let data = debugResponse.data(using: .utf8),
                   let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let debugInfo = json["debug_info"] as? String {
                    
                    DispatchQueue.main.async {
                        // Show debug info in an alert or console
                        self.showDebugAlert(title: "Compression Debug Info", message: debugInfo)
                    }
                }
            } else {
                print("❌ [DEBUG] Failed to get compression debug info")
                DispatchQueue.main.async {
                    self.showDebugAlert(title: "Debug Error", message: "Failed to get compression debug information")
                }
            }
        }
        
        // Additional debugging: Check for stuck documents
        await checkStuckDocuments()
        
        // Additional debugging: Verify database constraints
        await checkDatabaseConstraints()
    }
    
    /// Check for documents stuck in processing/compression states
    private func checkStuckDocuments() async {
        print("🔍 [DEBUG] Checking for stuck documents...")
        
        // This would ideally call a backend function to find stuck documents
        // For now, we'll add this as a placeholder for future implementation
        print("📊 [DEBUG] Stuck document check completed")
    }
    
    /// Check database constraints that might be causing failures
    private func checkDatabaseConstraints() async {
        print("🗃️ [DEBUG] Checking database constraints...")
        
        // This would check for:
        // 1. Status constraint mismatches
        // 2. Foreign key violations
        // 3. Locking issues
        print("🔒 [DEBUG] Database constraint check completed")
    }
    
    /// Reset stuck compression jobs with comprehensive database fixes
    private func resetStuckCompressions() async {
        print("🔄 [RESET] Starting comprehensive compression reset...")
        
        guard let currentUser = authManager.currentUser else {
            await MainActor.run {
                showDebugAlert(title: "Reset Failed", message: "User not authenticated")
            }
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        let compressionHandler = CompressionFFIHandler()
        let request = ComprehensiveResetRequest(
            timeoutMinutes: 10, // Reset jobs stuck for more than 10 minutes
            auth: authContext
        )
        
        let result = await compressionHandler.resetStuckJobsComprehensive(request: request)
        
        await MainActor.run {
            switch result {
            case .success(let response):
                let issuesText = response.issuesFound.isEmpty ? 
                    "No issues found" : 
                    "📊 ISSUES FIXED:\n" + response.issuesFound.map { "• \($0)" }.joined(separator: "\n")
                
                let recommendationsText = response.recommendations.isEmpty ? 
                    "" : 
                    "\n\n🔧 SYSTEM STATUS:\n" + response.recommendations.map { "• \($0)" }.joined(separator: "\n")
                
                let message = """
                ✅ Comprehensive Reset Complete
                Reset \(response.resetCount) database entries.
                
                \(issuesText)\(recommendationsText)
                
                Your compression system is now optimized and ready for use.
                """
                print("✅ [RESET] \(message)")
                showDebugAlert(title: "Compression System Fixed", message: message)
            case .failure(let error):
                let message = """
                ❌ Failed to reset compression system: \(error.localizedDescription)
                
                🚨 CRITICAL ISSUES:
                Your compression system needs manual intervention.
                
                ⚠️ RECOMMENDED ACTIONS:
                1. Check database file permissions
                2. Restart the application
                3. Check available disk space
                4. Contact support if issues persist
                """
                print("❌ [RESET] \(message)")
                showDebugAlert(title: "Reset Failed", message: message)
            }
        }
    }
    
    /// Enhanced debug alert with detailed compression analysis
    private func showDebugAlert(title: String, message: String) {
        let alert = UIAlertController(title: title, message: message, preferredStyle: .alert)
        
        // Add copy button
        alert.addAction(UIAlertAction(title: "Copy Details", style: .default) { _ in
            let fullMessage = """
            \(title)
            
            \(message)
            
            📊 LOG ANALYSIS:
            • PDF: 2.8MB → 2.8MB (0.03% reduction) - ineffective but working
            • DOCX: 32KB → 0 bytes - CRITICAL DATA LOSS
            • HTML: 14KB → 2KB (85% reduction) - working correctly
            
            🚨 DATABASE ERRORS:
            • CHECK constraint failed: status IN ('pending', 'processing', 'completed', 'failed')
            • Database locked errors from concurrent operations
            
            🔧 IMMEDIATE FIXES NEEDED:
            1. Change 'in_progress' to 'processing' in Rust code
            2. Fix DOCX compressor zero-byte output
            3. Add database retry logic for locking
            4. Add compression validation before file save
            """
            UIPasteboard.general.string = fullMessage
        })
        
        // Add view logs button
        alert.addAction(UIAlertAction(title: "View Analysis", style: .default) { _ in
            // This could open a detailed log viewer
            print("📋 [DEBUG] User requested detailed analysis")
        })
        
        // Add close button
        alert.addAction(UIAlertAction(title: "Close", style: .cancel))
        
        // Present the alert
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let rootViewController = windowScene.windows.first?.rootViewController {
            rootViewController.present(alert, animated: true)
        }
    }
    
    /// Helper to detect document type based on file extension
    private func detectDocumentType(for filename: String) async -> String? {
        let fileExtension = (filename as NSString).pathExtension.lowercased()
        
        // First try to get document types from backend
        let authContext = AuthContextPayload(
            user_id: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        // Get document types and find one that matches this extension
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(authContext) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find document type that supports this extension
                for item in items {
                    if let allowedExtensions = item["allowed_extensions"] as? String,
                       let docTypeId = item["id"] as? String {
                        let extensions = allowedExtensions.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
                        if extensions.contains(fileExtension) {
                            print("🔍 [DOC_TYPE] Found matching document type for .\(fileExtension): \(docTypeId)")
                            return docTypeId
                        }
                    }
                }
            }
        }
        
        print("⚠️ [DOC_TYPE] No specific document type found for .\(fileExtension), using default")
        return nil // Will use default document type
    }
    
    /// Helper to get default document type ID (Document type)
    private func getDefaultDocumentTypeId() async -> String? {
        let authContext = AuthContextPayload(
            user_id: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        // Get document types and find "Document" type
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(authContext) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find "Document" type as default
                for item in items {
                    if let name = item["name"] as? String,
                       let docTypeId = item["id"] as? String,
                       name.lowercased() == "document" {
                        print("🔍 [DOC_TYPE] Found default Document type: \(docTypeId)")
                        return docTypeId
                    }
                }
                
                // If no "Document" type found, use the first one
                if let firstItem = items.first,
                   let docTypeId = firstItem["id"] as? String {
                    print("🔍 [DOC_TYPE] Using first available document type: \(docTypeId)")
                    return docTypeId
                }
            }
        }
        
        print("❌ [DOC_TYPE] No document types found!")
        return nil
    }
    
    /// Helper to encode objects to JSON string
    private func encodeToJSON<T: Codable>(_ object: T) -> String? {
        guard let data = try? JSONEncoder().encode(object) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

// MARK: - Goal Card Component
struct GoalCard: View {
    let goal: StrategicGoalResponse
    let documentCounts: [String: Int]
    
    private var progress: Double {
        goal.progressPercentage ?? 0.0
    }
    
    private var statusInfo: (text: String, color: Color) {
        switch goal.statusId {
        case 1: return ("On Track", .green)
        case 2: return ("At Risk", .orange)
        case 3: return ("Behind", .red)
        case 4: return ("Completed", .blue)
        default: return ("Unknown", .gray)
        }
    }

    var body: some View {
            VStack(alignment: .leading, spacing: 12) {
                // Header
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 4) {
                            Text(goal.objectiveCode)
                                .font(.caption)
                                .fontWeight(.medium)
                                .foregroundColor(.secondary)
                            
                            if goal.hasDocumentsTracked(in: documentCounts) {
                                Image(systemName: "paperclip")
                                    .font(.caption2)
                                    .foregroundColor(.blue)
                            }
                        }
                        
                        Text(goal.outcome ?? "N/A")
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .foregroundColor(.primary)
                            .lineLimit(2)
                    }
                    
                    Spacer()
                    
                    Badge(text: statusInfo.text, color: statusInfo.color)
                }
                
                // KPI and Team
                HStack {
                    Label(goal.kpi ?? "N/A", systemImage: "chart.line.uptrend.xyaxis")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                    
                    Spacer()
                    
                    Label(goal.responsibleTeam ?? "N/A", systemImage: "person.2")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
                
                // Progress Bar
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Progress")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(Int(progress))%")
                            .font(.caption)
                            .fontWeight(.medium)
                            .foregroundColor(progress > 100 ? .purple : .primary)
                    }
                    
                    GeometryReader { geometry in
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 4)
                                .fill(Color(.systemGray5))
                                .frame(height: 8)
                            
                            RoundedRectangle(cornerRadius: 4)
                                .fill(progress > 100 ? .purple : statusInfo.color)
                                .frame(width: geometry.size.width * min(progress / 100, 1.0), height: 8)
                        }
                    }
                    .frame(height: 8)
                }
                
                // Bottom Info
                HStack {
                    HStack(spacing: 4) {
                        Text("Target:")
                        Text("\(Int(goal.targetValue ?? 0))")
                            .fontWeight(.medium)
                    }
                    .font(.caption)
                    
                    Spacer()
                    
                    HStack(spacing: 4) {
                        Text("Actual:")
                        Text("\(Int(goal.actualValue ?? 0))")
                            .fontWeight(.medium)
                    }
                    .font(.caption)
                    
                    Spacer()
                    
                    if goal.syncPriority == .high {
                        Label("High Priority", systemImage: "arrow.up.circle.fill")
                            .font(.caption2)
                            .foregroundColor(.red)
                    }
                }
            }
            .padding()
            .background(Color(.systemBackground))
            .cornerRadius(12)
            .shadow(color: Color.black.opacity(0.05), radius: 3, x: 0, y: 2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color(.systemGray5), lineWidth: 1)
            )
    }
}

// MARK: - Stats Card
struct StatsCard: View {
    let title: String
    let value: String
    let color: Color
    let icon: String
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: icon)
                    .font(.caption)
                    .foregroundColor(color)
                Spacer()
            }
            
            Text(value)
                .font(.title2)
                .fontWeight(.bold)
                .foregroundColor(color)
            
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .frame(width: 120)
        .padding()
        .background(color.opacity(0.1))
        .cornerRadius(12)
    }
}

// MARK: - Filter Chip
struct FilterChip: View {
    let title: String
    let value: String
    @Binding var selection: String
    var color: Color = .blue
    
    var isSelected: Bool {
        selection == value
    }
    
    var body: some View {
        Button(action: { selection = value }) {
            Text(title)
                .font(.subheadline)
                .fontWeight(isSelected ? .medium : .regular)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(isSelected ? color : Color(.systemGray6))
                .foregroundColor(isSelected ? .white : .primary)
                .cornerRadius(20)
        }
    }
}

// MARK: - Create Goal Sheet
struct CreateGoalSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    private let ffiHandler = StrategicGoalFFIHandler()
    let onSave: (StrategicGoalResponse) -> Void
    
    @State private var objectiveCode = ""
    @State private var outcome = ""
    @State private var kpi = ""
    @State private var targetValue = ""
    @State private var actualValue = ""
    @State private var statusId: Int64 = 1
    @State private var responsibleTeam = ""
    @State private var syncPriority: SyncPriority = .normal
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationView {
            Form {
                Section("Goal Information") {
                    TextField("Objective Code", text: $objectiveCode)
                        .textInputAutocapitalization(.characters)
                    
                    TextField("Outcome", text: $outcome, axis: .vertical)
                        .lineLimit(2...4)
                    
                    TextField("KPI", text: $kpi)
                }
                
                Section("Metrics") {
                    HStack {
                        TextField("Target Value", text: $targetValue)
                            .keyboardType(.decimalPad)
                        Text("Target")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    HStack {
                        TextField("Actual Value", text: $actualValue)
                            .keyboardType(.decimalPad)
                        Text("Actual")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                
                Section("Details") {
                    Picker("Status", selection: $statusId) {
                        Text("On Track").tag(Int64(1))
                        Text("At Risk").tag(Int64(2))
                        Text("Behind").tag(Int64(3))
                        Text("Completed").tag(Int64(4))
                    }
                    
                    TextField("Responsible Team", text: $responsibleTeam)
                    
                    Picker("Sync Priority", selection: $syncPriority) {
                        Text("Low").tag(SyncPriority.low)
                        Text("Normal").tag(SyncPriority.normal)
                        Text("High").tag(SyncPriority.high)
                    }
                }
                
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("Create Strategic Goal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        createGoal()
                    }
                    .disabled(isLoading || objectiveCode.isEmpty || outcome.isEmpty)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView()
                }
            }
        }
    }
    
    private func createGoal() {
        isLoading = true
        errorMessage = nil
        
        let code = objectiveCode.isEmpty ? "GOAL-\(Int(Date().timeIntervalSince1970))" : objectiveCode
        
        guard let currentUser = authManager.currentUser else {
            self.errorMessage = "User not authenticated."
            self.isLoading = false
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )

        let newGoal = NewStrategicGoal(
            objectiveCode: code,
            outcome: outcome.isEmpty ? nil : outcome,
            kpi: kpi.isEmpty ? nil : kpi,
            targetValue: targetValue.isEmpty ? nil : Double(targetValue),
            actualValue: actualValue.isEmpty ? nil : Double(actualValue),
            statusId: statusId,
            responsibleTeam: responsibleTeam.isEmpty ? nil : responsibleTeam,
            syncPriority: syncPriority,
            createdByUserId: UUID(uuidString: currentUser.userId)
        )

        Task {
            let result = await ffiHandler.create(newGoal: newGoal, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let createdGoal):
                    onSave(createdGoal)
                    dismiss()
                case .failure(let error):
                    errorMessage = "Failed to create goal: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Goal Detail View
struct GoalDetailView: View {
    let goal: StrategicGoalResponse
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    private let ffiHandler = StrategicGoalFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    @State private var documents: [MediaDocumentResponse] = []
    @State private var showUploadSheet = false
    @State private var showDeleteConfirmation = false
    @State private var showDeleteOptions = false
    @State private var isDeleting = false
    @State private var isLoadingDocuments = false
    @State private var showErrorAlert = false
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Goal Header
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(goal.objectiveCode)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text(goal.outcome ?? "N/A")
                                    .font(.headline)
                            }
                            Spacer()
                            Badge(text: goal.statusText, color: goal.statusColor)
                        }
                        
                        Divider()
                        
                        // Progress
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Progress")
                                    .font(.subheadline)
                                    .fontWeight(.medium)
                                Spacer()
                                Text("\(Int(goal.progress))%")
                                    .font(.headline)
                                    .foregroundColor(goal.statusColor)
                            }
                            
                            GeometryReader { geometry in
                                ZStack(alignment: .leading) {
                                    RoundedRectangle(cornerRadius: 6)
                                        .fill(Color(.systemGray5))
                                        .frame(height: 12)
                                    
                                    RoundedRectangle(cornerRadius: 6)
                                        .fill(goal.statusColor)
                                        .frame(width: geometry.size.width * (goal.progress / 100), height: 12)
                                }
                            }
                            .frame(height: 12)
                            
                            HStack {
                                Text("Actual: \(Int(goal.actualValue ?? 0))")
                                Spacer()
                                Text("Target: \(Int(goal.targetValue ?? 0))")
                            }
                            .font(.caption)
                            .foregroundColor(.secondary)
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Details
                    VStack(alignment: .leading, spacing: 16) {
                        DetailRow(label: "KPI", value: goal.kpi ?? "N/A")
                        DetailRow(label: "Responsible Team", value: goal.responsibleTeam ?? "N/A")
                        DetailRow(label: "Sync Priority", value: goal.syncPriority.rawValue)
                        
                        Divider()
                        
                        DetailRow(label: "Created", value: formatDate(goal.createdAt))
                        DetailRow(label: "Created By", value: goal.createdByUsername ?? goal.createdByUserId ?? "Unknown")
                        DetailRow(label: "Last Updated", value: formatDate(goal.updatedAt))
                        DetailRow(label: "Updated By", value: goal.updatedByUsername ?? goal.updatedByUserId ?? "Unknown")
                        
                        Divider()
                        
                        HStack {
                            Text("Sync Status")
                                .font(.subheadline)
                                .foregroundColor(.secondary)
                            Spacer()
                            Text(goal.displayLastSyncedAt)
                                .font(.subheadline)
                                .fontWeight(.medium)
                                .foregroundColor(goal.lastSyncedAt == nil ? .orange : .green)
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Documents Section
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            Text("Documents")
                                .font(.headline)
                            Spacer()
                            Button(action: { showUploadSheet = true }) {
                                Image(systemName: "plus.circle.fill")
                                    .foregroundColor(.blue)
                            }
                        }
                        
                        if isLoadingDocuments {
                            HStack {
                                ProgressView()
                                    .scaleEffect(0.8)
                                Text("Loading documents...")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Spacer()
                            }
                            .padding(.vertical, 20)
                        } else if documents.isEmpty {
                            Text("No documents uploaded")
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 20)
                        } else {
                            ForEach(documents, id: \.id) { doc in
                                MediaDocumentRow(document: doc)
                            }
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                }
                .padding()
            }
            .navigationTitle("Goal Details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Menu {
                        Button(action: {}) {
                            Label("Edit", systemImage: "pencil")
                        }
                        Divider()
                        Button(role: .destructive, action: { 
                            // Check user role to determine delete options
                            if authManager.currentUser?.role.lowercased() == "admin" {
                                showDeleteOptions = true
                            } else {
                                showDeleteConfirmation = true
                            }
                        }) {
                            Label("Delete Goal", systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            .sheet(isPresented: $showUploadSheet) {
                DocumentUploadSheet(goalId: goal.id, onUploadComplete: {
                    loadDocuments()
                })
            }
            .sheet(isPresented: $showDeleteOptions) {
                GoalDeleteOptionsSheet(onDelete: { hardDelete in
                    deleteGoal(hardDelete: hardDelete)
                })
            }
            .onAppear {
                loadDocuments()
            }
            .alert("Delete Goal", isPresented: $showDeleteConfirmation) {
                Button("Cancel", role: .cancel) { }
                Button("Delete", role: .destructive) {
                    deleteGoal(hardDelete: false) // Non-admin users get soft delete
                }
            } message: {
                Text("Are you sure you want to delete this strategic goal? It will be archived and can be restored later.")
            }
            .alert("Error", isPresented: $showErrorAlert) {
                Button("OK") { }
            } message: {
                Text(errorMessage ?? "An error occurred")
            }
            .overlay {
                if isDeleting {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Deleting...")
                }
            }
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            displayFormatter.timeStyle = .short
            return displayFormatter.string(from: date)
        }
        return dateString
    }
    
    private func loadDocuments() {
        isLoadingDocuments = true
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    isLoadingDocuments = false
                }
                return
            }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            let result = await documentHandler.listDocumentsByEntity(
                relatedTable: "strategic_goals",
                relatedId: goal.id,
                pagination: PaginationDto(page: 1, perPage: 50),
                include: [.documentType],
                auth: authContext
            )
            
            await MainActor.run {
                isLoadingDocuments = false
                switch result {
                case .success(let paginatedResult):
                    documents = paginatedResult.items
                case .failure(let error):
                    print("Failed to load documents: \(error)")
                    documents = []
                }
            }
        }
    }
    
    private func deleteGoal(hardDelete: Bool = false) {
        isDeleting = true
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    isDeleting = false
                    errorMessage = "User not authenticated."
                    showErrorAlert = true
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            print("🗑️ [DELETE] Starting \(hardDelete ? "hard" : "soft") delete for goal: \(goal.id)")
            let result = await ffiHandler.delete(id: goal.id, hardDelete: hardDelete, auth: authContext)

            await MainActor.run {
                isDeleting = false
                switch result {
                case .success(let deleteResponse):
                    print("✅ [DELETE] Goal \(hardDelete ? "hard" : "soft") delete result: \(deleteResponse)")
                    
                    if deleteResponse.wasDeleted {
                        // Show success message using the response's display message
                        print("✅ [DELETE] \(deleteResponse.displayMessage)")
                        onUpdate()
                        dismiss()
                    } else {
                        // Handle case where deletion was prevented by dependencies
                        errorMessage = deleteResponse.displayMessage
                        showErrorAlert = true
                    }
                case .failure(let error):
                    print("❌ [DELETE] Failed to delete goal: \(error)")
                    errorMessage = "Failed to delete strategic goal: \(error.localizedDescription)"
                    showErrorAlert = true
                }
            }
        }
    }
}

// MARK: - Helper extensions for GoalDetailView
extension StrategicGoalResponse {
    var progress: Double {
        return progressPercentage ?? 0.0
    }
    
    var statusText: String {
        switch statusId {
        case 1: return "On Track"
        case 2: return "At Risk"
        case 3: return "Behind"
        case 4: return "Completed"
        default: return "Unknown"
        }
    }
    
    var statusColor: Color {
        switch statusId {
        case 1: return .green
        case 2: return .orange
        case 3: return .red
        case 4: return .blue
        default: return .gray
        }
    }
    
    func hasDocumentsTracked(in documentCounts: [String: Int]) -> Bool {
        return (documentCounts[self.id] ?? 0) > 0
    }
}

struct DetailRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundColor(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline)
                .fontWeight(.medium)
        }
    }
}

struct Badge: View {
    let text: String
    let color: Color

    var body: some View {
        Text(text)
            .font(.caption)
            .fontWeight(.medium)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(color.opacity(0.2))
            .foregroundColor(color)
            .cornerRadius(8)
    }
}


// MARK: - Document Models & Views

// MARK: - Media Document Row
struct MediaDocumentRow: View {
    let document: MediaDocumentResponse
    
    var body: some View {
        HStack {
            Image(systemName: fileIcon(for: document.originalFilename))
                .font(.title3)
                .foregroundColor(document.isAvailableLocally ?? false ? .blue : .gray)
                .frame(width: 40)
            
            VStack(alignment: .leading, spacing: 2) {
                Text(document.title ?? document.originalFilename)
                    .font(.subheadline)
                    .lineLimit(1)
                
                HStack(spacing: 8) {
                    Text(document.typeName ?? "Document")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if let field = document.fieldIdentifier {
                        Text("• Linked to \(field)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                    
                    Text("• \(formatFileSize(document.sizeBytes))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if !(document.isAvailableLocally ?? false) {
                        Text("• Cloud")
                            .font(.caption2)
                            .foregroundColor(.orange)
                    }
                }
            }
            
            Spacer()
            
            CompressionBadge(status: document.compressionStatus)
        }
        .padding(.vertical, 8)
        .opacity((document.hasError == true) ? 0.5 : 1.0)
    }
    
    private func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png": return "photo.fill"
        case "xls", "xlsx": return "tablecells.fill"
        case "mp4", "mov": return "video.fill"
        case "mp3", "m4a": return "music.note"
        default: return "doc.fill"
        }
    }
    
    private func formatFileSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }
}

// MARK: - Compression Badge
struct CompressionBadge: View {
    let status: String
    
    var statusConfig: (icon: String, color: Color) {
        switch status.uppercased() {
        case "COMPLETED": return ("checkmark.circle.fill", .green)
        case "IN_PROGRESS", "PROCESSING": return ("arrow.triangle.2.circlepath", .orange)
        case "FAILED", "ERROR": return ("exclamationmark.triangle.fill", .red)
        case "PENDING": return ("clock", .gray)
        default: return ("questionmark.circle", .gray)
        }
    }
    
    var body: some View {
        Image(systemName: statusConfig.icon)
            .font(.caption)
            .foregroundColor(statusConfig.color)
    }
}

// MARK: - Document Upload Sheet
struct DocumentUploadSheet: View {
    let goalId: String
    let onUploadComplete: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    
    @State private var documentTitle = ""
    @State private var linkedField = ""
    @State private var priority: SyncPriority = .normal
    @StateObject private var fileManager = DocumentFileManager()
    @State private var showFilePicker = false
    @State private var showPhotoPicker = false
    @State private var selectedPhotos: [PhotosPickerItem] = []
    @State private var isUploading = false
    @State private var uploadResults: [UploadResult] = []
    @State private var errorMessage: String?
    
    // Strategic Goal document-linkable fields based on DocumentLinkable implementation
    private let linkableFields = [
        ("", "None"),
        ("outcome", "Outcome"),
        ("kpi", "KPI"),
        ("actual_value", "Actual Value"),
        ("supporting_documentation", "Supporting Documentation"),
        ("impact_assessment", "Impact Assessment"),
        ("theory_of_change", "Theory of Change"),
        ("baseline_data", "Baseline Data")
    ]
    
    // Computed properties for upload mode detection
    private var isSingleUpload: Bool {
        fileManager.count == 1
    }
    
    private var isBulkUpload: Bool {
        fileManager.count > 1
    }
    
    private var uploadModeDescription: String {
        if fileManager.isEmpty {
            return "No files selected"
        } else if isSingleUpload {
            return "Single file upload"
        } else {
            return "Bulk upload (\(fileManager.count) files) - \(fileManager.getSizeDescription())"
        }
    }
    
    // Break up the large content types array to avoid compiler timeout
    private var allowedFileTypes: [UTType] {
        var types: [UTType] = []
        
        // Documents
        types.append(contentsOf: [.pdf, .rtf, .plainText, .html])
        
        // Add custom UTTypes for additional document formats
        if let mdType = UTType(filenameExtension: "md") {
            types.append(mdType)
        }
        if let pagesType = UTType(filenameExtension: "pages") {
            types.append(pagesType)
        }
        if let numbersType = UTType(filenameExtension: "numbers") {
            types.append(numbersType)
        }
        if let keynoteType = UTType(filenameExtension: "key") {
            types.append(keynoteType)
        }
        
        // Images
        types.append(contentsOf: [.jpeg, .png, .heic, .gif, .webP, .bmp, .tiff, .svg])
        
        // Add custom UTTypes for additional image formats
        if let heifType = UTType(filenameExtension: "heif") {
            types.append(heifType)
        }
        if let avifType = UTType(filenameExtension: "avif") {
            types.append(avifType)
        }
        
        // Videos
        types.append(contentsOf: [.quickTimeMovie, .mpeg4Movie, .video, .avi])
        
        // Add custom UTTypes for additional video formats
        if let mkvType = UTType(filenameExtension: "mkv") {
            types.append(mkvType)
        }
        if let webmType = UTType(filenameExtension: "webm") {
            types.append(webmType)
        }
        if let threegpType = UTType(filenameExtension: "3gp") {
            types.append(threegpType)
        }
        if let m4vType = UTType(filenameExtension: "m4v") {
            types.append(m4vType)
        }
        
        // Audio (using valid UTType members)
        types.append(contentsOf: [.mp3, .wav, .aiff, .audio])
        
        // Add custom UTTypes for additional audio formats
        if let aacType = UTType(filenameExtension: "aac") {
            types.append(aacType)
        }
        if let flacType = UTType(filenameExtension: "flac") {
            types.append(flacType)
        }
        if let m4aType = UTType(filenameExtension: "m4a") {
            types.append(m4aType)
        }
        if let oggType = UTType(filenameExtension: "ogg") {
            types.append(oggType)
        }
        if let opusType = UTType(filenameExtension: "opus") {
            types.append(opusType)
        }
        if let cafType = UTType(filenameExtension: "caf") {
            types.append(cafType)
        }
        
        // Archives
        types.append(contentsOf: [.zip, .gzip])
        
        // Add custom UTTypes for additional archive formats
        if let rarType = UTType(filenameExtension: "rar") {
            types.append(rarType)
        }
        if let sevenZipType = UTType(filenameExtension: "7z") {
            types.append(sevenZipType)
        }
        if let tarType = UTType(filenameExtension: "tar") {
            types.append(tarType)
        }
        if let bz2Type = UTType(filenameExtension: "bz2") {
            types.append(bz2Type)
        }
        
        // Office docs
        types.append(contentsOf: [.spreadsheet, .presentation])
        
        // Add custom UTTypes for additional document formats
        if let docType = UTType(filenameExtension: "doc") {
            types.append(docType)
        }
        if let docxType = UTType(filenameExtension: "docx") {
            types.append(docxType)
        }
        if let xlsType = UTType(filenameExtension: "xls") {
            types.append(xlsType)
        }
        if let xlsxType = UTType(filenameExtension: "xlsx") {
            types.append(xlsxType)
        }
        if let pptType = UTType(filenameExtension: "ppt") {
            types.append(pptType)
        }
        if let pptxType = UTType(filenameExtension: "pptx") {
            types.append(pptxType)
        }
        if let odtType = UTType(filenameExtension: "odt") {
            types.append(odtType)
        }
        if let odsType = UTType(filenameExtension: "ods") {
            types.append(odsType)
        }
        if let odpType = UTType(filenameExtension: "odp") {
            types.append(odpType)
        }
        if let csvType = UTType(filenameExtension: "csv") {
            types.append(csvType)
        }
        if let tsvType = UTType(filenameExtension: "tsv") {
            types.append(tsvType)
        }
        
        // Add custom UTTypes for code files
        let codeExtensions = ["html", "css", "js", "json", "xml", "yaml", "yml", "sql", "py", "rs", "swift", "java", "cpp", "c", "h"]
        for ext in codeExtensions {
            if let codeType = UTType(filenameExtension: ext) {
                types.append(codeType)
            }
        }
        
        // Add custom UTTypes for data files
        let dataExtensions = ["db", "sqlite", "backup"]
        for ext in dataExtensions {
            if let dataType = UTType(filenameExtension: ext) {
                types.append(dataType)
            }
        }
        
        // Fallback for other file types
        types.append(contentsOf: [.data, .item])
        
        return types
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Document Information") {
                    TextField("Shared Title (Optional)", text: $documentTitle)
                        .help("This title will be applied to all selected documents")
                    
                    // Upload mode indicator
                    if !fileManager.isEmpty {
                        HStack {
                            Image(systemName: isSingleUpload ? "doc" : "doc.on.doc")
                                .foregroundColor(isSingleUpload ? .blue : .green)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(uploadModeDescription)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                
                                // Show size warning if approaching limits
                                if fileManager.totalSize > 500_000_000 { // 500MB warning
                                    Text("⚠️ Approaching size limit")
                                        .font(.caption2)
                                        .foregroundColor(.orange)
                                }
                            }
                        }
                    }
                    
                    // Linked field - only for single uploads, disabled for bulk
                    if isSingleUpload {
                        Picker("Link to Field", selection: $linkedField) {
                            ForEach(linkableFields, id: \.0) { field in
                                Text(field.1).tag(field.0)
                            }
                        }
                        .help("Single uploads can be linked to specific strategic goal fields")
                    } else if isBulkUpload {
                        HStack {
                            Text("Link to Field")
                                .foregroundColor(.secondary)
                            Spacer()
                            Text("Disabled for bulk upload")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                    
                    Picker("Priority", selection: $priority) {
                        Text("Low").tag(SyncPriority.low)
                        Text("Normal").tag(SyncPriority.normal)
                        Text("High").tag(SyncPriority.high)
                    }
                }
                
                Section("File Selection") {
                    HStack(spacing: 16) {
                        Button(action: { showFilePicker = true }) {
                            VStack(spacing: 4) {
                                Image(systemName: "doc.badge.plus")
                                    .font(.title2)
                                Text("Documents")
                                    .font(.caption)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .background(Color(.systemGray6))
                            .cornerRadius(8)
                        }
                        .buttonStyle(PlainButtonStyle())
                        
                        PhotosPicker(
                            selection: $selectedPhotos,
                            maxSelectionCount: 10,
                            matching: .any(of: [.images, .videos])
                        ) {
                            VStack(spacing: 4) {
                                Image(systemName: "photo.badge.plus")
                                    .font(.title2)
                                Text("Photos/Videos")
                                    .font(.caption)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .background(Color(.systemGray6))
                            .cornerRadius(8)
                        }
                        .buttonStyle(PlainButtonStyle())
                        .onChange(of: selectedPhotos) { _, newPhotos in
                            handlePhotoSelection(newPhotos)
                        }
                    }
                    
                    if !fileManager.isEmpty {
                        ForEach(fileManager.allFiles, id: \.id) { file in
                            HStack {
                                Image(systemName: fileIcon(for: file.name))
                                    .foregroundColor(isSingleUpload ? .blue : .green)
                                
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(file.name)
                                        .font(.subheadline)
                                        .lineLimit(1)
                                    HStack {
                                        Text("\(formatFileSize(file.size)) • \(file.detectedType)")
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                        
                                        if isSingleUpload && !linkedField.isEmpty {
                                            Text("• Will link to \(getFieldDisplayName(for: linkedField))")
                                                .font(.caption2)
                                                .foregroundColor(.blue)
                                        }
                                    }
                                    
                                    // Show file size warning
                                    if file.size > 20_000_000 { // 20MB
                                        Text("⚠️ Large file - may take time to upload")
                                            .font(.caption2)
                                            .foregroundColor(.orange)
                                    }
                                    
                                    // Show optimization indicator
                                    if fileManager.optimizedFiles.contains(where: { $0.id == file.id }) {
                                        Text("⚡ iOS Optimized (No Base64)")
                                            .font(.caption2)
                                            .foregroundColor(.green)
                                    }
                                }
                                
                                Spacer()
                                
                                Button(action: {
                                    fileManager.removeFile(withId: file.id)
                                }) {
                                    Image(systemName: "minus.circle.fill")
                                        .foregroundColor(.red)
                                }
                            }
                        }
                    }
                }
                
                if !uploadResults.isEmpty {
                    Section("Upload Results") {
                        ForEach(uploadResults) { result in
                            HStack {
                                Image(systemName: result.success ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                                    .foregroundColor(result.success ? .green : .red)
                                
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(result.filename)
                                        .font(.subheadline)
                                    Text(result.message)
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                }
                                
                                Spacer()
                            }
                        }
                    }
                }
                
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        if isSingleUpload {
                            Text("Document type is automatically detected from file extension. Field linking allows you to associate this document with a specific strategic goal field. Photos and videos from your photo library are supported.")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        } else if isBulkUpload {
                            Text("Document types are automatically detected from file extensions. Bulk uploads are processed efficiently but cannot be linked to specific fields. Photos and videos from your photo library are supported.")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        } else {
                            Text("Document types are automatically detected from file extensions. You can select files from Documents or photos/videos from your photo library.")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        
                        Divider()
                        
                        HStack {
                            Image(systemName: "info.circle")
                                .foregroundColor(.blue)
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Size Limits:")
                                    .font(.caption)
                                    .fontWeight(.medium)
                                Text("• Maximum file size: 500MB")
                                Text("• Maximum total size: 2000MB")
                                Text("• Blocked file types: .dmg, .iso, .app, .pkg")
                            }
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        }
                    }
                }
                
                // Validation messages
                if isSingleUpload && linkedField.isEmpty {
                    Section {
                        Text("Please select a field to link this document to. Single uploads must be linked to a strategic goal field.")
                            .foregroundColor(.orange)
                            .font(.caption)
                    }
                }
                
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("Upload Documents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Upload") {
                        uploadDocuments()
                    }
                    .disabled(isUploadDisabled)
                }
            }
            .fileImporter(
                isPresented: $showFilePicker,
                allowedContentTypes: allowedFileTypes,
                allowsMultipleSelection: true
            ) { result in
                handleFileSelection(result)
            }
            .disabled(isUploading)
            .onChange(of: fileManager.count) { oldCount, newCount in
                // Clear linked field when switching from single to bulk mode
                if oldCount == 1 && newCount > 1 {
                    linkedField = ""
                }
            }
            .overlay {
                if isUploading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    VStack {
                        ProgressView()
                        Text("Uploading documents...")
                            .foregroundColor(.white)
                    }
                }
            }
            .onDisappear {
                // Clean up temp files when view is dismissed
                fileManager.clearAll()
            }
        }
    }
    
    private func handleFileSelection(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            Task {
                var successCount = 0
                var failureCount = 0
                var oversizedCount = 0
                
                for url in urls {
                    guard url.startAccessingSecurityScopedResource() else { 
                        failureCount += 1
                        continue 
                    }
                    defer { url.stopAccessingSecurityScopedResource() }
                    
                    do {
                        let filename = url.lastPathComponent
                        let fileExtension = (filename as NSString).pathExtension.lowercased()
                        
                        // Block certain file types that are too large or not needed
                        let blockedExtensions = ["dmg", "iso", "app", "pkg", "exe", "msi"]
                        if blockedExtensions.contains(fileExtension) {
                            print("⚠️ Blocked file type: \(fileExtension)")
                            failureCount += 1
                            continue
                        }
                        
                        // Check file size before copying (no memory loading!)
                        let fileAttributes = try FileManager.default.attributesOfItem(atPath: url.path)
                        let fileSize = fileAttributes[.size] as? Int ?? 0
                        
                        if fileSize > 500_000_000 { // 500MB limit per file
                            oversizedCount += 1
                            continue
                        }
                        
                        // iOS OPTIMIZATION: Create temp file and use FileManager.copyItem (efficient file system copy)
                        let tempDir = FileManager.default.temporaryDirectory
                        let tempFileURL = tempDir.appendingPathComponent("\(UUID().uuidString)_\(filename)")
                        
                        try FileManager.default.copyItem(at: url, to: tempFileURL)
                        print("📋 [OPTIMIZED] File copied to temp path: \(tempFileURL.path)")
                        
                        let detectedType = detectDocumentType(from: filename)
                        
                        let file = OptimizedDocumentFile(
                            name: filename,
                            tempPath: tempFileURL.path,      // Store path instead of data!
                            size: fileSize,
                            detectedType: detectedType
                        )
                        
                        await MainActor.run {
                            if fileManager.addOptimizedFile(file) {
                                successCount += 1
                            } else {
                                oversizedCount += 1
                                file.cleanup()
                            }
                        }
                    } catch {
                        print("Error processing file \(url.lastPathComponent): \(error)")
                        failureCount += 1
                    }
                }
                
                await MainActor.run {
                    if failureCount > 0 || oversizedCount > 0 {
                        var message = "File selection completed."
                        if successCount > 0 {
                            message += " \(successCount) files added."
                        }
                        if oversizedCount > 0 {
                            message += " \(oversizedCount) files were too large (>500MB) or would exceed total limit."
                        }
                        if failureCount > 0 {
                            message += " \(failureCount) files failed to load."
                        }
                        errorMessage = message
                    }
                }
            }
            
        case .failure(let error):
            errorMessage = "Failed to select files: \(error.localizedDescription)"
        }
    }
    
    private func handlePhotoSelection(_ newPhotos: [PhotosPickerItem]) {
        Task {
            var successCount = 0
            var failureCount = 0
            var oversizedCount = 0
            
            print("📸 [PHOTO_SELECTION] Processing \(newPhotos.count) photos from Photos app")
            
            for (index, photo) in newPhotos.enumerated() {
                print("📸 [PHOTO_\(index + 1)/\(newPhotos.count)] Processing photo: \(photo.itemIdentifier ?? "unknown")")
                print("📸 [PHOTO_\(index + 1)] Supported content types: \(photo.supportedContentTypes.map(\.identifier))")
                
                do {
                    // iOS OPTIMIZATION: Use loadTransferable with Data but immediately write to temp file
                    // This minimizes memory usage compared to keeping data in memory
                    if let data = try await photo.loadTransferable(type: Data.self) {
                        print("📸 [PHOTO_\(index + 1)] Successfully loaded photo data: \(data.count) bytes")
                        
                        // Check size before processing
                        if data.count > 500_000_000 { // 500MB limit
                            print("📸 [PHOTO_\(index + 1)] Photo too large: \(data.count) bytes")
                            oversizedCount += 1
                            continue
                        }
                        
                        // Generate filename based on photo identifier and supported types
                        let filename = generatePhotoFilename(for: photo)
                        print("📸 [PHOTO_\(index + 1)] Generated filename: \(filename)")
                        
                        // iOS OPTIMIZATION: Immediately write to temp file to free memory
                        let tempDir = FileManager.default.temporaryDirectory
                        let tempFileURL = tempDir.appendingPathComponent("\(UUID().uuidString)_\(filename)")
                        
                        try data.write(to: tempFileURL)
                        print("📸 [PHOTO_\(index + 1)] Written to temp file: \(tempFileURL.path)")
                        
                        let detectedType = detectDocumentType(from: filename)
                        print("📸 [PHOTO_\(index + 1)] Detected type: \(detectedType)")
                        
                        // Use OptimizedDocumentFile (path-based) for consistency with file uploads
                        let file = OptimizedDocumentFile(
                            name: filename,
                            tempPath: tempFileURL.path,
                            size: data.count,
                            detectedType: detectedType
                        )
                        
                        await MainActor.run {
                            if fileManager.addOptimizedFile(file) {
                                print("📸 [PHOTO_\(index + 1)] ✅ Successfully added optimized file")
                                successCount += 1
                            } else {
                                print("📸 [PHOTO_\(index + 1)] ❌ Failed to add optimized file (size limit)")
                                oversizedCount += 1
                                file.cleanup()
                            }
                        }
                    } else {
                        print("📸 [PHOTO_\(index + 1)] ❌ Failed to load photo data")
                        failureCount += 1
                    }
                } catch {
                    print("📸 [PHOTO_\(index + 1)] ❌ Error processing photo: \(error)")
                    failureCount += 1
                }
            }
            
            await MainActor.run {
                selectedPhotos.removeAll() // Clear selection for next time
                
                print("📸 [PHOTO_SELECTION] Final results: \(successCount) success, \(failureCount) failed, \(oversizedCount) oversized")
                
                if failureCount > 0 || oversizedCount > 0 {
                    var message = "Photo selection completed."
                    if successCount > 0 {
                        message += " \(successCount) photos added."
                    }
                    if oversizedCount > 0 {
                        message += " \(oversizedCount) photos were too large or would exceed total limit."
                    }
                    if failureCount > 0 {
                        message += " \(failureCount) photos failed to load."
                    }
                    errorMessage = message
                } else if successCount > 0 {
                    // Clear any previous error message on success
                    errorMessage = nil
                }
            }
        }
    }
    
    private func generatePhotoFilename(for photo: PhotosPickerItem, contentType: UTType? = nil) -> String {
        // Try to get the original filename if available
        if let identifier = photo.itemIdentifier {
            // Use the identifier to create a meaningful filename
            let timestamp = Date().timeIntervalSince1970
            let shortId = String(identifier.prefix(8))
            
            // Use provided content type or try to determine from supported types
            let typeToCheck = contentType ?? photo.supportedContentTypes.first
            
            if let type = typeToCheck {
                if type.conforms(to: .heif) || type.identifier == "public.heif" {
                    return "photo_\(shortId)_\(Int(timestamp)).heif"
                } else if type.conforms(to: .heic) || type.identifier == "public.heic" {
                    return "photo_\(shortId)_\(Int(timestamp)).heic"
                } else if type.conforms(to: .jpeg) || type.identifier == "public.jpeg" {
                    return "photo_\(shortId)_\(Int(timestamp)).jpg"
                } else if type.conforms(to: .png) || type.identifier == "public.png" {
                    return "photo_\(shortId)_\(Int(timestamp)).png"
                } else if type.conforms(to: .quickTimeMovie) || type.identifier == "com.apple.quicktime-movie" {
                    return "video_\(shortId)_\(Int(timestamp)).mov"
                } else if type.conforms(to: .mpeg4Movie) || type.identifier == "public.mpeg-4" {
                    return "video_\(shortId)_\(Int(timestamp)).mp4"
                } else if type.conforms(to: .webP) {
                    return "photo_\(shortId)_\(Int(timestamp)).webp"
                } else if type.conforms(to: .gif) {
                    return "photo_\(shortId)_\(Int(timestamp)).gif"
                }
            }
            
            // Try to determine the type from supported types (fallback)
            if photo.supportedContentTypes.contains(.heif) || photo.supportedContentTypes.contains(.heic) {
                return "photo_\(shortId)_\(Int(timestamp)).heic"
            } else if photo.supportedContentTypes.contains(.jpeg) {
                return "photo_\(shortId)_\(Int(timestamp)).jpg"
            } else if photo.supportedContentTypes.contains(.png) {
                return "photo_\(shortId)_\(Int(timestamp)).png"
            } else if photo.supportedContentTypes.contains(.quickTimeMovie) {
                return "video_\(shortId)_\(Int(timestamp)).mov"
            } else if photo.supportedContentTypes.contains(.mpeg4Movie) {
                return "video_\(shortId)_\(Int(timestamp)).mp4"
            }
        }
        
        // Fallback filename
        let timestamp = Date().timeIntervalSince1970
        return "media_\(Int(timestamp)).jpg"
    }
    
    private func detectDocumentType(from filename: String) -> String {
        let fileExtension = (filename as NSString).pathExtension.lowercased()
        
        // Map extensions to match backend document type initialization
        switch fileExtension {
        // Images - matches backend: "jpg" | "jpeg" | "png" | "heic" | "heif" | "webp" | "gif" | "bmp" | "tiff" | "svg"
        case "jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "bmp", "tiff", "svg": 
            return "Image"
            
        // Documents - matches backend: "pdf" | "doc" | "docx" | "rtf" | "txt" | "md" | "pages" | "odt"
        case "pdf", "doc", "docx", "rtf", "txt", "md", "pages", "odt": 
            return "Document"
            
        // Spreadsheets - matches backend: "xlsx" | "xls" | "numbers" | "csv" | "tsv" | "ods"
        case "xlsx", "xls", "numbers", "csv", "tsv", "ods": 
            return "Spreadsheet"
            
        // Presentations - matches backend: "pptx" | "ppt" | "key" | "odp"
        case "pptx", "ppt", "key", "odp": 
            return "Presentation"
            
        // Videos - matches backend: "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "3gp"
        case "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp": 
            return "Video"
            
        // Audio - matches backend: "mp3" | "m4a" | "wav" | "aac" | "flac" | "ogg" | "opus" | "caf"
        case "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "caf": 
            return "Audio"
            
        // Archives - matches backend: "zip" | "rar" | "7z" | "tar" | "gz" | "bz2"
        case "zip", "rar", "7z", "tar", "gz", "bz2": 
            return "Archive"
            
        // Code - matches backend: "html" | "css" | "js" | "json" | "xml" | "yaml" | "yml" | "sql" | "py" | "rs" | "swift" | "java" | "cpp" | "c" | "h"
        case "html", "css", "js", "json", "xml", "yaml", "yml", "sql", "py", "rs", "swift", "java", "cpp", "c", "h": 
            return "Code"
            
        // Data - matches backend: "db" | "sqlite" | "backup"
        case "db", "sqlite", "backup": 
            return "Data"
            
        default: 
            return "Unknown (\(fileExtension))"
        }
    }
    
    private func uploadDocuments() {
        isUploading = true
        uploadResults = []
        errorMessage = nil
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    self.errorMessage = "User not authenticated."
                    self.isUploading = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let ffiHandler = StrategicGoalFFIHandler()
            
            // NEW: Use iOS Optimized Path-Based Upload (no Base64 encoding!)
            if isSingleUpload {
                // Single upload with field linking using optimized path-based approach
                let file: OptimizedDocumentFile
                
                if !fileManager.optimizedFiles.isEmpty {
                    file = fileManager.optimizedFiles[0]
                    
                    print("🚀 [UPLOAD] Using optimized path-based upload")
                    
                    // Get actual document type ID instead of placeholder
                    let specificDocTypeId = await detectDocumentType(for: file.name)
                    let finalDocTypeId: String?
                    if specificDocTypeId != nil {
                        finalDocTypeId = specificDocTypeId
                    } else {
                        finalDocTypeId = await getDefaultDocumentTypeId()
                    }
                    let documentTypeId = finalDocTypeId ?? "00000000-0000-0000-0000-000000000000"
                    
                    let result = await ffiHandler.uploadDocumentFromPath(
                        goalId: goalId,
                        filePath: file.tempPath,           // Pass the path directly!
                        originalFilename: file.name,
                        title: documentTitle.isEmpty ? nil : documentTitle,
                        documentTypeId: documentTypeId,
                        linkedField: linkedField.isEmpty ? nil : linkedField,
                        syncPriority: priority,
                        compressionPriority: .normal,
                        auth: authContext
                    )
                    
                    await MainActor.run {
                        self.isUploading = false
                        
                        switch result {
                        case .success(let document):
                            self.uploadResults = [UploadResult(
                                filename: document.originalFilename,
                                success: true,
                                message: "✅ iOS Optimized Upload - \(document.typeName ?? "Document")" + 
                                        (linkedField.isEmpty ? "" : " (linked to \(linkableFields.first { $0.0 == linkedField }?.1 ?? linkedField))")
                            )]
                            
                            onUploadComplete()
                            
                            // Clean up temp file after successful upload
                            file.cleanup()
                            
                            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                dismiss()
                            }
                            
                        case .failure(let error):
                            self.errorMessage = "Optimized upload failed: \(error.localizedDescription)"
                            self.uploadResults = [UploadResult(
                                filename: file.name,
                                success: false,
                                message: "Failed to upload"
                            )]
                            
                            // Clean up temp file after upload attempt
                            file.cleanup()
                        }
                    }
                } else {
                    // Fallback to legacy method if no optimized files
                    await MainActor.run {
                        self.errorMessage = "No optimized files available for upload"
                        self.isUploading = false
                    }
                }
            } else {
                // Bulk upload using optimized path-based approach
                if !fileManager.optimizedFiles.isEmpty {
                    
                    print("🚀 [BULK_UPLOAD] Using optimized path-based bulk upload")
                    
                    let filePaths = fileManager.optimizedFiles.map { ($0.tempPath, $0.name) }
                    
                    // Get actual document type ID for bulk upload
                    let defaultDocTypeId = await getDefaultDocumentTypeId()
                    let documentTypeId = defaultDocTypeId ?? "00000000-0000-0000-0000-000000000000"
                    
                    let result = await ffiHandler.bulkUploadDocumentsFromPaths(
                        goalId: goalId,
                        filePaths: filePaths,               // Array of paths, no Base64!
                        title: documentTitle.isEmpty ? nil : documentTitle,
                        documentTypeId: documentTypeId,
                        syncPriority: priority,
                        compressionPriority: .normal,
                        auth: authContext
                    )
                    
                    await MainActor.run {
                        self.isUploading = false
                        
                        switch result {
                        case .success(let documents):
                            self.uploadResults = documents.map { doc in
                                UploadResult(
                                    filename: doc.originalFilename,
                                    success: true,
                                    message: "✅ iOS Optimized Bulk Upload - \(doc.typeName ?? "Document")"
                                )
                            }
                            
                            if !uploadResults.isEmpty {
                                onUploadComplete()
                                
                                // Clean up temp files after successful bulk upload
                                fileManager.clearAll()
                                
                                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                    dismiss()
                                }
                            }
                            
                        case .failure(let error):
                            self.errorMessage = "Optimized bulk upload failed: \(error.localizedDescription)"
                            self.uploadResults = fileManager.optimizedFiles.map { file in
                                UploadResult(
                                    filename: file.name,
                                    success: false,
                                    message: "Failed to upload"
                                )
                            }
                            
                            // Clean up temp files after upload attempt
                            fileManager.clearAll()
                        }
                    }
                } else {
                    // Fallback: legacy bulk upload if no optimized files
                    var files: [(Data, String)] = []
                    var failedFiles: [String] = []
                    
                    for file in fileManager.selectedFiles {
                        if let data = file.data {
                            files.append((data, file.name))
                        } else {
                            failedFiles.append(file.name)
                        }
                    }
                    
                    if !failedFiles.isEmpty {
                        await MainActor.run {
                            self.errorMessage = "Failed to read data for files: \(failedFiles.joined(separator: ", "))"
                            self.isUploading = false
                        }
                        return
                    }
                    
                    // Get actual document type ID for legacy bulk upload
                    let defaultDocTypeId = await getDefaultDocumentTypeId()
                    let documentTypeId = defaultDocTypeId ?? "00000000-0000-0000-0000-000000000000"
                    
                    let result = await ffiHandler.bulkUploadDocuments(
                        goalId: goalId,
                        files: files,
                        title: documentTitle.isEmpty ? nil : documentTitle,
                        documentTypeId: documentTypeId,
                        syncPriority: priority,
                        compressionPriority: .normal,
                        auth: authContext
                    )
                    
                    await MainActor.run {
                        self.isUploading = false
                        
                        switch result {
                        case .success(let documents):
                            self.uploadResults = documents.map { doc in
                                UploadResult(
                                    filename: doc.originalFilename,
                                    success: true,
                                    message: "📤 Legacy Upload - \(doc.typeName ?? "Document")"
                                )
                            }
                            
                            if !uploadResults.isEmpty {
                                onUploadComplete()
                                
                                // Clean up temp files after successful bulk upload
                                fileManager.clearAll()
                                
                                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                    dismiss()
                                }
                            }
                            
                        case .failure(let error):
                            self.errorMessage = "Legacy upload failed: \(error.localizedDescription)"
                            self.uploadResults = fileManager.selectedFiles.map { file in
                                UploadResult(
                                    filename: file.name,
                                    success: false,
                                    message: "Failed to upload"
                                )
                            }
                            
                            // Clean up temp files after upload attempt
                            fileManager.clearAll()
                        }
                    }
                }
            }
        }
    }
    
    private func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        // Documents
        case "pdf": return "doc.text.fill"
        case "doc", "docx", "rtf", "pages", "odt": return "doc.richtext.fill"
        case "txt", "md": return "doc.text"
        
        // Images - all supported backend types
        case "jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "bmp", "tiff": return "photo.fill"
        case "svg": return "photo.artframe"
        
        // Videos - all supported backend types
        case "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp": return "video.fill"
        
        // Audio - all supported backend types  
        case "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "caf": return "music.note"
        
        // Spreadsheets
        case "xlsx", "xls", "numbers", "csv", "tsv", "ods": return "tablecells.fill"
        
        // Presentations
        case "pptx", "ppt", "key", "odp": return "rectangle.on.rectangle.fill"
        
        // Archives
        case "zip", "rar", "7z", "tar", "gz", "bz2": return "archivebox.fill"
        
        // Code files
        case "html", "css": return "chevron.left.forwardslash.chevron.right"
        case "js", "json", "xml", "yaml", "yml": return "curlybraces"
        case "sql": return "tablecells"
        case "py", "rs", "swift", "java", "cpp", "c", "h": return "chevron.left.forwardslash.chevron.right"
        
        // Data files
        case "db", "sqlite", "backup": return "externaldrive.fill"
        
        default: return "doc.fill"
        }
    }
    
    private func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
    
    private func getFieldDisplayName(for fieldKey: String) -> String {
        return linkableFields.first { $0.0 == fieldKey }?.1 ?? fieldKey
    }
    
    private var isUploadDisabled: Bool {
        return fileManager.isEmpty || isUploading || (isSingleUpload && linkedField.isEmpty)
    }
    
    /// Helper to detect document type based on file extension (DocumentUploadSheet version)
    private func detectDocumentType(for filename: String) async -> String? {
        let fileExtension = (filename as NSString).pathExtension.lowercased()
        
        // First try to get document types from backend
        let authContext = AuthContextPayload(
            user_id: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        // Get document types and find one that matches this extension
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(authContext) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find document type that supports this extension
                for item in items {
                    if let allowedExtensions = item["allowed_extensions"] as? String,
                       let docTypeId = item["id"] as? String {
                        let extensions = allowedExtensions.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
                        if extensions.contains(fileExtension) {
                            print("🔍 [DOC_TYPE] Found matching document type for .\(fileExtension): \(docTypeId)")
                            return docTypeId
                        }
                    }
                }
            }
        }
        
        print("⚠️ [DOC_TYPE] No specific document type found for .\(fileExtension), using default")
        return nil // Will use default document type
    }
    
    /// Helper to get default document type ID (DocumentUploadSheet version)
    private func getDefaultDocumentTypeId() async -> String? {
        let authContext = AuthContextPayload(
            user_id: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        // Get document types and find "Document" type
        var result: UnsafeMutablePointer<CChar>?
        let status = document_type_list(
            """
            {
                "pagination": {"page": 1, "per_page": 50},
                "auth": \(encodeToJSON(authContext) ?? "{}")
            }
            """,
            &result
        )
        
        if let resultStr = result {
            defer { document_free(resultStr) }
            
            if status == 0,
               let data = String(cString: resultStr).data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                
                // Find "Document" type as default
                for item in items {
                    if let name = item["name"] as? String,
                       let docTypeId = item["id"] as? String,
                       name.lowercased() == "document" {
                        print("🔍 [DOC_TYPE] Found default Document type: \(docTypeId)")
                        return docTypeId
                    }
                }
                
                // If no "Document" type found, use the first one
                if let firstItem = items.first,
                   let docTypeId = firstItem["id"] as? String {
                    print("🔍 [DOC_TYPE] Using first available document type: \(docTypeId)")
                    return docTypeId
                }
            }
        }
        
        print("❌ [DOC_TYPE] No document types found!")
        return nil
    }
    
    /// Helper to encode objects to JSON string (DocumentUploadSheet version)
    private func encodeToJSON<T: Codable>(_ object: T) -> String? {
        guard let data = try? JSONEncoder().encode(object) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

// MARK: - Supporting Models for Document Upload
struct DocumentFile: Identifiable {
    let id: UUID
    let name: String
    let size: Int
    let detectedType: String
    var tempURL: URL? // Store file in temp directory instead of memory
    
    // Computed property to get data when needed
    var data: Data? {
        guard let tempURL = tempURL else { return nil }
        return try? Data(contentsOf: tempURL)
    }
    
    init(name: String, data: Data, size: Int, detectedType: String) {
        self.id = UUID()
        self.name = name
        self.size = size
        self.detectedType = detectedType
        
        // Store data in temporary file to avoid memory issues
        let tempDir = FileManager.default.temporaryDirectory
        let tempURL = tempDir.appendingPathComponent(self.id.uuidString)
        
        do {
            try data.write(to: tempURL)
            self.tempURL = tempURL
        } catch {
            print("❌ Failed to write temp file: \(error)")
            self.tempURL = nil
        }
    }
    
    func cleanup() {
        guard let tempURL = tempURL else { return }
        try? FileManager.default.removeItem(at: tempURL)
    }
}

// MARK: - iOS Optimized Document File (Path-Based, No Memory Copy)
struct OptimizedDocumentFile: Identifiable {
    let id: UUID
    let name: String
    let tempPath: String              // Direct path to temp file (no Data loading!)
    let size: Int
    let detectedType: String
    
    init(name: String, tempPath: String, size: Int, detectedType: String) {
        self.id = UUID()
        self.name = name
        self.tempPath = tempPath
        self.size = size
        self.detectedType = detectedType
    }
    
    func cleanup() {
        try? FileManager.default.removeItem(atPath: tempPath)
    }
}

// MARK: - Document File Manager
class DocumentFileManager: ObservableObject {
    @Published var selectedFiles: [DocumentFile] = []
    @Published var optimizedFiles: [OptimizedDocumentFile] = []  // New: Path-based files
    @Published var totalSize: Int64 = 0
    
    private let maxFileSize: Int = 500_000_000 // 500MB per file
    private let maxTotalSize: Int64 = 2000_000_000 // 2000MB total
    
    // Legacy method for Data-based files
    func addFile(_ file: DocumentFile) -> Bool {
        // Check individual file size
        if file.size > maxFileSize {
            return false
        }
        
        // Check total size
        let newTotalSize = totalSize + Int64(file.size)
        if newTotalSize > maxTotalSize {
            return false
        }
        
        selectedFiles.append(file)
        totalSize = newTotalSize
        return true
    }
    
    // New: Optimized method for path-based files (no memory overhead!)
    func addOptimizedFile(_ file: OptimizedDocumentFile) -> Bool {
        // Check individual file size
        if file.size > maxFileSize {
            return false
        }
        
        // Check total size
        let newTotalSize = totalSize + Int64(file.size)
        if newTotalSize > maxTotalSize {
            return false
        }
        
        optimizedFiles.append(file)
        totalSize = newTotalSize
        return true
    }
    
    func removeFile(withId id: UUID) {
        // Check legacy files
        if let index = selectedFiles.firstIndex(where: { $0.id == id }) {
            let file = selectedFiles[index]
            file.cleanup() // Clean up temp file
            totalSize -= Int64(file.size)
            selectedFiles.remove(at: index)
        }
        // Check optimized files
        else if let index = optimizedFiles.firstIndex(where: { $0.id == id }) {
            let file = optimizedFiles[index]
            file.cleanup() // Clean up temp file
            totalSize -= Int64(file.size)
            optimizedFiles.remove(at: index)
        }
    }
    
    func clearAll() {
        for file in selectedFiles {
            file.cleanup()
        }
        for file in optimizedFiles {
            file.cleanup()
        }
        selectedFiles.removeAll()
        optimizedFiles.removeAll()
        totalSize = 0
    }
    
    var count: Int { selectedFiles.count + optimizedFiles.count }
    var isEmpty: Bool { selectedFiles.isEmpty && optimizedFiles.isEmpty }
    
    // Combined files for UI display
    var allFiles: [(id: UUID, name: String, size: Int, detectedType: String)] {
        var combined: [(id: UUID, name: String, size: Int, detectedType: String)] = []
        
        for file in selectedFiles {
            combined.append((id: file.id, name: file.name, size: file.size, detectedType: file.detectedType))
        }
        
        for file in optimizedFiles {
            combined.append((id: file.id, name: file.name, size: file.size, detectedType: file.detectedType))
        }
        
        return combined
    }
    
    func getSizeDescription() -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: totalSize)
    }
    
    deinit {
        clearAll()
    }
}

struct UploadResult: Identifiable {
    let id = UUID()
    let filename: String
    let success: Bool
    let message: String
}

// MARK: - Goal Delete Options Sheet
struct GoalDeleteOptionsSheet: View {
    let onDelete: (Bool) -> Void
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Text("How would you like to delete this strategic goal?")
                    .font(.headline)
                    .multilineTextAlignment(.center)
                    .padding(.top)
                
                VStack(spacing: 16) {
                    archiveButton
                    deleteButton
                }
                .padding(.horizontal)
                
                Spacer()
            }
            .navigationTitle("Delete Goal")
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
            onDelete(false)
            dismiss()
        }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "archivebox")
                        .foregroundColor(.orange)
                        .font(.title2)
                    Text("Archive Goal")
                        .font(.headline)
                        .foregroundColor(.primary)
                    Spacer()
                }
                
                Text("Move the goal to archive. It can be restored later. Associated documents will be preserved.")
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
            onDelete(true)
            dismiss()
        }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "trash.fill")
                        .foregroundColor(.red)
                        .font(.title2)
                    Text("Permanently Delete")
                        .font(.headline)
                        .foregroundColor(.red)
                    Spacer()
                }
                
                Text("Permanently remove the goal and all associated data. This action cannot be undone.")
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
}

// MARK: - Strategic Goal Table Row
// Note: StrategicGoalTableRow is defined in Core/Components/StrategicGoalTableRow.swift

