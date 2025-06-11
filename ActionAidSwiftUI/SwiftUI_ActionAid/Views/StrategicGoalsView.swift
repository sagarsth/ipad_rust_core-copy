//
//  StrategicGoalsView.swift
//  ActionAid SwiftUI
//
//  Strategic Goals management with real UI
//

import SwiftUI
import UniformTypeIdentifiers

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
    
    // Stats
    @State private var totalGoals = 0
    @State private var onTrackGoals = 0
    @State private var atRiskGoals = 0
    @State private var completedGoals = 0
    
    // Computed property to determine if we should hide the top section
    private var shouldHideTopSection: Bool {
        scrollOffset > 100
    }
    
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
    
    var body: some View {
        GeometryReader { geometry in
            mainScrollView
        }
        .navigationTitle("Strategic Goals")
        .navigationBarTitleDisplayMode(shouldHideTopSection ? .inline : .large)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showCreateSheet = true }) {
                    Image(systemName: "plus.circle.fill")
                        .font(.title3)
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateGoalSheet(onSave: { newGoal in
                loadGoals()
            })
        }
        .sheet(item: $selectedGoal) { goal in
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
                GoalCard(goal: goal)
            },
            tableColumns: Self.tableColumns,
            rowContent: { goal, columns in
                StrategicGoalTableRow(goal: goal, columns: columns)
            },
            domainName: "strategic_goals",
            userRole: authManager.currentUser?.role,
            isInSelectionMode: $isInSelectionMode,
            selectedItems: $selectedItems
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

            let result = await ffiHandler.list(pagination: PaginationDto(page: 1, perPage: 100), include: nil, auth: authContext)
            
            await MainActor.run {
                isLoading = false
                switch result {
                case .success(let paginatedResult):
                    self.goals = paginatedResult.items
                    updateStats()
                case .failure(let error):
                    self.errorMessage = "Failed to load goals: \(error.localizedDescription)"
                    self.showErrorAlert = true
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
}

// MARK: - Goal Card Component
struct GoalCard: View {
    let goal: StrategicGoalResponse
    
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
                        Text(goal.objectiveCode)
                            .font(.caption)
                            .fontWeight(.medium)
                            .foregroundColor(.secondary)
                        
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
    @State private var documents: [GoalDocument] = []
    @State private var showUploadSheet = false
    @State private var showDeleteConfirmation = false
    @State private var isDeleting = false
    
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
                        DetailRow(label: "Created", value: formatDate(goal.createdAt))
                        DetailRow(label: "Last Updated", value: formatDate(goal.updatedAt))
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
                        
                        if documents.isEmpty {
                            Text("No documents uploaded")
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 20)
                        } else {
                            ForEach(documents) { doc in
                                DocumentRow(document: doc)
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
                        Button(role: .destructive, action: { showDeleteConfirmation = true }) {
                            Label("Delete", systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                }
            }
            .sheet(isPresented: $showUploadSheet) {
                DocumentUploadSheet(goalId: goal.id)
            }
            .alert("Delete Goal", isPresented: $showDeleteConfirmation) {
                Button("Cancel", role: .cancel) { }
                Button("Delete", role: .destructive) {
                    deleteGoal()
                }
            } message: {
                Text("Are you sure you want to delete this strategic goal? This action cannot be undone.")
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
    
    private func deleteGoal() {
        isDeleting = true
        
        Task {
            guard let currentUser = authManager.currentUser else {
                // Handle not authenticated
                isDeleting = false
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            let result = await ffiHandler.delete(id: goal.id, hardDelete: true, auth: authContext)

            await MainActor.run {
                isDeleting = false
                switch result {
                case .success:
                    onUpdate()
                    dismiss()
                case .failure:
                    // Show an error to the user
                    break
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


// MARK: - Document Models & Views (To be refactored or kept as is)
struct GoalDocument: Identifiable {
    let id: String
    let filename: String
    let documentTypeId: String
    let documentTypeName: String
    let linkedField: String?
    let fileSize: Int64
    let uploadDate: Date
    let compressionStatus: String?
}

// MARK: - Document Row
struct DocumentRow: View {
    let document: GoalDocument
    
    var body: some View {
        HStack {
            Image(systemName: fileIcon(for: document.filename))
                .font(.title3)
                .foregroundColor(.blue)
                .frame(width: 40)
            
            VStack(alignment: .leading, spacing: 2) {
                Text(document.filename)
                    .font(.subheadline)
                    .lineLimit(1)
                
                HStack(spacing: 8) {
                    Text(document.documentTypeName)
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if let field = document.linkedField {
                        Text("• Linked to \(field)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                    
                    Text("• \(formatFileSize(document.fileSize))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
            
            Spacer()
            
            if let status = document.compressionStatus {
                CompressionBadge(status: status)
            }
        }
        .padding(.vertical, 8)
    }
    
    private func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png": return "photo.fill"
        case "xls", "xlsx": return "tablecells.fill"
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
    @Environment(\.dismiss) var dismiss
    @State private var selectedDocumentType = ""
    @State private var documentTitle = ""
    @State private var linkedField = ""
    @State private var priority = "Normal"
    
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
    
    var body: some View {
        NavigationView {
            Form {
                Section("Document Information") {
                    TextField("Document Title", text: $documentTitle)
                    
                    Picker("Document Type", selection: $selectedDocumentType) {
                        Text("Select Type").tag("")
                        Text("Strategic Plan").tag("strategic_plan")
                        Text("Progress Report").tag("progress_report")
                        Text("Evidence").tag("evidence")
                        Text("Impact Assessment").tag("impact_assessment")
                        Text("Theory of Change").tag("theory_of_change")
                        Text("Baseline Data").tag("baseline_data")
                        Text("Supporting Documentation").tag("supporting_documentation")
                    }
                    
                    Picker("Link to Field", selection: $linkedField) {
                        ForEach(linkableFields, id: \.0) { field in
                            Text(field.1).tag(field.0)
                        }
                    }
                    
                    Picker("Priority", selection: $priority) {
                        Text("Low").tag("Low")
                        Text("Normal").tag("Normal")
                        Text("High").tag("High")
                    }
                }
                
                Section("Upload") {
                    Button(action: selectDocument) {
                        Label("Select Document", systemImage: "doc.badge.plus")
                    }
                }
                
                Section {
                    Text("Documents can be linked to specific fields for better organization and validation.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            .navigationTitle("Upload Document")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Upload") {
                        uploadDocument()
                    }
                    .disabled(documentTitle.isEmpty || selectedDocumentType.isEmpty)
                }
            }
        }
    }
    
    private func selectDocument() {
        // TODO: Implement document picker
        print("Select document for goal: \(goalId)")
    }
    
    private func uploadDocument() {
        // TODO: Implement document upload
        print("Upload document - Title: \(documentTitle), Type: \(selectedDocumentType), Field: \(linkedField), Priority: \(priority)")
        dismiss()
    }
}