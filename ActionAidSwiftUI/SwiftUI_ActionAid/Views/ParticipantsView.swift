//
//  ParticipantsView.swift
//  ActionAid SwiftUI
//
//  Participants management with advanced card-based UI and shared components
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI
import QuickLook

// MARK: - Main View
struct ParticipantsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    private let ffiHandler = ParticipantFFIHandler()

    // Core data state
    @State private var participants: [ParticipantResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedFilters: Set<String> = ["all"]
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var isActionBarCollapsed: Bool = false
    
    // Shared component managers
    @StateObject private var selectionManager = SelectionManager()
    @StateObject private var exportManager = ExportManager(service: ParticipantExportService())
    @StateObject private var crudManager = CRUDSheetManager<ParticipantResponse>(config: CRUDSheetConfig(entityName: "Participant"))
    @StateObject private var documentTracker = DocumentCountTracker(config: .participants)
    @StateObject private var backendStatsManager = BackendParticipantStatsManager()
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    
    // Participant detail view state
    @State private var selectedParticipant: ParticipantResponse?
    
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
    
    /// Properly filtered participants matching backend filtering logic
    var filteredParticipants: [ParticipantResponse] {
        participants.filter { participant in
            // Search logic matches backend: name, disability_type, location
            let matchesSearch = searchText.isEmpty ||
                participant.name.localizedCaseInsensitiveContains(searchText) ||
                (participant.location ?? "").localizedCaseInsensitiveContains(searchText) ||
                (participant.disabilityType ?? "").localizedCaseInsensitiveContains(searchText)
            
            // Filter logic matches backend ParticipantFilter implementation
            let matchesFilter: Bool
            if selectedFilters.contains("all") {
                matchesFilter = true
            } else {
                // Debug: Print active filters to verify ID uniqueness
                print("🔍 [DEBUG] Active filters: \(selectedFilters)")
                
                // Check each active filter - using OR logic within categories, AND logic between categories
                var genderMatch = true
                var ageGroupMatch = true
                var disabilityMatch = true
                var disabilityTypeMatch = true
                
                // Gender filtering (OR logic within gender category)
                let mappedGenders = mapGenderFilters(selectedFilters)
                if !mappedGenders.isEmpty {
                    print("🔍 [DEBUG] Mapped genders: \(mappedGenders)")
                    genderMatch = mappedGenders.contains(participant.gender ?? "")
                }
                
                // Age group filtering (OR logic within age group category)
                let mappedAgeGroups = mapAgeGroupFilters(selectedFilters)
                if !mappedAgeGroups.isEmpty {
                    print("🔍 [DEBUG] Mapped age groups: \(mappedAgeGroups)")
                    ageGroupMatch = mappedAgeGroups.contains(participant.ageGroup ?? "")
                }
                
                // Disability filtering (general disability status)
                if let disabilityFlag = getDisabilityFlag(selectedFilters) {
                    print("🔍 [DEBUG] Disability flag: \(disabilityFlag)")
                    disabilityMatch = participant.disability == disabilityFlag
                }
                
                // Disability type filtering (OR logic within disability types)
                let mappedDisabilityTypes = mapDisabilityTypeFilters(selectedFilters)
                if !mappedDisabilityTypes.isEmpty {
                    print("🔍 [DEBUG] Mapped disability types: \(mappedDisabilityTypes)")
                    // If specific disability types are selected, only show participants with those types
                    if let participantDisabilityType = participant.disabilityType {
                        disabilityTypeMatch = mappedDisabilityTypes.contains(participantDisabilityType)
                    } else {
                        disabilityTypeMatch = false // No disability type means doesn't match specific type filter
                    }
                }
                
                matchesFilter = genderMatch && ageGroupMatch && disabilityMatch && disabilityTypeMatch
            }
            
            return matchesSearch && matchesFilter
        }
    }
    
    // MARK: - Setup and Helper Methods
    
    /// Setup callbacks for shared component managers
    private func setupCallbacks() {
        // CRUD manager callbacks
        crudManager.onEntityCreated = { newParticipant in
            loadParticipants()
        }
        crudManager.onEntityUpdated = { updatedParticipant in
            loadParticipants()
        }
        crudManager.onEntityDeleted = { _ in
            loadParticipants()
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
            // Main Entity List with properly configured document tracking
            EntityListView(
                entities: filteredParticipants,
                isLoading: isLoading,
                emptyStateConfig: .participants,
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                filterOptions: [], // Use empty regular filters
                groupedFilterOptions: FilterOption.participantGroupedFilters, // Use new grouped filters
                currentViewStyle: $currentViewStyle,
                onViewStyleChange: { newStyle in
                    currentViewStyle = newStyle
                    viewStyleManager.setViewStyle(newStyle, for: "participants")
                },
                selectionManager: selectionManager,
                onFilterBasedSelectAll: {
                    Task {
                        await getFilteredParticipantIds()
                    }
                },
                onItemTap: { participant in
                    selectedParticipant = participant
                },
                cardContent: { participant in
                    ParticipantCard(participant: participant, documentTracker: documentTracker)
                },
                tableColumns: ParticipantTableConfig.columns,
                rowContent: { participant, columns in
                    ParticipantTableRow(
                        participant: participant,
                        columns: columns,
                        documentCounts: documentTracker.documentCounts
                    )
                },
                domainName: "participants",
                userRole: authManager.currentUser?.role,
                showColumnCustomizer: $showColumnCustomizer
            )
        }
        .navigationTitle("Participants")
        .navigationBarTitleDisplayMode(UIDevice.current.userInterfaceIdiom == .pad ? .large : .inline)
        .navigationBarHidden(isActionBarCollapsed)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                HStack(spacing: 8) {
                    ViewStyleSwitcher(
                        currentViewStyle: currentViewStyle,
                        onViewStyleChange: { newStyle in
                            currentViewStyle = newStyle
                            viewStyleManager.setViewStyle(newStyle, for: "participants")
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
                CreateParticipantSheet(ffiHandler: self.ffiHandler, onSave: { newParticipant in
                    crudManager.completeOperation(.create, result: newParticipant)
                })
            },
            editSheet: { participant in
                EditParticipantSheet(participant: participant, ffiHandler: self.ffiHandler, onSave: { updatedParticipant in
                    crudManager.completeOperation(.edit, result: updatedParticipant)
                })
            },
            onDelete: { participant, hardDelete, force in
                performSingleDelete(participant: participant, hardDelete: hardDelete, force: force)
            }
        )
        .withDocumentCounting(
            entities: participants,
            tracker: documentTracker,
            auth: createAuthContext()
        )
        .fullScreenCover(item: $selectedParticipant) { participant in
            ParticipantDetailView(
                participant: participant,
                onUpdate: {
                    loadParticipants()
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
                DeleteResultsSheet(results: results, entityName: "Participant", entityNamePlural: "Participants")
            }
        }
        .sheet(isPresented: $showExportOptions) {
            GenericExportOptionsSheet(
                selectedItemCount: selectionManager.selectedCount,
                entityName: "Participant",
                entityNamePlural: "Participants",
                onExport: { includeBlobs, format in
                    performExportFromSelection(includeBlobs: includeBlobs, format: format)
                },
                isExporting: $exportManager.isExporting,
                exportError: $exportManager.exportError
            )
        }
        .onAppear {
            setupCallbacks()
            loadParticipants()
            // Load saved view style preference
            currentViewStyle = viewStyleManager.getViewStyle(for: "participants")
        }
        .onChange(of: participants.count) { oldCount, newCount in
            // Fetch backend stats when participants data changes
            if newCount != oldCount {
                Task {
                    await fetchBackendStats()
                }
            }
        }
        .onAppear {
            // Fetch backend stats on first appearance if participants are loaded
            if !participants.isEmpty {
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
            sharedStatsContext.entityName = "Participants"
            
            print("📊 Registered participant stats with shared context: \(backendStatsManager.stats.count) stats")
        }
    }
    
    // MARK: - Core Data Operations
    
    /// Load participants from backend
    private func loadParticipants() {
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
                include: [.all], // Use .all to get workshop and livelihood counts from enriched data
                auth: authContext
            )
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    print("📋 [PARTICIPANT_LIST] Loaded \(paginatedResult.items.count) participants")
                    participants = paginatedResult.items
                case .failure(let error):
                    print("❌ [PARTICIPANT_LIST] Failed to load participants: \(error.localizedDescription)")
                    crudManager.errorMessage = "Failed to load participants: \(error.localizedDescription)"
                    crudManager.showErrorAlert = true
                }
            }
            
            // Fetch backend stats after loading participants
            await fetchBackendStats()
        }
    }
    
    // MARK: - Selection and Bulk Operations
    
    /// Perform single entity delete (called from CRUD manager)
    private func performSingleDelete(participant: ParticipantResponse, hardDelete: Bool, force: Bool) {
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
            
            let result = await ffiHandler.delete(id: participant.id, hardDelete: hardDelete, auth: authContext)
            
            await MainActor.run {
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        crudManager.completeOperation(.delete, result: participant)
                        loadParticipants()
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
    
    /// Get filtered participant IDs for bulk selection based on current UI filters
    private func getFilteredParticipantIds() async {
        // Backend filter operation
        
        guard !selectionManager.isLoadingFilteredIds else { 
            return 
        }
        
        // Check if we have any backend filters active
        let hasBackendFilters = !searchText.isEmpty || !selectedFilters.contains("all")
        
        // If no backend filters are applied, select all visible items
        if !hasBackendFilters {
            await MainActor.run {
                let allVisibleIds = Set(filteredParticipants.map(\.id))
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
        let currentFilter = createFilterFromUI()
        
        let result = await ffiHandler.findIdsByFilter(filter: currentFilter, auth: authContext)
        
        await MainActor.run {
            selectionManager.isLoadingFilteredIds = false
            switch result {
            case .success(let filteredIds):
                // Only select IDs that are currently visible
                let visibleIds = Set(filteredParticipants.map(\.id))
                let filteredVisibleIds = Set(filteredIds).intersection(visibleIds)
                selectionManager.selectAllItems(filteredVisibleIds)
            case .failure(let error):
                crudManager.errorMessage = "Failed to get filtered IDs: \(error.localizedDescription)"
                crudManager.showErrorAlert = true
            }
        }
    }
    
    /// Create ParticipantFilter from current UI state - matches backend structure exactly
    private func createFilterFromUI() -> ParticipantFilter {
        var genders: [String]? = nil
        var ageGroups: [String]? = nil
        var disability: Bool? = nil
        var disabilityTypes: [String]? = nil
        
        if !selectedFilters.contains("all") {
            // Gender filters - map prefixed IDs to backend values
            let mappedGenders = mapGenderFilters(selectedFilters)
            if !mappedGenders.isEmpty {
                genders = mappedGenders
            }
            
            // Age group filters - map prefixed IDs to backend values
            let mappedAgeGroups = mapAgeGroupFilters(selectedFilters)
            if !mappedAgeGroups.isEmpty {
                ageGroups = mappedAgeGroups
            }
            
            // Disability type filters - map prefixed IDs to backend values
            let mappedDisabilityTypes = mapDisabilityTypeFilters(selectedFilters)
            if !mappedDisabilityTypes.isEmpty {
                disabilityTypes = mappedDisabilityTypes
                // Don't set general disability flag when specific types are selected
                // Backend will handle this automatically
            } else {
                // Only apply general disability filter if no specific types are selected
                disability = getDisabilityFlag(selectedFilters)
            }
        }
        
        return ParticipantFilter(
            genders: genders,
            ageGroups: ageGroups,
            locations: nil, // Could be expanded later
            disability: disability,
            disabilityTypes: disabilityTypes,
            searchText: searchText.isEmpty ? nil : searchText,
            dateRange: nil,
            createdByUserIds: nil,
            workshopIds: nil,
            hasDocuments: nil,
            documentLinkedFields: nil,
            excludeDeleted: true
        )
    }
    
    // MARK: - Filter ID Mapping Helpers
    
    /// Map prefixed UI filter IDs to backend values
    private func mapGenderFilters(_ selectedFilters: Set<String>) -> [String] {
        let genderMapping: [String: String] = [
            "gender_male": "male",
            "gender_female": "female", 
            "gender_other": "other",
            "gender_prefer_not_to_say": "prefer_not_to_say"
        ]
        
        return selectedFilters.compactMap { filterId in
            genderMapping[filterId]
        }
    }
    
    private func mapAgeGroupFilters(_ selectedFilters: Set<String>) -> [String] {
        let ageMapping: [String: String] = [
            "age_child": "child",
            "age_youth": "youth",
            "age_adult": "adult", 
            "age_elderly": "elderly"
        ]
        
        return selectedFilters.compactMap { filterId in
            ageMapping[filterId]
        }
    }
    
    private func mapDisabilityTypeFilters(_ selectedFilters: Set<String>) -> [String] {
        let disabilityTypeMapping: [String: String] = [
            "disability_type_visual": "visual",
            "disability_type_hearing": "hearing",
            "disability_type_physical": "physical",
            "disability_type_intellectual": "intellectual", 
            "disability_type_psychosocial": "psychosocial",
            "disability_type_multiple": "multiple",
            "disability_type_other": "other"
        ]
        
        return selectedFilters.compactMap { filterId in
            disabilityTypeMapping[filterId]
        }
    }
    
    private func getDisabilityFlag(_ selectedFilters: Set<String>) -> Bool? {
        let hasWithDisability = selectedFilters.contains("disability_with")
        let hasWithoutDisability = selectedFilters.contains("disability_without")
        
        if hasWithDisability && hasWithoutDisability {
            return nil // Both selected = show all
        } else if hasWithDisability {
            return true
        } else if hasWithoutDisability {
            return false
        }
        return nil
    }
    
    // MARK: - Export Operations
    
    private func performExportFromSelection(includeBlobs: Bool = false, format: ExportFormat = .default) {
        guard !selectionManager.selectedItems.isEmpty else { return }
        
        // Starting export operation
        
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
                    self.selectionManager.clearSelection()
                    self.showExportOptions = false
                },
                onCompletion: { success in
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
            print("🗑️ [BULK_DELETE] Starting individual delete for \(selectedIds.count) participants")
            print("🗑️ [BULK_DELETE] Hard delete: \(hardDelete), Force: \(force)")
            
            // Perform individual deletes since bulk delete FFI is not available
            var deletedCount = 0
            var failedCount = 0
            var failedIds: [String] = []
            var errors: [String: String] = [:]
            
            for participantId in selectedIds {
                let result = await ffiHandler.delete(id: participantId, hardDelete: hardDelete, auth: authContext)
                
                switch result {
                case .success(let deleteResponse):
                    if deleteResponse.wasDeleted {
                        deletedCount += 1
                    } else {
                        failedCount += 1
                        failedIds.append(participantId)
                        errors[participantId] = deleteResponse.displayMessage
                    }
                case .failure(let error):
                    failedCount += 1
                    failedIds.append(participantId)
                    errors[participantId] = error.localizedDescription
                }
            }
            
            // Create a mock BatchDeleteResult for consistent UI handling
            let mockBatchResult = BatchDeleteResult(
                hardDeleted: hardDelete ? Array(selectedIds.prefix(deletedCount)) : [],
                softDeleted: hardDelete ? [] : Array(selectedIds.prefix(deletedCount)),
                failed: failedIds,
                dependencies: [:], // Individual deletes don't return dependency info
                errors: errors
            )
            
            await MainActor.run {
                self.isPerformingBulkDelete = false
                
                print("✅ [BULK_DELETE] Individual delete completed")
                print("✅ [BULK_DELETE] Deleted: \(deletedCount)")
                print("✅ [BULK_DELETE] Failed: \(failedCount)")
                
                // Store results for display
                self.bulkDeleteResults = mockBatchResult
                
                // Clear selection and refresh data
                self.selectionManager.clearSelection()
                
                // Refresh the participants list to reflect changes
                loadParticipants()
                
                // Show results if there were any failures
                if failedCount > 0 {
                    self.showBulkDeleteResults = true
                } else if deletedCount > 0 {
                    // Show a simple success message for successful bulk operations
                    let message = hardDelete ? "Permanently deleted \(deletedCount) participants" : "Archived \(deletedCount) participants"
                    print("✅ [BULK_DELETE] \(message)")
                }
            }
        }
    }
}



// MARK: - Participant Card Component
struct ParticipantCard: View {
    let participant: ParticipantResponse
    let documentTracker: DocumentCountTracker

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(participant.name)
                        .font(.subheadline)
                        .fontWeight(.medium)
                        .foregroundColor(.primary)
                        .lineLimit(1)
                    
                    if let location = participant.location, !location.isEmpty {
                        Label(location, systemImage: "mappin.circle.fill")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                }
                
                Spacer()
                
                HStack(spacing: 4) {
                    if participant.disability {
                        Image(systemName: "figure.roll")
                            .font(.caption)
                            .foregroundColor(.purple)
                    }
                    
                    // Use DocumentCountTracker's documentCounts directly
                    if (documentTracker.documentCounts[participant.id] ?? 0) > 0 {
                        Image(systemName: "paperclip")
                            .font(.caption2)
                            .foregroundColor(.blue)
                    }
                }
            }
            
            // Demographics
            HStack(spacing: 16) {
                if let gender = participant.gender {
                    Label(gender.capitalized, systemImage: "person.fill")
                        .font(.caption)
                        .foregroundColor(.blue)
                }
                
                if let ageGroup = participant.ageGroup {
                    Label(ageGroup.capitalized, systemImage: "calendar")
                        .font(.caption)
                        .foregroundColor(.orange)
                }
            }
            
            // Disability info if applicable
            if participant.disability, let disabilityType = participant.disabilityType {
                Text("Disability: \(disabilityType.capitalized)")
                    .font(.caption)
                    .foregroundColor(.purple)
                    .lineLimit(1)
            }
            
            // Engagement metrics
            HStack(spacing: 0) {
                VStack(spacing: 2) {
                    Text("\(participant.workshopCount ?? 0)")
                        .font(.caption)
                        .fontWeight(.semibold)
                    Text("Workshops")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
                
                VStack(spacing: 2) {
                    Text("\(participant.livelihoodCount ?? 0)")
                        .font(.caption)
                        .fontWeight(.semibold)
                    Text("Livelihoods")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
                
                VStack(spacing: 2) {
                    Text("\(documentTracker.documentCounts[participant.id] ?? 0)")
                        .font(.caption)
                        .fontWeight(.semibold)
                    Text("Documents")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                .frame(maxWidth: .infinity)
            }
            
            // Bottom Info
            HStack {
                Text("Updated: \(formatDate(participant.updatedAt))")
                    .font(.caption2)
                    .foregroundColor(.secondary)
                
                Spacer()
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

// Note: ParticipantTableRow is now defined in Core/Components/ParticipantTableRow.swift to avoid duplication

// MARK: - Create Participant Sheet
struct CreateParticipantSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let ffiHandler: ParticipantFFIHandler
    let onSave: (ParticipantResponse) -> Void
    
    // Form fields
    @State private var name = ""
    @State private var gender: String?
    @State private var ageGroup: String?
    @State private var location = ""
    @State private var hasDisability = false
    @State private var disabilityType: String?
    @State private var syncPriority: SyncPriority = .normal
    
    // State management
    @State private var isLoading = false
    @State private var errorMessage: String?
    @FocusState private var focusedField: Field?
    
    // Duplicate detection state
    @State private var isCheckingDuplicates = false
    @State private var foundDuplicates: [ParticipantDuplicateInfo] = []
    @State private var showDuplicateAlert = false
    @State private var duplicateCheckPerformed = false
    
    enum Field: Hashable {
        case name, location, disabilityType
    }
    
    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Basic Information") {
                    TextField("Name", text: $name)
                        .focused($focusedField, equals: .name)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .location }
                    
                    Picker("Gender", selection: $gender) {
                        Text("Not Specified").tag(String?.none)
                        Text("Male").tag(String?.some("male"))
                        Text("Female").tag(String?.some("female"))
                        Text("Other").tag(String?.some("other"))
                        Text("Prefer Not to Say").tag(String?.some("prefer_not_to_say"))
                    }
                    
                    Picker("Age Group", selection: $ageGroup) {
                        Text("Not Specified").tag(String?.none)
                        Text("Child").tag(String?.some("child"))
                        Text("Youth").tag(String?.some("youth"))
                        Text("Adult").tag(String?.some("adult"))
                        Text("Elderly").tag(String?.some("elderly"))
                    }
                    
                    TextField("Location (Optional)", text: $location)
                        .focused($focusedField, equals: .location)
                        .submitLabel(.next)
                        .onSubmit { 
                            if hasDisability {
                                focusedField = .disabilityType
                            } else {
                                focusedField = nil
                            }
                        }
                }
                
                Section("Disability Information") {
                    Toggle("Has Disability", isOn: $hasDisability)
                    
                    if hasDisability {
                        Picker("Disability Type", selection: $disabilityType) {
                            Text("Not Specified").tag(String?.none)
                            Text("Visual").tag(String?.some("visual"))
                            Text("Hearing").tag(String?.some("hearing"))
                            Text("Physical").tag(String?.some("physical"))
                            Text("Intellectual").tag(String?.some("intellectual"))
                            Text("Psychosocial").tag(String?.some("psychosocial"))
                            Text("Multiple").tag(String?.some("multiple"))
                            Text("Other").tag(String?.some("other"))
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
            .navigationTitle("Create Participant")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { 
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        dismiss() 
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                        checkForDuplicatesAndCreate()
                    } label: {
                        HStack(spacing: 4) {
                            if isCheckingDuplicates {
                                ProgressView()
                                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                                    .scaleEffect(0.8)
                            }
                            Text("Create")
                        }
                    }
                    .disabled(!canSave || isCheckingDuplicates)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Creating participant...")
                        .scaleEffect(1.2)
                } else if isCheckingDuplicates {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Checking for duplicates...")
                        .scaleEffect(1.2)
                }
            }
        }
        .interactiveDismissDisabled(isLoading || isCheckingDuplicates)
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                focusedField = .name
            }
        }
        .sheet(isPresented: $showDuplicateAlert) {
            DuplicateDetectionPopup(
                duplicates: foundDuplicates,
                onContinue: {
                    // User chose to create anyway
                    createParticipant()
                },
                onCancel: {
                    // User chose to cancel
                    foundDuplicates = []
                }
            )
        }
    }
    
    private func checkForDuplicatesAndCreate() {
        // First, check for duplicates
        guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        
        isCheckingDuplicates = true
        errorMessage = nil
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    isCheckingDuplicates = false
                    errorMessage = "User not authenticated."
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.checkDuplicates(name: name.trimmingCharacters(in: .whitespacesAndNewlines), auth: authContext)
            
            await MainActor.run {
                isCheckingDuplicates = false
                
                switch result {
                case .success(let duplicates):
                    if duplicates.isEmpty {
                        // No duplicates found, proceed with creation
                        createParticipant()
                    } else {
                        // Duplicates found, show popup
                        foundDuplicates = duplicates
                        showDuplicateAlert = true
                    }
                case .failure(let error):
                    // If duplicate check fails, proceed with creation anyway
                    print("Duplicate check failed: \(error), proceeding with creation")
                    createParticipant()
                }
            }
        }
    }
    
    private func createParticipant() {
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
        
        let newParticipant = NewParticipant(
            name: name.trimmingCharacters(in: .whitespacesAndNewlines),
            gender: gender,
            disability: hasDisability ? hasDisability : nil,
            disabilityType: hasDisability ? disabilityType : nil,
            ageGroup: ageGroup,
            location: location.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : location.trimmingCharacters(in: .whitespacesAndNewlines),
            createdByUserId: currentUser.userId,
            syncPriority: syncPriority
        )
        
        Task {
            let result = await ffiHandler.create(newParticipant: newParticipant, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let createdParticipant):
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(createdParticipant)
                    dismiss()
                case .failure(let error):
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to create participant: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Edit Participant Sheet
struct EditParticipantSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let participant: ParticipantResponse
    let ffiHandler: ParticipantFFIHandler
    let onSave: (ParticipantResponse) -> Void
    
    @State private var name = ""
    @State private var gender: String?
    @State private var ageGroup: String?
    @State private var location = ""
    @State private var hasDisability = false
    @State private var disabilityType: String?
    @State private var syncPriority: SyncPriority = .normal
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    // Track original values
    @State private var originalName = ""
    @State private var originalGender: String?
    @State private var originalAgeGroup: String?
    @State private var originalLocation = ""
    @State private var originalHasDisability = false
    @State private var originalDisabilityType: String?
    @State private var originalSyncPriority: SyncPriority = .normal
    
    @FocusState private var focusedField: Field?
    
    private enum Field: Hashable {
        case name, location, disabilityType
    }
    
    private var canSave: Bool {
        !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        !isLoading
    }
    
    private var hasChanges: Bool {
        name != originalName ||
        gender != originalGender ||
        ageGroup != originalAgeGroup ||
        location != originalLocation ||
        hasDisability != originalHasDisability ||
        disabilityType != originalDisabilityType ||
        syncPriority != originalSyncPriority
    }
    
    var body: some View {
        NavigationView {
            Form {
                Section("Basic Information") {
                    TextField("Name", text: $name)
                        .focused($focusedField, equals: .name)
                        .textInputAutocapitalization(.words)
                        .submitLabel(.next)
                        .onSubmit { focusedField = .location }
                    
                    Picker("Gender", selection: $gender) {
                        Text("Not Specified").tag(String?.none)
                        Text("Male").tag(String?.some("male"))
                        Text("Female").tag(String?.some("female"))
                        Text("Other").tag(String?.some("other"))
                        Text("Prefer Not to Say").tag(String?.some("prefer_not_to_say"))
                    }
                    
                    Picker("Age Group", selection: $ageGroup) {
                        Text("Not Specified").tag(String?.none)
                        Text("Child").tag(String?.some("child"))
                        Text("Youth").tag(String?.some("youth"))
                        Text("Adult").tag(String?.some("adult"))
                        Text("Elderly").tag(String?.some("elderly"))
                    }
                    
                    TextField("Location (Optional)", text: $location)
                        .focused($focusedField, equals: .location)
                        .submitLabel(.next)
                        .onSubmit { 
                            if hasDisability {
                                focusedField = .disabilityType
                            } else {
                                focusedField = nil
                            }
                        }
                }
                
                Section("Disability Information") {
                    Toggle("Has Disability", isOn: $hasDisability)
                    
                    if hasDisability {
                        Picker("Disability Type", selection: $disabilityType) {
                            Text("Not Specified").tag(String?.none)
                            Text("Visual").tag(String?.some("visual"))
                            Text("Hearing").tag(String?.some("hearing"))
                            Text("Physical").tag(String?.some("physical"))
                            Text("Intellectual").tag(String?.some("intellectual"))
                            Text("Psychosocial").tag(String?.some("psychosocial"))
                            Text("Multiple").tag(String?.some("multiple"))
                            Text("Other").tag(String?.some("other"))
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
            .navigationTitle("Edit Participant")
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
                        updateParticipant()
                    }
                    .disabled(!canSave)
                }
            }
            .disabled(isLoading)
            .overlay {
                if isLoading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    ProgressView("Updating participant...")
                        .scaleEffect(1.2)
                }
            }
        }
        .interactiveDismissDisabled(isLoading)
        .onAppear {
            populateFields()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                focusedField = .name
            }
        }
    }
    
    private func populateFields() {
        name = participant.name
        gender = participant.gender
        ageGroup = participant.ageGroup
        location = participant.location ?? ""
        hasDisability = participant.disability
        disabilityType = participant.disabilityType
        syncPriority = .normal // Default since not in response
        
        // Store original values
        originalName = participant.name
        originalGender = participant.gender
        originalAgeGroup = participant.ageGroup
        originalLocation = participant.location ?? ""
        originalHasDisability = participant.disability
        originalDisabilityType = participant.disabilityType
        originalSyncPriority = .normal
    }
    
    private func updateParticipant() {
        focusedField = nil
        
        if !hasChanges {
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
        
        let updateParticipant = UpdateParticipant(
            name: name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : name.trimmingCharacters(in: .whitespacesAndNewlines),
            gender: gender,
            disability: hasDisability,
            disabilityType: hasDisability ? disabilityType : nil,
            ageGroup: ageGroup,
            location: location.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : location.trimmingCharacters(in: .whitespacesAndNewlines),
            updatedByUserId: currentUser.userId,
            syncPriority: syncPriority
        )
        
        Task {
            let result = await ffiHandler.update(id: participant.id, update: updateParticipant, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let updatedParticipant):
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    onSave(updatedParticipant)
                    dismiss()
                case .failure(let error):
                    UINotificationFeedbackGenerator().notificationOccurred(.error)
                    errorMessage = "Failed to update participant: \(error.localizedDescription)"
                }
            }
        }
    }
}

// MARK: - Participant Detail View
struct ParticipantDetailView: View {
    @State private var currentParticipant: ParticipantResponse
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    private let ffiHandler = ParticipantFFIHandler()
    private let documentHandler = DocumentFFIHandler()
    
    // Document state
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
    
    // Workshop and livelihood data
    @State private var workshops: [WorkshopSummary] = []
    @State private var livelihoods: [LivelihoodSummary] = []
    @State private var isLoadingRelationships = false
    
    // Document refresh mechanism
    @State private var refreshTimer: Timer?
    @State private var lastRefreshTime = Date()
    @State private var hasActiveCompressions = false
    @State private var lastCompressionCount = 0
    
    init(participant: ParticipantResponse, onUpdate: @escaping () -> Void) {
        _currentParticipant = State(initialValue: participant)
        self.onUpdate = onUpdate
    }
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Participant Header
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(currentParticipant.name)
                                    .font(.largeTitle)
                                    .fontWeight(.bold)
                                
                                if let location = currentParticipant.location {
                                    Label(location, systemImage: "mappin.circle.fill")
                                        .font(.subheadline)
                                        .foregroundColor(.secondary)
                                }
                            }
                            Spacer()
                            if currentParticipant.disability {
                                Image(systemName: "figure.roll")
                                    .font(.title2)
                                    .foregroundColor(.purple)
                            }
                        }
                        
                        Divider()
                        
                        // Key Metrics Row
                        HStack(spacing: 0) {
                            VStack(alignment: .center, spacing: 4) {
                                Text("Workshops")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentParticipant.workshopCount ?? 0)")
                                    .font(.headline)
                                    .fontWeight(.semibold)
                                    .foregroundColor(.blue)
                            }
                            .frame(maxWidth: .infinity)
                            
                            VStack(alignment: .center, spacing: 4) {
                                Text("Livelihoods")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentParticipant.livelihoodCount ?? 0)")
                                    .font(.headline)
                                    .fontWeight(.semibold)
                                    .foregroundColor(.green)
                            }
                            .frame(maxWidth: .infinity)
                            
                            VStack(alignment: .center, spacing: 4) {
                                Text("Documents")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                Text("\(currentParticipant.documentCount ?? 0)")
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
                    
                    // Participant Details
                    VStack(alignment: .leading, spacing: 16) {
                        DetailRow(label: "Gender", value: currentParticipant.genderDisplayName)
                        DetailRow(label: "Age Group", value: currentParticipant.ageGroupDisplayName)
                        
                        if currentParticipant.disability {
                            DetailRow(label: "Disability", value: currentParticipant.disabilityDescription)
                        }
                        
                        Divider()
                        
                        DetailRow(label: "Created", value: formatDate(currentParticipant.createdAt))
                        DetailRow(label: "Last Updated", value: formatDate(currentParticipant.updatedAt))
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                    
                    // Workshops Section
                    if !workshops.isEmpty {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Workshops")
                                .font(.headline)
                            
                            ForEach(workshops, id: \.id) { workshop in
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(workshop.name)
                                            .font(.subheadline)
                                            .fontWeight(.medium)
                                        if let date = workshop.date {
                                            Text(date)
                                                .font(.caption)
                                                .foregroundColor(.secondary)
                                        }
                                    }
                                    Spacer()
                                    if workshop.hasCompleted {
                                        Image(systemName: "checkmark.circle.fill")
                                            .foregroundColor(.green)
                                    }
                                }
                                .padding(.vertical, 4)
                            }
                        }
                        .padding()
                        .background(Color(.systemGray6))
                        .cornerRadius(12)
                    }
                    
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
            .navigationTitle("Participant Details")
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
                            if authManager.currentUser?.role.lowercased() == "admin" {
                                showDeleteOptions = true
                            } else {
                                showDeleteConfirmation = true
                            }
                        }) {
                            Label("Delete Participant", systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            .sheet(isPresented: $showUploadSheet) {
                DocumentUploadSheet(
                    entity: currentParticipant.asDocumentUploadAdapter(),
                    config: .standard,
                    onUploadComplete: {
                        print("📤 [PARTICIPANT_DETAIL] Document upload completed, refreshing data...")
                        loadDocuments()
                        onUpdate()
                        reloadParticipantData()
                    }
                )
            }
            .sheet(isPresented: $showEditSheet) {
                EditParticipantSheet(participant: currentParticipant, ffiHandler: ffiHandler) { updatedParticipant in
                    print("💾 [PARTICIPANT_DETAIL] Edit completed")
                    currentParticipant = updatedParticipant
                    // Refresh main list
                    onUpdate()
                    loadDocuments()
                }
                .environmentObject(authManager)
            }
            .sheet(isPresented: $showDeleteOptions) {
                EntityDeleteOptionsSheet(
                    config: DeleteConfiguration(
                        entityName: "Participant",
                        entityNamePlural: "Participants",
                        showForceDelete: true,
                        archiveDescription: "Archive the participant. Data will be preserved and can be restored later.",
                        deleteDescription: "Permanently delete participant if no dependencies exist.",
                        forceDeleteDescription: "⚠️ DANGER: Force delete participant regardless of dependencies. This cannot be undone."
                    ),
                    selectedCount: 1,
                    userRole: authManager.currentUser?.role ?? "",
                    onDelete: { hardDelete, force in
                        deleteParticipant(hardDelete: hardDelete, force: force)
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
            .onAppear {
                print("👁️ [PARTICIPANT_DETAIL] View appeared, loading fresh data...")
                loadDocuments()
                loadRelationships()
                reloadParticipantData()
            }
            .onDisappear {
                stopDocumentRefreshTimer()
            }
            .alert("Delete Participant", isPresented: $showDeleteConfirmation) {
                Button("Cancel", role: .cancel) { }
                Button("Delete", role: .destructive) {
                    deleteParticipant(hardDelete: false, force: false)
                }
            } message: {
                Text("Are you sure you want to delete this participant? They will be archived and can be restored later.")
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
                relatedTable: "participants",
                relatedId: currentParticipant.id,
                pagination: PaginationDto(page: 1, perPage: 50),
                include: [.documentType],
                auth: authContext
            )
            
            await MainActor.run {
                isLoadingDocuments = false
                switch result {
                case .success(let paginatedResult):
                    updateDocumentsPreservingScroll(newDocuments: paginatedResult.items)
                    updateCompressionStatus()
                    updateParticipantDocumentCount(actualCount: paginatedResult.items.count)
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
    
    /// Update the participant's document count to reflect the actual loaded documents
    private func updateParticipantDocumentCount(actualCount: Int) {
        let storedCount = Int(currentParticipant.documentCount ?? 0)
        if storedCount != actualCount {
            print("📊 [PARTICIPANT_DETAIL] Document count mismatch: stored=\(storedCount), actual=\(actualCount)")
            print("📊 [PARTICIPANT_DETAIL] Reloading participant data to get updated counts...")
            reloadParticipantData()
        }
    }
    
    /// Reload participant data from backend to get updated counts and metadata
    private func reloadParticipantData() {
                        // Reloading participant data
        
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.get(
                id: currentParticipant.id,
                include: [.all], // Use .all to get workshop and livelihood counts from enriched data
                auth: authContext
            )
            
            await MainActor.run {
                switch result {
                case .success(let updatedParticipant):
                    print("✅ [PARTICIPANT_DETAIL] Participant data reloaded. Document count: \(updatedParticipant.documentCount ?? 0)")
                    currentParticipant = updatedParticipant
                    
                case .failure(let error):
                    print("❌ [PARTICIPANT_DETAIL] Failed to reload participant data: \(error)")
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
        if documents.isEmpty {
            documents = newDocuments
            return
        }
        
        let documentsChanged = !documentsAreEqual(documents, newDocuments)
        
        if documentsChanged {
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
        let timeSinceLastRefresh = Date().timeIntervalSince(lastRefreshTime)
        guard timeSinceLastRefresh >= 30.0 else { return }
        guard hasActiveCompressions else { return }
        
        lastRefreshTime = Date()
        
        Task {
            guard let currentUser = authManager.currentUser else { return }
            
            let authContext = AuthCtxDto(
                userId: currentUser.userId,
                role: currentUser.role,
                deviceId: authManager.getDeviceId(),
                offlineMode: false
            )
            
            let result = await documentHandler.listDocumentsByEntity(
                relatedTable: "participants",
                relatedId: currentParticipant.id,
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
                }
            }
        }
    }
    
    private func updateCompressionStatus() {
        let processingStatuses = ["pending", "processing", "in_progress"]
        let activeCompressions = documents.filter { doc in
            processingStatuses.contains(doc.compressionStatus.lowercased())
        }
        
        let newHasActiveCompressions = !activeCompressions.isEmpty
        let newCompressionCount = activeCompressions.count
        
        let compressionsFinished = lastCompressionCount > newCompressionCount && lastCompressionCount > 0
        
        if newHasActiveCompressions != hasActiveCompressions {
            hasActiveCompressions = newHasActiveCompressions
            
            if hasActiveCompressions {
                // Documents compressing status
                startDocumentRefreshTimer()
            } else {
                print("✅ [COMPRESSION_STATUS] All compressions completed - stopping refresh timer")
                stopDocumentRefreshTimer()
            }
        } else if compressionsFinished {
            print("⚡ [COMPRESSION_STATUS] \(lastCompressionCount - newCompressionCount) compression(s) just finished")
        }
        
        lastCompressionCount = newCompressionCount
    }
    
    private func startDocumentRefreshTimer() {
        guard refreshTimer == nil && hasActiveCompressions else { return }
        
        print("⏰ [TIMER] Starting document refresh timer (30s interval)")
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30.0, repeats: true) { _ in
            if self.hasActiveCompressions {
                Task { @MainActor in
                    self.smartRefreshDocuments()
                }
            } else {
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
    
    private func loadRelationships() {
        isLoadingRelationships = true
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    isLoadingRelationships = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            // Load workshop data
            let workshopResult = await ffiHandler.getWithWorkshops(id: currentParticipant.id, auth: authContext)
            
            await MainActor.run {
                isLoadingRelationships = false
                switch workshopResult {
                case .success(let participantWithWorkshops):
                    workshops = participantWithWorkshops.workshops
                case .failure(let error):
                    print("Failed to load workshops: \(error)")
                }
            }
        }
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
                        
                        let fileURL: URL
                        if filePath.hasPrefix("file://") {
                            fileURL = URL(string: filePath)!
                        } else {
                            fileURL = URL(fileURLWithPath: filePath)
                        }
                        
                        if FileManager.default.fileExists(atPath: fileURL.path) {
                            print("📖 [DOCUMENT_OPEN] ✅ File exists, opening with QuickLook")
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
    
    private func deleteParticipant(hardDelete: Bool = false, force: Bool = false) {
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
            
            let result = await ffiHandler.delete(id: currentParticipant.id, hardDelete: hardDelete, auth: authContext)
            
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
                    errorMessage = "Failed to delete participant: \(error.localizedDescription)"
                    showErrorAlert = true
                }
            }
        }
    }
}



// MARK: - Helper Extensions
// Note: Display name extensions are defined in Core/Models/ParticipantModels.swift

// MARK: - Participant Export Service
class ParticipantExportService: DomainExportService {
    var domainName: String { "Participants" }
    var filePrefix: String { "participants" }
    
    func exportByIds(
        ids: [String],
        includeBlobs: Bool,
        format: ExportFormat,
        targetPath: String,
        token: String
    ) async throws -> ExportJobResponse {
        // TODO: Implement when backend export functionality is added
        throw NSError(
            domain: "ParticipantExport",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "Export functionality not yet implemented for participants"]
        )
    }
    
    func getExportStatus(jobId: String) async throws -> ExportJobResponse {
        // TODO: Implement when backend export functionality is added  
        throw NSError(
            domain: "ParticipantExport",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "Export status check not yet implemented for participants"]
        )
    }
}


