//
//  ProjectsView.swift
//  ActionAid SwiftUI
//
//  Projects management demonstrating another domain implementation
//

import SwiftUI

// MARK: - Models
struct Project: Identifiable, Codable {
    let id: String
    let code: String
    let name: String
    let description: String?
    let donor_id: String?
    let donor_name: String?
    let budget: Double
    let spent: Double
    let start_date: String
    let end_date: String
    let status: String
    let manager_id: String
    let manager_name: String?
    let created_at: String
    let updated_at: String
    
    var progress: Double {
        let startDate = ISO8601DateFormatter().date(from: start_date) ?? Date()
        let endDate = ISO8601DateFormatter().date(from: end_date) ?? Date()
        let now = Date()
        
        let total = endDate.timeIntervalSince(startDate)
        let elapsed = now.timeIntervalSince(startDate)
        
        // Guard against division by zero and invalid calculations
        guard total > 0 else { return 0 }
        let rawProgress = elapsed / total
        
        // Ensure progress is a valid number and within bounds
        if rawProgress.isNaN || rawProgress.isInfinite {
            return 0
        }
        return min(max(rawProgress, 0), 1)
    }
    
    var budgetUtilization: Double {
        guard budget > 0 else { return 0 }
        let utilization = spent / budget
        
        // Ensure utilization is a valid number
        if utilization.isNaN || utilization.isInfinite {
            return 0
        }
        return max(0, utilization)
    }
    
    var daysRemaining: Int {
        let endDate = ISO8601DateFormatter().date(from: end_date) ?? Date()
        let days = Calendar.current.dateComponents([.day], from: Date(), to: endDate).day ?? 0
        return max(days, 0)
    }
    
    var isOverBudget: Bool {
        spent > budget
    }
    
    var statusColor: Color {
        Theme.Colors.statusColor(for: status)
    }
}

struct ProjectStats {
    let totalProjects: Int
    let activeProjects: Int
    let completedProjects: Int
    let totalBudget: Double
    let totalSpent: Double
    let averageProgress: Double
}

// MARK: - Main View
struct ProjectsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var projects: [Project] = []
    @State private var stats: ProjectStats?
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedStatus = "all"
    @State private var selectedDonor = "all"
    @State private var showCreateSheet = false
    @State private var selectedProject: Project?
    @State private var showProjectDetail = false
    @State private var errorMessage: String?
    @State private var showErrorAlert = false
    @State private var viewMode: ViewMode = .grid
    
    enum ViewMode {
        case grid, list
    }
    
    var uniqueDonors: [String] {
        let donors = projects.compactMap { $0.donor_name }.filter { !$0.isEmpty }
        return Array(Set(donors)).sorted()
    }
    
    var filteredProjects: [Project] {
        projects.filter { project in
            let matchesSearch = searchText.isEmpty ||
                project.name.localizedCaseInsensitiveContains(searchText) ||
                project.code.localizedCaseInsensitiveContains(searchText) ||
                (project.manager_name ?? "").localizedCaseInsensitiveContains(searchText)
            
            let matchesStatus = selectedStatus == "all" || project.status == selectedStatus
            let matchesDonor = selectedDonor == "all" || project.donor_name == selectedDonor
            
            return matchesSearch && matchesStatus && matchesDonor
        }
    }
    
    var body: some View {
        VStack(spacing: 0) {
            // Stats Overview
            if let stats = stats {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: Theme.Spacing.medium) {
                        StatsCard(
                            title: "Total Projects",
                            value: "\(stats.totalProjects)",
                            color: Theme.Colors.projects,
                            icon: Theme.Icons.projects
                        )
                        StatsCard(
                            title: "Active",
                            value: "\(stats.activeProjects)",
                            color: Theme.Colors.statusActive,
                            icon: "play.circle.fill"
                        )
                        StatsCard(
                            title: "Total Budget",
                            value: "$\(Int(stats.totalBudget / 1000))K",
                            color: Theme.Colors.info,
                            icon: Theme.Icons.funding
                        )
                        StatsCard(
                            title: "Spent",
                            value: "$\(Int(stats.totalSpent / 1000))K",
                            color: Theme.Colors.warning,
                            icon: "chart.pie.fill"
                        )
                        StatsCard(
                            title: "Avg Progress",
                            value: "\(Int(stats.averageProgress * 100))%",
                            color: Theme.Colors.strategicGoals,
                            icon: "percent"
                        )
                    }
                    .padding(.horizontal)
                }
                .padding(.vertical)
            }
            
            // Search and Filters
            VStack(spacing: Theme.Spacing.small) {
                // Search Bar
                SearchBar(text: $searchText, placeholder: "Search projects...")
                
                // Filters and View Toggle
                HStack(spacing: Theme.Spacing.small) {
                    FilterMenu(
                        title: "All Status",
                        selection: $selectedStatus,
                        options: [
                            ("all", "All Status"),
                            ("Planning", "Planning"),
                            ("Active", "Active"),
                            ("On Hold", "On Hold"),
                            ("Completed", "Completed"),
                            ("Cancelled", "Cancelled")
                        ]
                    )
                    
                    FilterMenu(
                        title: "All Donors",
                        selection: $selectedDonor,
                        options: [("all", "All Donors")] + uniqueDonors.map { ($0, $0) }
                    )
                    
                    Spacer()
                    
                    // View Mode Toggle
                    Picker("View Mode", selection: $viewMode) {
                        Image(systemName: "square.grid.2x2").tag(ViewMode.grid)
                        Image(systemName: "list.bullet").tag(ViewMode.list)
                    }
                    .pickerStyle(.segmented)
                    .frame(width: 80)
                }
            }
            .padding(.horizontal)
            
            // Projects Content
            if isLoading {
                LoadingView(message: "Loading projects...")
            } else if filteredProjects.isEmpty {
                EmptyStateView(
                    icon: Theme.Icons.projects,
                    title: "No projects found",
                    message: searchText.isEmpty && selectedStatus == "all" && selectedDonor == "all" 
                        ? "Create your first project to get started"
                        : "Try adjusting your filters",
                    actionTitle: "Clear Filters",
                    action: {
                        searchText = ""
                        selectedStatus = "all"
                        selectedDonor = "all"
                    }
                )
            } else {
                ScrollView {
                    if viewMode == .grid {
                        LazyVGrid(columns: Theme.Layout.twoColumnGrid, spacing: Theme.Spacing.medium) {
                            ForEach(filteredProjects) { project in
                                ProjectGridCard(project: project) {
                                    selectedProject = project
                                    showProjectDetail = true
                                }
                            }
                        }
                        .padding()
                    } else {
                        LazyVStack(spacing: Theme.Spacing.small) {
                            ForEach(filteredProjects) { project in
                                ProjectListRow(project: project) {
                                    selectedProject = project
                                    showProjectDetail = true
                                }
                            }
                        }
                        .padding()
                    }
                }
            }
        }
        .navigationTitle("Projects")
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showCreateSheet = true }) {
                    Image(systemName: Theme.Icons.addCircle)
                        .font(.title3)
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateProjectSheet(onSave: {
                loadProjects()
            })
        }
        .sheet(item: $selectedProject) { project in
            ProjectDetailView(project: project, onUpdate: {
                loadProjects()
            })
        }
        .alert("Error", isPresented: $showErrorAlert) {
            Button("OK") { }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
        .onAppear {
            loadProjects()
            loadStats()
        }
    }
    
    // MARK: - Data Loading
    private func loadProjects() {
        isLoading = true
        
        // Mock data - replace with FFI call
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.projects = [
                Project(
                    id: "1",
                    code: "PROJ-2024-001",
                    name: "Rural Water Access Initiative",
                    description: "Providing clean water access to rural communities",
                    donor_id: "1",
                    donor_name: "UNICEF",
                    budget: 250000,
                    spent: 125000,
                    start_date: "2024-01-01T00:00:00Z",
                    end_date: "2024-12-31T23:59:59Z",
                    status: "Active",
                    manager_id: "1",
                    manager_name: "Sarah Johnson",
                    created_at: "2024-01-01T00:00:00Z",
                    updated_at: "2024-01-15T00:00:00Z"
                ),
                Project(
                    id: "2",
                    code: "PROJ-2024-002",
                    name: "Education Enhancement Program",
                    description: "Improving educational facilities and resources",
                    donor_id: "2",
                    donor_name: "World Bank",
                    budget: 500000,
                    spent: 300000,
                    start_date: "2024-02-01T00:00:00Z",
                    end_date: "2025-01-31T23:59:59Z",
                    status: "Active",
                    manager_id: "2",
                    manager_name: "Michael Chen",
                    created_at: "2024-02-01T00:00:00Z",
                    updated_at: "2024-02-15T00:00:00Z"
                ),
                Project(
                    id: "3",
                    code: "PROJ-2023-015",
                    name: "Healthcare Infrastructure",
                    description: "Building and equipping health centers",
                    donor_id: "3",
                    donor_name: "Gates Foundation",
                    budget: 750000,
                    spent: 780000,
                    start_date: "2023-06-01T00:00:00Z",
                    end_date: "2023-12-31T23:59:59Z",
                    status: "Completed",
                    manager_id: "3",
                    manager_name: "Emily Davis",
                    created_at: "2023-06-01T00:00:00Z",
                    updated_at: "2024-01-05T00:00:00Z"
                )
            ]
            self.isLoading = false
        }
    }
    
    private func loadStats() {
        // Calculate from loaded projects - in real app might be a separate FFI call
        let active = projects.filter { $0.status == "Active" }.count
        let completed = projects.filter { $0.status == "Completed" }.count
        let totalBudget = projects.reduce(0) { $0 + $1.budget }
        let totalSpent = projects.reduce(0) { $0 + $1.spent }
        let avgProgress = projects.isEmpty ? 0 : projects.reduce(0) { $0 + $1.progress } / Double(projects.count)
        
        stats = ProjectStats(
            totalProjects: projects.count,
            activeProjects: active,
            completedProjects: completed,
            totalBudget: totalBudget,
            totalSpent: totalSpent,
            averageProgress: avgProgress
        )
    }
}

// MARK: - Project Grid Card
struct ProjectGridCard: View {
    let project: Project
    let onTap: () -> Void
    
    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: Theme.Spacing.small) {
                // Header
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(project.code)
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                        
                        Text(project.name)
                            .font(Theme.Typography.subheadline)
                            .fontWeight(.medium)
                            .foregroundColor(.primary)
                            .lineLimit(2)
                            .multilineTextAlignment(.leading)
                    }
                    
                    Spacer()
                    
                    if project.isOverBudget {
                        Image(systemName: Theme.Icons.warning)
                            .foregroundColor(Theme.Colors.danger)
                            .font(.caption)
                    }
                }
                
                // Status
                Badge(text: project.status, color: project.statusColor)
                
                // Budget Progress
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Budget")
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(Int(project.budgetUtilization * 100))%")
                            .font(Theme.Typography.caption)
                            .fontWeight(.medium)
                            .foregroundColor(project.isOverBudget ? Theme.Colors.danger : .primary)
                    }
                    
                    ProgressBar(
                        value: project.budgetUtilization,
                        color: project.isOverBudget ? Theme.Colors.danger : Theme.Colors.projects,
                        height: 6
                    )
                }
                
                // Timeline Progress
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Timeline")
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(project.daysRemaining) days left")
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                    }
                    
                    ProgressBar(
                        value: project.progress,
                        color: Theme.Colors.info,
                        height: 6
                    )
                }
                
                Divider()
                
                // Footer
                HStack {
                    if let donorName = project.donor_name {
                        Label(donorName, systemImage: Theme.Icons.donors)
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                    Spacer()
                    if let managerName = project.manager_name {
                        Text(managerName)
                            .font(Theme.Typography.caption2)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                }
            }
            .padding()
            .frame(maxWidth: .infinity)
            .background(Theme.Colors.background)
            .cornerRadius(Theme.CornerRadius.large)
            .shadow(
                color: Theme.Shadow.small.color,
                radius: Theme.Shadow.small.radius,
                x: Theme.Shadow.small.x,
                y: Theme.Shadow.small.y
            )
            .overlay(
                RoundedRectangle(cornerRadius: Theme.CornerRadius.large)
                    .stroke(Theme.Colors.gray5, lineWidth: 1)
            )
        }
        .buttonStyle(PlainButtonStyle())
    }
}

// MARK: - Project List Row
struct ProjectListRow: View {
    let project: Project
    let onTap: () -> Void
    
    var body: some View {
        Button(action: onTap) {
            CardContainer {
                HStack(spacing: Theme.Spacing.medium) {
                    // Project Icon
                    Image(systemName: Theme.Icons.projects)
                        .font(.title2)
                        .foregroundColor(Theme.Colors.projects)
                        .frame(width: 44, height: 44)
                        .background(Theme.Colors.projects.opacity(0.1))
                        .cornerRadius(Theme.CornerRadius.medium)
                    
                    // Project Info
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(project.name)
                                    .font(Theme.Typography.subheadline)
                                    .fontWeight(.medium)
                                    .lineLimit(1)
                                
                                Text(project.code)
                                    .font(Theme.Typography.caption)
                                    .foregroundColor(.secondary)
                            }
                            
                            Spacer()
                            
                            Badge(text: project.status, color: project.statusColor)
                        }
                        
                        // Progress Bars
                        HStack(spacing: Theme.Spacing.large) {
                            VStack(alignment: .leading, spacing: 2) {
                                HStack {
                                    Text("Budget: \(Int(project.budgetUtilization * 100))%")
                                        .font(Theme.Typography.caption2)
                                        .foregroundColor(.secondary)
                                    if project.isOverBudget {
                                        Image(systemName: Theme.Icons.warning)
                                            .font(.caption2)
                                            .foregroundColor(Theme.Colors.danger)
                                    }
                                }
                                ProgressBar(
                                    value: project.budgetUtilization,
                                    color: project.isOverBudget ? Theme.Colors.danger : Theme.Colors.projects,
                                    height: 4
                                )
                                .frame(width: 80)
                            }
                            
                            VStack(alignment: .leading, spacing: 2) {
                                Text("Time: \(Int(project.progress * 100))%")
                                    .font(Theme.Typography.caption2)
                                    .foregroundColor(.secondary)
                                ProgressBar(
                                    value: project.progress,
                                    color: Theme.Colors.info,
                                    height: 4
                                )
                                .frame(width: 80)
                            }
                        }
                        
                        // Bottom Info
                        HStack {
                            if let donor = project.donor_name {
                                Text(donor)
                                    .font(Theme.Typography.caption2)
                                    .foregroundColor(.secondary)
                            }
                            
                            Spacer()
                            
                            Text("\(project.daysRemaining) days remaining")
                                .font(Theme.Typography.caption2)
                                .foregroundColor(.secondary)
                        }
                    }
                }
            }
        }
        .buttonStyle(PlainButtonStyle())
    }
}

// MARK: - Create Project Sheet
struct CreateProjectSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let onSave: () -> Void
    
    @State private var code = ""
    @State private var name = ""
    @State private var description = ""
    @State private var donorId = ""
    @State private var budget = ""
    @State private var startDate = Date()
    @State private var endDate = Date().addingTimeInterval(365 * 24 * 60 * 60) // 1 year
    @State private var managerId = ""
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationView {
            Form {
                Section("Project Information") {
                    TextField("Project Code", text: $code)
                        .textInputAutocapitalization(.characters)
                    
                    TextField("Project Name", text: $name)
                    
                    TextField("Description", text: $description, axis: .vertical)
                        .lineLimit(3...6)
                }
                
                Section("Budget & Timeline") {
                    HStack {
                        Text("$")
                        TextField("Budget", text: $budget)
                            .keyboardType(.numberPad)
                    }
                    
                    DatePicker("Start Date", selection: $startDate, displayedComponents: .date)
                    DatePicker("End Date", selection: $endDate, displayedComponents: .date)
                }
                
                Section("Assignment") {
                    Picker("Donor", selection: $donorId) {
                        Text("Select Donor").tag("")
                        Text("UNICEF").tag("1")
                        Text("World Bank").tag("2")
                        Text("Gates Foundation").tag("3")
                    }
                    
                    Picker("Project Manager", selection: $managerId) {
                        Text("Select Manager").tag("")
                        Text("Current User").tag(authManager.currentUser?.userId ?? "")
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
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        createProject()
                    }
                    .disabled(isLoading || code.isEmpty || name.isEmpty)
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
    
    private func createProject() {
        // In real app, call FFI to create project
        onSave()
        dismiss()
    }
}

// MARK: - Project Detail View
struct ProjectDetailView: View {
    let project: Project
    let onUpdate: () -> Void
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: Theme.Spacing.large) {
                    // Header Card
                    CardContainer {
                        VStack(alignment: .leading, spacing: Theme.Spacing.medium) {
                            HStack {
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(project.code)
                                        .font(Theme.Typography.caption)
                                        .foregroundColor(.secondary)
                                    Text(project.name)
                                        .font(Theme.Typography.headline)
                                }
                                Spacer()
                                Badge(text: project.status, color: project.statusColor)
                            }
                            
                            if let description = project.description {
                                Text(description)
                                    .font(Theme.Typography.body)
                                    .foregroundColor(.secondary)
                            }
                        }
                    }
                    
                    // Budget Card
                    CardContainer {
                        VStack(alignment: .leading, spacing: Theme.Spacing.medium) {
                            SectionHeader(title: "Budget Overview")
                            
                            VStack(spacing: Theme.Spacing.small) {
                                DetailRow(
                                    label: "Total Budget",
                                    value: "$\(Int(project.budget).formatted())"
                                )
                                DetailRow(
                                    label: "Spent",
                                    value: "$\(Int(project.spent).formatted())"
                                )
                                HStack {
                                    Text("Remaining")
                                        .font(.subheadline)
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Text("$\(Int(project.budget - project.spent).formatted())")
                                        .font(.subheadline)
                                        .fontWeight(.medium)
                                        .foregroundColor(project.isOverBudget ? Theme.Colors.danger : Theme.Colors.success)
                                }
                            }
                            
                            ProgressBar(
                                value: project.budgetUtilization,
                                color: project.isOverBudget ? Theme.Colors.danger : Theme.Colors.projects,
                                showPercentage: true
                            )
                            
                            if project.isOverBudget {
                                HStack {
                                    Image(systemName: Theme.Icons.warning)
                                    Text("Project is over budget")
                                }
                                .font(Theme.Typography.caption)
                                .foregroundColor(Theme.Colors.danger)
                            }
                        }
                    }
                    
                    // Timeline Card
                    CardContainer {
                        VStack(alignment: .leading, spacing: Theme.Spacing.medium) {
                            SectionHeader(title: "Timeline")
                            
                            VStack(spacing: Theme.Spacing.small) {
                                DetailRow(
                                    label: "Start Date",
                                    value: formatDate(project.start_date)
                                )
                                DetailRow(
                                    label: "End Date",
                                    value: formatDate(project.end_date)
                                )
                                DetailRow(
                                    label: "Days Remaining",
                                    value: "\(project.daysRemaining) days"
                                )
                            }
                            
                            ProgressBar(
                                value: project.progress,
                                color: Theme.Colors.info,
                                showPercentage: true
                            )
                        }
                    }
                    
                    // Details Card
                    CardContainer {
                        VStack(alignment: .leading, spacing: Theme.Spacing.medium) {
                            SectionHeader(title: "Project Details")
                            
                            VStack(spacing: Theme.Spacing.small) {
                                if let donor = project.donor_name {
                                    DetailRow(label: "Donor", value: donor)
                                }
                                if let manager = project.manager_name {
                                    DetailRow(label: "Project Manager", value: manager)
                                }
                                DetailRow(label: "Created", value: formatDate(project.created_at))
                                DetailRow(label: "Last Updated", value: formatDate(project.updated_at))
                            }
                        }
                    }
                    
                    // Quick Actions
                    VStack(spacing: Theme.Spacing.small) {
                        ActionButton(
                            title: "View Activities",
                            icon: Theme.Icons.activities,
                            action: { },
                            style: .primary
                        )
                        .frame(maxWidth: .infinity)
                        
                        ActionButton(
                            title: "Upload Documents",
                            icon: Theme.Icons.document,
                            action: { },
                            style: .secondary
                        )
                        .frame(maxWidth: .infinity)
                        
                        ActionButton(
                            title: "Financial Report",
                            icon: "chart.pie",
                            action: { },
                            style: .secondary
                        )
                        .frame(maxWidth: .infinity)
                    }
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
                        Button(action: {}) {
                            Label("Edit", systemImage: Theme.Icons.edit)
                        }
                        Button(action: {}) {
                            Label("Export", systemImage: Theme.Icons.share)
                        }
                        Button(role: .destructive, action: {}) {
                            Label("Delete", systemImage: Theme.Icons.delete)
                        }
                    } label: {
                        Image(systemName: Theme.Icons.moreCircle)
                    }
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
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}