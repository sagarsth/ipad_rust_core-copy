//
//  DashboardView.swift
//  ActionAid SwiftUI
//
//  Main dashboard matching the TypeScript UI design
//

import SwiftUI
import Foundation

// MARK: - Domain Model
struct Domain: Identifiable {
    let id: String
    let name: String
    let description: String
    let icon: String
    let color: Color
    let stats: DomainStats
    let destination: DomainDestination
}

struct DomainStats {
    let total: Int
    let active: Int
    let recent: Int
    let trend: TrendType
}

enum TrendType {
    case up, down, stable
    
    var symbol: String {
        switch self {
        case .up: return "↗"
        case .down: return "↘"
        case .stable: return "→"
        }
    }
    
    var color: Color {
        switch self {
        case .up: return .green
        case .down: return .red
        case .stable: return .gray
        }
    }
}

enum DomainDestination {
    case users
    case livelihoods
    case strategicGoals
    case projects
    case activities
    case workshops
    case participants
    case donors
    case funding
}

// MARK: - Navigation State Manager
@MainActor
class NavigationStateManager: ObservableObject {
    @Published var isInEntityView = false
    @Published var currentEntityName: String?
    
    func enterEntityView(_ entityName: String) {
        isInEntityView = true
        currentEntityName = entityName
    }
    
    func exitEntityView() {
        isInEntityView = false
        currentEntityName = nil
    }
    
    func forceExitEntityView() {
        isInEntityView = false
        currentEntityName = nil
    }
}

// MARK: - Main Tab View
struct MainTabView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @StateObject private var navigationState = NavigationStateManager()
    @StateObject private var sharedStatsContext = SharedStatsContext()
    @State private var selectedTab = 0
    
    var body: some View {
        TabView(selection: $selectedTab) {
            NavigationStack {
                DashboardView()
                    .onAppear {
                        // Clear entity view state when returning to dashboard
                        navigationState.forceExitEntityView()
                        sharedStatsContext.clearStats()
                    }
            }
            .tabItem {
                Image(systemName: "square.grid.2x2")
                Text("Dashboard")
            }
            .tag(0)
            
            NavigationStack {
                if navigationState.isInEntityView {
                    StatsTabView(sharedContext: sharedStatsContext)
                } else {
                    ProfileView()
                }
            }
            .tabItem {
                Image(systemName: navigationState.isInEntityView ? "chart.bar.fill" : "person.circle")
                Text(navigationState.isInEntityView ? "Stats" : "Profile")
            }
            .tag(1)
        }
        .environmentObject(navigationState)
        .environmentObject(sharedStatsContext)
    }
}

// MARK: - Dashboard View
struct DashboardView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var domains: [Domain] = []
    @State private var showingStats = false
    @State private var refreshTimer: Timer?
    
    // Filter domains based on user role - hide funding and donors for non-admin users
    private var filteredDomains: [Domain] {
        guard let currentUser = authManager.currentUser else { return domains }
        
        if currentUser.role.lowercased() != "admin" {
            // Hide funding and donor domains for non-admin users
            return domains.filter { domain in
                domain.id != "funding" && domain.id != "donors"
            }
        }
        
        return domains
    }
    
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                // Header
                VStack(alignment: .leading, spacing: 8) {
                    Text("Manage your organization's data across all domains")
                        .font(.body)
                        .foregroundColor(.secondary)
                }
                .padding(.horizontal)
                .padding(.top)
                
                // Domain Cards Grid
                LazyVGrid(columns: [
                    GridItem(.flexible()),
                    GridItem(.flexible()),
                    GridItem(.flexible())
                ], spacing: 20) {
                    ForEach(filteredDomains) { domain in
                        DomainCard(domain: domain, showingStats: showingStats)
                    }
                }
                .padding(.horizontal)
                .animation(.easeInOut(duration: 0.3), value: showingStats)
                
                // Spacer to ensure proper spacing between domain cards and bottom cards
                Rectangle()
                    .fill(Color.clear)
                    .frame(height: 20)
                
                // Bottom Cards
                HStack(spacing: 20) {
                    QuickActionsCard()
                    SystemStatusCard()
                }
                .padding(.horizontal)
                .padding(.bottom)
            }
        }
        .background(Color(.systemGray6))
        .navigationTitle("System Dashboard")
        .navigationBarTitleDisplayMode(.large)
        .onAppear {
            loadDomains()
            startStatsAnimation()
        }
        .onDisappear {
            refreshTimer?.invalidate()
        }
    }
    
    private func loadDomains() {
        // Load real stats from the backend if authentication is available
        Task {
            await loadRealStats()
        }
        
        domains = [
            Domain(
                id: "users",
                name: "Users",
                description: "User management and authentication",
                icon: "person.2",
                color: .blue,
                stats: DomainStats(total: 3, active: 3, recent: 0, trend: .stable),
                destination: .users
            ),
            Domain(
                id: "livelihoods",
                name: "Livelihoods",
                description: "Livelihood programs and grants",
                icon: "heart",
                color: .green,
                stats: DomainStats(total: 89, active: 76, recent: 8, trend: .up),
                destination: .livelihoods
            ),
            Domain(
                id: "strategic_goals",
                name: "Strategic Goals",
                description: "Strategic planning and objectives",
                icon: "target",
                color: .purple,
                stats: DomainStats(total: 24, active: 18, recent: 3, trend: .stable),
                destination: .strategicGoals
            ),
            Domain(
                id: "projects",
                name: "Projects",
                description: "Project management and tracking",
                icon: "folder",
                color: .orange,
                stats: DomainStats(total: 45, active: 32, recent: 5, trend: .up),
                destination: .projects
            ),
            Domain(
                id: "activities",
                name: "Activities",
                description: "Project activities and milestones",
                icon: "chart.line.uptrend.xyaxis",
                color: .red,
                stats: DomainStats(total: 234, active: 198, recent: 28, trend: .up),
                destination: .activities
            ),
            Domain(
                id: "workshops",
                name: "Workshops",
                description: "Training workshops and events",
                icon: "graduationcap",
                color: .indigo,
                stats: DomainStats(total: 67, active: 12, recent: 4, trend: .down),
                destination: .workshops
            ),
            Domain(
                id: "participants",
                name: "Participants",
                description: "Program participants and beneficiaries",
                icon: "person.crop.circle.badge.checkmark",
                color: .teal,
                stats: DomainStats(total: 1247, active: 1156, recent: 89, trend: .up),
                destination: .participants
            ),
            Domain(
                id: "donors",
                name: "Donors",
                description: "Donor management and relationships",
                icon: "building.2",
                color: .yellow,
                stats: DomainStats(total: 34, active: 28, recent: 2, trend: .stable),
                destination: .donors
            ),
            Domain(
                id: "funding",
                name: "Funding",
                description: "Financial tracking and funding sources",
                icon: "dollarsign.circle",
                color: Color(.systemGreen),
                stats: DomainStats(total: 78, active: 45, recent: 6, trend: .up),
                destination: .funding
            )
        ]
    }
    
    private func startStatsAnimation() {
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { _ in
            withAnimation(.easeInOut(duration: 0.3)) {
                showingStats = true
            }
            
            // Update stats with random variations
            domains = domains.map { domain in
                let variance = Int.random(in: -5...5)
                var updatedStats = domain.stats
                let newRecent = max(0, domain.stats.recent + variance)
                let newTrend: TrendType = variance > 0 ? .up : (variance < 0 ? .down : .stable)
                
                updatedStats = DomainStats(
                    total: domain.stats.total,
                    active: domain.stats.active,
                    recent: newRecent,
                    trend: newTrend
                )
                
                return Domain(
                    id: domain.id,
                    name: domain.name,
                    description: domain.description,
                    icon: domain.icon,
                    color: domain.color,
                    stats: updatedStats,
                    destination: domain.destination
                )
            }
            
            // Hide stats after 2 seconds
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                withAnimation(.easeInOut(duration: 0.3)) {
                    showingStats = false
                }
            }
        }
    }
    
    private func loadRealStats() async {
        guard let currentUser = authManager.currentUser else { return }
        
        do {
            // Fixed: Use UserFFIHandler for consistency with user management
            let userHandler = UserFFIHandler()
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            let users = try await userHandler.getAllUsers(auth: authContext).get()
            
            await MainActor.run {
                // Update users domain with real stats
                if let userIndex = domains.firstIndex(where: { $0.id == "users" }) {
                    domains[userIndex] = Domain(
                        id: "users",
                        name: "Users",
                        description: "User management and authentication",
                        icon: "person.2",
                        color: .blue,
                        stats: DomainStats(
                            total: users.count,
                            active: users.filter { $0.active }.count,
                            recent: 0,
                            trend: .stable
                        ),
                        destination: .users
                    )
                }
            }
        } catch {
            print("Failed to load real stats: \(error)")
        }
    }
}

// MARK: - Domain Card Component
struct DomainCard: View {
    let domain: Domain
    let showingStats: Bool
    @EnvironmentObject var navigationState: NavigationStateManager
    
    var body: some View {
        NavigationLink(destination: destinationView) {
            VStack(alignment: .leading, spacing: 0) {
                // Colored top bar
                Rectangle()
                    .fill(domain.color)
                    .frame(height: 4)
                
                VStack(alignment: .leading, spacing: 12) {
                    // Header
                    HStack {
                        Image(systemName: domain.icon)
                            .font(.title2)
                            .foregroundColor(.white)
                            .frame(width: 40, height: 40)
                            .background(domain.color)
                            .cornerRadius(8)
                        
                        Spacer()
                        
                        Badge(text: "\(domain.stats.active) active", color: .secondary)
                    }
                    
                    // Title and Description
                    VStack(alignment: .leading, spacing: 4) {
                        Text(domain.name)
                            .font(.headline)
                            .foregroundColor(.primary)
                        
                        Text(domain.description)
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .lineLimit(2)
                    }
                    
                    // Stats
                    VStack(spacing: 8) {
                        HStack {
                            Text("Total Records")
                                .font(.caption)
                                .foregroundColor(.secondary)
                            Spacer()
                            Text("\(domain.stats.total)")
                                .font(.headline)
                                .foregroundColor(.primary)
                        }
                        
                        // Recent Activity Section (with consistent height)
                        VStack {
                            if showingStats {
                                HStack {
                                    Text("Recent Activity")
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    HStack(spacing: 4) {
                                        Text("\(domain.stats.recent)")
                                            .font(.subheadline)
                                            .fontWeight(.medium)
                                        Text(domain.stats.trend.symbol)
                                            .font(.caption)
                                            .foregroundColor(domain.stats.trend.color)
                                    }
                                }
                                .padding(8)
                                .background(Color(.systemGray5))
                                .cornerRadius(6)
                                .transition(.opacity.combined(with: .scale(scale: 0.9)))
                            } else {
                                // Invisible placeholder to maintain consistent height
                                HStack {
                                    Text("Recent Activity")
                                        .font(.caption)
                                        .foregroundColor(.clear)
                                    Spacer()
                                    Text("0")
                                        .font(.subheadline)
                                        .foregroundColor(.clear)
                                }
                                .padding(8)
                                .background(Color.clear)
                                .cornerRadius(6)
                            }
                        }
                    }
                }
                .padding()
            }
            .background(Color(.systemBackground))
            .cornerRadius(12)
            .shadow(color: Color.black.opacity(0.05), radius: 5, x: 0, y: 2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color(.systemGray5), lineWidth: 1)
            )
            .scaleEffect(1.0)
            .animation(.easeInOut(duration: 0.3), value: showingStats)
        }
        .buttonStyle(PlainButtonStyle())
    }
    
    @ViewBuilder
    private var destinationView: some View {
        switch domain.destination {
        case .users:
            EntityViewWrapper(entityName: "Users") {
                UsersListView()
            }
        case .strategicGoals:
            EntityViewWrapper(entityName: "Strategic Goals") {
                StrategicGoalsView()
            }
        case .livelihoods:
            EntityViewWrapper(entityName: "Livelihoods") {
                LivelihoodsView()
            }
        case .projects:
            EntityViewWrapper(entityName: "Projects") {
                ProjectsView()
            }
        default:
            ComingSoonView(domainName: domain.name)
        }
    }
}

// MARK: - Entity View Wrapper
struct EntityViewWrapper<Content: View>: View {
    let entityName: String
    let content: () -> Content
    
    @EnvironmentObject var navigationState: NavigationStateManager
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    
    init(entityName: String, @ViewBuilder content: @escaping () -> Content) {
        self.entityName = entityName
        self.content = content
    }
    
    var body: some View {
        content()
            .onAppear {
                navigationState.enterEntityView(entityName)
            }
            // Remove onDisappear entirely to prevent clearing state on tab switches
    }
}

// MARK: - Quick Actions Card
struct QuickActionsCard: View {
    @EnvironmentObject var authManager: AuthenticationManager
    
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Quick Actions")
                    .font(.headline)
                Text("Common tasks and operations")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            VStack(spacing: 12) {
                // Only show create user button for admin users
                if authManager.currentUser?.role.lowercased() == "admin" {
                    QuickActionButton(icon: "person.badge.plus", title: "Create New User", color: .blue)
                }
                QuickActionButton(icon: "heart.fill", title: "Add Livelihood Program", color: .green)
                QuickActionButton(icon: "calendar.badge.plus", title: "Schedule Workshop", color: .orange)
                QuickActionButton(icon: "square.and.arrow.up", title: "Export Data", color: .purple)
            }
        }
        .padding()
        .frame(maxWidth: .infinity)
        .background(Color(.systemBackground))
        .cornerRadius(12)
        .shadow(color: Color.black.opacity(0.05), radius: 5, x: 0, y: 2)
    }
}

// MARK: - System Status Card
struct SystemStatusCard: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("System Status")
                    .font(.headline)
                Text("Current system health and metrics")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            
            VStack(spacing: 12) {
                StatusRow(label: "Database Status", value: "Online", badge: .green)
                StatusRow(label: "Sync Status", value: "Syncing", badge: .blue)
                StatusRow(label: "Document Storage", value: "78% Used", badge: .yellow)
                StatusRow(label: "Active Users", value: "24", badge: nil)
            }
        }
        .padding()
        .frame(maxWidth: .infinity)
        .background(Color(.systemBackground))
        .cornerRadius(12)
        .shadow(color: Color.black.opacity(0.05), radius: 5, x: 0, y: 2)
    }
}

// MARK: - Helper Components
// Badge struct removed due to duplicate definition in StrategicGoalsView.swift

struct QuickActionButton: View {
    let icon: String
    let title: String
    let color: Color
    
    var body: some View {
        Button(action: {}) {
            HStack {
                Image(systemName: icon)
                    .font(.body)
                    .foregroundColor(color)
                Text(title)
                    .font(.subheadline)
                    .foregroundColor(.primary)
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color(.systemGray6))
            .cornerRadius(8)
        }
    }
}

struct StatusRow: View {
    let label: String
    let value: String
    let badge: Color?
    
    var body: some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
            Spacer()
            if let badgeColor = badge {
                Badge(text: value, color: badgeColor)
            } else {
                Text(value)
                    .font(.subheadline)
                    .fontWeight(.medium)
            }
        }
    }
}

// MARK: - Profile View
struct ProfileView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    
    var body: some View {
        VStack(spacing: 24) {
            // User Info
            VStack(spacing: 16) {
                Image(systemName: "person.circle.fill")
                    .font(.system(size: 80))
                    .foregroundColor(.blue)
                
                VStack(spacing: 4) {
                    Text(authManager.currentUser?.email ?? "")
                        .font(.headline)
                    Badge(text: authManager.currentUser?.role ?? "", color: .blue)
                }
            }
            .padding(.top, 40)
            
            // User Details
            VStack(alignment: .leading, spacing: 16) {
                DetailRow(label: "User ID", value: String(authManager.currentUser?.userId.prefix(8) ?? "") + "...")
                DetailRow(label: "Role", value: authManager.currentUser?.role ?? "")
                DetailRow(label: "Device ID", value: String(UIDevice.current.identifierForVendor?.uuidString.prefix(8) ?? "") + "...")
                DetailRow(label: "Login Time", value: formatDate(authManager.currentUser?.loginTime))
            }
            .padding()
            .background(Color(.systemGray6))
            .cornerRadius(12)
            .padding(.horizontal)
            
            Spacer()
            
            // Logout Button
            Button(action: {
                authManager.logout()
            }) {
                Text("Sign Out")
                    .fontWeight(.semibold)
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(Color.red)
                    .cornerRadius(10)
            }
            .padding(.horizontal)
            .padding(.bottom)
        }
        .navigationTitle("Profile")
    }
    
    private func formatDate(_ date: Date?) -> String {
        guard let date = date else { return "Unknown" }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}

// DetailRow struct removed due to duplicate definition in StrategicGoalsView.swift

// MARK: - Coming Soon View
struct ComingSoonView: View {
    let domainName: String
    
    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "hammer.fill")
                .font(.system(size: 60))
                .foregroundColor(.orange)
            
            Text("\(domainName) Management")
                .font(.title)
                .fontWeight(.bold)
            
            Text("This section is under development")
                .font(.headline)
                .foregroundColor(.secondary)
        }
        .navigationTitle(domainName)
        .navigationBarTitleDisplayMode(.inline)
    }
}