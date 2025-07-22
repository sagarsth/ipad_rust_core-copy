//
//  StrategicGoalsView.swift
//  ActionAid SwiftUI
//
//  Strategic Goals management with clean extracted components
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI
import QuickLook

// MARK: - Main View
struct StrategicGoalsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    private let ffiHandler = StrategicGoalFFIHandler()

    // Core data state
    @State private var goals: [StrategicGoalResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedFilters: Set<String> = ["all"]
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var isActionBarCollapsed: Bool = false
    
    // Shared component managers
    @StateObject private var selectionManager = SelectionManager()
    @StateObject private var exportManager = ExportManager(service: StrategicGoalExportService())
    @StateObject private var crudManager = CRUDSheetManager<StrategicGoalResponse>(config: .strategicGoal)
    @StateObject private var documentTracker = DocumentCountTracker(config: .strategicGoals)
    @StateObject private var backendStatsManager = BackendStrategicGoalStatsManager()
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    
    // Goal detail view state
    @State private var selectedGoal: StrategicGoalResponse?
    
    // Column customization state
    @State private var showColumnCustomizer = false
    
    // Document viewing state
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Bulk operations state
    @State private var isPerformingBulkDelete = false
    @State private var bulkDeleteResults: BatchDeleteResult?
    @State private var showBulkDeleteResults = false
    @State private var showExportOptions = false
    
    // MARK: - Computed Properties
    
    /// Properly filtered goals with working OR gate logic for status filters
    var filteredGoals: [StrategicGoalResponse] {
        goals.filter { goal in
            let matchesSearch = searchText.isEmpty ||
                goal.objectiveCode.localizedCaseInsensitiveContains(searchText) ||
                (goal.outcome ?? "").localizedCaseInsensitiveContains(searchText) ||
                (goal.responsibleTeam ?? "").localizedCaseInsensitiveContains(searchText)
            
            // OR gate logic for status filters (fixed from EntityListView)
            let matchesStatus = selectedFilters.contains("all") ||
                (selectedFilters.contains("on_track") && goal.statusId == 1) ||
                (selectedFilters.contains("at_risk") && goal.statusId == 2) ||
                (selectedFilters.contains("behind") && goal.statusId == 3) ||
                (selectedFilters.contains("completed") && goal.statusId == 4)
            
            return matchesSearch && matchesStatus
        }
    }
    
    // MARK: - Setup and Helper Methods
    
    /// Setup callbacks for shared component managers
    private func setupCallbacks() {
        // CRUD manager callbacks
        crudManager.onEntityCreated = { newGoal in
            loadGoals()
            
            // ✅ Notify cache of new strategic goal for instant picker updates
            Task {
                await StrategicGoalsCache.shared.notifyStrategicGoalCreated(newGoal, authManager: authManager)
            }
        }
        crudManager.onEntityUpdated = { updatedGoal in
            loadGoals()
            
            // ✅ Notify cache of updated strategic goal for instant picker updates
            Task {
                await StrategicGoalsCache.shared.notifyStrategicGoalUpdated(updatedGoal)
            }
        }
        crudManager.onEntityDeleted = { _ in
            loadGoals()
        }
    }
    
    /// Create auth context for API calls
    private func createAuthContext() -> AuthCtxDto {
        return AuthCtxDto(
            userId: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            deviceId: authManager.getDeviceId(),
            offlineMode: false
        )
    }
    
    var body: some View {
        VStack(spacing: 0) {
            // Main Entity List with properly filtered goals (fixed status filtering)
            EntityListView(
                entities: filteredGoals,
                isLoading: isLoading,
                emptyStateConfig: .strategicGoals,
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                filterOptions: FilterOption.strategicGoalFilters,
                currentViewStyle: $currentViewStyle,
                onViewStyleChange: { newStyle in
                    currentViewStyle = newStyle
                    // Save preference for next time (following shared architecture pattern)
                    viewStyleManager.setViewStyle(newStyle, for: "strategic_goals")
                },
                selectionManager: selectionManager,
                onFilterBasedSelectAll: {
                    Task {
                        await getFilteredGoalIds()
                    }
                },
                onItemTap: { goal in
                    selectedGoal = goal
                },
                cardContent: { goal in
                    GoalCard(goal: goal, documentTracker: documentTracker)
                },
                tableColumns: StrategicGoalTableConfig.columns,
                rowContent: { goal, columns in
                    StrategicGoalTableRow(
                        goal: goal, 
                        columns: columns, 
                        documentCounts: documentTracker.documentCounts
                    )
                },
                domainName: "strategic_goals",
                userRole: authManager.currentUser?.role,
                showColumnCustomizer: $showColumnCustomizer
            )
        }
        .navigationTitle("Strategic Goals")
        .navigationBarTitleDisplayMode(UIDevice.current.userInterfaceIdiom == .pad ? .large : .inline)
        .navigationBarHidden(isActionBarCollapsed)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack(spacing: 8) {
                    ViewStyleSwitcher(
                        currentViewStyle: currentViewStyle,
                        onViewStyleChange: { newStyle in
                            currentViewStyle = newStyle
                            viewStyleManager.setViewStyle(newStyle, for: "strategic_goals")
                            // Clear selection when switching views
                            selectionManager.clearSelection()
                        },
                        onShowColumnCustomizer: {
                            showColumnCustomizer = true
                        }
                    )
                    
                    Button(action: { crudManager.presentCreateSheet() }) {
                        Image(systemName: "plus.circle.fill")
                            .font(.title3)
                    }
                }
            }
        }
        .overlay(
            // Selection action bar using shared component
            Group {
                if selectionManager.isInSelectionMode && selectionManager.hasSelection && !isActionBarCollapsed {
                    SelectionActionBar(
                        selectedCount: selectionManager.selectedCount,
                        userRole: authManager.currentUser?.role,
                        isPerformingBulkOperation: isPerformingBulkDelete,
                        onClearSelection: {
                            selectionManager.clearSelection()
                        },
                        onExport: {
                            showExportOptions = true
                        },
                        onDelete: {
                            performBulkDelete(hardDelete: false, force: false)
                        }
                    )
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            },
            alignment: .bottom
        )
        .withCRUDSheets(
            manager: crudManager,
            userRole: authManager.currentUser?.role,
            createSheet: {
                CreateGoalSheet(ffiHandler: self.ffiHandler, onSave: { newGoal in
                    crudManager.completeOperation(.create, result: newGoal)
                    // loadGoals() is now handled by the onEntityCreated callback
                })
            },
            editSheet: { goal in
                EditGoalSheet(goal: goal, ffiHandler: self.ffiHandler, onSave: { updatedGoal in
                    crudManager.completeOperation(.edit, result: updatedGoal)
                    // loadGoals() is now handled by the onEntityUpdated callback
                })
            },
            onDelete: { goal, hardDelete, force in
                performSingleDelete(goal: goal, hardDelete: hardDelete, force: force)
            }
        )
        .withDocumentCounting(
            entities: goals,
            tracker: documentTracker,
            auth: createAuthContext()
        )
        .fullScreenCover(item: $selectedGoal) { goal in
            GoalDetailView(
                goal: goal, 
                onUpdate: {
                    loadGoals()
                }
            )
        }
        .fullScreenCover(item: $selectedDocumentURL) { identifiableURL in
            NavigationView {
                QuickLookView(url: identifiableURL.url) {
                    selectedDocumentURL = nil
                }
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .navigationBarLeading) {
                        Button("Close") {
                            selectedDocumentURL = nil
                        }
                    }
                }
            }
        }
        .sheet(isPresented: $showBulkDeleteResults) {
            if let results = bulkDeleteResults {
                DeleteResultsSheet(results: results, entityName: "Strategic Goal", entityNamePlural: "Strategic Goals")
            }
        }
        .sheet(isPresented: $showExportOptions) {
            GenericExportOptionsSheet(
                selectedItemCount: selectionManager.selectedCount,
                entityName: "Strategic Goal",
                entityNamePlural: "Strategic Goals",
                onExport: { includeBlobs, format in
                    performExportFromSelection(includeBlobs: includeBlobs, format: format)
                },
                isExporting: $exportManager.isExporting,
                exportError: $exportManager.exportError
            )
        }
        .onAppear {
            setupCallbacks()
            loadGoals()
            // Load saved view style preference (following shared architecture pattern)
            currentViewStyle = viewStyleManager.getViewStyle(for: "strategic_goals")
        }
        .onChange(of: goals.count) { oldCount, newCount in
            // Fetch backend stats when goals data changes
            if newCount != oldCount {
                Task {
                    await fetchBackendStats()
                }
            }
        }
        .onAppear {
            // Fetch backend stats on first appearance if goals are loaded
            if !goals.isEmpty {
                Task {
                    await fetchBackendStats()
                }
            }
        }
    }
    
    // MARK: - Backend Statistics
    
    private func fetchBackendStats() async {
        guard let currentUser = authManager.currentUser else { return }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        await backendStatsManager.fetchStats(auth: authContext)
        
        // Register with shared context for Stats tab display
        await MainActor.run {
            let anyStatsManager = backendStatsManager.createAnyStatsManager()
            sharedStatsContext.currentEntityStats = anyStatsManager
            sharedStatsContext.entityName = "Strategic Goals"
            
            print("📊 Registered strategic goal stats with shared context: \(backendStatsManager.stats.count) stats")
        }
    }
    
    // MARK: - Core Data Operations
    
    /// Load strategic goals from backend
    private func loadGoals() {
        isLoading = true
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    crudManager.errorMessage = "User not authenticated."
                    crudManager.showErrorAlert = true
                    isLoading = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )

            let result = await ffiHandler.list(
                pagination: PaginationDto(page: 1, perPage: 100), 
                include: [], // Document counts handled by DocumentCountTracker
                auth: authContext
            )
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    goals = paginatedResult.items
                case .failure(let error):
                    crudManager.errorMessage = "Failed to load goals: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
            
            // Fetch backend stats after loading goals
            await fetchBackendStats()
        }
    }
    
    // MARK: - Selection and Bulk Operations
    
    /// Perform single entity delete (called from CRUD manager)
    private func performSingleDelete(goal: StrategicGoalResponse, hardDelete: Bool, force: Bool) {
        crudManager.startOperation(.delete)
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    crudManager.completeOperation(.delete, error: NSError(domain: "Auth", code: 1, userInfo: [NSLocalizedDescriptionKey: "User not authenticated"]))
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.delete(id: goal.id, hardDelete: hardDelete, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        crudManager.completeOperation(.delete, result: goal)
                        loadGoals() // Refresh the list
                    } else {
                        crudManager.completeOperation(.delete, error: NSError(domain: "Delete", code: 2, userInfo: [NSLocalizedDescriptionKey: deleteResponse.displayMessage]))
                    }
                case .failure(let error):
                    crudManager.completeOperation(.delete, error: error)
                }
            }
        }
    }
    
    // MARK: - Filter-Aware Bulk Selection
    
    /// Get filtered goal IDs for bulk selection based on current UI filters
    private func getFilteredGoalIds() async {
        print("🔄 [BACKEND_FILTER] getFilteredGoalIds called")
        print("🔄 [BACKEND_FILTER] selectedFilters: \(selectedFilters)")
        print("🔄 [BACKEND_FILTER] isSelectAllActive: \(selectionManager.isSelectAllActive)")
        
        guard !selectionManager.isLoadingFilteredIds else { 
            print("🔄 [BACKEND_FILTER] ❌ Already loading, returning")
            return 
        }
        
        // Check if we have any backend filters active (search, status, etc.)
        let hasBackendFilters = !searchText.isEmpty || !selectedFilters.contains("all")
        print("🔄 [BACKEND_FILTER] hasBackendFilters: \(hasBackendFilters)")
        
        // If no backend filters are applied, select all visible items
        if !hasBackendFilters {
            await MainActor.run {
                let allVisibleIds = Set(filteredGoals.map(\.id))
                print("🔄 [BACKEND_FILTER] No backend filters, selecting \(allVisibleIds.count) visible items")
                selectionManager.selectAllItems(allVisibleIds)
            }
            return
        }
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = true
        }
        
        guard let currentUser = authManager.currentUser else {
            await MainActor.run {
                selectionManager.isLoadingFilteredIds = false
                crudManager.errorMessage = "User not authenticated."
                crudManager.showErrorAlert = true
            }
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        // Create filter based on current UI state
        var statusIds: [Int64]? = nil
        if !selectedFilters.contains("all") {
            statusIds = mapFilterIdsToStatusIds(selectedFilters)
        }
        
        print("🔄 [BACKEND_FILTER] Mapped statusIds: \(statusIds ?? [])")
        
        let currentFilter = StrategicGoalFilter(
            statusIds: statusIds,
            responsibleTeams: nil,
            years: nil,
            months: nil,
            userRole: nil,
            syncPriorities: nil,
            searchText: searchText.isEmpty ? nil : searchText,
            progressRange: nil,
            targetValueRange: nil,
            actualValueRange: nil,
            dateRange: nil,
            daysStale: nil,
            excludeDeleted: true
        )
        
        let result = await ffiHandler.getFilteredIds(filter: currentFilter, auth: authContext)
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = false
            switch result {
            case .success(let filteredIds):
                print("🔄 [BACKEND_FILTER] ✅ Backend returned \(filteredIds.count) filtered IDs")
                // Only select IDs that are currently visible (intersection with filtered data)
                let visibleIds = Set(filteredGoals.map(\.id))
                let filteredVisibleIds = Set(filteredIds).intersection(visibleIds)
                print("🔄 [BACKEND_FILTER] Visible IDs: \(visibleIds.count), Final selection: \(filteredVisibleIds.count)")
                selectionManager.selectAllItems(filteredVisibleIds)
            case .failure(let error):
                print("🔄 [BACKEND_FILTER] ❌ Backend error: \(error.localizedDescription)")
                crudManager.errorMessage = "Failed to get filtered IDs: \(error.localizedDescription)"
                crudManager.showErrorAlert = true
            }
        }
    }
    
    /// Convert UI filter IDs to backend status IDs
    private func mapFilterIdsToStatusIds(_ filterIds: Set<String>) -> [Int64] {
        var ids: [Int64] = []
        if filterIds.contains("on_track") { ids.append(1) }
        if filterIds.contains("at_risk") { ids.append(2) }
        if filterIds.contains("behind") { ids.append(3) }
        if filterIds.contains("completed") { ids.append(4) }
        return ids.isEmpty ? [] : ids
    }
    
    // MARK: - Export Operations
    
    private func performExportFromSelection(includeBlobs: Bool = false, format: ExportFormat = .default) {
        guard !selectionManager.selectedItems.isEmpty else { return }
        
        print("🔄 Starting export from selection mode for \(selectionManager.selectedCount) items, includeBlobs: \(includeBlobs), format: \(format.displayName)")
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    crudManager.errorMessage = "User not authenticated."
                    crudManager.showErrorAlert = true
                }
                return
            }
            
            let selectedIdsArray = Array(selectionManager.selectedItems)
            
            await exportManager.exportSelectedItems(
                    ids: selectedIdsArray,
                    includeBlobs: includeBlobs,
                    format: format,
                authToken: currentUser.token,
                onClearSelection: {
                    // This will be called by the export manager when export completes
                    self.selectionManager.clearSelection()
                    self.showExportOptions = false
                },
                onCompletion: { success in
                    // Handle completion if needed
                    if !success {
                        print("❌ Export completed with errors")
                    }
                }
            )
        }
    }
    
    // MARK: - Bulk Delete Methods
    
    private func performBulkDelete(hardDelete: Bool, force: Bool = false) {
        guard !selectionManager.selectedItems.isEmpty else { return }
        
        isPerformingBulkDelete = true
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    crudManager.errorMessage = "User not authenticated."
                    crudManager.showErrorAlert = true
                    isPerformingBulkDelete = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let selectedIds = Array(selectionManager.selectedItems)
            print("🗑️ [BULK_DELETE] Starting bulk delete for \(selectedIds.count) strategic goals")
            print("🗑️ [BULK_DELETE] Hard delete: \(hardDelete), Force: \(force)")
            
            let result = await ffiHandler.bulkDelete(
                ids: selectedIds,
                hardDelete: hardDelete,
                force: force,
                auth: authContext
            )
            
            await MainActor.run {
                self.isPerformingBulkDelete = false
                
                switch result {
                case .success(let batchResult):
                    print("✅ [BULK_DELETE] Bulk delete completed")
                    print("✅ [BULK_DELETE] Hard deleted: \(batchResult.hardDeleted.count)")
                    print("✅ [BULK_DELETE] Soft deleted: \(batchResult.softDeleted.count)")
                    print("✅ [BULK_DELETE] Failed: \(batchResult.failed.count)")
                    
                    // Store results for display
                    self.bulkDeleteResults = batchResult
                    
                    // Clear selection and refresh data using shared manager
                    self.selectionManager.clearSelection()
                    
                    // Refresh the goals list to reflect changes
                    loadGoals()
                    
                    // Show results if there were any failures or mixed results
                    if !batchResult.failed.isEmpty || !batchResult.dependencies.isEmpty {
                        self.showBulkDeleteResults = true
                    }
                    
                case .failure(let error):
                    print("❌ [BULK_DELETE] Bulk delete failed: \(error)")
                    crudManager.errorMessage = "Bulk delete failed: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
        }
    }
}

// MARK: - Goal Card Component
struct GoalCard: View {
    let goal: StrategicGoalResponse
    let documentTracker: DocumentCountTracker
    
    private var progress: Double {
        let rawProgress = goal.progressPercentage ?? 0.0
        // Ensure progress is a valid number and within bounds
        if rawProgress.isNaN || rawProgress.isInfinite {
            return 0.0
        }
        return max(0.0, rawProgress)
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
                            
                            // Use DocumentCountTracker's documentCounts directly
                            if (documentTracker.documentCounts[goal.id] ?? 0) > 0 {
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







// MARK: - Create Goal Sheet
struct CreateGoalSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let ffiHandler: StrategicGoalFFIHandler
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
    
    // For focus management
    private enum Field: Hashable {
        case objectiveCode, outcome, kpi, targetValue, actualValue, responsibleTeam
    }
    @FocusState private var focusedField: Field?
    
    private var canSave: Bool {
        !objectiveCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !outcome.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Goal Information") {
                    TextField("Objective Code", text: $objectiveCode)
                        .focused($focusedField, equals: .objectiveCode)
                        .textInputAutocapitalization(.characters)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    TextField("Outcome", text: $outcome, axis: .vertical)
                        .focused($focusedField, equals: .outcome)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    TextField("KPI", text: $kpi)
                        .focused($focusedField, equals: .kpi)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                }
                
                Section("Metrics") {
                    HStack {
                        TextField("Target Value", text: $targetValue)
                            .focused($focusedField, equals: .targetValue)
                            .keyboardType(.decimalPad)
                            .submitLabel(.next)
                            .disableAutocorrection(true)
                            .textFieldStyle(.plain) // Reduces rendering overhead
                        Text("Target")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    HStack {
                        TextField("Actual Value", text: $actualValue)
                            .focused($focusedField, equals: .actualValue)
                            .keyboardType(.decimalPad)
                            .submitLabel(.done)
                            .disableAutocorrection(true)
                            .textFieldStyle(.plain) // Reduces rendering overhead
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
                    .pickerStyle(.menu) // More responsive than default picker
                    
                    TextField("Responsible Team", text: $responsibleTeam)
                        .focused($focusedField, equals: .responsibleTeam)
                        .submitLabel(.done)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    Picker("Sync Priority", selection: $syncPriority) {
                        Text("Low").tag(SyncPriority.low)
                        Text("Normal").tag(SyncPriority.normal)
                        Text("High").tag(SyncPriority.high)
                    }
                    .pickerStyle(.menu) // More responsive than default picker
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
                    Button("Cancel") { 
                        // Haptic feedback for better UX
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        dismiss() 
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        // Haptic feedback for better UX
                        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                        createGoal()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Creating goal...")
                        .scaleEffect(1.2)
                }
            }
        }     // end NavigationView
         .interactiveDismissDisabled(isLoading) // Prevent accidental dismissal during creation
         .onAppear {
             // Give focus to the first field when the sheet appears
             focusedField = .objectiveCode
         }
    } // end body of CreateGoalSheet

    private func createGoal() {
        // Dismiss keyboard before starting creation
        focusedField = nil
        
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
            createdByUserId: currentUser.userId
        )

        Task {
            let result = await ffiHandler.create(newGoal: newGoal, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let createdGoal):
                    // Success haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(createdGoal)
                    dismiss()
                case .failure(let error):
                    // Error haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to create goal: \(error.localizedDescription)"
                }
            }
                    }
            }
}

// MARK: - Edit Goal Sheet
struct EditGoalSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let goal: StrategicGoalResponse
    let ffiHandler: StrategicGoalFFIHandler
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
    
    // Track original values to detect changes
    @State private var originalObjectiveCode = ""
    @State private var originalOutcome = ""
    @State private var originalKpi = ""
    @State private var originalTargetValue = ""
    @State private var originalActualValue = ""
    @State private var originalStatusId: Int64 = 1
    @State private var originalResponsibleTeam = ""
    @State private var originalSyncPriority: SyncPriority = .normal
    
    // For focus management
    private enum Field: Hashable {
        case objectiveCode, outcome, kpi, targetValue, actualValue, responsibleTeam
    }
    @FocusState private var focusedField: Field?
    
    private var canSave: Bool {
        !objectiveCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !outcome.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    /// Check if any field values have actually changed
    private var hasChanges: Bool {
        objectiveCode != originalObjectiveCode ||
        outcome != originalOutcome ||
        kpi != originalKpi ||
        targetValue != originalTargetValue ||
        actualValue != originalActualValue ||
        statusId != originalStatusId ||
        responsibleTeam != originalResponsibleTeam ||
        syncPriority != originalSyncPriority
    }
    
    var body: some View {
        NavigationView {
            Form { 
                Section("Goal Information") {
                    TextField("Objective Code", text: $objectiveCode)
                        .focused($focusedField, equals: .objectiveCode)
                        .textInputAutocapitalization(.characters)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    TextField("Outcome", text: $outcome, axis: .vertical)
                        .focused($focusedField, equals: .outcome)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    TextField("KPI", text: $kpi)
                        .focused($focusedField, equals: .kpi)
                        .submitLabel(.next)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                }
                
                Section("Metrics") {
                    HStack {
                        TextField("Target Value", text: $targetValue)
                            .focused($focusedField, equals: .targetValue)
                            .keyboardType(.decimalPad)
                            .submitLabel(.next)
                            .disableAutocorrection(true)
                            .textFieldStyle(.plain)
                        Text("Target")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    HStack {
                        TextField("Actual Value", text: $actualValue)
                            .focused($focusedField, equals: .actualValue)
                            .keyboardType(.decimalPad)
                            .submitLabel(.done)
                            .disableAutocorrection(true)
                            .textFieldStyle(.plain)
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
                    .pickerStyle(.menu)
                    
                    TextField("Responsible Team", text: $responsibleTeam)
                        .focused($focusedField, equals: .responsibleTeam)
                        .submitLabel(.done)
                        .disableAutocorrection(true)
                        .textFieldStyle(.plain)
                    
                    Picker("Sync Priority", selection: $syncPriority) {
                        Text("Low").tag(SyncPriority.low)
                        Text("Normal").tag(SyncPriority.normal)
                        Text("High").tag(SyncPriority.high)
                    }
                    .pickerStyle(.menu)
                }
                
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("Edit Strategic Goal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { 
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        dismiss() 
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(hasChanges ? "Save" : "Done") {
                        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                        updateGoal()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Updating goal...")
                        .scaleEffect(1.2)
                }
            }
        }
        .interactiveDismissDisabled(isLoading)
        .onAppear {
            populateFields()
            focusedField = .objectiveCode
        }
    }
    
    private func populateFields() {
        // Set current field values
        objectiveCode = goal.objectiveCode
        outcome = goal.outcome ?? ""
        kpi = goal.kpi ?? ""
        targetValue = goal.targetValue != nil ? String(goal.targetValue!) : ""
        actualValue = goal.actualValue != nil ? String(goal.actualValue!) : ""
        statusId = goal.statusId ?? 1
        responsibleTeam = goal.responsibleTeam ?? ""
        syncPriority = goal.syncPriority
        
        // Store original values for change detection
        originalObjectiveCode = goal.objectiveCode
        originalOutcome = goal.outcome ?? ""
        originalKpi = goal.kpi ?? ""
        originalTargetValue = goal.targetValue != nil ? String(goal.targetValue!) : ""
        originalActualValue = goal.actualValue != nil ? String(goal.actualValue!) : ""
        originalStatusId = goal.statusId ?? 1
        originalResponsibleTeam = goal.responsibleTeam ?? ""
        originalSyncPriority = goal.syncPriority
    }
    
    private func updateGoal() {
        focusedField = nil
        
        // Check if any changes were actually made
        if !hasChanges {
            // No changes detected, just dismiss without calling API or onSave
            UINotificationFeedbackGenerator().notificationOccurred(.success)
            dismiss()
            return
        }
        
        isLoading = true
        errorMessage = nil
        
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

        let updateGoal = UpdateStrategicGoal(
            objectiveCode: objectiveCode.isEmpty ? nil : objectiveCode,
            outcome: outcome.isEmpty ? nil : outcome,
            kpi: kpi.isEmpty ? nil : kpi,
            targetValue: targetValue.isEmpty ? nil : Double(targetValue),
            actualValue: actualValue.isEmpty ? nil : Double(actualValue),
            statusId: statusId,
            responsibleTeam: responsibleTeam.isEmpty ? nil : responsibleTeam,
            syncPriority: syncPriority,
            updatedByUserId: currentUser.userId
        )

        Task {
            let result = await ffiHandler.update(id: goal.id, update: updateGoal, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let updatedGoal):
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(updatedGoal)
                    dismiss()
                case .failure(let error):
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to update goal: \(error.localizedDescription)"
                }
            }
        }
    }
}


// MARK: - Goal Detail View
struct GoalDetailView: View {
    @State private var currentGoal: StrategicGoalResponse
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    private let ffiHandler = StrategicGoalFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    @State private var documents: [MediaDocumentResponse] = []
    @State private var showUploadSheet = false
    @State private var showEditSheet = false
    @State private var showDeleteConfirmation = false
    @State private var showDeleteOptions = false
    @State private var isDeleting = false
    @State private var isLoadingDocuments = false
    @State private var showErrorAlert = false
    @State private var errorMessage: String?
    
    // Initialize with goal data
    init(goal: StrategicGoalResponse, onUpdate: @escaping () -> Void) {
        self._currentGoal = State(initialValue: goal)
        self.onUpdate = onUpdate
    }
    
    // Document viewing state
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Document refresh mechanism
    @State private var refreshTimer: Timer?
    @State private var lastRefreshTime = Date()
    @State private var hasActiveCompressions = false
    @State private var lastCompressionCount = 0
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Goal Header
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(currentGoal.objectiveCode)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text(currentGoal.outcome ?? "N/A")
                                    .font(.headline)
                            }
                            Spacer()
                            Badge(text: currentGoal.statusText, color: currentGoal.statusColor)
                        }
                        
                        Divider()
                        
                        // Progress
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Progress")
                                    .font(.subheadline)
                                    .fontWeight(.medium)
                                Spacer()
                                Text("\(Int(currentGoal.progress))%")
                                    .font(.headline)
                                    .foregroundColor(currentGoal.statusColor)
                            }
                            
                            GeometryReader { geometry in
                                ZStack(alignment: .leading) {
                                    RoundedRectangle(cornerRadius: 6)
                                        .fill(Color(.systemGray5))
                                        .frame(height: 12)
                                    
                                    RoundedRectangle(cornerRadius: 6)
                                        .fill(currentGoal.statusColor)
                                        .frame(width: geometry.size.width * (currentGoal.progress / 100), height: 12)
                                }
                            }
                            .frame(height: 12)
                            
                            HStack {
                                Text("Actual: \(Int(currentGoal.actualValue ?? 0))")
                                Spacer()
                                Text("Target: \(Int(currentGoal.targetValue ?? 0))")
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
                        DetailRow(label: "KPI", value: currentGoal.kpi ?? "N/A")
                        DetailRow(label: "Responsible Team", value: currentGoal.responsibleTeam ?? "N/A")
                        DetailRow(label: "Sync Priority", value: currentGoal.syncPriority.rawValue)
                        
                        Divider()
                        
                        DetailRow(label: "Created", value: formatDate(currentGoal.createdAt))
                        DetailRow(label: "Created By", value: currentGoal.createdByUsername ?? currentGoal.createdByUserId ?? "Unknown")
                        DetailRow(label: "Last Updated", value: formatDate(currentGoal.updatedAt))
                        DetailRow(label: "Updated By", value: currentGoal.updatedByUsername ?? currentGoal.updatedByUserId ?? "Unknown")
                        
                        Divider()
                        
                        HStack {
                            Text("Sync Status")
                                .font(.subheadline)
                                .foregroundColor(.secondary)
                            Spacer()
                            Text(currentGoal.displayLastSyncedAt)
                                .font(.subheadline)
                                .fontWeight(.medium)
                                .foregroundColor(currentGoal.lastSyncedAt == nil ? .orange : .green)
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
                                Text(hasActiveCompressions ? "Refreshing compression status..." : "Loading documents...")
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
                                DocumentRow(
                                    document: doc,
                                    onTap: { openDocument(doc) }
                                )
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
                        Button(action: { showEditSheet = true }) {
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
                DocumentUploadSheet(
                    entity: currentGoal.asDocumentUploadable(),
                    config: .standard,
                    onUploadComplete: {
                    loadDocuments()
                    // Timer will be started automatically by updateCompressionStatus() if needed
                    // FIXED: Immediately update main view document counts after upload
                    onUpdate()
                    }
                )
            }
            .sheet(isPresented: $showEditSheet) {
                EditGoalSheet(goal: currentGoal, ffiHandler: self.ffiHandler, onSave: { updatedGoal in
                    // Update the current goal with fresh data
                    currentGoal = updatedGoal
                    loadDocuments()
                    onUpdate()
                })
            }
            .sheet(isPresented: $showDeleteOptions) {
                GoalDeleteOptionsSheet(
                    userRole: authManager.currentUser?.role ?? "",
                    onDelete: { hardDelete, force in
                        deleteGoal(hardDelete: hardDelete, force: force)
                    }
                )
            }
            .fullScreenCover(item: $selectedDocumentURL) { identifiableURL in
                NavigationView {
                    QuickLookView(url: identifiableURL.url) {
                        // Cleanup when document viewer is dismissed
                        selectedDocumentURL = nil
                    }
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        ToolbarItem(placement: .navigationBarLeading) {
                            Button("Close") {
                                selectedDocumentURL = nil
                            }
                        }
                    }
                }
            }
            .onAppear {
                loadDocuments()
                // Timer will be started automatically by updateCompressionStatus() if needed
            }
            .onDisappear {
                stopDocumentRefreshTimer()
            }
            .alert("Delete Goal", isPresented: $showDeleteConfirmation) {
                Button("Cancel", role: .cancel) { }
                Button("Delete", role: .destructive) {
                    deleteGoal(hardDelete: false, force: false) // Non-admin users get soft delete
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
                relatedId: currentGoal.id,
                pagination: PaginationDto(page: 1, perPage: 50),
                include: [.documentType],
                auth: authContext
            )
            
            await MainActor.run {
                isLoadingDocuments = false
                switch result {
                case .success(let paginatedResult):
                    // Preserve scroll position by updating documents more intelligently
                    updateDocumentsPreservingScroll(newDocuments: paginatedResult.items)
                    updateCompressionStatus()
                case .failure(let error):
                    print("Failed to load documents: \(error)")
                    if documents.isEmpty {
                        documents = []
                    }
                    hasActiveCompressions = false
                }
            }
        }
    }
    
    private func refreshDocuments() {
        lastRefreshTime = Date()
        loadDocuments()
    }
    
    /// Update documents while preserving scroll position
    private func updateDocumentsPreservingScroll(newDocuments: [MediaDocumentResponse]) {
        // If this is the initial load, just set the documents
        if documents.isEmpty {
            documents = newDocuments
            return
        }
        
        // For subsequent updates, only update if there are actual changes
        // This prevents unnecessary UI rebuilds that reset scroll position
        let documentsChanged = !documentsAreEqual(documents, newDocuments)
        
        if documentsChanged {
            // Update documents with minimal disruption using withAnimation
            withAnimation(.none) {
                documents = newDocuments
            }
            print("📄 [DOCUMENT_REFRESH] Updated \(newDocuments.count) documents with preserved scroll")
        } else {
            print("📄 [DOCUMENT_REFRESH] No changes detected, scroll position preserved")
        }
    }
    
    /// Compare two document arrays to detect meaningful changes
    private func documentsAreEqual(_ docs1: [MediaDocumentResponse], _ docs2: [MediaDocumentResponse]) -> Bool {
        guard docs1.count == docs2.count else { return false }
        
        // Check if documents are the same (by ID and compression status)
        for (doc1, doc2) in zip(docs1, docs2) {
            if doc1.id != doc2.id || 
               doc1.compressionStatus != doc2.compressionStatus ||
               doc1.isAvailableLocally != doc2.isAvailableLocally {
                return false
            }
        }
        return true
    }
    
    private func smartRefreshDocuments() {
        // Check if enough time has passed since last refresh to avoid spam
        let timeSinceLastRefresh = Date().timeIntervalSince(lastRefreshTime)
        guard timeSinceLastRefresh >= 30.0 else { return } // Increased interval to reduce disruption
        
        // Only refresh if we actually have active compressions
        guard hasActiveCompressions else { return }
        
        lastRefreshTime = Date()
        
        // Use a lighter refresh that doesn't show loading indicator
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            let result = await documentHandler.listDocumentsByEntity(
                relatedTable: "strategic_goals",
                relatedId: currentGoal.id,
                pagination: PaginationDto(page: 1, perPage: 50),
                include: [.documentType],
                auth: authContext
            )
            
            await MainActor.run {
                switch result {
                case .success(let paginatedResult):
                    updateDocumentsPreservingScroll(newDocuments: paginatedResult.items)
                    updateCompressionStatus()
                case .failure(let error):
                    print("📄 [SMART_REFRESH] Failed: \(error)")
                    // Don't clear documents on refresh failure
                }
            }
        }
    }
    
    private func updateCompressionStatus() {
        // Check if we have any documents that are currently being compressed
        let processingStatuses = ["pending", "processing", "in_progress"]
        let activeCompressions = documents.filter { doc in
            processingStatuses.contains(doc.compressionStatus.lowercased())
        }
        
        let newHasActiveCompressions = !activeCompressions.isEmpty
        let newCompressionCount = activeCompressions.count
        
        // Check if compressions have finished (count went down)
        let compressionsFinished = lastCompressionCount > newCompressionCount && lastCompressionCount > 0
        
        if newHasActiveCompressions != hasActiveCompressions {
            hasActiveCompressions = newHasActiveCompressions
            
            if hasActiveCompressions {
                print("🔄 [COMPRESSION_STATUS] \(activeCompressions.count) documents are compressing")
                startDocumentRefreshTimer() // Ensure timer is running
            } else {
                print("✅ [COMPRESSION_STATUS] All compressions completed - stopping refresh timer")
                stopDocumentRefreshTimer() // Stop timer when no active compressions
            }
        } else if compressionsFinished {
            print("⚡ [COMPRESSION_STATUS] \(lastCompressionCount - newCompressionCount) compression(s) just finished")
        }
        
        lastCompressionCount = newCompressionCount
    }
    
    private func startDocumentRefreshTimer() {
        // Only start timer if we don't already have one and we have active compressions
        guard refreshTimer == nil && hasActiveCompressions else { return }
        
        print("⏰ [TIMER] Starting document refresh timer (30s interval)")
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { _ in
            // Double-check we still have active compressions before refreshing
            if self.hasActiveCompressions {
                Task { @MainActor in
                    self.smartRefreshDocuments()
                }
            } else {
                // If no active compressions, stop the timer
                Task { @MainActor in
                    self.stopDocumentRefreshTimer()
                }
            }
        }
    }
    
    private func stopDocumentRefreshTimer() {
        if refreshTimer != nil {
            print("⏰ [TIMER] Stopping document refresh timer")
            refreshTimer?.invalidate()
            refreshTimer = nil
        }
    }
    
    private func deleteGoal(hardDelete: Bool = false, force: Bool = false) {
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
            
            if force {
                // Use bulk delete with force option for single goal
                print("🗑️ [DELETE] Starting force delete for goal: \(currentGoal.id)")
                let result = await ffiHandler.bulkDelete(ids: [currentGoal.id], hardDelete: hardDelete, force: force, auth: authContext)
                
                await MainActor.run {
                    isDeleting = false
                    switch result {
                    case .success(let batchResult):
                        print("✅ [FORCE_DELETE] Force delete result: \(batchResult)")
                        
                        if !batchResult.hardDeleted.isEmpty || !batchResult.softDeleted.isEmpty {
                            // Successfully deleted
                            let wasHardDeleted = !batchResult.hardDeleted.isEmpty
                            let message = wasHardDeleted ? "Goal permanently deleted" : "Goal archived"
                            print("✅ [FORCE_DELETE] \(message)")
                            onUpdate()
                            dismiss()
                        } else if !batchResult.failed.isEmpty {
                            // Force delete failed (very rare)
                            let errorMsg = batchResult.errors[currentGoal.id] ?? "Force delete failed"
                            errorMessage = errorMsg
                            showErrorAlert = true
                        }
                    case .failure(let error):
                        print("❌ [FORCE_DELETE] Failed to force delete goal: \(error)")
                        errorMessage = "Failed to force delete strategic goal: \(error.localizedDescription)"
                        showErrorAlert = true
                    }
                }
            } else {
                // Use regular delete (with dependency checks)
                print("🗑️ [DELETE] Starting \(hardDelete ? "hard" : "soft") delete for goal: \(currentGoal.id)")
                let result = await ffiHandler.delete(id: currentGoal.id, hardDelete: hardDelete, auth: authContext)

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
    
    /// Open a document for viewing
    private func openDocument(_ document: MediaDocumentResponse) {
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    self.errorMessage = "User not authenticated."
                    self.showErrorAlert = true
                }
                return
            }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            print("📖 [DOCUMENT_OPEN] Opening document: \(document.title ?? document.originalFilename)")
            
            let result = await documentHandler.openDocument(id: document.id, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let openResponse):
                    if let filePath = openResponse.filePath {
                        print("📖 [DOCUMENT_OPEN] ✅ Got file path: \(filePath)")
                        
                        // Convert file path to URL
                        let fileURL: URL
                        if filePath.hasPrefix("file://") {
                            fileURL = URL(string: filePath)!
                        } else {
                            fileURL = URL(fileURLWithPath: filePath)
                        }
                        
                        // Check if file exists before trying to open
                        if FileManager.default.fileExists(atPath: fileURL.path) {
                            print("📖 [DOCUMENT_OPEN] ✅ File exists, opening with QuickLook")
                            
                            // Debug: Check actual file type vs filename
                            let filename = fileURL.lastPathComponent
                            let fileExtension = (filename as NSString).pathExtension.lowercased()
                            print("📖 [DOCUMENT_OPEN] File extension from name: .\(fileExtension)")
                            
                            // Try to detect actual file type by reading file header
                            if let fileData = try? Data(contentsOf: fileURL, options: [.mappedIfSafe]) {
                                let fileSize = fileData.count
                                print("📖 [DOCUMENT_OPEN] Actual file size: \(fileSize) bytes")
                                
                                if fileSize >= 8 {
                                    let header = fileData.prefix(8)
                                    let headerHex = header.map { String(format: "%02x", $0) }.joined()
                                    print("📖 [DOCUMENT_OPEN] File header (hex): \(headerHex)")
                                    
                                    // Check for common video file signatures
                                    if headerHex.hasPrefix("00000018") || headerHex.hasPrefix("00000020") {
                                        print("📖 [DOCUMENT_OPEN] 🎬 DETECTED: This appears to be an MP4 video file!")
                                    } else if headerHex.hasPrefix("ffd8ff") {
                                        print("📖 [DOCUMENT_OPEN] 📸 DETECTED: This appears to be a JPEG image file")
                                    } else if headerHex.hasPrefix("89504e47") {
                                        print("📖 [DOCUMENT_OPEN] 📸 DETECTED: This appears to be a PNG image file")
                                    } else {
                                        print("📖 [DOCUMENT_OPEN] ❓ DETECTED: Unknown file type with header: \(headerHex)")
                                    }
                                }
                            }
                            
                            // Open with QuickLook
                            self.selectedDocumentURL = IdentifiableURL(url: fileURL)
                        } else {
                            print("📖 [DOCUMENT_OPEN] ❌ File does not exist at path: \(fileURL.path)")
                            self.errorMessage = "Document file not found on device. It may need to be downloaded first."
                            self.showErrorAlert = true
                        }
                    } else {
                        print("📖 [DOCUMENT_OPEN] ❌ No file path returned")
                        self.errorMessage = "Document is not available locally. It may need to be downloaded first."
                        self.showErrorAlert = true
                    }
                    
                case .failure(let error):
                    print("📖 [DOCUMENT_OPEN] ❌ Failed to open document: \(error)")
                    
                    // Check if it's a compression-related error
                    let errorMessage = error.localizedDescription
                    if errorMessage.contains("being compressed") {
                        self.errorMessage = "Document is currently being compressed. You can still view it, but there may be a brief delay."
                        self.showErrorAlert = true
                    } else {
                        self.errorMessage = "Failed to open document: \(errorMessage)"
                        self.showErrorAlert = true
                    }
                }
            }
        }
    }
}

// MARK: - Helper extensions moved to Core/Models/StrategicGoalModels.swift




// MARK: - Document Models & Views
// Shared components now used from DocumentManagement/

// MARK: - Goal Delete Options Sheet
struct GoalDeleteOptionsSheet: View {
    let userRole: String
    let onDelete: (Bool, Bool) -> Void  // (hardDelete, force)
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
                    if userRole.lowercased() == "admin" {
                        deleteButton
                        forceDeleteButton
                    }
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
            onDelete(false, false)
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
                
                Text("Move the goal to archive. It can be restored later. Associated documents will be preserved. Projects will remain linked.")
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
                    Text("Conditional Delete")
                        .font(.headline)
                        .foregroundColor(.red)
                    Spacer()
                }
                
                Text("Permanently delete goal if no dependencies exist. Goals with projects will fail to delete.")
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
                
                Text("⚠️ DANGER: Force delete goal regardless of dependencies. Projects will lose their strategic goal link. This cannot be undone.")
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

// MARK: - Shared Components Used:
// - EntityDeleteOptionsSheet for delete options
// - DeleteResultsSheet for bulk delete results  
// - QuickLookView for document viewing


