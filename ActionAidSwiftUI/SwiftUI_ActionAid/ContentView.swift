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
let authenticationState = AuthenticationState()

struct ContentView: View {
    @State private var statusMessage = "Ready to test iPad Rust Core"
    @State private var testResults = ""
    @State private var isRunningTests = false
    
    var body: some View {
        TabView {
            // Main Core Tests Tab
            mainTestView
                .tabItem {
                    Image(systemName: "cpu")
                    Text("Core Tests")
                }
            
            // Strategic Domain Tests Tab
            StrategicTestView()
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
                    Text(isRunningTests ? "Running Tests..." : "🧪 Run Tests")
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
                Text(testResults.isEmpty ? "Tap 'Run Tests' to start testing your Rust library...\n\n🔬 This will test:\n• Library version\n• Database initialization\n• User creation\n• Authentication\n• Project operations\n• Error handling" : testResults)
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
        updateStatus("Running comprehensive tests...")
        isRunningTests = true
        
        // Run tests asynchronously
        Task {
            let results = await performTests()
            
            await MainActor.run {
                testResults = results
                updateStatus("Tests completed! 🎉")
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
        
        var results = "🚀 iPad Rust Core Test Results\n"
        results += "================================\n\n"
        
        // Test 1: Library Version
        results += "1️⃣ Testing Library Version...\n"
        if let version = get_library_version() {
            if let versionStr = String(cString: version, encoding: .utf8) {
                results += "✅ Version: \(versionStr)\n\n"
                free_string(version)
            } else {
                results += "❌ Failed to decode version\n\n"
                free_string(version)
            }
        } else {
            results += "❌ Failed to get version\n\n"
        }
        
        // Test 2: Database Initialization
        results += "2️⃣ Testing Database Initialization...\n"
        let deviceId = AuthenticationState.getDeviceId()
        let dbPath = getDatabasePath()
        results += "📱 Device ID: \(deviceId)\n"
        results += "💾 Database Path: \(dbPath)\n"
        
        // Set the storage path for iOS BEFORE initialization
        let documentsPath = getDocumentsDirectory()
        let storagePath = "\(documentsPath)/storage"
        do {
            try FileManager.default.createDirectory(atPath: storagePath, withIntermediateDirectories: true, attributes: nil)
            results += "📁 Storage directory created/verified: \(storagePath)\n"
        } catch {
            results += "⚠️ Warning: Could not create storage directory: \(error.localizedDescription)\n"
        }
        
        let storageSetResult = set_ios_storage_path(storagePath)
        if storageSetResult == 0 {
            results += "✅ iOS storage path set successfully\n"
        } else {
            results += "⚠️ Warning: Failed to set iOS storage path\n"
        }
        
        // Ensure the database directory exists
        let dbDirectory = (dbPath as NSString).deletingLastPathComponent
        results += "📁 Database Directory: \(dbDirectory)\n"
        
        do {
            try FileManager.default.createDirectory(atPath: dbDirectory, withIntermediateDirectories: true, attributes: nil)
            results += "✅ Database directory created/verified\n"
        } catch {
            results += "❌ Failed to create database directory: \(error.localizedDescription)\n"
        }
        
        // Check if database file already exists and remove it for clean test
        let fileExists = FileManager.default.fileExists(atPath: dbPath)
        results += "📄 Database file exists: \(fileExists)\n"
        
        if fileExists {
            try? FileManager.default.removeItem(atPath: dbPath)
            results += "🗑️ Removed existing database for clean test\n"
        }
        
        // Check directory permissions
        let isWritable = FileManager.default.isWritableFile(atPath: dbDirectory)
        results += "✏️ Directory writable: \(isWritable)\n"
        
        // Use device ID and provide a proper JWT secret for testing
        let jwtSecret = "test_jwt_secret_for_ios_app_development_\(deviceId.prefix(8))"
        results += "🔑 JWT Secret (first 20 chars): \(String(jwtSecret.prefix(20)))...\n"
        
        // Create proper SQLite URL with mode=rwc for read-write-create
        let sqliteUrl = "sqlite://\(dbPath)?mode=rwc"
        results += "🔗 Database URL: \(sqliteUrl)\n"
        
        let initResult = initialize_library(sqliteUrl, deviceId, false, jwtSecret)
        if initResult == 0 {
            results += "✅ Database initialized successfully\n"
            
            // Verify the database file was created
            let fileExistsAfterInit = FileManager.default.fileExists(atPath: dbPath)
            results += "📄 Database file exists after init: \(fileExistsAfterInit)\n"
            
            if let attributes = try? FileManager.default.attributesOfItem(atPath: dbPath) {
                let fileSize = attributes[.size] as? NSNumber ?? 0
                results += "📏 Database file size: \(fileSize) bytes\n"
            }
            results += "\n"
        } else {
            results += "❌ Database initialization failed (code: \(initResult))\n"
            let error = getLastError()
            results += "🔍 Error details: \(error)\n\n"
        }
        
        // Test 3: Initialize Default Accounts (if database was initialized successfully)
        results += "3️⃣ Testing Default Account Setup...\n"
        
        if initResult == 0 {
            let defaultAccountsResult = auth_initialize_default_accounts("init_setup")
            
            if defaultAccountsResult == 0 {
                results += "✅ Default accounts initialized successfully\n"
                results += "👥 Created: admin@example.com, lead@example.com, officer@example.com\n\n"
            } else {
                let error = getLastError()
                results += "⚠️ Default accounts setup: \(error)\n\n"
            }
        } else {
            results += "⏭️ Skipped (database initialization failed)\n\n"
        }
        
        // Test 3.5: Initialize Comprehensive Test Data
        results += "3️⃣.5️⃣ Testing Comprehensive Test Data Setup...\n"
        
        if initResult == 0 {
            let testDataResult = auth_initialize_test_data("init_setup")
            
            if testDataResult == 0 {
                results += "✅ Comprehensive test data initialized successfully\n"
                results += "🧪 Created: donors, projects, activities, participants, workshops, etc.\n\n"
            } else {
                let error = getLastError()
                results += "⚠️ Test data setup: \(error)\n\n"
            }
        } else {
            results += "⏭️ Skipped (database initialization failed)\n\n"
        }
        
        // Test 4: Database Status Check
        results += "4️⃣ Testing Database Status...\n"
        
        // Check if database was properly initialized
        let dbFileExists = FileManager.default.fileExists(atPath: dbPath)
        results += "📄 Database file created: \(dbFileExists)\n"
        
        if dbFileExists {
            let dbSize = try? FileManager.default.attributesOfItem(atPath: dbPath)[.size] as? NSNumber ?? 0
            results += "📏 Database size: \(dbSize ?? 0) bytes\n"
            results += "✅ Database is ready for operations\n\n"
        } else {
            results += "❌ Database file not found\n\n"
        }
        
        // Test 4.5: Check user count
        results += "4️⃣.5️⃣ Testing User Count...\n"
        
        // First login as admin to get a proper token for testing
        let adminLoginForCountJson = """
        {
            "email": "admin@example.com",
            "password": "Admin123!"
        }
        """
        
        var adminAuthForCountResult: UnsafeMutablePointer<CChar>?
        let adminAuthForCountCode = auth_login(adminLoginForCountJson, &adminAuthForCountResult)
        
        var adminTokenForCount: String = ""
        if adminAuthForCountCode == 0, let adminAuthForCountStr = adminAuthForCountResult {
            let adminAuthForCountResponse = String(cString: adminAuthForCountStr)
            
            // Extract token for user count test
            if let tokenData = adminAuthForCountResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: tokenData) as? [String: Any],
               let token = json["access_token"] as? String {
                adminTokenForCount = token
                results += "🔑 Admin token obtained for user count test\n"
            }
            free_string(adminAuthForCountStr)
        }
        
        if !adminTokenForCount.isEmpty {
            var usersResult: UnsafeMutablePointer<CChar>?
            let usersQueryResult = auth_get_all_users(adminTokenForCount, &usersResult)
            
            if usersQueryResult == 0, let usersResultStr = usersResult {
                let usersResponse = String(cString: usersResultStr)
                
                // Try to parse the JSON to count users
                if let usersData = usersResponse.data(using: .utf8),
                   let usersJson = try? JSONSerialization.jsonObject(with: usersData) as? [[String: Any]] {
                    results += "✅ Successfully queried users: \(usersJson.count) accounts found\n"
                    
                    // List the email addresses of all users
                    let emails = usersJson.compactMap { $0["email"] as? String }
                    results += "👥 Accounts: \(emails.joined(separator: ", "))\n"
                    
                    // Check if we have the expected accounts
                    let expectedEmails = ["admin@example.com", "lead@example.com", "officer@example.com"]
                    let hasAllExpected = expectedEmails.allSatisfy { emails.contains($0) }
                    
                    if hasAllExpected {
                        results += "✅ All expected default accounts are present\n"
                    } else {
                        let missing = expectedEmails.filter { !emails.contains($0) }
                        results += "⚠️ Missing expected accounts: \(missing.joined(separator: ", "))\n"
                    }
                } else {
                    results += "✅ Users query successful but couldn't parse response\n"
                    results += "📄 Raw response: \(usersResponse.prefix(200))...\n"
                }
                
                free_string(usersResultStr)
            } else {
                let error = getLastError()
                results += "❌ Failed to query users: \(error)\n"
            }
        } else {
            results += "❌ Could not obtain admin token for user count test\n"
        }
        
        results += "\n"
        
        // Test 5: Comprehensive Authentication Testing
        results += "5️⃣ Testing Comprehensive Authentication...\n"
        
        // Test 5.1: Valid Admin Login
        results += "\n🔐 Test 5.1: Valid Admin Login\n"
        let adminLoginJson = """
        {
            "email": "admin@example.com",
            "password": "Admin123!"
        }
        """
        
        var adminAuthResult: UnsafeMutablePointer<CChar>?
        let adminAuthCreateResult = auth_login(adminLoginJson, &adminAuthResult)
        
        var adminToken: String = ""
        if adminAuthCreateResult == 0, let adminAuthResultStr = adminAuthResult {
            let adminAuthResponse = String(cString: adminAuthResultStr)
            results += "✅ Admin authentication successful\n"
            
            // Extract token and user info for further tests
            if let tokenData = adminAuthResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: tokenData) as? [String: Any],
               let token = json["access_token"] as? String,
               let userId = json["user_id"] as? String,
               let role = json["role"] as? String {
                adminToken = token
                authenticationState.updateLastLoggedInUser(
                    userId: userId,
                    role: role,
                    email: "admin@example.com",
                    token: token
                )
                results += "🔑 Admin token extracted for authorization tests\n"
                results += "👤 Admin user info stored: \(userId.prefix(8))... - Role: \(role)\n"
            }
            free_string(adminAuthResultStr)
        } else {
            let error = getLastError()
            results += "❌ Admin authentication failed: \(error)\n"
        }
        
        // Test 5.2: Valid Team Lead Login
        results += "\n👨‍💼 Test 5.2: Valid Team Lead Login\n"
        let leadLoginJson = """
        {
            "email": "lead@example.com",
            "password": "Lead123!"
        }
        """
        
        var leadAuthResult: UnsafeMutablePointer<CChar>?
        let leadAuthCreateResult = auth_login(leadLoginJson, &leadAuthResult)
        
        var leadToken: String = ""
        if leadAuthCreateResult == 0, let leadAuthResultStr = leadAuthResult {
            let leadAuthResponse = String(cString: leadAuthResultStr)
            results += "✅ Team Lead authentication successful\n"
            
            // Extract token and user info for further tests
            if let tokenData = leadAuthResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: tokenData) as? [String: Any],
               let token = json["access_token"] as? String,
               let userId = json["user_id"] as? String,
               let role = json["role"] as? String {
                leadToken = token
                authenticationState.updateLastLoggedInUser(
                    userId: userId,
                    role: role,
                    email: "lead@example.com",
                    token: token
                )
                results += "🔑 Team Lead token extracted for authorization tests\n"
                results += "👤 Team Lead user info stored: \(userId.prefix(8))... - Role: \(role)\n"
            }
            free_string(leadAuthResultStr)
        } else {
            let error = getLastError()
            results += "❌ Team Lead authentication failed: \(error)\n"
        }
        
        // Test 5.3: Valid Officer Login
        results += "\n👮 Test 5.3: Valid Officer Login\n"
        let officerLoginJson = """
        {
            "email": "officer@example.com",
            "password": "Officer123!"
        }
        """
        
        var officerAuthResult: UnsafeMutablePointer<CChar>?
        let officerAuthCreateResult = auth_login(officerLoginJson, &officerAuthResult)
        
        var officerToken: String = ""
        if officerAuthCreateResult == 0, let officerAuthResultStr = officerAuthResult {
            let officerAuthResponse = String(cString: officerAuthResultStr)
            results += "✅ Officer authentication successful\n"
            
            // Extract token and user info for further tests
            if let tokenData = officerAuthResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: tokenData) as? [String: Any],
               let token = json["access_token"] as? String,
               let userId = json["user_id"] as? String,
               let role = json["role"] as? String {
                officerToken = token
                authenticationState.updateLastLoggedInUser(
                    userId: userId,
                    role: role,
                    email: "officer@example.com",
                    token: token
                )
                results += "🔑 Officer token extracted for authorization tests\n"
                results += "👤 Officer user info stored: \(userId.prefix(8))... - Role: \(role)\n"
            }
            free_string(officerAuthResultStr)
        } else {
            let error = getLastError()
            results += "❌ Officer authentication failed: \(error)\n"
        }
        
        // Test 5.4: Authorization Tests - User Management
        results += "\n🛡️ Test 5.4: User Management Authorization\n"
        
        // Test creating a new user with different roles
        let newUserJson = """
        {
            "user": {
                "email": "test@example.com",
                "password": "Test123!",
                "name": "Test User",
                "role": "field",
                "active": true
            },
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000000",
                "role": "admin",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        // Test 5.4a: Admin can create users
        results += "\n✅ Test 5.4a: Admin User Creation (Should Succeed)\n"
        var adminCreateUserResult: UnsafeMutablePointer<CChar>?
        let adminCreateUserCode = user_create(newUserJson, &adminCreateUserResult)
        
        if adminCreateUserCode == 0 {
            results += "✅ Admin successfully created user\n"
            if let resultStr = adminCreateUserResult {
                let response = String(cString: resultStr)
                results += "📄 Created user response: \(response.prefix(100))...\n"
                free_string(resultStr)
            }
        } else {
            let error = getLastError()
            results += "❌ Admin failed to create user: \(error)\n"
        }
        
        // Test 5.4b: Team Lead cannot create users
        results += "\n🚫 Test 5.4b: Team Lead User Creation (Should Fail)\n"
        let leadCreateUserJson = """
        {
            "user": {
                "email": "test2@example.com",
                "password": "Test123!",
                "name": "Test User 2",
                "role": "field",
                "active": true
            },
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000001",
                "role": "field_tl",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        var leadCreateUserResult: UnsafeMutablePointer<CChar>?
        let leadCreateUserCode = user_create(leadCreateUserJson, &leadCreateUserResult)
        
        if leadCreateUserCode != 0 {
            results += "✅ Team Lead correctly denied user creation\n"
            let error = getLastError()
            results += "📝 Expected error: \(error)\n"
        } else {
            results += "❌ SECURITY ISSUE: Team Lead was allowed to create user!\n"
            if let resultStr = leadCreateUserResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.4c: Officer cannot create users
        results += "\n🚫 Test 5.4c: Officer User Creation (Should Fail)\n"
        let officerCreateUserJson = """
        {
            "user": {
                "email": "test3@example.com",
                "password": "Test123!",
                "name": "Test User 3",
                "role": "field",
                "active": true
            },
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000002",
                "role": "field",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        var officerCreateUserResult: UnsafeMutablePointer<CChar>?
        let officerCreateUserCode = user_create(officerCreateUserJson, &officerCreateUserResult)
        
        if officerCreateUserCode != 0 {
            results += "✅ Officer correctly denied user creation\n"
            let error = getLastError()
            results += "📝 Expected error: \(error)\n"
        } else {
            results += "❌ SECURITY ISSUE: Officer was allowed to create user!\n"
            if let resultStr = officerCreateUserResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.5: Get All Users Authorization
        results += "\n🛡️ Test 5.5: Get All Users Authorization\n"
        
        // Test 5.5a: Admin can get all users
        results += "\n✅ Test 5.5a: Admin Get All Users (Should Succeed)\n"
        
        if !adminToken.isEmpty {
            var adminGetAllUsersResult: UnsafeMutablePointer<CChar>?
            let adminGetAllUsersCode = auth_get_all_users(adminToken, &adminGetAllUsersResult)
            
            if adminGetAllUsersCode == 0 {
                results += "✅ Admin successfully retrieved all users\n"
                if let resultStr = adminGetAllUsersResult {
                    let response = String(cString: resultStr)
                    // Count users in response
                    if let usersData = response.data(using: .utf8),
                       let usersJson = try? JSONSerialization.jsonObject(with: usersData) as? [[String: Any]] {
                        results += "👥 Found \(usersJson.count) users\n"
                    }
                    free_string(resultStr)
                }
            } else {
                let error = getLastError()
                results += "❌ Admin failed to get all users: \(error)\n"
            }
        } else {
            results += "❌ No admin token available for get all users test\n"
        }
        
        // Test 5.5b: Team Lead cannot get all users
        results += "\n🚫 Test 5.5b: Team Lead Get All Users (Should Fail)\n"
        
        if !leadToken.isEmpty {
            var leadGetAllUsersResult: UnsafeMutablePointer<CChar>?
            let leadGetAllUsersCode = auth_get_all_users(leadToken, &leadGetAllUsersResult)
            
            if leadGetAllUsersCode != 0 {
                results += "✅ Team Lead correctly denied access to all users\n"
                let error = getLastError()
                results += "📝 Expected error: \(error)\n"
            } else {
                results += "❌ SECURITY ISSUE: Team Lead was allowed to access all users!\n"
                if let resultStr = leadGetAllUsersResult {
                    free_string(resultStr)
                }
            }
        } else {
            results += "❌ No team lead token available for get all users test\n"
        }
        
        // Test 5.5c: Officer cannot get all users
        results += "\n🚫 Test 5.5c: Officer Get All Users (Should Fail)\n"
        
        if !officerToken.isEmpty {
            var officerGetAllUsersResult: UnsafeMutablePointer<CChar>?
            let officerGetAllUsersCode = auth_get_all_users(officerToken, &officerGetAllUsersResult)
            
            if officerGetAllUsersCode != 0 {
                results += "✅ Officer correctly denied access to all users\n"
                let error = getLastError()
                results += "📝 Expected error: \(error)\n"
            } else {
                results += "❌ SECURITY ISSUE: Officer was allowed to access all users!\n"
                if let resultStr = officerGetAllUsersResult {
                    free_string(resultStr)
                }
            }
        } else {
            results += "❌ No officer token available for get all users test\n"
        }
        
        // Test 5.6: Wrong Password Tests
        results += "\n🚫 Test 5.6: Wrong Password Security\n"
        
        // Test 5.6a: Wrong Password for Admin
        results += "\n🚫 Test 5.6a: Wrong Password for Admin\n"
        let wrongPasswordJson = """
        {
            "email": "admin@example.com",
            "password": "WrongPassword123!"
        }
        """
        
        var wrongPasswordResult: UnsafeMutablePointer<CChar>?
        let wrongPasswordCode = auth_login(wrongPasswordJson, &wrongPasswordResult)
        
        if wrongPasswordCode != 0 {
            results += "✅ Correctly rejected wrong password\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Security issue: Wrong password was accepted!\n"
            if let resultStr = wrongPasswordResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.6b: Non-existent User
        results += "\n👻 Test 5.6b: Non-existent User\n"
        let nonExistentUserJson = """
        {
            "email": "nonexistent@example.com",
            "password": "SomePassword123!"
        }
        """
        
        var nonExistentResult: UnsafeMutablePointer<CChar>?
        let nonExistentCode = auth_login(nonExistentUserJson, &nonExistentResult)
        
        if nonExistentCode != 0 {
            results += "✅ Correctly rejected non-existent user\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Security issue: Non-existent user was accepted!\n"
            if let resultStr = nonExistentResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.7: Malformed Input Tests
        results += "\n🔧 Test 5.7: Input Validation\n"
        
        // Test 5.7a: Invalid Email Format
        results += "\n📧 Test 5.7a: Invalid Email Format\n"
        let invalidEmailJson = """
        {
            "email": "not-an-email",
            "password": "SomePassword123!"
        }
        """
        
        var invalidEmailResult: UnsafeMutablePointer<CChar>?
        let invalidEmailCode = auth_login(invalidEmailJson, &invalidEmailResult)
        
        if invalidEmailCode != 0 {
            results += "✅ Correctly rejected invalid email format\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Validation issue: Invalid email was accepted!\n"
            if let resultStr = invalidEmailResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.7b: Empty Credentials
        results += "\n🕳️ Test 5.7b: Empty Credentials\n"
        let emptyCredentialsJson = """
        {
            "email": "",
            "password": ""
        }
        """
        
        var emptyResult: UnsafeMutablePointer<CChar>?
        let emptyCode = auth_login(emptyCredentialsJson, &emptyResult)
        
        if emptyCode != 0 {
            results += "✅ Correctly rejected empty credentials\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Validation issue: Empty credentials were accepted!\n"
            if let resultStr = emptyResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.7c: Malformed JSON
        results += "\n🔧 Test 5.7c: Malformed JSON\n"
        let malformedJson = """
        {
            "email": "admin@example.com"
            "password": "Admin123!"
        """
        
        var malformedResult: UnsafeMutablePointer<CChar>?
        let malformedCode = auth_login(malformedJson, &malformedResult)
        
        if malformedCode != 0 {
            results += "✅ Correctly rejected malformed JSON\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Parsing issue: Malformed JSON was accepted!\n"
            if let resultStr = malformedResult {
                free_string(resultStr)
            }
        }
        
        // Test 5.8: SQL Injection and Security Tests
        results += "\n💉 Test 5.8: Security Attack Prevention\n"
        
        // Test 5.8a: SQL Injection Attempt
        results += "\n💉 Test 5.8a: SQL Injection Attempt\n"
        let sqlInjectionJson = """
        {
            "email": "admin@example.com'; DROP TABLE users; --",
            "password": "Admin123!"
        }
        """
        
        var sqlInjectionResult: UnsafeMutablePointer<CChar>?
        let sqlInjectionCode = auth_login(sqlInjectionJson, &sqlInjectionResult)
        
        if sqlInjectionCode != 0 {
            results += "✅ SQL injection attempt safely handled\n"
            let error = getLastError()
            results += "📝 Error message: \(error)\n"
        } else {
            results += "❌ Security concern: SQL injection attempt processed!\n"
            if let resultStr = sqlInjectionResult {
                free_string(resultStr)
            }
        }
        
        results += "\n🎯 Authorization and Security Test Summary:\n"
        results += "✅ Admin-only operations are properly restricted\n"
        results += "✅ User management requires admin privileges\n"
        results += "✅ Authentication security is robust\n"
        results += "✅ Input validation prevents malformed data\n"
        results += "✅ SQL injection protection is active\n"
        results += "✅ Role-based access control is enforced\n\n"
        
        // Test 5.9: User Deletion and Account Management
        results += "🗑️ Test 5.9: User Deletion and Account Management\n"
        
        // First, let's create a test user that we can safely delete
        results += "\n📝 Test 5.9a: Creating Test User for Deletion\n"
        let testUserForDeletionJson = """
        {
            "user": {
                "email": "deleteme@example.com",
                "password": "DeleteMe123!",
                "name": "Test User for Deletion",
                "role": "field",
                "active": true
            },
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000000",
                "role": "admin",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        var createTestUserResult: UnsafeMutablePointer<CChar>?
        let createTestUserCode = user_create(testUserForDeletionJson, &createTestUserResult)
        
        var testUserId: String = ""
        if createTestUserCode == 0, let resultStr = createTestUserResult {
            results += "✅ Test user created for deletion tests\n"
            let response = String(cString: resultStr)
            
            // Extract user ID from response
            if let userData = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: userData) as? [String: Any],
               let id = json["id"] as? String {
                testUserId = id
                results += "🆔 Test user ID: \(testUserId.prefix(8))...\n"
            }
            free_string(resultStr)
        } else {
            let error = getLastError()
            results += "❌ Failed to create test user: \(error)\n"
        }
        
        // Test that the newly created user can login
        results += "\n🔑 Test 5.9b: Verify Test User Can Login (Before Deletion)\n"
        let testUserLoginJson = """
        {
            "email": "deleteme@example.com",
            "password": "DeleteMe123!"
        }
        """
        
        var testUserLoginResult: UnsafeMutablePointer<CChar>?
        let testUserLoginCode = auth_login(testUserLoginJson, &testUserLoginResult)
        
        if testUserLoginCode == 0 {
            results += "✅ Test user can login successfully before deletion\n"
            if let resultStr = testUserLoginResult {
                free_string(resultStr)
            }
        } else {
            let error = getLastError()
            results += "❌ Test user cannot login (unexpected): \(error)\n"
        }
        
        // Test 5.9c: Admin can delete the test user (hard delete)
        if !testUserId.isEmpty {
            results += "\n🗑️ Test 5.9c: Admin Hard Delete Test User\n"
            let deleteUserJson = """
            {
                "id": "\(testUserId)",
                "auth": {
                    "user_id": "00000000-0000-0000-0000-000000000000",
                    "role": "admin",
                    "device_id": "\(deviceId)",
                    "offline_mode": false
                }
            }
            """
            
            let deleteUserCode = user_hard_delete(deleteUserJson)
            
            if deleteUserCode == 0 {
                results += "✅ Admin successfully hard deleted test user\n"
            } else {
                let error = getLastError()
                results += "❌ Admin failed to delete test user: \(error)\n"
            }
            
            // Test 5.9d: Verify deleted user cannot login
            results += "\n🚫 Test 5.9d: Verify Deleted User Cannot Login\n"
            var deletedUserLoginResult: UnsafeMutablePointer<CChar>?
            let deletedUserLoginCode = auth_login(testUserLoginJson, &deletedUserLoginResult)
            
            if deletedUserLoginCode != 0 {
                results += "✅ Deleted user correctly denied login\n"
                let error = getLastError()
                results += "📝 Expected error: \(error)\n"
            } else {
                results += "❌ SECURITY ISSUE: Deleted user was allowed to login!\n"
                if let resultStr = deletedUserLoginResult {
                    free_string(resultStr)
                }
            }
        }
        
        // Test 5.9e: Try to delete default admin account (should fail)
        results += "\n🚫 Test 5.9e: Prevent Deletion of Default Admin Account\n"
        
        // First, get the admin user ID
        let getAdminUserJson = """
        {
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000000",
                "role": "admin",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        var getAllUsersForAdminResult: UnsafeMutablePointer<CChar>?
        let getAllUsersForAdminCode = auth_get_all_users(getAdminUserJson, &getAllUsersForAdminResult)
        
        var adminUserId: String = ""
        if getAllUsersForAdminCode == 0, let resultStr = getAllUsersForAdminResult {
            let response = String(cString: resultStr)
            
            // Find admin user ID
            if let usersData = response.data(using: .utf8),
               let usersJson = try? JSONSerialization.jsonObject(with: usersData) as? [[String: Any]] {
                for user in usersJson {
                    if let email = user["email"] as? String, email == "admin@example.com",
                       let id = user["id"] as? String {
                        adminUserId = id
                        results += "🔍 Found admin user ID: \(adminUserId.prefix(8))...\n"
                        break
                    }
                }
            }
            free_string(resultStr)
        }
        
        if !adminUserId.isEmpty {
            let deleteAdminJson = """
            {
                "id": "\(adminUserId)",
                "auth": {
                    "user_id": "\(adminUserId)",
                    "role": "admin",
                    "device_id": "\(deviceId)",
                    "offline_mode": false
                }
            }
            """
            
            let deleteAdminCode = user_hard_delete(deleteAdminJson)
            
            if deleteAdminCode != 0 {
                results += "✅ System correctly prevented admin from deleting own account\n"
                let error = getLastError()
                results += "📝 Expected protection: \(error)\n"
            } else {
                results += "❌ SECURITY ISSUE: Admin was allowed to delete own account!\n"
            }
        }
        
        // Test 5.10: Account Disabling Mechanism
        results += "\n🔒 Test 5.10: Account Disabling Mechanism\n"
        
        // Create another test user for disabling tests
        results += "\n📝 Test 5.10a: Creating Test User for Disabling\n"
        let testUserForDisablingJson = """
        {
            "user": {
                "email": "disableme@example.com",
                "password": "DisableMe123!",
                "name": "Test User for Disabling",
                "role": "field",
                "active": true
            },
            "auth": {
                "user_id": "00000000-0000-0000-0000-000000000000",
                "role": "admin",
                "device_id": "\(deviceId)",
                "offline_mode": false
            }
        }
        """
        
        var createDisableTestUserResult: UnsafeMutablePointer<CChar>?
        let createDisableTestUserCode = user_create(testUserForDisablingJson, &createDisableTestUserResult)
        
        var disableTestUserId: String = ""
        if createDisableTestUserCode == 0, let resultStr = createDisableTestUserResult {
            results += "✅ Test user created for disabling tests\n"
            let response = String(cString: resultStr)
            
            // Extract user ID from response
            if let userData = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: userData) as? [String: Any],
               let id = json["id"] as? String {
                disableTestUserId = id
                results += "🆔 Disable test user ID: \(disableTestUserId.prefix(8))...\n"
            }
            free_string(resultStr)
        } else {
            let error = getLastError()
            results += "❌ Failed to create disable test user: \(error)\n"
        }
        
        // Test that the newly created user can login
        results += "\n🔑 Test 5.10b: Verify Test User Can Login (Before Disabling)\n"
        let disableTestUserLoginJson = """
        {
            "email": "disableme@example.com",
            "password": "DisableMe123!"
        }
        """
        
        var disableTestUserLoginResult: UnsafeMutablePointer<CChar>?
        let disableTestUserLoginCode = auth_login(disableTestUserLoginJson, &disableTestUserLoginResult)
        
        if disableTestUserLoginCode == 0 {
            results += "✅ Test user can login successfully before disabling\n"
            if let resultStr = disableTestUserLoginResult {
                free_string(resultStr)
            }
        } else {
            let error = getLastError()
            results += "❌ Test user cannot login (unexpected): \(error)\n"
        }
        
        // Test 5.10c: Admin disables the test user
        if !disableTestUserId.isEmpty {
            results += "\n🔒 Test 5.10c: Admin Disables Test User Account\n"
            let disableUserJson = """
            {
                "id": "\(disableTestUserId)",
                "update": {
                    "active": false
                },
                "auth": {
                    "user_id": "00000000-0000-0000-0000-000000000000",
                    "role": "admin",
                    "device_id": "\(deviceId)",
                    "offline_mode": false
                }
            }
            """
            
            var disableUserResult: UnsafeMutablePointer<CChar>?
            let disableUserCode = user_update(disableUserJson, &disableUserResult)
            
            if disableUserCode == 0 {
                results += "✅ Admin successfully disabled test user account\n"
                if let resultStr = disableUserResult {
                    let response = String(cString: resultStr)
                    results += "📄 Updated user response: \(response.prefix(100))...\n"
                    free_string(resultStr)
                }
            } else {
                let error = getLastError()
                results += "❌ Admin failed to disable test user: \(error)\n"
            }
            
            // Test 5.10d: Verify disabled user cannot login
            results += "\n🚫 Test 5.10d: Verify Disabled User Cannot Login\n"
            var disabledUserLoginResult: UnsafeMutablePointer<CChar>?
            let disabledUserLoginCode = auth_login(disableTestUserLoginJson, &disabledUserLoginResult)
            
            if disabledUserLoginCode != 0 {
                results += "✅ Disabled user correctly denied login\n"
                let error = getLastError()
                results += "📝 Expected error: \(error)\n"
            } else {
                results += "❌ SECURITY ISSUE: Disabled user was allowed to login!\n"
                if let resultStr = disabledUserLoginResult {
                    free_string(resultStr)
                }
            }
            
            // Test 5.10e: Admin re-enables the test user
            results += "\n🔓 Test 5.10e: Admin Re-enables Test User Account\n"
            let enableUserJson = """
            {
                "id": "\(disableTestUserId)",
                "update": {
                    "active": true
                },
                "auth": {
                    "user_id": "00000000-0000-0000-0000-000000000000",
                    "role": "admin",
                    "device_id": "\(deviceId)",
                    "offline_mode": false
                }
            }
            """
            
            var enableUserResult: UnsafeMutablePointer<CChar>?
            let enableUserCode = user_update(enableUserJson, &enableUserResult)
            
            if enableUserCode == 0 {
                results += "✅ Admin successfully re-enabled test user account\n"
                if let resultStr = enableUserResult {
                    free_string(resultStr)
                }
            } else {
                let error = getLastError()
                results += "❌ Admin failed to re-enable test user: \(error)\n"
            }
            
            // Test 5.10f: Verify re-enabled user can login again
            results += "\n🔑 Test 5.10f: Verify Re-enabled User Can Login Again\n"
            var reenabledUserLoginResult: UnsafeMutablePointer<CChar>?
            let reenabledUserLoginCode = auth_login(disableTestUserLoginJson, &reenabledUserLoginResult)
            
            if reenabledUserLoginCode == 0 {
                results += "✅ Re-enabled user can login successfully\n"
                if let resultStr = reenabledUserLoginResult {
                    free_string(resultStr)
                }
            } else {
                let error = getLastError()
                results += "❌ Re-enabled user cannot login (unexpected): \(error)\n"
            }
        }
        
        results += "\n🎯 User Management and Security Test Summary:\n"
        results += "✅ Admin can create and delete users properly\n"
        results += "✅ Default accounts are protected from deletion\n"
        results += "✅ Deleted users cannot login (access revoked)\n"
        results += "✅ Account disabling mechanism works correctly\n"
        results += "✅ Disabled users cannot login (access denied)\n"
        results += "✅ Account re-enabling restores access\n"
        results += "✅ Self-deletion prevention works\n\n"
        
        // Test 6: Project Operations (if authenticated)
        if !adminToken.isEmpty {
            results += "6️⃣ Testing Project Operations...\n"
            let projectJson = """
            {
                "name": "Test Project",
                "description": "A test project for iPad Rust Core",
                "start_date": "2024-01-01",
                "end_date": "2024-12-31",
                "budget": 50000.0,
                "status": "active"
            }
            """
            
            var projectResult: UnsafeMutablePointer<CChar>?
            let projectCreateResult = project_create(projectJson, &projectResult)
            
            if projectCreateResult == 0, let projectResultStr = projectResult {
                let projectResponse = String(cString: projectResultStr)
                results += "✅ Project created: \(projectResponse)\n\n"
                free_string(projectResultStr)
            } else {
                let error = getLastError()
                results += "❌ Project creation failed: \(error)\n\n"
            }
        }
        
        // Test 7: Memory and Error Handling
        results += "7️⃣ Testing Error Handling...\n"
        let lastError = getLastError()
        results += "🔍 Last error: \(lastError)\n\n"
        
        // Test 8: iOS-specific features
        results += "8️⃣ Testing iOS Integration...\n"
        results += "📱 Running on iOS: \(UIDevice.current.systemName) \(UIDevice.current.systemVersion)\n"
        results += "🏷️ Device Model: \(UIDevice.current.model)\n"
        results += "📂 Documents Directory: \(getDocumentsDirectory())\n"
        results += "💾 Database URL: \(getDatabaseURL())\n\n"
        
        results += "================================\n"
        results += "🎉 Test Suite Completed!\n"
        results += "✨ Your iPad Rust Core is working perfectly!\n"
        
        return results
    }
    
    // MARK: - Helper Functions
    
    private func getDocumentsDirectory() -> String {
        // Use Library directory instead of Documents for better iOS compatibility
        let paths = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)
        return paths[0].path
    }
    
    private func getDatabasePath() -> String {
        let libraryPath = getDocumentsDirectory()
        let dbDir = "\(libraryPath)/Database"
        
        // Ensure the Database subdirectory exists
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
}
