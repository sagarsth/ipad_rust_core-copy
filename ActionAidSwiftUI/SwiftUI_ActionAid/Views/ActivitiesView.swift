//
//  ActivitiesView.swift
//  ActionAid SwiftUI
//
//  Activities management with advanced card-based UI and shared components
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI
import QuickLook

// MARK: - Main View
struct ActivitiesView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    private let ffiHandler = ActivityFFIHandler()

    // Core data state
    @State private var activities: [ActivityResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedFilters: Set<String> = ["all"]
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var isActionBarCollapsed: Bool = false
    
    // Shared component managers
    @StateObject private var selectionManager = SelectionManager()
    @StateObject private var exportManager = ExportManager(service: ActivityExportService())
    @StateObject private var crudManager = CRUDSheetManager<ActivityResponse>(config: .activity)
    @StateObject private var backendStatsManager = BackendActivityStatsManager()
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    
    // Activity detail view state
    @State private var selectedActivity: ActivityResponse?
    
    // Column customization state
    @State private var showColumnCustomizer = false
    
    // Document viewing state
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Bulk operations state
    @State private var isPerformingBulkDelete = false
    @State private var bulkDeleteResults: BatchDeleteResult?
    @State private var showBulkDeleteResults = false
    @State private var showExportOptions = false
    @State private var showBulkStatusUpdate = false
    @State private var selectedBulkStatus: Int64 = 1
    
    // MARK: - Computed Properties
    
    var filteredActivities: [ActivityResponse] {
        activities.filter { activity in
            let matchesSearch = searchText.isEmpty ||
                (activity.description ?? "").localizedCaseInsensitiveContains(searchText) ||
                (activity.kpi ?? "").localizedCaseInsensitiveContains(searchText) ||
                (activity.projectName ?? "").localizedCaseInsensitiveContains(searchText)
            
            let matchesStatus = selectedFilters.contains("all") ||
                (selectedFilters.contains("completed") && activity.statusId == 1) ||
                (selectedFilters.contains("in_progress") && activity.statusId == 2) ||
                (selectedFilters.contains("pending") && activity.statusId == 3) ||
                (selectedFilters.contains("blocked") && activity.statusId == 4)
            
            return matchesSearch && matchesStatus
        }
    }
    
    // MARK: - Setup and Helper Methods
    
    private func setupCallbacks() {
        crudManager.onEntityCreated = { newActivity in
            print("🎯 [ACTIVITIES_VIEW] onEntityCreated callback triggered for activity: \(newActivity.id)")
            print("🔄 [ACTIVITIES_VIEW] Calling loadActivities() to refresh list...")
            loadActivities()
        }
        crudManager.onEntityUpdated = { updatedActivity in
            print("📝 [ACTIVITIES_VIEW] onEntityUpdated callback triggered for activity: \(updatedActivity.id)")
            print("🔄 [ACTIVITIES_VIEW] Calling loadActivities() to refresh list...")
            loadActivities()
        }
        crudManager.onEntityDeleted = { deletedActivity in
            print("🗑️ [ACTIVITIES_VIEW] onEntityDeleted callback triggered for activity: \(deletedActivity.id)")
            print("🔄 [ACTIVITIES_VIEW] Calling loadActivities() to refresh list...")
            loadActivities()
        }
    }
    
    private func createAuthContext() -> AuthContextPayload {
        return AuthContextPayload(
            user_id: authManager.currentUser?.userId ?? "",
            role: authManager.currentUser?.role ?? "",
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
    }
    
    var body: some View {
        VStack(spacing: 0) {
            EntityListView(
                entities: filteredActivities,
                isLoading: isLoading,
                emptyStateConfig: .activities,
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                filterOptions: FilterOption.activityFilters,
                currentViewStyle: $currentViewStyle,
                onViewStyleChange: { newStyle in
                    currentViewStyle = newStyle
                    viewStyleManager.setViewStyle(newStyle, for: "activities")
                },
                selectionManager: selectionManager,
                onFilterBasedSelectAll: {
                    Task {
                        await getFilteredActivityIds()
                    }
                },
                onItemTap: { activity in
                    selectedActivity = activity
                },
                cardContent: { activity in
                    ActivityCard(activity: activity)
                },
                tableColumns: ActivityTableConfig.columns,
                rowContent: { activity, columns in
                    ActivityTableRow(
                        activity: activity,
                        columns: columns
                    )
                },
                domainName: "activities",
                userRole: authManager.currentUser?.role,
                showColumnCustomizer: $showColumnCustomizer
            )
        }
        .navigationTitle("Activities")
        .navigationBarTitleDisplayMode(UIDevice.current.userInterfaceIdiom == .pad ? .large : .inline)
        .navigationBarHidden(isActionBarCollapsed)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack(spacing: 8) {
                    ViewStyleSwitcher(
                        currentViewStyle: currentViewStyle,
                        onViewStyleChange: { newStyle in
                            currentViewStyle = newStyle
                            viewStyleManager.setViewStyle(newStyle, for: "activities")
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
                CreateActivitySheet(
                    onSave: { newActivity in
                        crudManager.completeOperation(.create, result: newActivity)
                    }
                )
            },
            editSheet: { activity in
                EditActivitySheet(
                    activity: activity,
                    onSave: { updatedActivity in
                        crudManager.completeOperation(.edit, result: updatedActivity)
                    }
                )
            },
            onDelete: { activity, hardDelete, force in
                performSingleDelete(activity: activity, hardDelete: hardDelete, force: force)
            }
        )
        .fullScreenCover(item: $selectedActivity) { activity in
            ActivityDetailView(
                activity: activity,
                onUpdate: {
                    loadActivities()
                }
            )
        }
        .sheet(isPresented: $showBulkDeleteResults) {
            if let results = bulkDeleteResults {
                DeleteResultsSheet(results: results, entityName: "Activity", entityNamePlural: "Activities")
            }
        }
        .sheet(isPresented: $showExportOptions) {
            GenericExportOptionsSheet(
                selectedItemCount: selectionManager.selectedCount,
                entityName: "Activity",
                entityNamePlural: "Activities",
                onExport: { includeBlobs, format in
                    performExportFromSelection(includeBlobs: includeBlobs, format: format)
                },
                isExporting: $exportManager.isExporting,
                exportError: $exportManager.exportError
            )
        }
        .sheet(isPresented: $showBulkStatusUpdate) {
            BulkStatusUpdateSheet(
                selectedCount: selectionManager.selectedCount,
                currentStatus: selectedBulkStatus,
                onUpdate: { newStatus in
                    performBulkStatusUpdate(newStatus: newStatus)
                }
            )
        }
        .onAppear {
            setupCallbacks()
            loadActivities()
            currentViewStyle = viewStyleManager.getViewStyle(for: "activities")
        }
        .onChange(of: activities.count) { oldCount, newCount in
            if newCount != oldCount {
                Task {
                    await fetchBackendStats()
                }
            }
        }
    }
    
    // MARK: - Backend Statistics
    
    private func fetchBackendStats() async {
        guard let currentUser = authManager.currentUser else { return }
        
        let authContext = createAuthContext()
        
        await backendStatsManager.fetchStats(auth: authContext)
        
        await MainActor.run {
            let anyStatsManager = backendStatsManager.createAnyStatsManager()
            sharedStatsContext.currentEntityStats = anyStatsManager
            sharedStatsContext.entityName = "Activities"
        }
    }
    
    // MARK: - Data Loading
    
    private func loadActivities() {
        print("📋 [LOAD_ACTIVITIES] Starting to load activities list...")
        isLoading = true
        Task {
            guard let currentUser = authManager.currentUser else {
                print("❌ [LOAD_ACTIVITIES] User not authenticated")
                await MainActor.run {
                    isLoading = false
                }
                return
            }
            
            print("✅ [LOAD_ACTIVITIES] User authenticated: \(currentUser.userId)")
            
            let authContext = createAuthContext()
            print("🔐 [LOAD_ACTIVITIES] Auth context created, calling FFI list...")
            
            let result = await ffiHandler.list(
                pagination: PaginationDto(page: 1, perPage: 100),
                include: [.project, .status, .documents],
                auth: authContext
            )
            
            print("🌐 [LOAD_ACTIVITIES] FFI list call returned")
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    print("✅ [LOAD_ACTIVITIES] SUCCESS! Loaded \(paginatedResult.items.count) activities")
                    print("✅ [LOAD_ACTIVITIES] Activities count before update: \(activities.count)")
                    activities = paginatedResult.items
                    print("✅ [LOAD_ACTIVITIES] Activities count after update: \(activities.count)")
                    
                    if activities.count > 0 {
                        print("📝 [LOAD_ACTIVITIES] Sample activity IDs: \(activities.prefix(3).map(\.id))")
                    }
                case .failure(let error):
                    print("❌ [LOAD_ACTIVITIES] FAILED! Error: \(error)")
                    print("❌ [LOAD_ACTIVITIES] Error type: \(type(of: error))")
                    crudManager.errorMessage = "Failed to load activities: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
            
            print("📊 [LOAD_ACTIVITIES] Fetching backend stats...")
            await fetchBackendStats()
        }
    }
    
    // MARK: - Bulk Operations
    
    private func performSingleDelete(activity: ActivityResponse, hardDelete: Bool, force: Bool) {
        crudManager.startOperation(.delete)
        
        Task {
            let authContext = createAuthContext()
            let result = await ffiHandler.delete(id: activity.id, hardDelete: hardDelete, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        crudManager.completeOperation(.delete, result: activity)
                        loadActivities()
                    } else {
                        crudManager.completeOperation(.delete, error: NSError(domain: "Delete", code: 2, userInfo: [NSLocalizedDescriptionKey: deleteResponse.displayMessage]))
                    }
                case .failure(let error):
                    crudManager.completeOperation(.delete, error: error)
                }
            }
        }
    }
    
    private func getFilteredActivityIds() async {
        guard !selectionManager.isLoadingFilteredIds else { return }
        
        let hasBackendFilters = !searchText.isEmpty || !selectedFilters.contains("all")
        
        if !hasBackendFilters {
            await MainActor.run {
                let allVisibleIds = Set(filteredActivities.map(\.id))
                selectionManager.selectAllItems(allVisibleIds)
            }
            return
        }
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = true
        }
        
        let authContext = createAuthContext()
        
        var statusIds: [Int64]? = nil
        if !selectedFilters.contains("all") {
            statusIds = mapFilterIdsToStatusIds(selectedFilters)
        }
        
        let currentFilter = ActivityFilter(
            statusIds: statusIds,
            projectIds: nil,
            searchText: searchText.isEmpty ? nil : searchText,
            dateRange: nil,
            targetValueRange: nil,
            actualValueRange: nil,
            excludeDeleted: true
        )
        
        let result = await ffiHandler.getFilteredIds(filter: currentFilter, auth: authContext)
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = false
            switch result {
            case .success(let filteredIds):
                let visibleIds = Set(filteredActivities.map(\.id))
                let filteredVisibleIds = Set(filteredIds).intersection(visibleIds)
                selectionManager.selectAllItems(filteredVisibleIds)
            case .failure(let error):
                crudManager.errorMessage = "Failed to get filtered IDs: \(error.localizedDescription)"
                crudManager.showErrorAlert = true
            }
        }
    }
    
    private func mapFilterIdsToStatusIds(_ filterIds: Set<String>) -> [Int64] {
        var ids: [Int64] = []
        if filterIds.contains("completed") { ids.append(1) }
        if filterIds.contains("in_progress") { ids.append(2) }
        if filterIds.contains("pending") { ids.append(3) }
        if filterIds.contains("blocked") { ids.append(4) }
        return ids
    }
    
    private func performBulkDelete(hardDelete: Bool, force: Bool = false) {
        guard !selectionManager.selectedItems.isEmpty else { return }
        
        isPerformingBulkDelete = true
        
        Task {
            let authContext = createAuthContext()
            let selectedIds = Array(selectionManager.selectedItems)
            
            // Since we don't have a bulk delete method in ActivityFFIHandler,
            // we'll delete them individually
            var softDeleted: [String] = []
            var hardDeleted: [String] = []
            var failed: [String] = []
            var errors: [String: String] = [:]
            
            for id in selectedIds {
                let result = await ffiHandler.delete(id: id, hardDelete: hardDelete, auth: authContext)
                
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        if deleteResponse.isHardDeleted {
                            hardDeleted.append(id)
                        } else {
                            softDeleted.append(id)
                        }
                    } else {
                        failed.append(id)
                        errors[id] = deleteResponse.displayMessage
                    }
                case .failure(let error):
                    failed.append(id)
                    errors[id] = error.localizedDescription
                }
            }
            
            let batchResult = BatchDeleteResult(
                hardDeleted: hardDeleted,
                softDeleted: softDeleted,
                failed: failed,
                dependencies: [:],
                errors: errors
            )
            
            await MainActor.run {
                self.isPerformingBulkDelete = false
                self.bulkDeleteResults = batchResult
                self.selectionManager.clearSelection()
                loadActivities()
                
                if !batchResult.failed.isEmpty {
                    self.showBulkDeleteResults = true
                }
            }
        }
    }
    
    private func performBulkStatusUpdate(newStatus: Int64) {
        guard !selectionManager.selectedItems.isEmpty else { return }
        
        Task {
            let authContext = createAuthContext()
            let selectedIds = Array(selectionManager.selectedItems)
            
            let result = await ffiHandler.bulkUpdateStatus(
                activityIds: selectedIds,
                statusId: newStatus,
                auth: authContext
            )
            
            await MainActor.run {
                switch result {
                case .success(let response):
                    print("✅ Updated \(response.updatedCount) activities to status \(response.statusId)")
                    selectionManager.clearSelection()
                    loadActivities()
                case .failure(let error):
                    crudManager.errorMessage = "Failed to update status: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
        }
    }
    
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
}

// MARK: - Activity Card Component
struct ActivityCard: View {
    let activity: ActivityResponse
    
    private var statusInfo: (text: String, color: Color) {
        switch activity.statusId {
        case 1: return ("Completed", .green)
        case 2: return ("In Progress", .blue)
        case 3: return ("Pending", .orange)
        case 4: return ("Blocked", .red)
        default: return ("Unknown", .gray)
        }
    }
    
    private var progressColor: Color {
        guard let progress = activity.progressPercentage else { return .gray }
        if progress >= 80 { return .green }
        else if progress >= 50 { return .blue }
        else if progress > 0 { return .orange }
        else { return .red }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    if let description = activity.description {
                        Text(description)
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .foregroundColor(.primary)
                            .lineLimit(2)
                    } else {
                        Text("No Description")
                            .font(.subheadline)
                            .fontWeight(.medium)
                            .foregroundColor(.secondary)
                            .italic()
                    }
                    
                    if let projectName = activity.projectName {
                        Label(projectName, systemImage: "folder")
                            .font(.caption)
                            .foregroundColor(.blue)
                            .lineLimit(1)
                    }
                }
                
                Spacer()
                
                Badge(text: statusInfo.text, color: statusInfo.color)
            }
            
            // KPI and Progress
            if let kpi = activity.kpi {
                VStack(alignment: .leading, spacing: 4) {
                    Text("KPI")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                        .fontWeight(.medium)
                    
                    Text(kpi)
                        .font(.caption)
                        .foregroundColor(.primary)
                        .lineLimit(2)
                }
            }
            
            // Progress Bar
            if let progress = activity.progressPercentage {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Progress")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        
                        Spacer()
                        
                        Text(activity.formattedProgress)
                            .font(.caption)
                            .fontWeight(.medium)
                            .foregroundColor(progressColor)
                    }
                    
                    GeometryReader { geometry in
                        ZStack(alignment: .leading) {
                            RoundedRectangle(cornerRadius: 4)
                                .fill(Color(.systemGray5))
                                .frame(height: 8)
                            
                            RoundedRectangle(cornerRadius: 4)
                                .fill(progressColor)
                                .frame(width: geometry.size.width * (progress / 100), height: 8)
                        }
                    }
                    .frame(height: 8)
                }
            }
            
            // Target vs Actual
            if let target = activity.targetValue {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Target")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        Text(String(format: "%.0f", target))
                            .font(.caption)
                            .fontWeight(.medium)
                    }
                    
                    Spacer()
                    
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("Actual")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                        Text(String(format: "%.0f", activity.actualValue ?? 0))
                            .font(.caption)
                            .fontWeight(.medium)
                            .foregroundColor(activity.actualValue ?? 0 >= target ? .green : .orange)
                    }
                }
            }
            
            // Bottom Info
            HStack {
                HStack(spacing: 4) {
                    Text("Updated:")
                    Text(formatDate(activity.updatedAt))
                        .fontWeight(.medium)
                }
                .font(.caption2)
                .foregroundColor(.secondary)
                
                Spacer()
                
                if activity.syncPriority == .high {
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

// MARK: - Create Activity Sheet
struct CreateActivitySheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let onSave: (ActivityResponse) -> Void
    
    @State private var description = ""
    @State private var kpi = ""
    @State private var targetValue = ""
    @State private var actualValue = ""
    @State private var statusId: Int64 = 2 // Default to "In Progress"
    @State private var projectId: String?
    @State private var syncPriority: SyncPriority = .normal
    
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var projects: [ProjectResponse] = []
    @State private var isLoadingProjects = false
    
    @FocusState private var focusedField: Field?
    
    enum Field: Hashable {
        case description, kpi, targetValue, actualValue
    }
    
    private let ffiHandler = ActivityFFIHandler()
    private let projectHandler = ProjectFFIHandler()
    
    private var canSave: Bool {
        !isLoading
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Activity Details") {
                    TextField("Description (Optional)", text: $description, axis: .vertical)
                        .focused($focusedField, equals: .description)
                        .lineLimit(2...4)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .kpi }
                    
                    TextField("KPI (Optional)", text: $kpi)
                        .focused($focusedField, equals: .kpi)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .targetValue }
                }
                
                Section("Metrics") {
                    TextField("Target Value", text: $targetValue)
                        .focused($focusedField, equals: .targetValue)
                        .keyboardType(.decimalPad)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .actualValue }
                    
                    TextField("Actual Value", text: $actualValue)
                        .focused($focusedField, equals: .actualValue)
                        .keyboardType(.decimalPad)
                        .submitLabel(.done)
                        .onSubmit { focusedField = nil }
                    
                    Picker("Status", selection: $statusId) {
                        Text("Completed").tag(Int64(1))
                        Text("In Progress").tag(Int64(2))
                        Text("Pending").tag(Int64(3))
                        Text("Blocked").tag(Int64(4))
                    }
                }
                
                Section("Project") {
                    if isLoadingProjects {
                        HStack {
                            ProgressView()
                                .scaleEffect(0.8)
                            Text("Loading projects...")
                                .foregroundColor(.secondary)
                            Spacer()
                        }
                    } else {
                        Picker("Project (Optional)", selection: $projectId) {
                            Text("No Project").tag(String?.none)
                            ForEach(projects, id: \.id) { project in
                                Text(project.name).tag(String?.some(project.id))
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
            .navigationTitle("Create Activity")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        createActivity()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Creating activity...")
                        .scaleEffect(1.2)
                }
            }
        }
        .onAppear {
            loadProjects()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                focusedField = .description
            }
        }
    }
    
    private func loadProjects() {
        isLoadingProjects = true
        
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await projectHandler.list(
                pagination: PaginationDto(page: 1, perPage: 50),
                include: nil,
                auth: authContext
            )
            
            await MainActor.run {
                isLoadingProjects = false
                switch result {
                case .success(let paginatedResult):
                    projects = paginatedResult.items
                case .failure(let error):
                    print("Failed to load projects: \(error)")
                    projects = []
                }
            }
        }
    }
    
    private func createActivity() {
        print("🚀 [ACTIVITY_CREATE] Starting activity creation...")
        print("🚀 [ACTIVITY_CREATE] Form data - Description: '\(description)', KPI: '\(kpi)', Target: '\(targetValue)', Actual: '\(actualValue)', Status: \(statusId), Project: \(projectId ?? "nil")")
        
        focusedField = nil
        isLoading = true
        errorMessage = nil
        
        guard let currentUser = authManager.currentUser else {
            print("❌ [ACTIVITY_CREATE] User not authenticated")
            errorMessage = "User not authenticated."
            isLoading = false
            return
        }
        
        print("✅ [ACTIVITY_CREATE] User authenticated: \(currentUser.userId), role: \(currentUser.role)")
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        print("🔐 [ACTIVITY_CREATE] Auth context created: user_id=\(authContext.user_id), device_id=\(authContext.device_id)")
        
        let targetVal = Double(targetValue.trimmingCharacters(in: .whitespacesAndNewlines))
        let actualVal = Double(actualValue.trimmingCharacters(in: .whitespacesAndNewlines))
        
        print("📊 [ACTIVITY_CREATE] Parsed values - Target: \(targetVal?.description ?? "nil"), Actual: \(actualVal?.description ?? "nil")")
        
        let newActivity = NewActivity(
            projectId: projectId,
            description: description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : description.trimmingCharacters(in: .whitespacesAndNewlines),
            kpi: kpi.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : kpi.trimmingCharacters(in: .whitespacesAndNewlines),
            targetValue: targetVal,
            actualValue: actualVal,
            statusId: statusId,
            syncPriority: syncPriority,
            createdByUserId: currentUser.userId
        )
        
        print("📝 [ACTIVITY_CREATE] NewActivity object created:")
        print("📝 [ACTIVITY_CREATE]   - projectId: \(newActivity.projectId ?? "nil")")
        print("📝 [ACTIVITY_CREATE]   - description: \(newActivity.description ?? "nil")")
        print("📝 [ACTIVITY_CREATE]   - kpi: \(newActivity.kpi ?? "nil")")
        print("📝 [ACTIVITY_CREATE]   - targetValue: \(newActivity.targetValue?.description ?? "nil")")
        print("📝 [ACTIVITY_CREATE]   - actualValue: \(newActivity.actualValue?.description ?? "nil")")
        print("📝 [ACTIVITY_CREATE]   - statusId: \(newActivity.statusId)")
        print("📝 [ACTIVITY_CREATE]   - syncPriority: \(newActivity.syncPriority)")
        print("📝 [ACTIVITY_CREATE]   - createdByUserId: \(newActivity.createdByUserId)")
        
        Task {
            print("🌐 [ACTIVITY_CREATE] Calling FFI handler...")
            let result = await ffiHandler.create(newActivity: newActivity, auth: authContext)
            print("🌐 [ACTIVITY_CREATE] FFI handler returned")
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let createdActivity):
                    print("✅ [ACTIVITY_CREATE] SUCCESS! Created activity with ID: \(createdActivity.id)")
                    print("✅ [ACTIVITY_CREATE] Activity details:")
                    print("✅ [ACTIVITY_CREATE]   - ID: \(createdActivity.id)")
                    print("✅ [ACTIVITY_CREATE]   - Description: \(createdActivity.description ?? "nil")")
                    print("✅ [ACTIVITY_CREATE]   - KPI: \(createdActivity.kpi ?? "nil")")
                    print("✅ [ACTIVITY_CREATE]   - Created at: \(createdActivity.createdAt)")
                    print("✅ [ACTIVITY_CREATE] Calling onSave callback...")
                    onSave(createdActivity)
                    print("✅ [ACTIVITY_CREATE] onSave callback completed, dismissing sheet...")
                    dismiss()
                    print("✅ [ACTIVITY_CREATE] Sheet dismissed successfully!")
                case .failure(let error):
                    print("❌ [ACTIVITY_CREATE] FAILED! Error: \(error)")
                    print("❌ [ACTIVITY_CREATE] Error type: \(type(of: error))")
                    print("❌ [ACTIVITY_CREATE] Error localized description: \(error.localizedDescription)")
                    errorMessage = "Failed to create activity: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Edit Activity Sheet
struct EditActivitySheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let activity: ActivityResponse
    let onSave: (ActivityResponse) -> Void
    
    @State private var description = ""
    @State private var kpi = ""
    @State private var targetValue = ""
    @State private var actualValue = ""
    @State private var statusId: Int64 = 1
    @State private var projectId: String?
    @State private var syncPriority: SyncPriority = .normal
    
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var projects: [ProjectResponse] = []
    @State private var isLoadingProjects = false
    
    private let ffiHandler = ActivityFFIHandler()
    private let projectHandler = ProjectFFIHandler()
    
    private var canSave: Bool {
        !isLoading
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Activity Details") {
                    TextField("Description (Optional)", text: $description, axis: .vertical)
                        .lineLimit(2...4)
                    
                    TextField("KPI (Optional)", text: $kpi)
                }
                
                Section("Metrics") {
                    TextField("Target Value", text: $targetValue)
                        .keyboardType(.decimalPad)
                    
                    TextField("Actual Value", text: $actualValue)
                        .keyboardType(.decimalPad)
                    
                    Picker("Status", selection: $statusId) {
                        Text("Completed").tag(Int64(1))
                        Text("In Progress").tag(Int64(2))
                        Text("Pending").tag(Int64(3))
                        Text("Blocked").tag(Int64(4))
                    }
                }
                
                Section("Project") {
                    if isLoadingProjects {
                        HStack {
                            ProgressView()
                                .scaleEffect(0.8)
                            Text("Loading projects...")
                                .foregroundColor(.secondary)
                            Spacer()
                        }
                    } else {
                        Picker("Project (Optional)", selection: $projectId) {
                            Text("No Project").tag(String?.none)
                            ForEach(projects, id: \.id) { project in
                                Text(project.name).tag(String?.some(project.id))
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
            .navigationTitle("Edit Activity")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        updateActivity()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Updating activity...")
                        .scaleEffect(1.2)
                }
            }
        }
        .onAppear {
            populateFields()
            loadProjects()
        }
    }
    
    private func populateFields() {
        description = activity.description ?? ""
        kpi = activity.kpi ?? ""
        targetValue = activity.targetValue.map { String(format: "%.0f", $0) } ?? ""
        actualValue = activity.actualValue.map { String(format: "%.0f", $0) } ?? ""
        statusId = activity.statusId ?? 1
        projectId = activity.projectId
        syncPriority = activity.syncPriority
    }
    
    private func loadProjects() {
        isLoadingProjects = true
        
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await projectHandler.list(
                pagination: PaginationDto(page: 1, perPage: 50),
                include: nil,
                auth: authContext
            )
            
            await MainActor.run {
                isLoadingProjects = false
                switch result {
                case .success(let paginatedResult):
                    projects = paginatedResult.items
                case .failure(let error):
                    print("Failed to load projects: \(error)")
                    projects = []
                }
            }
        }
    }
    
    private func updateActivity() {
        isLoading = true
        errorMessage = nil
        
        guard let currentUser = authManager.currentUser else {
            errorMessage = "User not authenticated."
            isLoading = false
            return
        }
        
        let authContext = AuthContextPayload(
            user_id: currentUser.userId,
            role: currentUser.role,
            device_id: authManager.getDeviceId(),
            offline_mode: false
        )
        
        let targetVal = Double(targetValue.trimmingCharacters(in: .whitespacesAndNewlines))
        let actualVal = Double(actualValue.trimmingCharacters(in: .whitespacesAndNewlines))
        
        // Handle double optional for project ID
        let projectIdUpdate: String??
        if projectId != activity.projectId {
            projectIdUpdate = .some(projectId)
        } else {
            projectIdUpdate = .none
        }
        
        let updateActivity = UpdateActivity(
            projectId: projectIdUpdate,
            description: description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : description.trimmingCharacters(in: .whitespacesAndNewlines),
            kpi: kpi.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : kpi.trimmingCharacters(in: .whitespacesAndNewlines),
            targetValue: targetVal,
            actualValue: actualVal,
            statusId: statusId,
            syncPriority: syncPriority,
            updatedByUserId: currentUser.userId
        )
        
        Task {
            let result = await ffiHandler.update(id: activity.id, update: updateActivity, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let updatedActivity):
                    onSave(updatedActivity)
                    dismiss()
                case .failure(let error):
                    errorMessage = "Failed to update activity: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Activity Detail View
struct ActivityDetailView: View {
    @State private var currentActivity: ActivityResponse
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    
    @State private var showUploadSheet = false
    @State private var showEditSheet = false
    @State private var documents: [MediaDocumentResponse] = []
    @State private var isLoadingDocuments = false
    @State private var selectedDocumentURL: IdentifiableURL?
    
    private let ffiHandler = ActivityFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    
    init(activity: ActivityResponse, onUpdate: @escaping () -> Void) {
        _currentActivity = State(initialValue: activity)
        self.onUpdate = onUpdate
    }
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Activity Header
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                if let description = currentActivity.description {
                                    Text(description)
                                        .font(.largeTitle)
                                        .fontWeight(.bold)
                                } else {
                                    Text("No Description")
                                        .font(.largeTitle)
                                        .fontWeight(.bold)
                                        .foregroundColor(.secondary)
                                        .italic()
                                }
                                
                                if let projectName = currentActivity.projectName {
                                    Label(projectName, systemImage: "folder")
                                        .font(.subheadline)
                                        .foregroundColor(.blue)
                                }
                            }
                            Spacer()
                            Badge(text: currentActivity.statusName ?? "Unknown", color: currentActivity.statusColor)
                        }
                        
                        Divider()
                        
                        // Progress Section
                        if let progress = currentActivity.progressPercentage {
                            VStack(alignment: .leading, spacing: 8) {
                                HStack {
                                    Text("Progress")
                                        .font(.headline)
                                    Spacer()
                                    Text(currentActivity.formattedProgress)
                                        .font(.title2)
                                        .fontWeight(.bold)
                                        .foregroundColor(currentActivity.progressColor)
                                }
                                
                                GeometryReader { geometry in
                                    ZStack(alignment: .leading) {
                                        RoundedRectangle(cornerRadius: 8)
                                            .fill(Color(.systemGray5))
                                            .frame(height: 16)
                                        
                                        RoundedRectangle(cornerRadius: 8)
                                            .fill(currentActivity.progressColor)
                                            .frame(width: geometry.size.width * (progress / 100), height: 16)
                                    }
                                }
                                .frame(height: 16)
                            }
                        }
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Activity Details
                    VStack(alignment: .leading, spacing: 16) {
                        if let kpi = currentActivity.kpi {
                            DetailRow(label: "KPI", value: kpi)
                        }
                        
                        if let target = currentActivity.targetValue {
                            DetailRow(label: "Target Value", value: String(format: "%.0f", target))
                        }
                        
                        if let actual = currentActivity.actualValue {
                            DetailRow(label: "Actual Value", value: String(format: "%.0f", actual))
                        }
                        
                        DetailRow(label: "Sync Priority", value: currentActivity.syncPriority.rawValue.capitalized)
                        
                        Divider()
                        
                        DetailRow(label: "Created", value: formatDate(currentActivity.createdAt))
                        DetailRow(label: "Created By", value: currentActivity.createdByUsername ?? "Unknown")
                        DetailRow(label: "Last Updated", value: formatDate(currentActivity.updatedAt))
                        DetailRow(label: "Updated By", value: currentActivity.updatedByUsername ?? "Unknown")
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
            .navigationTitle("Activity Details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Close") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: { showEditSheet = true }) {
                        Image(systemName: "pencil")
                    }
                }
            }
            .sheet(isPresented: $showUploadSheet) {
                DocumentUploadSheet(
                    entity: currentActivity.asDocumentUploadAdapter(),
                    config: .standard,
                    onUploadComplete: {
                        loadDocuments()
                        onUpdate()
                        reloadActivityData()
                    }
                )
            }
            .sheet(isPresented: $showEditSheet) {
                EditActivitySheet(activity: currentActivity, onSave: { updatedActivity in
                    currentActivity = updatedActivity
                    onUpdate()
                    loadDocuments()
                })
                .environmentObject(authManager)
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
            .onAppear {
                loadDocuments()
                reloadActivityData()
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
                relatedTable: "activities",
                relatedId: currentActivity.id,
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
    
    private func reloadActivityData() {
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.get(
                id: currentActivity.id,
                include: [.project, .status, .documents],
                auth: authContext
            )
            
            await MainActor.run {
                switch result {
                case .success(let updatedActivity):
                    currentActivity = updatedActivity
                case .failure(let error):
                    print("Failed to reload activity data: \(error)")
                }
            }
        }
    }
    
    private func openDocument(_ document: MediaDocumentResponse) {
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            let result = await documentHandler.openDocument(id: document.id, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let openResponse):
                    if let filePath = openResponse.filePath {
                        let fileURL: URL
                        if filePath.hasPrefix("file://") {
                            fileURL = URL(string: filePath)!
                        } else {
                            fileURL = URL(fileURLWithPath: filePath)
                        }
                        
                        if FileManager.default.fileExists(atPath: fileURL.path) {
                            self.selectedDocumentURL = IdentifiableURL(url: fileURL)
                        }
                    }
                case .failure(let error):
                    print("Failed to open document: \(error)")
                }
            }
        }
    }
}

// MARK: - Bulk Status Update Sheet
struct BulkStatusUpdateSheet: View {
    let selectedCount: Int
    @State var currentStatus: Int64
    let onUpdate: (Int64) -> Void
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            Form {
                Section {
                    Text("Update status for \(selectedCount) selected activities")
                        .font(.headline)
                }
                
                Section {
                    Picker("New Status", selection: $currentStatus) {
                        Text("Completed").tag(Int64(1))
                        Text("In Progress").tag(Int64(2))
                        Text("Pending").tag(Int64(3))
                        Text("Blocked").tag(Int64(4))
                    }
                    .pickerStyle(WheelPickerStyle())
                }
            }
            .navigationTitle("Update Status")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Update") {
                        onUpdate(currentStatus)
                        dismiss()
                    }
                }
            }
        }
    }
}

// Note: ActivityTableRow is now defined in Core/Components/ActivityTableRow.swift to avoid duplication

// MARK: - Supporting Types





// Export service stub (implement when backend supports it)
class ActivityExportService: DomainExportService {
    var domainName: String { "Activities" }
    var filePrefix: String { "activities" }
    
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        return try await ActivityService.shared.exportActivitiesByIds(
            ids: ids,
            includeBlobs: includeBlobs,
            format: format,
            targetPath: targetPath,
            token: token
        )
    }
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        return try await ActivityService.shared.getExportStatus(jobId: jobId)
    }
}