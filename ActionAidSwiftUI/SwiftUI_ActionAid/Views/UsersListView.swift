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
        case "field_tl": return .blue
        case "field": return .green
        default: return .gray
        }
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
    
    // Confirmation dialog states
    @State private var showDeactivateConfirmation = false
    @State private var showDeleteConfirmation = false
    @State private var userToToggle: UserResponse?
    @State private var userToDelete: UserResponse?
    
    // Success message state
    @State private var successMessage: String?
    @State private var showSuccessToast = false
    
    // View Details and Edit states
    @State private var selectedUserForDetails: UserResponse?
    @State private var selectedUserForEdit: UserResponse?
    
    var filteredUsers: [UserResponse] {
        users.filter { user in
            let matchesSearch = searchText.isEmpty ||
                user.email.localizedCaseInsensitiveContains(searchText) ||
                user.name.localizedCaseInsensitiveContains(searchText)
            
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
                    UserStatsCard(title: "Total Users", value: "\(totalUsers)", color: .blue, icon: "person.2.fill")
                    UserStatsCard(title: "Active Users", value: "\(activeUsers)", color: .green, icon: "checkmark.circle.fill")
                    UserStatsCard(title: "Admins", value: "\(adminCount)", color: .red, icon: "shield.fill")
                    UserStatsCard(title: "Inactive", value: "\(inactiveUsers)", color: .gray, icon: "person.crop.circle.badge.xmark")
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
                        Button("Field TL") { selectedRole = "field_tl" }
                        Button("Field Officer") { selectedRole = "field" }
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
                    
                    // Show different messages based on user role
                    if authManager.currentUser?.role.lowercased() != "admin" {
                        Text("You can view user information but need admin privileges to create or modify users.")
                            .font(.caption)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal)
                    }
                    
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
                            UserRow(
                                user: user,
                                onToggleStatus: {
                                    userToToggle = user
                                    showDeactivateConfirmation = true
                                },
                                onDelete: {
                                    userToDelete = user
                                    showDeleteConfirmation = true
                                },
                                onViewDetails: {
                                    selectedUserForDetails = user
                                },
                                onEdit: {
                                    selectedUserForEdit = user
                                }
                            )
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
            // Only show create user button if current user is admin
            if authManager.currentUser?.role.lowercased() == "admin" {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button(action: { showCreateSheet = true }) {
                        Image(systemName: "person.badge.plus")
                            .font(.title3)
                    }
                }
            }
        }
        .sheet(isPresented: $showCreateSheet) {
            CreateUserSheet(onSave: {
                loadUsers()
            })
        }
        .sheet(item: $selectedUserForDetails) { user in
            UserDetailsSheet(user: user)
        }
        .sheet(item: $selectedUserForEdit) { user in
            EditUserSheet(user: user, onSave: {
                loadUsers()
            })
        }
        .alert("Error", isPresented: $showErrorAlert) {
            Button("OK") { }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
        .alert(
            userToToggle?.active == true ? "Deactivate User" : "Activate User",
            isPresented: $showDeactivateConfirmation
        ) {
            if let user = userToToggle {
                Button(user.active ? "Deactivate" : "Activate", role: user.active ? .destructive : nil) {
                    toggleUserStatus(user)
                }
                Button("Cancel", role: .cancel) { }
            }
        } message: {
            if let user = userToToggle {
                Text(user.active ? 
                     "Are you sure you want to deactivate \(user.displayName)? They will no longer be able to log in." :
                     "Are you sure you want to activate \(user.displayName)? They will be able to log in again."
                )
            }
        }
        .alert(
            userToDelete?.active == true ? "Deactivate User Instead?" : "Permanently Delete User?",
            isPresented: $showDeleteConfirmation
        ) {
            if let user = userToDelete {
                // Only show deactivate option if user is currently active
                if user.active {
                    Button("Deactivate", role: .destructive) {
                        // Change the user to toggle and show deactivate confirmation
                        userToToggle = user
                        userToDelete = nil
                        showDeleteConfirmation = false
                        showDeactivateConfirmation = true
                    }
                }
                Button("Permanently Delete", role: .destructive) {
                    deleteUser(user)
                }
                Button("Cancel", role: .cancel) { }
            }
        } message: {
            if let user = userToDelete {
                if user.active {
                    Text("Deleting \(user.displayName) permanently may cause database violations. Would you prefer to deactivate them instead? They can be reactivated later.")
                } else {
                    Text("Are you sure you want to permanently delete \(user.displayName)? This user is already inactive and this action cannot be undone.")
                }
            }
        }
        .onAppear {
            loadUsers()
        }
        .overlay(
            // Success Toast
            Group {
                if showSuccessToast {
                    VStack {
                        Spacer()
                        HStack {
                            Image(systemName: "checkmark.circle.fill")
                                .foregroundColor(.white)
                            Text(successMessage ?? "Success")
                                .foregroundColor(.white)
                                .font(.subheadline)
                        }
                        .padding()
                        .background(Color.green)
                        .cornerRadius(8)
                        .shadow(radius: 4)
                        .padding(.horizontal)
                        .padding(.bottom, 100)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                        .onAppear {
                            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                                withAnimation {
                                    showSuccessToast = false
                                }
                            }
                        }
                    }
                    .animation(.easeInOut, value: showSuccessToast)
                }
            }
        )
    }
    
    private func loadUsers() {
        isLoading = true
        
        Task {
            do {
                guard let currentUser = authManager.currentUser else {
                    throw NSError(domain: "AuthError", code: 401, userInfo: [NSLocalizedDescriptionKey: "Not authenticated"])
                }
                
                // Fixed: Use UserFFIHandler with corrected JSON encoding
                print("🔍 Using fixed UserFFIHandler")
                let userHandler = UserFFIHandler()
                let authContext = AuthContextPayload(
                    user_id: currentUser.userId,
                    role: currentUser.role,
                    device_id: authManager.getDeviceId(),
                    offline_mode: false
                )
                let result = try await userHandler.getAllUsers(auth: authContext).get()
                
                // TODO: Debug UserFFIHandler JSON encoding issue
                // let userHandler = UserFFIHandler()
                // let authContext = AuthContextPayload(
                //     user_id: currentUser.userId,
                //     role: currentUser.role,
                //     device_id: authManager.getDeviceId(),
                //     offline_mode: false
                // )
                // 
                // // Debug the JSON payload being sent to Rust
                // let encoder = JSONEncoder()
                // encoder.keyEncodingStrategy = .convertToSnakeCase  // This might be the issue!
                // let testPayload = try encoder.encode(authContext)
                // print("🔍 UserFFIHandler would send: \(String(data: testPayload, encoding: .utf8) ?? "invalid")")
                //
                // let result = try await userHandler.getAllUsers(auth: authContext).get()
                
                await MainActor.run {
                    self.users = result
                    print("📋 Loaded \(result.count) users:")
                    for user in result {
                        print("   👤 \(user.name) (\(user.email)) - Active: \(user.active)")
                    }
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
        // Prevent users from deactivating themselves
        if user.id == authManager.currentUser?.userId && user.active {
            errorMessage = "You cannot deactivate your own account"
            showErrorAlert = true
            return
        }
        
        print("🔄 Toggling user status for: \(user.displayName) (ID: \(user.id)) from \(user.active) to \(!user.active)")
        
        // Show loading state
        Task {
            await MainActor.run {
                self.isLoading = true
                self.errorMessage = nil
            }
            
            do {
                guard let currentUser = authManager.currentUser else {
                    throw NSError(domain: "AuthError", code: 401, userInfo: [NSLocalizedDescriptionKey: "Not authenticated"])
                }
                
                // Fixed: Use UserFFIHandler consistently for user updates
                let userHandler = UserFFIHandler()
                let authContext = AuthContextPayload(
                    user_id: currentUser.userId,
                    role: currentUser.role,
                    device_id: authManager.getDeviceId(),
                    offline_mode: false
                )
                
                let updateUser = UpdateUser(
                    email: nil,
                    password: nil,
                    name: nil,
                    role: nil,
                    active: !user.active
                )
                
                print("🔄 Updating user \(user.displayName) active status to: \(!user.active)")
                
                let updateResult = await userHandler.updateUser(userId: user.id, update: updateUser, auth: authContext)
                
                await MainActor.run {
                    self.isLoading = false
                    
                    switch updateResult {
                    case .success(let updatedUser):
                        print("✅ User updated successfully: \(updatedUser.name) - Active: \(updatedUser.active)")
                        
                        // Show success message
                        let action = user.active ? "deactivated" : "activated"
                        self.successMessage = "User \(action) successfully"
                        self.showSuccessToast = true
                        
                        // Refresh the user list using the same UserFFIHandler
                        print("🔄 Refreshing user list...")
                        Task {
                            do {
                                let refreshedUsers = try await userHandler.getAllUsers(auth: authContext).get()
                                
                                await MainActor.run {
                                    self.users = refreshedUsers
                                    print("📋 Refreshed \(refreshedUsers.count) users:")
                                    for refreshedUser in refreshedUsers {
                                        print("   👤 \(refreshedUser.name) (\(refreshedUser.email)) - Active: \(refreshedUser.active)")
                                    }
                                    self.updateStats()
                                }
                            } catch {
                                print("❌ Refresh failed: \(error)")
                                // Fallback to regular load
                                self.loadUsers()
                            }
                        }
                        
                    case .failure(let error):
                        print("❌ Update failed: \(error)")
                        let action = user.active ? "deactivate" : "activate"
                        self.errorMessage = "Failed to \(action) user: \(error.localizedDescription)"
                        self.showErrorAlert = true
                    }
                }
                
            } catch {
                await MainActor.run {
                    self.isLoading = false
                    self.errorMessage = error.localizedDescription
                    self.showErrorAlert = true
                    print("💥 Exception: \(error)")
                }
            }
        }
    }
    
    private func deleteUser(_ user: UserResponse) {
        // Prevent users from deleting themselves
        if user.id == authManager.currentUser?.userId {
            errorMessage = "You cannot delete your own account"
            showErrorAlert = true
            return
        }
        
        // Show loading state
        Task {
            await MainActor.run {
                self.isLoading = true
                self.errorMessage = nil
            }
            
            do {
                guard let currentUser = authManager.currentUser else {
                    throw NSError(domain: "AuthError", code: 401, userInfo: [NSLocalizedDescriptionKey: "Not authenticated"])
                }
                
                // Fixed: Use UserFFIHandler consistently for user deletion
                let userHandler = UserFFIHandler()
                let authContext = AuthContextPayload(
                    user_id: currentUser.userId,
                    role: currentUser.role,
                    device_id: authManager.getDeviceId(),
                    offline_mode: false
                )
                
                print("🗑️ Deleting user \(user.displayName)")
                
                let deleteResult = await userHandler.hardDeleteUser(userId: user.id, auth: authContext)
                
                await MainActor.run {
                    self.isLoading = false
                    
                    switch deleteResult {
                    case .success:
                        print("✅ User deleted successfully")
                        // Success - reload users
                        self.successMessage = "User deleted successfully"
                        self.showSuccessToast = true
                        loadUsers()
                        
                    case .failure(let error):
                        print("❌ Delete failed: \(error)")
                        self.errorMessage = "Failed to delete user: \(error.localizedDescription)"
                        self.showErrorAlert = true
                    }
                }
                
            } catch {
                await MainActor.run {
                    self.isLoading = false
                    self.errorMessage = error.localizedDescription
                    self.showErrorAlert = true
                }
            }
        }
    }
}

// MARK: - User Row Component
struct UserRow: View {
    let user: UserResponse
    let onToggleStatus: () -> Void
    let onDelete: () -> Void
    let onViewDetails: () -> Void
    let onEdit: () -> Void
    @State private var showActions = false
    @EnvironmentObject var authManager: AuthenticationManager
    
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
                UserBadge(text: user.role.capitalized, color: user.roleColor)
                
                // Status Badge
                UserBadge(
                    text: user.active ? "Active" : "Inactive",
                    color: user.active ? .green : .gray
                )
                
                // Actions
                Menu {
                    // Only show admin actions if current user is admin
                    if authManager.currentUser?.role.lowercased() == "admin" {
                        Button(action: onViewDetails) {
                            Label("View Details", systemImage: "eye")
                        }
                        
                        Button(action: onEdit) {
                            Label("Edit", systemImage: "pencil")
                        }
                        
                        Divider()
                        
                        Button(action: onToggleStatus) {
                            Label(
                                user.active ? "Deactivate Account" : "Activate Account",
                                systemImage: user.active ? "person.crop.circle.badge.xmark" : "person.crop.circle.badge.checkmark"
                            )
                        }
                        
                        Divider()
                        
                        Button(role: .destructive, action: onDelete) {
                            Label("Permanently Delete", systemImage: "trash")
                        }
                    } else {
                        // Show view-only option for non-admin users
                        Button(action: onViewDetails) {
                            Label("View Details", systemImage: "eye")
                        }
                        
                        Divider()
                        
                        // Show disabled message
                        Text("Admin access required for user management")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                            .padding(.horizontal)
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
    @State private var role = "field"
    @State private var active = true
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
                        Text("Field TL").tag("field_tl")
                        Text("Field Officer").tag("field")
                    }
                    .pickerStyle(.segmented)
                }
                
                Section("Status") {
                    Toggle("Active User", isOn: $active)
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
        
        // Get current user ID for created_by_user_id field
        guard let currentUser = authManager.currentUser else {
            errorMessage = "Not authenticated"
            isLoading = false
            return
        }
        
        let payload = """
        {
            "user": {
                "email": "\(email)",
                "name": "\(name)",
                "password": "\(password)",
                "role": "\(role)",
                "active": \(active),
                "created_by_user_id": "\(currentUser.userId)"
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

// MARK: - Users View Components

/// User Badge component for displaying status labels
struct UserBadge: View {
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

/// User Stats card component for displaying user statistics
struct UserStatsCard: View {
    let title: String
    let value: String
    let color: Color
    let icon: String
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: icon)
                    .font(.title3)
                    .foregroundColor(color)
                Spacer()
            }
            
            Text(value)
                .font(.title2)
                .fontWeight(.bold)
                .foregroundColor(.primary)
            
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
        .frame(minWidth: 100)
        .background(Color(.systemBackground))
        .cornerRadius(12)
        .shadow(color: Color.black.opacity(0.05), radius: 3, x: 0, y: 2)
    }
}

// MARK: - User Details Sheet
struct UserDetailsSheet: View {
    @Environment(\.dismiss) var dismiss
    let user: UserResponse
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // User Avatar and Basic Info
                    VStack(spacing: 16) {
                        Circle()
                            .fill(user.roleColor.opacity(0.2))
                            .overlay(
                                Text(user.displayName.prefix(1).uppercased())
                                    .font(.largeTitle)
                                    .fontWeight(.bold)
                                    .foregroundColor(user.roleColor)
                            )
                            .frame(width: 80, height: 80)
                        
                        VStack(spacing: 8) {
                            Text(user.displayName)
                                .font(.title2)
                                .fontWeight(.bold)
                            
                            Text(user.email)
                                .font(.subheadline)
                                .foregroundColor(.secondary)
                            
                            HStack(spacing: 12) {
                                UserBadge(text: user.role.capitalized, color: user.roleColor)
                                UserBadge(
                                    text: user.active ? "Active" : "Inactive",
                                    color: user.active ? .green : .gray
                                )
                            }
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical)
                    
                    // Details Section
                    VStack(alignment: .leading, spacing: 16) {
                        Text("Details")
                            .font(.headline)
                            .fontWeight(.semibold)
                        
                        VStack(spacing: 12) {
                            UserDetailRow(label: "User ID", value: user.id)
                            UserDetailRow(label: "Email", value: user.email)
                            UserDetailRow(label: "Name", value: user.name)
                            UserDetailRow(label: "Role", value: user.role.capitalized)
                            UserDetailRow(label: "Status", value: user.active ? "Active" : "Inactive")
                            
                            if let lastLogin = user.last_login {
                                UserDetailRow(label: "Last Login", value: formatDate(lastLogin))
                            } else {
                                UserDetailRow(label: "Last Login", value: "Never")
                            }
                        }
                        .padding()
                        .background(Color(.systemGray6))
                        .cornerRadius(12)
                    }
                    
                    // Timestamps Section
                    VStack(alignment: .leading, spacing: 16) {
                        Text("Timestamps")
                            .font(.headline)
                            .fontWeight(.semibold)
                        
                        VStack(spacing: 12) {
                            UserDetailRow(label: "Created", value: formatDate(user.created_at))
                            UserDetailRow(label: "Last Updated", value: user.updated_at != nil ? formatDate(user.updated_at!) : "Never updated")
                        }
                        .padding()
                        .background(Color(.systemGray6))
                        .cornerRadius(12)
                    }
                    
                    // Audit Trail Section
                    VStack(alignment: .leading, spacing: 16) {
                        Text("Audit Trail")
                            .font(.headline)
                            .fontWeight(.semibold)
                        
                        VStack(spacing: 12) {
                            if let createdBy = user.created_by {
                                UserDetailRow(label: "Created By", value: createdBy)
                            } else if let createdById = user.created_by_user_id {
                                UserDetailRow(label: "Created By ID", value: createdById)
                            } else {
                                UserDetailRow(label: "Created By", value: "System")
                            }
                            
                            if let updatedBy = user.updated_by {
                                UserDetailRow(label: "Last Updated By", value: updatedBy)
                            } else if let updatedById = user.updated_by_user_id {
                                UserDetailRow(label: "Last Updated By ID", value: updatedById)
                            } else {
                                UserDetailRow(label: "Last Updated By", value: "Unknown")
                            }
                            
                            if let createdDeviceId = user.created_by_device_id {
                                UserDetailRow(label: "Created From Device", value: String(createdDeviceId.prefix(8)) + "...")
                            }
                            
                            if let updatedDeviceId = user.updated_by_device_id {
                                UserDetailRow(label: "Last Updated From Device", value: String(updatedDeviceId.prefix(8)) + "...")
                            }
                        }
                        .padding()
                        .background(Color(.systemGray6))
                        .cornerRadius(12)
                    }
                }
                .padding()
            }
            .navigationTitle("User Details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") { dismiss() }
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
}

// MARK: - Edit User Sheet
struct EditUserSheet: View {
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    let user: UserResponse
    let onSave: () -> Void
    
    @State private var email: String
    @State private var name: String
    @State private var role: String
    @State private var active: Bool
    @State private var password: String = ""
    @State private var confirmPassword: String = ""
    @State private var changePassword = false
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    init(user: UserResponse, onSave: @escaping () -> Void) {
        self.user = user
        self.onSave = onSave
        self._email = State(initialValue: user.email)
        self._name = State(initialValue: user.name)
        self._role = State(initialValue: user.role)
        self._active = State(initialValue: user.active)
    }
    
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
                }
                
                Section("Role & Status") {
                    // Only allow role changes if current user is admin and not editing themselves
                    if authManager.currentUser?.role.lowercased() == "admin" && user.id != authManager.currentUser?.userId {
                        Picker("Role", selection: $role) {
                            Text("Admin").tag("admin")
                            Text("Field TL").tag("field_tl")
                            Text("Field Officer").tag("field")
                        }
                        .pickerStyle(.segmented)
                        
                        Toggle("Active User", isOn: $active)
                    } else {
                        HStack {
                            Text("Role")
                            Spacer()
                            Text(role.capitalized)
                                .foregroundColor(.secondary)
                        }
                        
                        HStack {
                            Text("Status")
                            Spacer()
                            Text(active ? "Active" : "Inactive")
                                .foregroundColor(.secondary)
                        }
                        
                        if user.id == authManager.currentUser?.userId {
                            Text("You cannot change your own role or status")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }
                
                Section("Password") {
                    Toggle("Change Password", isOn: $changePassword)
                    
                    if changePassword {
                        SecureField("New Password", text: $password)
                        SecureField("Confirm Password", text: $confirmPassword)
                        
                        if !password.isEmpty && password != confirmPassword {
                            Text("Passwords do not match")
                                .foregroundColor(.red)
                                .font(.caption)
                        }
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
            .navigationTitle("Edit User")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        saveUser()
                    }
                    .disabled(isLoading || hasValidationErrors)
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
    
    private var hasValidationErrors: Bool {
        email.isEmpty || name.isEmpty || 
        (changePassword && (password.isEmpty || password != confirmPassword))
    }
    
    private func saveUser() {
        isLoading = true
        errorMessage = nil
        
        guard let currentUser = authManager.currentUser else {
            errorMessage = "Not authenticated"
            isLoading = false
            return
        }
        
        Task {
            let userHandler = UserFFIHandler()
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            // Check what fields have changed
            var hasChanges = false
            var updateUser = UpdateUser()
            
            if email != user.email {
                updateUser.email = email
                hasChanges = true
            }
            
            if name != user.name {
                updateUser.name = name
                hasChanges = true
            }
            
            if role != user.role {
                updateUser.role = role
                hasChanges = true
            }
            
            if active != user.active {
                updateUser.active = active
                hasChanges = true
            }
            
            if changePassword && !password.isEmpty {
                updateUser.password = password
                hasChanges = true
            }
            
            if !hasChanges {
                await MainActor.run {
                    self.isLoading = false
                    dismiss()
                }
                return
            }
            
            let result = await userHandler.updateUser(userId: user.id, update: updateUser, auth: authContext)
            
            await MainActor.run {
                self.isLoading = false
                
                switch result {
                case .success:
                    onSave()
                    dismiss()
                case .failure(let error):
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}

/// User Detail row component for key-value pairs
struct UserDetailRow: View {
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
