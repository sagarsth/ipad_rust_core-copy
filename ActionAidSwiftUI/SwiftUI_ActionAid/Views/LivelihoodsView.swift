//
//  LivelihoodsView.swift
//  ActionAid SwiftUI
//
//  Livelihoods management matching TypeScript UI
//

import SwiftUI

// MARK: - Models
struct Livelihood: Identifiable, Codable {
    let id: String
    let participant_name: String
    let project_name: String
    let business_type: String
    let grant_amount: Double
    let start_date: String
    let status: String
    let outcome_documented: Bool
    let subsequent_grants: Int
    let created_at: String
    
    var statusColor: Color {
        switch status {
        case "Active": return .green
        case "Completed": return .blue
        case "Pending": return .yellow
        case "Cancelled": return .red
        default: return .gray
        }
    }
}

struct LivelihoodStats: Codable {
    let total_livelihoods: Int
    let active_programs: Int
    let total_grants_disbursed: Double
    let success_rate: Double
    let avg_grant_amount: Double
    let participants_with_outcomes: Int
}

// MARK: - Main View
struct LivelihoodsView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var livelihoods: [Livelihood] = []
    @State private var stats: LivelihoodStats?
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedStatus = "all"
    @State private var selectedProject = "all"
    @State private var selectedTab = "overview"
    @State private var showCreateSheet = false
    @State private var errorMessage: String?
    @State private var showErrorAlert = false
    
    var uniqueProjects: [String] {
        Array(Set(livelihoods.map { $0.project_name })).sorted()
    }
    
    var filteredLivelihoods: [Livelihood] {
        livelihoods.filter { livelihood in
            let matchesSearch = searchText.isEmpty ||
                livelihood.participant_name.localizedCaseInsensitiveContains(searchText) ||
                livelihood.business_type.localizedCaseInsensitiveContains(searchText)
            
            let matchesStatus = selectedStatus == "all" || livelihood.status == selectedStatus
            let matchesProject = selectedProject == "all" || livelihood.project_name == selectedProject
            
            return matchesSearch && matchesStatus && matchesProject
        }
    }
    
    var body: some View {
        VStack(spacing: 0) {
            // Tabs
            HStack(spacing: 0) {
                TabButton(title: "Overview", value: "overview", selection: $selectedTab)
                TabButton(title: "Programs", value: "programs", selection: $selectedTab)
                TabButton(title: "Analytics", value: "analytics", selection: $selectedTab)
                TabButton(title: "Outcomes", value: "outcomes", selection: $selectedTab)
            }
            .padding(.horizontal)
            .padding(.top)
            
            // Content
            switch selectedTab {
            case "overview":
                overviewTab
            case "programs":
                programsTab
            case "analytics":
                analyticsTab
            case "outcomes":
                outcomesTab
            default:
                Text("Invalid tab")
            }
        }
        .navigationTitle("Livelihood Programs")
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showCreateSheet = true }) {
                    Image(systemName: "plus.circle.fill")
                        .font(.title3)
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateLivelihoodSheet(onSave: {
                loadLivelihoods()
            })
        }
        .alert("Error", isPresented: $showErrorAlert) {
            Button("OK") { }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
        .onAppear {
            loadLivelihoods()
            loadStats()
        }
    }
    
    // MARK: - Overview Tab
    @ViewBuilder
    private var overviewTab: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Stats Cards
                if let stats = stats {
                    LazyVGrid(columns: [
                        GridItem(.flexible()),
                        GridItem(.flexible()),
                        GridItem(.flexible())
                    ], spacing: 16) {
                        StatsCard(
                            title: "Total Programs",
                            value: "\(stats.total_livelihoods)",
                            color: .purple,
                            icon: "heart.fill"
                        )
                        StatsCard(
                            title: "Active Programs",
                            value: "\(stats.active_programs)",
                            color: .green,
                            icon: "arrow.up.right.circle.fill"
                        )
                        StatsCard(
                            title: "Total Disbursed",
                            value: "$\(Int(stats.total_grants_disbursed).formatted())",
                            color: .blue,
                            icon: "dollarsign.circle.fill"
                        )
                        StatsCard(
                            title: "Success Rate",
                            value: "\(Int(stats.success_rate))%",
                            color: .orange,
                            icon: "trophy.fill"
                        )
                        StatsCard(
                            title: "Avg Grant",
                            value: "$\(Int(stats.avg_grant_amount).formatted())",
                            color: .indigo,
                            icon: "chart.bar.fill"
                        )
                        StatsCard(
                            title: "With Outcomes",
                            value: "\(stats.participants_with_outcomes)",
                            color: .pink,
                            icon: "doc.text.fill"
                        )
                    }
                }
                
                // Quick Actions
                VStack(alignment: .leading, spacing: 12) {
                    Text("Quick Actions")
                        .font(.headline)
                        .padding(.horizontal)
                    
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 12) {
                            QuickActionCard(
                                icon: "plus.circle.fill",
                                title: "New Program",
                                subtitle: "Create livelihood program",
                                color: .green
                            )
                            QuickActionCard(
                                icon: "doc.text.fill",
                                title: "Document Outcomes",
                                subtitle: "Add outcome documentation",
                                color: .blue
                            )
                            QuickActionCard(
                                icon: "chart.bar.xaxis",
                                title: "View Analytics",
                                subtitle: "Program performance metrics",
                                color: .purple
                            )
                            QuickActionCard(
                                icon: "dollarsign.circle.fill",
                                title: "Subsequent Grants",
                                subtitle: "Manage follow-up funding",
                                color: .orange
                            )
                        }
                        .padding(.horizontal)
                    }
                }
            }
            .padding(.vertical)
        }
    }
    
    // MARK: - Programs Tab
    @ViewBuilder
    private var programsTab: some View {
        VStack(spacing: 12) {
            // Filters
            VStack(spacing: 12) {
                // Search Bar
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.secondary)
                    TextField("Search by participant or business type...", text: $searchText)
                    if !searchText.isEmpty {
                        Button(action: { searchText = "" }) {
                            Image(systemName: "xmark.circle.fill")
                                .foregroundColor(.secondary)
                        }
                    }
                }
                .padding(10)
                .background(Color(.systemGray6))
                .cornerRadius(8)
                
                HStack(spacing: 12) {
                    // Status Filter
                    Menu {
                        Button("All Status") { selectedStatus = "all" }
                        Button("Active") { selectedStatus = "Active" }
                        Button("Completed") { selectedStatus = "Completed" }
                        Button("Pending") { selectedStatus = "Pending" }
                        Button("Cancelled") { selectedStatus = "Cancelled" }
                    } label: {
                        HStack {
                            Text(selectedStatus == "all" ? "All Status" : selectedStatus)
                                .font(.subheadline)
                            Image(systemName: "chevron.down")
                                .font(.caption)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Color(.systemGray6))
                        .cornerRadius(8)
                    }
                    
                    // Project Filter
                    Menu {
                        Button("All Projects") { selectedProject = "all" }
                        ForEach(uniqueProjects, id: \.self) { project in
                            Button(project) { selectedProject = project }
                        }
                    } label: {
                        HStack {
                            Text(selectedProject == "all" ? "All Projects" : selectedProject)
                                .font(.subheadline)
                                .lineLimit(1)
                            Image(systemName: "chevron.down")
                                .font(.caption)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Color(.systemGray6))
                        .cornerRadius(8)
                    }
                    
                    Spacer()
                }
            }
            .padding(.horizontal)
            .padding(.top)
            
            // Programs List
            if isLoading {
                Spacer()
                ProgressView("Loading programs...")
                Spacer()
            } else if filteredLivelihoods.isEmpty {
                Spacer()
                VStack(spacing: 16) {
                    Image(systemName: "heart.slash")
                        .font(.system(size: 60))
                        .foregroundColor(.secondary)
                    Text("No programs found")
                        .font(.headline)
                        .foregroundColor(.secondary)
                }
                Spacer()
            } else {
                ScrollView {
                    LazyVStack(spacing: 12) {
                        ForEach(filteredLivelihoods) { livelihood in
                            LivelihoodCard(livelihood: livelihood)
                        }
                    }
                    .padding(.horizontal)
                    .padding(.bottom)
                }
            }
        }
    }
    
    // MARK: - Analytics Tab
    @ViewBuilder
    private var analyticsTab: some View {
        ScrollView {
            VStack(spacing: 20) {
                // Program Status Distribution
                VStack(alignment: .leading, spacing: 16) {
                    Text("Program Status Distribution")
                        .font(.headline)
                    
                    VStack(spacing: 12) {
                        AnalyticsRow(
                            label: "Active Programs",
                            value: "38%",
                            color: .green
                        )
                        AnalyticsRow(
                            label: "Completed Programs",
                            value: "45%",
                            color: .blue
                        )
                        AnalyticsRow(
                            label: "Pending Programs",
                            value: "12%",
                            color: .yellow
                        )
                        AnalyticsRow(
                            label: "Cancelled Programs",
                            value: "5%",
                            color: .red
                        )
                    }
                }
                .padding()
                .background(Color(.systemGray6))
                .cornerRadius(12)
                
                // Outcome Documentation
                VStack(alignment: .leading, spacing: 16) {
                    Text("Outcome Documentation")
                        .font(.headline)
                    
                    VStack(spacing: 12) {
                        AnalyticsRow(
                            label: "With Outcomes",
                            value: "75%",
                            color: .green
                        )
                        AnalyticsRow(
                            label: "Pending Documentation",
                            value: "25%",
                            color: .orange
                        )
                    }
                }
                .padding()
                .background(Color(.systemGray6))
                .cornerRadius(12)
            }
            .padding()
        }
    }
    
    // MARK: - Outcomes Tab
    @ViewBuilder
    private var outcomesTab: some View {
        ScrollView {
            LazyVStack(spacing: 12) {
                ForEach(livelihoods.filter { $0.status == "Completed" }) { livelihood in
                    OutcomeCard(livelihood: livelihood)
                }
            }
            .padding()
        }
    }
    
    // MARK: - Data Loading
    private func loadLivelihoods() {
        isLoading = true
        
        // Mock data for now - in real app, call FFI
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            self.livelihoods = [
                Livelihood(
                    id: "1",
                    participant_name: "Sarah Johnson",
                    project_name: "Rural Empowerment Initiative",
                    business_type: "Small Retail Shop",
                    grant_amount: 5000,
                    start_date: "2024-01-15",
                    status: "Active",
                    outcome_documented: false,
                    subsequent_grants: 0,
                    created_at: "2024-01-15T10:00:00Z"
                ),
                Livelihood(
                    id: "2",
                    participant_name: "Michael Chen",
                    project_name: "Urban Skills Development",
                    business_type: "Food Processing",
                    grant_amount: 7500,
                    start_date: "2023-11-20",
                    status: "Completed",
                    outcome_documented: true,
                    subsequent_grants: 1,
                    created_at: "2023-11-20T09:00:00Z"
                ),
                Livelihood(
                    id: "3",
                    participant_name: "Fatima Al-Rashid",
                    project_name: "Women's Economic Empowerment",
                    business_type: "Tailoring Services",
                    grant_amount: 3500,
                    start_date: "2024-02-01",
                    status: "Active",
                    outcome_documented: false,
                    subsequent_grants: 0,
                    created_at: "2024-02-01T08:00:00Z"
                )
            ]
            self.isLoading = false
        }
    }
    
    private func loadStats() {
        // Mock stats - in real app, calculate from data or call FFI
        stats = LivelihoodStats(
            total_livelihoods: 89,
            active_programs: 34,
            total_grants_disbursed: 456000,
            success_rate: 78.5,
            avg_grant_amount: 6200,
            participants_with_outcomes: 67
        )
    }
}

// MARK: - Components
struct TabButton: View {
    let title: String
    let value: String
    @Binding var selection: String
    
    var isSelected: Bool {
        selection == value
    }
    
    var body: some View {
        Button(action: { selection = value }) {
            Text(title)
                .font(.subheadline)
                .fontWeight(isSelected ? .semibold : .regular)
                .foregroundColor(isSelected ? .blue : .secondary)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity)
                .overlay(alignment: .bottom) {
                    if isSelected {
                        Rectangle()
                            .fill(Color.blue)
                            .frame(height: 2)
                    }
                }
        }
    }
}

struct QuickActionCard: View {
    let icon: String
    let title: String
    let subtitle: String
    let color: Color
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(color)
            
            Text(title)
                .font(.subheadline)
                .fontWeight(.medium)
            
            Text(subtitle)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .frame(width: 140)
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
}

struct LivelihoodCard: View {
    let livelihood: Livelihood
    
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(livelihood.participant_name)
                        .font(.headline)
                    
                    Text(livelihood.business_type)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
                
                Badge(text: livelihood.status, color: livelihood.statusColor)
            }
            
            // Project and Grant Info
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(livelihood.project_name)
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    HStack(spacing: 4) {
                        Text("Grant:")
                        Text("$\(Int(livelihood.grant_amount).formatted())")
                            .fontWeight(.medium)
                        if livelihood.subsequent_grants > 0 {
                            Text("+\(livelihood.subsequent_grants) subsequent")
                                .font(.caption2)
                                .foregroundColor(.blue)
                        }
                    }
                    .font(.caption)
                }
                
                Spacer()
                
                VStack(alignment: .trailing, spacing: 4) {
                    Text("Started: \(formatDate(livelihood.start_date))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    Badge(
                        text: livelihood.outcome_documented ? "Documented" : "Pending",
                        color: livelihood.outcome_documented ? .green : .orange
                    )
                }
            }
            
            // Actions
            HStack(spacing: 12) {
                Button(action: {}) {
                    Label("Edit", systemImage: "pencil")
                        .font(.caption)
                }
                .buttonStyle(.bordered)
                
                Button(action: {}) {
                    Label("Documents", systemImage: "doc.text")
                        .font(.caption)
                }
                .buttonStyle(.bordered)
                
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
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        
        if let date = formatter.date(from: dateString) {
            formatter.dateStyle = .medium
            return formatter.string(from: date)
        }
        return dateString
    }
}

struct AnalyticsRow: View {
    let label: String
    let value: String
    let color: Color
    
    var body: some View {
        HStack {
            Text(label)
                .font(.subheadline)
            Spacer()
            Text(value)
                .font(.headline)
                .foregroundColor(color)
        }
    }
}

struct OutcomeCard: View {
    let livelihood: Livelihood
    
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(livelihood.participant_name)
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                Text(livelihood.business_type)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
            
            HStack(spacing: 12) {
                Badge(
                    text: livelihood.outcome_documented ? "Documented" : "Pending",
                    color: livelihood.outcome_documented ? .green : .orange
                )
                
                Button(livelihood.outcome_documented ? "View Outcomes" : "Add Outcomes") {
                    // Handle action
                }
                .font(.caption)
                .buttonStyle(.bordered)
            }
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(8)
    }
}

// MARK: - Create Livelihood Sheet
struct CreateLivelihoodSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let onSave: () -> Void
    
    @State private var participantName = ""
    @State private var projectName = ""
    @State private var businessType = ""
    @State private var grantAmount = ""
    @State private var startDate = Date()
    @State private var description = ""
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationView {
            Form {
                Section("Participant Information") {
                    TextField("Participant Name", text: $participantName)
                    
                    Picker("Project", selection: $projectName) {
                        Text("Select Project").tag("")
                        Text("Rural Empowerment Initiative").tag("Rural Empowerment Initiative")
                        Text("Urban Skills Development").tag("Urban Skills Development")
                        Text("Women's Economic Empowerment").tag("Women's Economic Empowerment")
                        Text("Agricultural Development").tag("Agricultural Development")
                    }
                }
                
                Section("Business Details") {
                    TextField("Business Type", text: $businessType)
                    
                    HStack {
                        Text("$")
                        TextField("Grant Amount", text: $grantAmount)
                            .keyboardType(.numberPad)
                    }
                    
                    DatePicker("Start Date", selection: $startDate, displayedComponents: .date)
                }
                
                Section("Description") {
                    TextEditor(text: $description)
                        .frame(minHeight: 100)
                }
                
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("Create Livelihood Program")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        createLivelihood()
                    }
                    .disabled(isLoading || participantName.isEmpty || projectName.isEmpty)
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
    
    private func createLivelihood() {
        // In real app, call FFI to create livelihood
        onSave()
        dismiss()
    }
}