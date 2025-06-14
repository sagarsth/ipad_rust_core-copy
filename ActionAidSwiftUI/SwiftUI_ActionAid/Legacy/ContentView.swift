//
//  ContentView.swift
//  ActionAid SwiftUI Test
//
//  iPad Rust Core Test Interface - SwiftUI
//

import SwiftUI

// MARK: - Shared Authentication State
class AuthenticationState: ObservableObject {
    @Published var lastLoggedInUser: LoggedInUser?
    
    struct LoggedInUser {
        let userId: String
        let role: String
        let email: String
        let token: String
        let loginTime: Date
        
        var authContext: [String: Any] {
            return [
                "user_id": userId,
                "role": role,
                "device_id": AuthenticationState.getDeviceId(),
                "offline_mode": false
            ]
        }
    }
    
    func updateLastLoggedInUser(userId: String, role: String, email: String, token: String) {
        lastLoggedInUser = LoggedInUser(
            userId: userId,
            role: role,
            email: email,
            token: token,
            loginTime: Date()
        )
        print("🔑 Updated last logged in user: \(email) (\(userId.prefix(8))...) - Role: \(role)")
    }
    
    func clearLastLoggedInUser() {
        lastLoggedInUser = nil
        print("🚪 Cleared last logged in user")
    }
    
    static func getDeviceId() -> String {
        if let uuid = UIDevice.current.identifierForVendor?.uuidString {
            return uuid
        }
        return "unknown-device"
    }
}

// Global shared instance
// let authenticationState = AuthenticationState() // REMOVED: Replaced by @EnvironmentObject

struct ContentView: View {
    @State private var statusMessage = "Ready to test iPad Rust Core"
    @State private var testResults = ""
    @State private var isRunningTests = false
    @EnvironmentObject var authState: AuthenticationState // ADDED: Injected from the environment
    
    var body: some View {
        TabView {
            // Main Core Tests Tab
            mainTestView
                .tabItem {
                    Image(systemName: "cpu")
                    Text("Core Tests")
                }
            
            // Legacy Strategic Tests Tab - Removed (StrategicTestView deleted)
            // Use the new StrategicGoalsView in the main app instead
            Text("Strategic tests moved to main app.\n\nUse the production StrategicGoalsView instead.")
                .multilineTextAlignment(.center)
                .foregroundColor(.secondary)
                .padding()
                .tabItem {
                    Image(systemName: "target")
                    Text("Strategic")
                }
        }
        .onAppear {
            updateStatus("Ready to test iPad Rust Core ✨")
        }
    }
    
    private var mainTestView: some View {
        VStack(spacing: 20) {
            // Header
            VStack(spacing: 10) {
                Text("🚀 iPad Rust Core")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text(statusMessage)
                    .font(.headline)
                    .foregroundColor(isRunningTests ? .orange : .primary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
            .padding(.top, 20)
            
            // Test Button
            Button(action: runTests) {
                HStack {
                    if isRunningTests {
                        ProgressView()
                            .scaleEffect(0.8)
                            .foregroundColor(.white)
                    }
                    Text(isRunningTests ? "Running Tests..." : "🧪 Run Core Initialization")
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding()
                .background(
                    LinearGradient(
                        gradient: Gradient(colors: isRunningTests ? [.orange, .red] : [.blue, .purple]),
                        startPoint: .leading,
                        endPoint: .trailing
                    )
                )
                .foregroundColor(.white)
                .cornerRadius(15)
                .shadow(radius: 5)
            }
            .disabled(isRunningTests)
            .padding(.horizontal)
            
            // Results Section - Full Height Scrollable
            ScrollView {
                Text(testResults.isEmpty ? "Tap 'Run Core Initialization' to set up the database and authenticate...\n\n🔬 This will:\n• Initialize database\n• Create default accounts\n• Login as administrator\n• Set up authentication state for Strategic tests" : testResults)
                    .font(.system(size: 10, design: .monospaced))
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(.systemGray6))
                    .cornerRadius(15)
                    .shadow(radius: 2)
            }
            .padding(.horizontal)
        }
        .background(
            LinearGradient(
                gradient: Gradient(colors: [Color(.systemBackground), Color(.systemGray6)]),
                startPoint: .top,
                endPoint: .bottom
            )
        )
    }
    
    private func runTests() {
        updateStatus("Initializing core system...")
        isRunningTests = true
        
        // Run tests asynchronously
        Task {
            let results = await performTests()
            
            await MainActor.run {
                testResults = results
                updateStatus("Initialization completed! 🎉")
                isRunningTests = false
            }
        }
    }
    
    private func updateStatus(_ message: String) {
        statusMessage = message
        print("📱 Status: \(message)")
    }
    
    private func performTests() async -> String {
        // Add small delay for better UX
        try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 seconds
        
        var results = "🚀 iPad Rust Core Initialization\n"
        results += "================================\n\n"
        
        // Get device ID
        let deviceId = AuthenticationState.getDeviceId()
        results += "📱 Device ID: \(deviceId)\n\n"
        
        // Test 1: Database Initialization
        results += "1️⃣ Database Initialization...\n"
        let dbPath = getDatabasePath()
        
        // Set the storage path for iOS BEFORE initialization
        let documentsPath = getDocumentsDirectory()
        let storagePath = "\(documentsPath)/ActionAid/storage"  // Use consistent ActionAid/storage structure
        
        // Add debug logging
        print("📂 [SWIFT] Documents path: \(documentsPath)")
        print("📂 [SWIFT] Storage path: \(storagePath)")
        
        do {
            try FileManager.default.createDirectory(atPath: storagePath, withIntermediateDirectories: true, attributes: nil)
            results += "✅ Storage directory ready at: \(storagePath)\n"
            print("📂 [SWIFT] Storage directory created successfully")
        } catch {
            results += "⚠️ Warning: Could not create storage directory: \(error.localizedDescription)\n"
            print("📂 [SWIFT] Failed to create storage directory: \(error)")
        }
        
        // Verify directory exists before setting
        let storageExists = FileManager.default.fileExists(atPath: storagePath)
        print("📂 [SWIFT] Storage directory exists: \(storageExists)")

        let storageSetResult = set_ios_storage_path(storagePath)
        if storageSetResult == 0 {
            results += "✅ iOS storage path configured: \(storagePath)\n"
        } else {
            results += "❌ Failed to set iOS storage path (code: \(storageSetResult))\n"
        }
        
        // Ensure the database directory exists
        let dbDirectory = (dbPath as NSString).deletingLastPathComponent
        do {
            try FileManager.default.createDirectory(atPath: dbDirectory, withIntermediateDirectories: true, attributes: nil)
            results += "✅ Database directory ready\n"
        } catch {
            results += "❌ Failed to create database directory: \(error.localizedDescription)\n"
            return results
        }
        
        // Check if database file already exists and remove it for clean test
        let fileExists = FileManager.default.fileExists(atPath: dbPath)
        if fileExists {
            try? FileManager.default.removeItem(atPath: dbPath)
            results += "🗑️ Cleaned existing database\n"
        }
        
        // Use device ID and provide a proper JWT secret for testing
        let jwtSecret = "test_jwt_secret_for_ios_app_development_\(deviceId.prefix(8))"
        
        // Create proper SQLite URL with mode=rwc for read-write-create
        let sqliteUrl = "sqlite://\(dbPath)?mode=rwc"
        
        let initResult = initialize_library(sqliteUrl, deviceId, false, jwtSecret)
        if initResult == 0 {
            results += "✅ Database initialized successfully\n"
            
            // Verify the database file was created
            let fileExistsAfterInit = FileManager.default.fileExists(atPath: dbPath)
            if fileExistsAfterInit {
            if let attributes = try? FileManager.default.attributesOfItem(atPath: dbPath) {
                let fileSize = attributes[.size] as? NSNumber ?? 0
                    results += "📏 Database size: \(fileSize) bytes\n"
                }
            }
            results += "\n"
        } else {
            results += "❌ Database initialization failed (code: \(initResult))\n"
            let error = getLastError()
            results += "🔍 Error details: \(error)\n\n"
            return results
        }
        
        // Test 2: Initialize Default Accounts
        results += "2️⃣ Setting up default accounts...\n"
        
            let defaultAccountsResult = auth_initialize_default_accounts("init_setup")
            
            if defaultAccountsResult == 0 {
            results += "✅ Default accounts created\n"
            results += "👥 Available: admin@example.com, lead@example.com, officer@example.com\n\n"
            } else {
                let error = getLastError()
                results += "⚠️ Default accounts setup: \(error)\n\n"
        }
        
        // Test 3: Initialize Test Data (for Strategic domain)
        results += "3️⃣ Setting up test data...\n"
        
            let testDataResult = auth_initialize_test_data("init_setup")
            
            if testDataResult == 0 {
            results += "✅ Test data initialized\n"
            results += "🧪 Created: status types, donors, projects, etc.\n\n"
            } else {
                let error = getLastError()
                results += "⚠️ Test data setup: \(error)\n\n"
        }
        
        // Test 4: Admin Login
        results += "4️⃣ Authenticating as Administrator...\n"
        let adminLoginJson = """
        {
            "email": "admin@example.com",
            "password": "Admin123!"
        }
        """
        
        var adminAuthResult: UnsafeMutablePointer<CChar>?
        let adminAuthCode = auth_login(adminLoginJson, &adminAuthResult)
        
        if adminAuthCode == 0, let adminAuthResultStr = adminAuthResult {
            let adminAuthResponse = String(cString: adminAuthResultStr)
            results += "✅ Administrator authenticated successfully\n"
            
            // Extract token and user info for Strategic tests
            if let tokenData = adminAuthResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: tokenData) as? [String: Any],
               let token = json["access_token"] as? String,
               let userId = json["user_id"] as? String,
               let role = json["role"] as? String {
                
                // Store in authentication state via the environment object
                await MainActor.run {
                    authState.updateLastLoggedInUser(
                        userId: userId,
                        role: role,
                        email: "admin@example.com",
                        token: token
                    )
                }
                
                results += "👤 User ID: \(userId.prefix(8))...\n"
                results += "🎭 Role: \(role)\n"
                results += "🔑 Authentication state ready for Strategic tests\n"
            }
            free_string(adminAuthResultStr)
        } else {
            let error = getLastError()
            results += "❌ Administrator authentication failed: \(error)\n"
        }
        
        results += "\n================================\n"
        results += "✅ Core Initialization Complete!\n"
        results += "🎯 You can now run Strategic Domain tests\n"
        results += "✨ Authentication state is shared between tabs\n"
        
        return results
    }
    
    // MARK: - Helper Functions
    
    private func getDocumentsDirectory() -> String {
        // Use Documents directory for read/write access to user files
        let paths = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
        return paths[0].path  // Use .path to get clean file path, not .absoluteString
    }
    
    private func getDatabasePath() -> String {
        let documentsPath = getDocumentsDirectory()
        let dbDir = "\(documentsPath)/ActionAid"  // Use consistent ActionAid directory
        
        // Ensure the ActionAid subdirectory exists
        do {
            try FileManager.default.createDirectory(atPath: dbDir, withIntermediateDirectories: true, attributes: nil)
        } catch {
            print("Failed to create database directory: \(error)")
        }
        
        return "\(dbDir)/actionaid_core.sqlite"
    }
    
    private func getDatabaseURL() -> String {
        return "sqlite://\(getDatabasePath())"
    }
    
    private func getLastError() -> String {
        if let errorPtr = get_last_error() {
            if let errorStr = String(cString: errorPtr, encoding: .utf8) {
                let result = errorStr.isEmpty ? "No error" : errorStr
                free_string(errorPtr)
                return result
            }
            free_string(errorPtr)
        }
        return "Unknown error"
    }
}

#Preview {
    ContentView()
        .environmentObject(AuthenticationState()) // Provide a default empty state for the preview
}
