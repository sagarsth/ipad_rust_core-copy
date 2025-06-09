//
//  UsersListView.swift
//  ActionAid SwiftUI
//
//  Users management matching TypeScript UI
//

import SwiftUI

// Using UserResponse from UserModels.swift instead of local User struct

// MARK: - Extensions
extension UserResponse: Identifiable {
    var displayName: String {
        name.isEmpty ? email.components(separatedBy: "@").first ?? "Unknown User" : name
    }
    
    var roleColor: Color {
        switch role.lowercased() {
        case "admin": return .red
        case "manager", "lead": return .blue
        case "user", "officer": return .green
        case "viewer": return .gray
        default: return .gray
        }
    }
    
    // Simulate last_login for compatibility (UserResponse doesn't have this field)
    var last_login: String? {
        return updated_at // Use updated_at as a proxy for last login
    }
}

// MARK: - Main View
struct UsersListView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var users: [UserResponse] = []
    @State private var isLoading = false
    @State private var searchText = ""
    @State private var selectedRole = "all"
    @State private var selectedStatus = "all"
    @State private var showCreateSheet = false
    @State private var errorMessage: String?
    @State private var showErrorAlert = false
    
    // Stats
    @State private var totalUsers = 0
    @State private var activeUsers = 0
    @State private var adminCount = 0
    @State private var inactiveUsers = 0
    
    var filteredUsers: [UserResponse] {
        users.filter { user in
            let matchesSearch = searchText.isEmpty ||
                user.email.localizedCaseInsensitiveContains(searchText) ||
                (user.name ?? "").localizedCaseInsensitiveContains(searchText)
            
            let matchesRole = selectedRole == "all" || user.role.lowercased() == selectedRole.lowercased()
            
            let matchesStatus = selectedStatus == "all" ||
                (selectedStatus == "active" && user.active) ||
                (selectedStatus == "inactive" && !user.active)
            
            return matchesSearch && matchesRole && matchesStatus
        }
    }
    
    var body: some View {
        VStack(spacing: 0) {
            // Stats Cards
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 16) {
                    StatsCard(title: "Total Users", value: "\(totalUsers)", color: .blue, icon: "person.2.fill")
                    StatsCard(title: "Active Users", value: "\(activeUsers)", color: .green, icon: "checkmark.circle.fill")
                    StatsCard(title: "Admins", value: "\(adminCount)", color: .red, icon: "shield.fill")
                    StatsCard(title: "Inactive", value: "\(inactiveUsers)", color: .gray, icon: "person.crop.circle.badge.xmark")
                }
                .padding(.horizontal)
            }
            .padding(.vertical)
            
            // Filters
            VStack(spacing: 12) {
                // Search Bar
                HStack {
                    Image(systemName: "magnifyingglass")
                        .foregroundColor(.secondary)
                    TextField("Search by name or email...", text: $searchText)
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
                
                // Role and Status Filters
                HStack(spacing: 12) {
                    // Role Filter
                    Menu {
                        Button("All Roles") { selectedRole = "all" }
                        Button("Admin") { selectedRole = "admin" }
                        Button("Manager") { selectedRole = "lead" }
                        Button("User") { selectedRole = "officer" }
                        Button("Viewer") { selectedRole = "viewer" }
                    } label: {
                        HStack {
                            Text(selectedRole == "all" ? "All Roles" : selectedRole.capitalized)
                                .font(.subheadline)
                            Image(systemName: "chevron.down")
                                .font(.caption)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(Color(.systemGray6))
                        .cornerRadius(8)
                    }
                    
                    // Status Filter
                    Menu {
                        Button("All Status") { selectedStatus = "all" }
                        Button("Active") { selectedStatus = "active" }
                        Button("Inactive") { selectedStatus = "inactive" }
                    } label: {
                        HStack {
                            Text(selectedStatus == "all" ? "All Status" : selectedStatus.capitalized)
                                .font(.subheadline)
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
            
            // Users List
            if isLoading {
                Spacer()
                ProgressView("Loading users...")
                Spacer()
            } else if filteredUsers.isEmpty {
                Spacer()
                VStack(spacing: 16) {
                    Image(systemName: "person.crop.circle.badge.questionmark")
                        .font(.system(size: 60))
                        .foregroundColor(.secondary)
                    Text("No users found")
                        .font(.headline)
                        .foregroundColor(.secondary)
                    if !searchText.isEmpty || selectedRole != "all" || selectedStatus != "all" {
                        Button("Clear Filters") {
                            searchText = ""
                            selectedRole = "all"
                            selectedStatus = "all"
                        }
                        .font(.caption)
                    }
                }
                Spacer()
            } else {
                ScrollView {
                    LazyVStack(spacing: 12) {
                        ForEach(filteredUsers) { user in
                            UserRow(user: user, onToggleStatus: {
                                toggleUserStatus(user)
                            }, onDelete: {
                                deleteUser(user)
                            })
                        }
                    }
                    .padding(.horizontal)
                    .padding(.bottom)
                }
            }
        }
        .navigationTitle("User Management")
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            ToolbarItem(placement: .navigationBarTrailing) {
                Button(action: { showCreateSheet = true }) {
                    Image(systemName: "person.badge.plus")
                        .font(.title3)
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateUserSheet(onSave: {
                loadUsers()
            })
        }
        .alert("Error", isPresented: $showErrorAlert) {
            Button("OK") { }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
        .onAppear {
            loadUsers()
        }
    }
    
    private func loadUsers() {
        isLoading = true
        
        Task {
            do {
                guard let currentUser = authManager.currentUser else {
                    throw NSError(domain: "AuthError", code: 401, userInfo: [NSLocalizedDescriptionKey: "Not authenticated"])
                }
                
                let authHandler = AuthFFIHandler()
                let result = try await authHandler.getAllUsers(token: currentUser.token).get()
                
                await MainActor.run {
                    self.users = result
                    updateStats()
                    isLoading = false
                }
            } catch {
                await MainActor.run {
                    isLoading = false
                    self.errorMessage = error.localizedDescription
                    self.showErrorAlert = true
                }
            }
        }
    }
    
    private func updateStats() {
        totalUsers = users.count
        activeUsers = users.filter { $0.active }.count
        adminCount = users.filter { $0.role.lowercased() == "admin" }.count
        inactiveUsers = users.filter { !$0.active }.count
    }
    
    private func toggleUserStatus(_ user: UserResponse) {
        // In real app, call FFI to update user status
        if let index = users.firstIndex(where: { $0.id == user.id }) {
            var updatedUser = user
            // This would need proper Codable implementation for mutability
            // For now, just reload
            loadUsers()
        }
    }
    
    private func deleteUser(_ user: UserResponse) {
        // In real app, call FFI to delete user
        loadUsers()
    }
}

// MARK: - User Row Component
struct UserRow: View {
    let user: UserResponse
    let onToggleStatus: () -> Void
    let onDelete: () -> Void
    @State private var showActions = false
    
    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                // User Avatar
                Circle()
                    .fill(user.roleColor.opacity(0.2))
                    .overlay(
                        Text(user.displayName.prefix(1).uppercased())
                            .font(.headline)
                            .foregroundColor(user.roleColor)
                    )
                    .frame(width: 44, height: 44)
                
                // User Info
                VStack(alignment: .leading, spacing: 4) {
                    Text(user.displayName)
                        .font(.subheadline)
                        .fontWeight(.medium)
                    
                    Text(user.email)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Spacer()
                
                // Role Badge
                Badge(text: user.role.capitalized, color: user.roleColor)
                
                // Status Badge
                Badge(
                    text: user.active ? "Active" : "Inactive",
                    color: user.active ? .green : .gray
                )
                
                // Actions
                Menu {
                    Button(action: {}) {
                        Label("Edit", systemImage: "pencil")
                    }
                    
                    Button(action: onToggleStatus) {
                        Label(
                            user.active ? "Deactivate" : "Activate",
                            systemImage: user.active ? "person.crop.circle.badge.xmark" : "person.crop.circle.badge.checkmark"
                        )
                    }
                    
                    Button(role: .destructive, action: onDelete) {
                        Label("Delete", systemImage: "trash")
                    }
                } label: {
                    Image(systemName: "ellipsis")
                        .font(.body)
                        .foregroundColor(.secondary)
                        .frame(width: 30, height: 30)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 12)
            
            // Additional Info
            HStack {
                Label(formatDate(user.created_at), systemImage: "calendar")
                    .font(.caption2)
                    .foregroundColor(.secondary)
                
                Spacer()
                
                if let lastLogin = user.last_login {
                    Label("Last login: \(formatDate(lastLogin))", systemImage: "clock")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                } else {
                    Text("Never logged in")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }
            .padding(.horizontal)
            .padding(.bottom, 12)
        }
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
            displayFormatter.dateStyle = .short
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}

// MARK: - Create User Sheet
struct CreateUserSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let onSave: () -> Void
    
    @State private var email = ""
    @State private var name = ""
    @State private var password = ""
    @State private var role = "officer"
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    var body: some View {
        NavigationView {
            Form {
                Section("User Information") {
                    TextField("Email", text: $email)
                        .keyboardType(.emailAddress)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    
                    TextField("Full Name", text: $name)
                        .textInputAutocapitalization(.words)
                    
                    SecureField("Password", text: $password)
                }
                
                Section("Role") {
                    Picker("Role", selection: $role) {
                        Text("Admin").tag("admin")
                        Text("Manager").tag("lead")
                        Text("User").tag("officer")
                        Text("Viewer").tag("viewer")
                    }
                    .pickerStyle(.segmented)
                }
                
                if let error = errorMessage {
                    Section {
                        Text(error)
                            .foregroundColor(.red)
                            .font(.caption)
                    }
                }
            }
            .navigationTitle("Create User")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Create") {
                        createUser()
                    }
                    .disabled(isLoading || email.isEmpty || password.isEmpty)
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
    
    private func createUser() {
        isLoading = true
        errorMessage = nil
        
        let payload = """
        {
            "user": {
                "email": "\(email)",
                "name": "\(name)",
                "password": "\(password)",
                "role": "\(role)"
            },
            "auth": \(authManager.getAuthContextJSON())
        }
        """
        
        Task {
            var result: UnsafeMutablePointer<CChar>?
            let status = user_create(payload, &result)
            
            await MainActor.run {
                isLoading = false
                
                if status == 0, let resultStr = result {
                    defer { user_free(resultStr) }
                    onSave()
                    dismiss()
                } else {
                    if let errorPtr = get_last_error() {
                        let error = String(cString: errorPtr)
                        free_string(errorPtr)
                        errorMessage = error
                    } else {
                        errorMessage = "Failed to create user"
                    }
                }
            }
        }
    }
}