//
//  ProjectsView.swift
//  ActionAid SwiftUI
//
//  Projects management with advanced card-based UI and shared components
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI
import QuickLook

// MARK: - Global Strategic Goals Cache (moved here from original, but could be extracted to shared location)
@MainActor
class StrategicGoalsCache: ObservableObject {
    static let shared = StrategicGoalsCache()
    
    @Published var goals: [StrategicGoalSummary] = []
    @Published var isLoading = false
    private var lastFetch: Date?
    private let cacheTimeout: TimeInterval = 300 // 5 minutes
    
    private init() {} // Singleton
    
    func loadIfNeeded(authManager: AuthenticationManager) async {
        // Return immediately if cache is fresh
        if let lastFetch = lastFetch,
           Date().timeIntervalSince(lastFetch) < cacheTimeout,
           !goals.isEmpty {
            print("📋 [CACHE] Using cached strategic goals (\(goals.count) items)")
            return
        }
        
        guard !isLoading else { 
            print("📋 [CACHE] Already loading strategic goals")
            return 
        }
        
        print("📋 [CACHE] Loading fresh strategic goals...")
        isLoading = true
        
        guard let currentUser = authManager.currentUser else {
            isLoading = false
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        let handler = StrategicGoalFFIHandler()
        let result = await handler.listSummaries(
            pagination: PaginationDto(page: 1, perPage: 50),
            auth: authContext
        )
        
        switch result {
        case .success(let paginatedResult):
            // Process mapping off main thread - now using lightweight response
            let summaries = paginatedResult.items.map { summary in
                StrategicGoalSummary(
                    id: summary.id,
                    objectiveCode: summary.objectiveCode,
                    outcome: nil // Not available in lightweight response
                )
            }
            
            await MainActor.run {
                self.goals = summaries
                self.lastFetch = Date()
                print("📋 [CACHE] ✅ Cached \(summaries.count) strategic goals")
            }
        case .failure(let error):
            print("📋 [CACHE] ❌ Failed to load strategic goals: \(error.localizedDescription)")
        }
        
        isLoading = false
    }
    
    func refresh(authManager: AuthenticationManager) async {
        lastFetch = nil
        await loadIfNeeded(authManager: authManager)
    }
    
    /// Force refresh cache (called when strategic goals are created/updated)
    func invalidateCache(authManager: AuthenticationManager) async {
        print("📋 [CACHE] ♻️ Invalidating cache due to strategic goal changes")
        lastFetch = nil
        goals.removeAll()
        await loadIfNeeded(authManager: authManager)
    }
    
    /// Called from strategic goal creation to keep cache fresh
    func notifyStrategicGoalCreated(_ newGoal: StrategicGoalResponse, authManager: AuthenticationManager) async {
        print("📋 [CACHE] ➕ Adding new strategic goal to cache: \(newGoal.objectiveCode)")
        
        let summary = StrategicGoalSummary(
            id: newGoal.id,
            objectiveCode: newGoal.objectiveCode,
            outcome: newGoal.outcome
        )
        
        await MainActor.run {
            // Add to existing cache
            self.goals.append(summary)
            
            // Sort by objective code for consistent ordering
            self.goals.sort { $0.objectiveCode < $1.objectiveCode }
            
            print("📋 [CACHE] ✅ Cache updated with new goal, total: \(self.goals.count)")
        }
    }
    
    /// Called from strategic goal updates to keep cache fresh  
    func notifyStrategicGoalUpdated(_ updatedGoal: StrategicGoalResponse) async {
        await MainActor.run {
            if let index = self.goals.firstIndex(where: { $0.id == updatedGoal.id }) {
                self.goals[index] = StrategicGoalSummary(
                    id: updatedGoal.id,
                    objectiveCode: updatedGoal.objectiveCode,
                    outcome: updatedGoal.outcome
                )
                print("📋 [CACHE] 🔄 Updated strategic goal in cache: \(updatedGoal.objectiveCode)")
            }
        }
    }
}

// MARK: - Main View
struct ProjectsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    private let ffiHandler = ProjectFFIHandler()

    // Core data state
    @State private var projects: [ProjectResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedFilters: Set<String> = ["all"]
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var isActionBarCollapsed: Bool = false
    
    // Shared component managers
    @StateObject private var selectionManager = SelectionManager()
    @StateObject private var exportManager = ExportManager(service: ProjectExportService())
    @StateObject private var crudManager = CRUDSheetManager<ProjectResponse>(config: .project)
    @StateObject private var backendStatsManager = BackendProjectStatsManager()
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    
    // Project detail view state
    @State private var selectedProject: ProjectResponse?
    
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
    
    /// Properly filtered projects with working OR gate logic for status filters
    var filteredProjects: [ProjectResponse] {
        projects.filter { project in
            let matchesSearch = searchText.isEmpty ||
                project.name.localizedCaseInsensitiveContains(searchText) ||
                (project.objective ?? "").localizedCaseInsensitiveContains(searchText) ||
                (project.outcome ?? "").localizedCaseInsensitiveContains(searchText) ||
                (project.responsibleTeam ?? "").localizedCaseInsensitiveContains(searchText)
            
            // OR gate logic for status filters (fixed from EntityListView)
            let matchesStatus = selectedFilters.contains("all") ||
                (selectedFilters.contains("on_track") && project.statusId == 1) ||
                (selectedFilters.contains("at_risk") && project.statusId == 2) ||
                (selectedFilters.contains("delayed") && project.statusId == 3) ||
                (selectedFilters.contains("completed") && project.statusId == 4)
            
            return matchesSearch && matchesStatus
        }
    }
    
    // MARK: - Setup and Helper Methods
    
    /// Setup callbacks for shared component managers
    private func setupCallbacks() {
        // CRUD manager callbacks
        crudManager.onEntityCreated = { newProject in
            loadProjects()
        }
        crudManager.onEntityUpdated = { updatedProject in
            loadProjects()
        }
        crudManager.onEntityDeleted = { _ in
            loadProjects()
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
            // DEBUG: Temporary debug button (remove after testing)
            HStack {
                Button("🔧 Debug Project Stats") {
                    Task {
                        await debugProjectStats()
                    }
                }
                .font(.caption)
                .foregroundColor(.blue)
                Spacer()
            }
            .padding(.horizontal)
            
            // Main Entity List with date filtering integration
            EntityListView(
                entities: filteredProjects,
                isLoading: isLoading,
                emptyStateConfig: .projects,
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                filterOptions: FilterOption.projectFilters,
                currentViewStyle: $currentViewStyle,
                onViewStyleChange: { newStyle in
                    currentViewStyle = newStyle
                    viewStyleManager.setViewStyle(newStyle, for: "projects")
                },
                selectionManager: selectionManager,
                onFilterBasedSelectAll: {
                    Task {
                        await getFilteredProjectIds()
                    }
                },
                onItemTap: { project in
                    selectedProject = project
                },
                cardContent: { project in
                    ProjectCard(project: project)
                },
                tableColumns: ProjectTableConfig.columns,
                rowContent: { project, columns in
                    ProjectTableRow(
                        project: project, 
                        columns: columns
                    )
                },
                domainName: "projects",
                userRole: authManager.currentUser?.role,
                showColumnCustomizer: $showColumnCustomizer
            )
        }
        .navigationTitle("Projects")
        .navigationBarTitleDisplayMode(UIDevice.current.userInterfaceIdiom == .pad ? .large : .inline)
        .navigationBarHidden(isActionBarCollapsed)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack(spacing: 8) {
                    ViewStyleSwitcher(
                        currentViewStyle: currentViewStyle,
                        onViewStyleChange: { newStyle in
                            currentViewStyle = newStyle
                            viewStyleManager.setViewStyle(newStyle, for: "projects")
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
                CreateProjectSheet(ffiHandler: self.ffiHandler, onSave: { newProject in
                    crudManager.completeOperation(.create, result: newProject)
                    // loadProjects() is now handled by the onEntityCreated callback
                })
            },
            editSheet: { project in
                EditProjectSheet(project: project, ffiHandler: self.ffiHandler, onSave: { updatedProject in
                    crudManager.completeOperation(.edit, result: updatedProject)
                    // loadProjects() is now handled by the onEntityUpdated callback
                })
            },
            onDelete: { project, hardDelete, force in
                performSingleDelete(project: project, hardDelete: hardDelete, force: force)
            }
        )
        .fullScreenCover(item: $selectedProject) { project in
            ProjectDetailView(
                project: project, 
                onUpdate: {
                    loadProjects()
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
                DeleteResultsSheet(results: results, entityName: "Project", entityNamePlural: "Projects")
            }
        }
        .sheet(isPresented: $showExportOptions) {
            GenericExportOptionsSheet(
                selectedItemCount: selectionManager.selectedCount,
                entityName: "Project",
                entityNamePlural: "Projects",
                onExport: { includeBlobs, format in
                    performExportFromSelection(includeBlobs: includeBlobs, format: format)
                },
                isExporting: $exportManager.isExporting,
                exportError: $exportManager.exportError
            )
        }
        .onAppear {
            setupCallbacks()
            loadProjects()
            // Load saved view style preference (following shared architecture pattern)
            currentViewStyle = viewStyleManager.getViewStyle(for: "projects")
        }
        .onChange(of: projects.count) { oldCount, newCount in
            // Fetch backend stats when projects data changes
            if newCount != oldCount {
                Task {
                    await fetchBackendStats()
                }
            }
        }
        .onAppear {
            // Fetch backend stats on first appearance if projects are loaded
            if !projects.isEmpty {
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
            sharedStatsContext.entityName = "Projects"
            
            print("📊 Registered project stats with shared context: \(backendStatsManager.stats.count) stats")
        }
    }
    
    // DEBUG: Temporary debug method (remove after testing)
    private func debugProjectStats() async {
        print("🔧 [DEBUG] Starting manual project stats test...")
        
        guard let currentUser = authManager.currentUser else {
            print("❌ [DEBUG] No authenticated user")
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        print("🔧 [DEBUG] Auth context: userId=\(authContext.user_id), role=\(authContext.role)")
        
        // Test individual FFI calls
        let projectHandler = ProjectFFIHandler()
        
        print("🔧 [DEBUG] Testing getStatistics...")
        let statsResult = await projectHandler.getStatistics(auth: authContext)
        switch statsResult {
        case .success(let stats):
            print("✅ [DEBUG] getStatistics SUCCESS: \(stats)")
        case .failure(let error):
            print("❌ [DEBUG] getStatistics FAILED: \(error)")
        }
        
        print("🔧 [DEBUG] Testing getStatusBreakdown...")
        let breakdownResult = await projectHandler.getStatusBreakdown(auth: authContext)
        switch breakdownResult {
        case .success(let breakdown):
            print("✅ [DEBUG] getStatusBreakdown SUCCESS: \(breakdown)")
        case .failure(let error):
            print("❌ [DEBUG] getStatusBreakdown FAILED: \(error)")
        }
        
        print("🔧 [DEBUG] Manual test complete.")
    }
    
    // MARK: - Core Data Operations
    
    /// Load projects from backend
    private func loadProjects() {
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
                include: [.strategicGoal, .activityCount, .workshopCount, .documents], // Include strategic goal data and all counts for proper display
                auth: authContext
            )
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    print("📋 [PROJECT_LIST] Loaded \(paginatedResult.items.count) projects")
                    projects = paginatedResult.items
                case .failure(let error):
                    print("❌ [PROJECT_LIST] Failed to load projects: \(error.localizedDescription)")
                    crudManager.errorMessage = "Failed to load projects: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
            
            // Fetch backend stats after loading projects
            await fetchBackendStats()
        }
    }
    
    // MARK: - Selection and Bulk Operations
    
    /// Perform single entity delete (called from CRUD manager)
    private func performSingleDelete(project: ProjectResponse, hardDelete: Bool, force: Bool) {
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
            
            let result = await ffiHandler.delete(id: project.id, hardDelete: hardDelete, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        crudManager.completeOperation(.delete, result: project)
                        loadProjects() // Refresh the list
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
    
    /// Get filtered project IDs for bulk selection based on current UI filters
    private func getFilteredProjectIds() async {
        // Backend filter operation
        
        guard !selectionManager.isLoadingFilteredIds else { 
            return 
        }
        
        // Check if we have any backend filters active (search, status, etc.)
        let hasBackendFilters = !searchText.isEmpty || !selectedFilters.contains("all")
        
        // If no backend filters are applied, select all visible items
        if !hasBackendFilters {
            await MainActor.run {
                let allVisibleIds = Set(filteredProjects.map(\.id))
                selectionManager.selectAllItems(allVisibleIds)
            }
            return
        }
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = true
        }
        
        guard let currentUser = authManager.currentUser else {
                            // User not authenticated
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
            print("🔄 [BACKEND_FILTER] Mapped statusIds: \(statusIds ?? [])")
        }
        
        let currentFilter = ProjectFilter(
            statusIds: statusIds,
            strategicGoalIds: nil,
            responsibleTeams: nil,
            searchText: searchText.isEmpty ? nil : searchText,
            dateRange: nil,
            excludeDeleted: true
        )
        
        let result = await ffiHandler.getFilteredIds(filter: currentFilter, auth: authContext)
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = false
            switch result {
            case .success(let filteredIds):
                // Only select IDs that are currently visible (intersection with filtered data)
                let visibleIds = Set(filteredProjects.map(\.id))
                let filteredVisibleIds = Set(filteredIds).intersection(visibleIds)
                selectionManager.selectAllItems(filteredVisibleIds)
            case .failure(let error):
                // Backend filter failed
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
        if filterIds.contains("delayed") { ids.append(3) }
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
            print("🗑️ [BULK_DELETE] Starting bulk delete for \(selectedIds.count) projects")
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
                    
                    // Refresh the projects list to reflect changes
                    loadProjects()
                    
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

// MARK: - Project Card Component
struct ProjectCard: View {
    let project: ProjectResponse
    
    private var statusInfo: (text: String, color: Color) {
        switch project.statusId {
        case 1: return ("On Track", .green)
        case 2: return ("At Risk", .orange)
        case 3: return ("Delayed", .red)
        case 4: return ("Completed", .blue)
        default: return ("Unknown", .gray)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(project.name)
                        .font(.subheadline)
                        .fontWeight(.medium)
                        .foregroundColor(.primary)
                        .lineLimit(1)
                    
                    if let objective = project.objective, !objective.isEmpty {
                        Text(objective)
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .lineLimit(2)
                    }
                }
                
                Spacer()
                
                Badge(text: statusInfo.text, color: statusInfo.color)
            }
            
            // Strategic Goal and Team
            HStack {
                if let strategicGoalName = project.effectiveStrategicGoalName {
                    Label(strategicGoalName, systemImage: "flag")
                        .font(.caption)
                        .foregroundColor(.blue)
                        .lineLimit(1)
                } else {
                    Label("No Strategic Goal", systemImage: "flag")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
                
                Spacer()
                
                if let team = project.responsibleTeam {
                    Label(team, systemImage: "person.2")
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
            }
            
            // Outcome (if available)
            if let outcome = project.outcome, !outcome.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Expected Outcome")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .fontWeight(.medium)
                    
                    Text(outcome)
                        .font(.caption)
                        .foregroundColor(.primary)
                        .lineLimit(3)
                }
                .padding(.top, 4)
            }
            
            // Bottom Info
            HStack {
                if let timeline = project.timeline {
                    HStack(spacing: 4) {
                        Image(systemName: "calendar")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        Text(timeline)
                            .font(.caption)
                            .foregroundColor(.primary)
                    }
                    .lineLimit(1)
                }
                
                Spacer()
                
                HStack(spacing: 4) {
                    Text("Updated:")
                    Text(formatDate(project.updatedAt))
                        .fontWeight(.medium)
                }
                .font(.caption2)
                .foregroundColor(.secondary)
                
                if project.syncPriority == .high {
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
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}

// MARK: - Create Project Sheet
struct CreateProjectSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let ffiHandler: ProjectFFIHandler
    let onSave: (ProjectResponse) -> Void
    
    // Form fields
    @State private var name = ""
    @State private var objective = ""
    @State private var outcome = ""
    @State private var timeline = ""
    @State private var responsibleTeam = ""
    @State private var statusId: Int64 = 1 // Default to "On Track"
    @State private var strategicGoalId: String?
    @State private var syncPriority: SyncPriority = .normal
    
    // State management
    @State private var isLoading = false
    @State private var errorMessage: String?
    @FocusState private var focusedField: Field?
    
    enum Field: Hashable {
        case name, objective, outcome, timeline, responsibleTeam
    }
    
    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Basic Information") {
                    TextField("Project Name", text: $name)
                        .focused($focusedField, equals: .name)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .objective }
                    
                    TextField("Objective (Optional)", text: $objective, axis: .vertical)
                        .focused($focusedField, equals: .objective)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .outcome }
                    
                    TextField("Outcome (Optional)", text: $outcome, axis: .vertical)
                        .focused($focusedField, equals: .outcome)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .timeline }
                }
                
                Section("Project Details") {
                    TextField("Timeline (Optional)", text: $timeline)
                        .focused($focusedField, equals: .timeline)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .responsibleTeam }
                    
                    TextField("Responsible Team (Optional)", text: $responsibleTeam)
                        .focused($focusedField, equals: .responsibleTeam)
                        .submitLabel(.done)
                        .onSubmit { focusedField = nil }
                    
                    Picker("Status", selection: $statusId) {
                        Text("On Track").tag(Int64(1))
                        Text("At Risk").tag(Int64(2))
                        Text("Delayed").tag(Int64(3))
                        Text("Completed").tag(Int64(4))
                    }
                }
                
                Section("Strategic Goal") {
                    if StrategicGoalsCache.shared.isLoading && StrategicGoalsCache.shared.goals.isEmpty {
                        HStack {
                            ProgressView()
                                .scaleEffect(0.8)
                            Text("Loading goals...")
                                .foregroundColor(.secondary)
                            Spacer()
                        }
                    } else {
                        Picker("Strategic Goal (Optional)", selection: $strategicGoalId) {
                            Text("No Strategic Goal").tag(String?.none)
                            ForEach(StrategicGoalsCache.shared.goals, id: \.id) { goal in
                                Text(goal.objectiveCode).tag(String?.some(goal.id))
                            }
                        }
                    }
                }
                
                Section("Advanced") {
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
            .navigationTitle("Create Project")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { 
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        dismiss() 
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                        createProject()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Creating project...")
                        .scaleEffect(1.2)
                }
            }
        }
        .interactiveDismissDisabled(isLoading)
        .onAppear {
            // Delay focus to prevent UI hang on sheet open
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                focusedField = .name
            }
            
            // Load strategic goals using cache (non-blocking)
            Task {
                await StrategicGoalsCache.shared.loadIfNeeded(authManager: authManager)
            }
        }
    }
    
    private func createProject() {
        // Dismiss keyboard before starting creation
        focusedField = nil
        
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
        
        let newProject = NewProject(
            strategicGoalId: strategicGoalId,
            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
            objective: objective.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : objective.trimmingCharacters(in: .whitespacesAndNewlines),
            outcome: outcome.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : outcome.trimmingCharacters(in: .whitespacesAndNewlines),
            statusId: statusId,
            timeline: timeline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : timeline.trimmingCharacters(in: .whitespacesAndNewlines),
            responsibleTeam: responsibleTeam.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : responsibleTeam.trimmingCharacters(in: .whitespacesAndNewlines),
            syncPriority: syncPriority,
            createdByUserId: currentUser.userId
        )
        
        Task {
            let result = await ffiHandler.create(newProject: newProject, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let createdProject):
                    // Success haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(createdProject)
                    dismiss()
                case .failure(let error):
                    // Error haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to create project: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Edit Project Sheet
struct EditProjectSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    @StateObject private var goalsCache = StrategicGoalsCache.shared
    let project: ProjectResponse
    let ffiHandler: ProjectFFIHandler
    let onSave: (ProjectResponse) -> Void
    
    @State private var name = ""
    @State private var objective = ""
    @State private var outcome = ""
    @State private var timeline = ""
    @State private var responsibleTeam = ""
    @State private var statusId: Int64 = 1
    @State private var strategicGoalId: String?
    @State private var syncPriority: SyncPriority = .normal
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    // Track original values to detect changes
    @State private var originalName = ""
    @State private var originalObjective = ""
    @State private var originalOutcome = ""
    @State private var originalTimeline = ""
    @State private var originalResponsibleTeam = ""
    @State private var originalStatusId: Int64 = 1
    @State private var originalStrategicGoalId: String?
    @State private var originalSyncPriority: SyncPriority = .normal
    
    // For focus management
    private enum Field: Hashable {
        case name, objective, outcome, timeline, responsibleTeam
    }
    @FocusState private var focusedField: Field?
    
    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    /// Check if any field values have actually changed
    private var hasChanges: Bool {
        let nameChanged = name != originalName
        let objectiveChanged = objective != originalObjective
        let outcomeChanged = outcome != originalOutcome
        let timelineChanged = timeline != originalTimeline
        let teamChanged = responsibleTeam != originalResponsibleTeam
        let statusChanged = statusId != originalStatusId
        let goalChanged = strategicGoalId != originalStrategicGoalId
        let priorityChanged = syncPriority != originalSyncPriority
        
        let hasAnyChanges = nameChanged || objectiveChanged || outcomeChanged || timelineChanged || teamChanged || statusChanged || goalChanged || priorityChanged
        
        // Debug logging for change detection
        if hasAnyChanges {
            print("📝 [CHANGE_DETECTION] Changes detected:")
            if nameChanged { print("   • Name: '\(originalName)' → '\(name)'") }
            if objectiveChanged { print("   • Objective: '\(originalObjective)' → '\(objective)'") }
            if outcomeChanged { print("   • Outcome: '\(originalOutcome)' → '\(outcome)'") }
            if timelineChanged { print("   • Timeline: '\(originalTimeline)' → '\(timeline)'") }
            if teamChanged { print("   • Team: '\(originalResponsibleTeam)' → '\(responsibleTeam)'") }
            if statusChanged { print("   • Status: '\(originalStatusId)' → '\(statusId)'") }
            if goalChanged { print("   • Strategic Goal: '\(originalStrategicGoalId ?? "nil")' → '\(strategicGoalId ?? "nil")'") }
            if priorityChanged { print("   • Priority: '\(originalSyncPriority)' → '\(syncPriority)'") }
        } else {
            print("📝 [CHANGE_DETECTION] No changes detected")
        }
        
        return hasAnyChanges
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Basic Information") {
                    TextField("Project Name", text: $name)
                        .focused($focusedField, equals: .name)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .objective }
                    
                    TextField("Objective (Optional)", text: $objective, axis: .vertical)
                        .focused($focusedField, equals: .objective)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .outcome }
                    
                    TextField("Outcome (Optional)", text: $outcome, axis: .vertical)
                        .focused($focusedField, equals: .outcome)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .timeline }
                }
                
                Section("Project Details") {
                    TextField("Timeline (Optional)", text: $timeline)
                        .focused($focusedField, equals: .timeline)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .responsibleTeam }
                    
                    TextField("Responsible Team (Optional)", text: $responsibleTeam)
                        .focused($focusedField, equals: .responsibleTeam)
                        .submitLabel(.done)
                        .onSubmit { focusedField = nil }
                    
                    Picker("Status", selection: $statusId) {
                        Text("On Track").tag(Int64(1))
                        Text("At Risk").tag(Int64(2))
                        Text("Delayed").tag(Int64(3))
                        Text("Completed").tag(Int64(4))
                    }
                }
                
                Section("Strategic Goal") {
                    if goalsCache.isLoading && goalsCache.goals.isEmpty {
                        HStack {
                            ProgressView()
                                .scaleEffect(0.8)
                            Text("Loading goals...")
                                .foregroundColor(.secondary)
                            Spacer()
                        }
                    } else {
                        Picker("Strategic Goal (Optional)", selection: $strategicGoalId) {
                            Text("No Strategic Goal").tag(String?.none)
                            ForEach(goalsCache.goals, id: \.id) { goal in
                                Text(goal.objectiveCode).tag(String?.some(goal.id))
                            }
                        }
                    }
                }
                
                Section("Advanced") {
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
            .navigationTitle("Edit Project")
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
                        updateProject()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Updating project...")
                        .scaleEffect(1.2)
                }
            }
        }
        .interactiveDismissDisabled(isLoading)
        .onAppear {
            populateFields()
            // Delay focus to prevent constraint conflicts
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                focusedField = .name
            }
            
            // Load strategic goals using cache (non-blocking)
            Task {
                await goalsCache.loadIfNeeded(authManager: authManager)
            }
        }
    }
    
    private func populateFields() {
        print("📋 [POPULATE_FIELDS] Populating edit form with project data")
        
        // Set current field values
        name = project.name
        objective = project.objective ?? ""
        outcome = project.outcome ?? ""
        timeline = project.timeline ?? ""
        responsibleTeam = project.responsibleTeam ?? ""
        statusId = project.statusId ?? 1
        strategicGoalId = project.strategicGoalId
        syncPriority = project.syncPriority
        
        // Store original values for change detection
        originalName = project.name
        originalObjective = project.objective ?? ""
        originalOutcome = project.outcome ?? ""
        originalTimeline = project.timeline ?? ""
        originalResponsibleTeam = project.responsibleTeam ?? ""
        originalStatusId = project.statusId ?? 1
        originalStrategicGoalId = project.strategicGoalId
        originalSyncPriority = project.syncPriority
        
        print("📋 [POPULATE_FIELDS] Original strategic goal ID: '\(originalStrategicGoalId ?? "nil")'")
        print("📋 [POPULATE_FIELDS] Current strategic goal ID: '\(strategicGoalId ?? "nil")'")
    }
    
    private func updateProject() {
        print("💾 [UPDATE_PROJECT] Save button tapped")
        
        // Dismiss keyboard before starting update
        focusedField = nil
        
        // Check if any changes were actually made
        if !hasChanges {
            print("💾 [UPDATE_PROJECT] No changes detected, dismissing without API call")
            // No changes detected, just dismiss without calling API or onSave
            UINotificationFeedbackGenerator().notificationOccurred(.success)
            dismiss()
            return
        }
        
        print("💾 [UPDATE_PROJECT] Changes detected, proceeding with API call")
        
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
        
        // Handle double optional for strategic goal - required for proper null handling
        let strategicGoalUpdate: String??
        if strategicGoalId != originalStrategicGoalId {
            // Strategic goal changed, send the new value (could be nil to unset)
            strategicGoalUpdate = .some(strategicGoalId)
            print("🎯 [PROJECT_UPDATE] Strategic goal changed: '\(originalStrategicGoalId ?? "nil")' → '\(strategicGoalId ?? "nil")'")
        } else {
            // Strategic goal unchanged, don't update this field
            strategicGoalUpdate = .none
            print("🎯 [PROJECT_UPDATE] Strategic goal unchanged: '\(strategicGoalId ?? "nil")'")
        }
        
        let updateProject = UpdateProject(
            strategicGoalId: strategicGoalUpdate,
            name: name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : name.trimmingCharacters(in: .whitespacesAndNewlines),
            objective: objective.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : objective.trimmingCharacters(in: .whitespacesAndNewlines),
            outcome: outcome.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : outcome.trimmingCharacters(in: .whitespacesAndNewlines),
            statusId: statusId,
            timeline: timeline.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : timeline.trimmingCharacters(in: .whitespacesAndNewlines),
            responsibleTeam: responsibleTeam.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : responsibleTeam.trimmingCharacters(in: .whitespacesAndNewlines),
            syncPriority: syncPriority,
            updatedByUserId: currentUser.userId
        )
        
        Task {
            let result = await ffiHandler.update(id: project.id, update: updateProject, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let updatedProject):
                    print("✅ [PROJECT_UPDATE] Update successful. Strategic goal in response: '\(updatedProject.strategicGoalId ?? "nil")'")
                    print("✅ [PROJECT_UPDATE] Strategic goal name in response: '\(updatedProject.effectiveStrategicGoalName ?? "nil")'")
                    // Success haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(updatedProject)
                    dismiss()
                case .failure(let error):
                    print("❌ [PROJECT_UPDATE] Update failed: \(error.localizedDescription)")
                    // Error haptic feedback
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to update project: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Project Detail View (Enhanced with Document Upload)
struct ProjectDetailView: View {
    @State private var currentProject: ProjectResponse
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    private let ffiHandler = ProjectFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    
    // Document upload state
    @State private var documents: [MediaDocumentResponse] = []
    @State private var showUploadSheet = false
    @State private var showEditSheet = false
    @State private var showDeleteConfirmation = false
    @State private var showDeleteOptions = false
    @State private var isDeleting = false
    @State private var isLoadingDocuments = false
    @State private var showErrorAlert = false
    @State private var errorMessage: String?
    
    // Document viewing state
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Document refresh mechanism
    @State private var refreshTimer: Timer?
    @State private var lastRefreshTime = Date()
    @State private var hasActiveCompressions = false
    
    init(project: ProjectResponse, onUpdate: @escaping () -> Void) {
        _currentProject = State(initialValue: project)
        self.onUpdate = onUpdate
    }
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Project Header
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(currentProject.name)
                                    .font(.largeTitle)
                                    .fontWeight(.bold)
                                
                                if let objective = currentProject.objective {
                                    Text(objective)
                                        .font(.subheadline)
                                        .foregroundColor(.secondary)
                                }
                            }
                            Spacer()
                            Badge(text: currentProject.statusName, color: currentProject.statusColor)
                        }
                        
                        Divider()
                        
                        // Key Metrics Row - Evenly Distributed
                        HStack(spacing: 0) {
                            // Activities
                            VStack(alignment: .center, spacing: 4) {
                                Text("Activities")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentProject.activityCount ?? 0)")
                                    .font(.headline)
                                    .fontWeight(.semibold)
                                    .foregroundColor(.blue)
                            }
                            .frame(maxWidth: .infinity)
                            
                            // Workshops
                            VStack(alignment: .center, spacing: 4) {
                                Text("Workshops")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentProject.workshopCount ?? 0)")
                                    .font(.headline)
                                    .fontWeight(.semibold)
                                    .foregroundColor(.green)
                            }
                            .frame(maxWidth: .infinity)
                            
                            // Documents
                            VStack(alignment: .center, spacing: 4) {
                                Text("Documents")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentProject.documentCount ?? 0)")
                                    .font(.headline)
                                    .fontWeight(.semibold)
                                    .foregroundColor(.orange)
                            }
                            .frame(maxWidth: .infinity)
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Project Details
                    VStack(alignment: .leading, spacing: 16) {
                        if let outcome = currentProject.outcome {
                            DetailRow(label: "Expected Outcome", value: outcome)
                        }
                        
                        if let team = currentProject.responsibleTeam {
                            DetailRow(label: "Responsible Team", value: team)
                        }
                        
                        if let timeline = currentProject.timeline {
                            DetailRow(label: "Timeline", value: timeline)
                        }
                        
                        if let strategicGoalName = currentProject.effectiveStrategicGoalName {
                            DetailRow(label: "Strategic Goal", value: strategicGoalName)
                        } else {
                            DetailRow(label: "Strategic Goal", value: "Not assigned")
                        }
                        
                        DetailRow(label: "Sync Priority", value: currentProject.syncPriority.rawValue.capitalized)
                        
                        Divider()
                        
                        DetailRow(label: "Created", value: formatDate(currentProject.createdAt))
                        DetailRow(label: "Created By", value: currentProject.createdByUsername ?? currentProject.createdByUserId ?? "Unknown")
                        DetailRow(label: "Last Updated", value: formatDate(currentProject.updatedAt))
                        DetailRow(label: "Updated By", value: currentProject.updatedByUsername ?? currentProject.updatedByUserId ?? "Unknown")
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
            .navigationTitle("Project Details")
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
                            Label("Delete Project", systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            .sheet(isPresented: $showUploadSheet) {
                DocumentUploadSheet(
                    entity: currentProject.asDocumentUploadAdapter(),
                    config: .standard,
                    onUploadComplete: {
                        print("📤 [PROJECT_DETAIL] Document upload completed, refreshing data...")
                        
                        // Reload documents list
                        loadDocuments()
                        
                        // Refresh main view document counts  
                        onUpdate()
                        
                        // ✅ Also reload project data to update counters in detail view
                        reloadProjectData()
                    }
                )
            }
            .sheet(isPresented: $showEditSheet) {
                EditProjectSheet(project: currentProject, ffiHandler: ffiHandler) { updatedProject in
                    print("💾 [PROJECT_DETAIL] Edit completed. Updated strategic goal: '\(updatedProject.strategicGoalId ?? "nil")'")
                    // Update local state and notify parent
                    currentProject = updatedProject
                    print("🔄 [PROJECT_DETAIL] Calling onUpdate() to refresh main list")
                    onUpdate()
                    // Reload documents after project update
                    loadDocuments()
                }
                .environmentObject(authManager)
            }
            .sheet(isPresented: $showDeleteOptions) {
                ProjectDeleteOptionsSheet(
                    userRole: authManager.currentUser?.role ?? "",
                    onDelete: { hardDelete, force in
                        deleteProject(hardDelete: hardDelete, force: force)
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
                print("👁️ [PROJECT_DETAIL] View appeared, loading fresh data...")
                
                // Load documents list
                loadDocuments()
                
                // Also reload project data to ensure counters are accurate
                reloadProjectData()
            }
            .onDisappear {
                stopDocumentRefreshTimer()
            }
            .alert("Delete Project", isPresented: $showDeleteConfirmation) {
                Button("Cancel", role: .cancel) { }
                Button("Delete", role: .destructive) {
                    deleteProject(hardDelete: false, force: false)
                }
            } message: {
                Text("Are you sure you want to delete this project? It will be archived and can be restored later.")
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
                relatedTable: "projects",
                relatedId: currentProject.id,
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
                    
                    // ✅ UPDATE PROJECT DOCUMENT COUNT to reflect actual count
                    updateProjectDocumentCount(actualCount: paginatedResult.items.count)
                    
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
    
    /// Update the project's document count to reflect the actual loaded documents
    private func updateProjectDocumentCount(actualCount: Int) {
        // Check if the document count in the project data is outdated
        let storedCount = Int(currentProject.documentCount ?? 0)
        if storedCount != actualCount {
            print("📊 [PROJECT_DETAIL] Document count mismatch: stored=\(storedCount), actual=\(actualCount)")
            print("📊 [PROJECT_DETAIL] Reloading project data to get updated counts...")
            
            // Reload project data from backend to get accurate counts
            reloadProjectData()
        }
    }
    
    /// Reload project data from backend to get updated counts and metadata
    private func reloadProjectData() {
        print("🔄 [PROJECT_DETAIL] Reloading project data to get updated counts...")
        
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.get(
                id: currentProject.id,
                include: [.strategicGoal, .activityCount, .workshopCount, .documents], // Include strategic goal data and all counts for complete info
                auth: authContext
            )
            
            await MainActor.run {
                switch result {
                case .success(let updatedProject):
                    print("✅ [PROJECT_DETAIL] Project data reloaded. Document count: \(updatedProject.documentCount ?? 0)")
                    currentProject = updatedProject
                    
                case .failure(let error):
                    print("❌ [PROJECT_DETAIL] Failed to reload project data: \(error)")
                    // Don't show error to user for this background operation
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
                relatedTable: "projects",
                relatedId: currentProject.id,
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
    
    /// Check if there are any active compressions and manage timer accordingly
    private func updateCompressionStatus() {
        let activeCompressions = documents.filter { doc in
            doc.compressionStatus == "inProgress" || doc.compressionStatus == "pending"
        }
        
        let wasActive = hasActiveCompressions
        hasActiveCompressions = !activeCompressions.isEmpty
        
        print("📄 [COMPRESSION_STATUS] Active compressions: \(activeCompressions.count), hasActiveCompressions: \(hasActiveCompressions)")
        
        if hasActiveCompressions && refreshTimer == nil {
            print("📄 [COMPRESSION_STATUS] Starting document refresh timer")
            startDocumentRefreshTimer()
        } else if !hasActiveCompressions && refreshTimer != nil {
            print("📄 [COMPRESSION_STATUS] Stopping document refresh timer")
            stopDocumentRefreshTimer()
        }
        
        // Log compression status for debugging
        for doc in activeCompressions {
            print("📄 [COMPRESSION_STATUS] Active: \(doc.title ?? doc.originalFilename) - Status: \(doc.compressionStatus)")
        }
    }
    
    /// Start timer for refreshing document compression status
    private func startDocumentRefreshTimer() {
        stopDocumentRefreshTimer() // Stop any existing timer
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { _ in
            Task { @MainActor in
                self.smartRefreshDocuments()
            }
        }
    }
    
    /// Stop document refresh timer
    private func stopDocumentRefreshTimer() {
        refreshTimer?.invalidate()
        refreshTimer = nil
    }
    
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
    
    private func deleteProject(hardDelete: Bool = false, force: Bool = false) {
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
            
            let result = await ffiHandler.delete(id: currentProject.id, hardDelete: hardDelete, auth: authContext)
            
            await MainActor.run {
                isDeleting = false
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        print("✅ [DELETE] \(deleteResponse.displayMessage)")
                        onUpdate()
                        dismiss()
                    } else {
                        errorMessage = deleteResponse.displayMessage
                        showErrorAlert = true
                    }
                case .failure(let error):
                    print("❌ [DELETE] Failed to delete project: \(error)")
                    errorMessage = "Failed to delete project: \(error.localizedDescription)"
                    showErrorAlert = true
                }
            }
        }
    }
}

// MARK: - Project Delete Options Sheet
struct ProjectDeleteOptionsSheet: View {
    let userRole: String
    let onDelete: (Bool, Bool) -> Void  // (hardDelete, force)
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Text("How would you like to delete this project?")
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
            .navigationTitle("Delete Project")
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
                    Text("Archive Project")
                        .font(.headline)
                        .foregroundColor(.primary)
                    Spacer()
                }
                
                Text("Move the project to archive. It can be restored later. Associated activities and documents will be preserved.")
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
                
                Text("Permanently delete project if no dependencies exist. Projects with activities or workshops will fail to delete.")
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
                
                Text("⚠️ DANGER: Force delete project regardless of dependencies. Activities and workshops will lose their project link. This cannot be undone.")
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