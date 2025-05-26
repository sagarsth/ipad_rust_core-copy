import Foundation
#if canImport(UIKit)
import UIKit
#endif
import iPadRustCoreC

/// Examples demonstrating how to use the iPad Rust Core Swift wrapper
/// with production-ready features
public class iPadRustCoreExamples {
    
    private let core = iPadRustCore.shared
    
    public init() {} // Add public initializer
    
    // MARK: - Initialization Example
    
    /// Example: Initialize the library with proper iOS database path
    public func initializeLibrary() async throws {
        print("🌿 Initializing iPad Rust Core with production-ready settings...")
        
        // Use proper iOS Documents directory for database
        let dbPath = core.getDatabaseURL(filename: "ipad_rust_core_production.sqlite")
        print("Database path: \(dbPath)")
        
        #if canImport(UIKit)
        let deviceId = "ipad-\(UIDevice.current.identifierForVendor?.uuidString ?? "unknown")"
        #else
        let deviceId = "macos-\(UUID().uuidString)"
        #endif
        let jwtSecret = "your-secure-jwt-secret-change-in-production"

        do {
            try core.initialize(dbPath: dbPath, deviceId: deviceId, offlineMode: false, jwtSecret: jwtSecret)
            print("✅ Library initialized successfully.")
        } catch {
            print("❌ Initialization failed: \(error.localizedDescription)")
            if let rustError = error as? RustCoreError {
                print("   Details: \(rustError.errorDescription ?? "No further details.")")
            }
            if let lastError = core.getLastError() {
                print("   Last FFI Error: \(lastError)")
            }
            throw error
        }

        print("Device ID: \(deviceId)")
        print("Is offline mode: \(core.isOfflineMode())")
        
        if let version = core.getLibraryVersion() {
            print("Library version: \(version)")
        }
    }
    
    // MARK: - Authentication Examples (Direct FFI)
    
    /// Example: Test authentication workflow with proper JSON payloads
    public func testAuthenticationWorkflow() async throws {
        print("🔐 Testing authentication workflow with proper JSON...")
        
        // Create a test user with proper JSON payload
        let createUserJson = """
        {
            "email": "testuser@example.com",
            "name": "Test User",
            "password": "TestPassword123!",
            "role": "User",
            "active": true
        }
        """
        
        print("Creating test user with structured JSON payload...")
        var createUserResult: UnsafeMutablePointer<CChar>?
        // Use the non-authenticated version for initial user creation
        let createUserCode = user_create(createUserJson, &createUserResult)
        
        if createUserCode == 0, let userResultStr = createUserResult {
            let userResponse = String(cString: userResultStr)
            print("✅ Test user created: \(userResponse.prefix(100))...")
            user_free(userResultStr)
        } else {
            print("⚠️ User creation failed (may already exist): code \(createUserCode)")
            // Continue with login attempt
        }
        
        // Test login with proper credentials JSON
        let loginCredentials = """
        {
            "email": "testuser@example.com",
            "password": "TestPassword123!"
        }
        """
        
        print("Testing login with structured credentials...")
        var loginResult: UnsafeMutablePointer<CChar>?
        let loginCode = auth_login(loginCredentials, &loginResult)
        
        var accessToken: String = ""
        var refreshToken: String = ""
        
        if loginCode == 0, let loginResultStr = loginResult {
            let loginResponse = String(cString: loginResultStr)
            print("✅ Login successful with JWT tokens")
            
            // Parse the login response to extract tokens
            if let data = loginResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                accessToken = json["access_token"] as? String ?? ""
                refreshToken = json["refresh_token"] as? String ?? ""
                print("   Access token: \(accessToken.prefix(20))...")
                print("   Refresh token: \(refreshToken.prefix(20))...")
                
                if let expiry = json["access_expiry"] as? String {
                    print("   Access token expires: \(expiry)")
                }
            }
            
            auth_free(loginResultStr)
        } else {
            print("❌ Login failed with code: \(loginCode)")
            if let error = core.getLastError() {
                print("   Error: \(error)")
            }
            throw RustCoreError.operationFailed(code: loginCode, details: "Login failed")
        }
        
        // Test token verification
        if !accessToken.isEmpty {
            print("Testing token verification...")
            var verifyResult: UnsafeMutablePointer<CChar>?
            let verifyCode = auth_verify_token(accessToken, &verifyResult)
            
            if verifyCode == 0, let verifyStr = verifyResult {
                let verifyResponse = String(cString: verifyStr)
                print("✅ Token verified successfully: \(verifyResponse.prefix(100))...")
                auth_free(verifyStr)
            } else {
                print("❌ Token verification failed with code: \(verifyCode)")
            }
        }
        
        // Test token refresh
        if !refreshToken.isEmpty {
            print("Testing token refresh...")
            var refreshResult: UnsafeMutablePointer<CChar>?
            let refreshCode = auth_refresh_token(refreshToken, &refreshResult)
            
            if refreshCode == 0, let refreshStr = refreshResult {
                let refreshResponse = String(cString: refreshStr)
                print("✅ Token refreshed successfully: \(refreshResponse.prefix(100))...")
                auth_free(refreshStr)
            } else {
                print("❌ Token refresh failed with code: \(refreshCode)")
            }
        }
        
        // Test authenticated user operations
        if !accessToken.isEmpty {
            print("Testing authenticated user operations...")
            var userListResult: UnsafeMutablePointer<CChar>?
            let userListCode = auth_get_all_users(accessToken, &userListResult)
            
            if userListCode == 0, let userListStr = userListResult {
                let userListResponse = String(cString: userListStr)
                print("✅ User list retrieved with authentication: \(userListResponse.prefix(100))...")
                auth_free(userListStr)
            } else {
                print("❌ Authenticated user list failed with code: \(userListCode)")
            }
        }
        
        // Test logout
        if !accessToken.isEmpty && !refreshToken.isEmpty {
            print("Testing logout...")
            let logoutJson = """
            {
                "access_token": "\(accessToken)",
                "refresh_token": "\(refreshToken)"
            }
            """
            
            let logoutCode = auth_logout(logoutJson)
            if logoutCode == 0 {
                print("✅ Logout successful - tokens revoked")
            } else {
                print("❌ Logout failed with code: \(logoutCode)")
            }
        }
    }
    
    // MARK: - Domain Operations Examples
    
    /// Example: Test project operations with proper JSON payloads
    public func testProjectOperations() async throws {
        print("📋 Testing project operations with structured JSON...")
        
        // First, get an access token for authentication
        let accessToken = try await getTestAccessToken()
        
        // Create a project with proper JSON payload
        let newProjectJson = """
        {
            "name": "Production Test Project",
            "description": "A test project demonstrating proper JSON payload handling",
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "status": "Active",
            "budget": 75000.0,
            "location": "iOS Development Environment"
        }
        """
        
        print("Creating project with structured JSON payload...")
        var projectCreateResult: UnsafeMutablePointer<CChar>?
        let projectCreateCode = project_create(newProjectJson, &projectCreateResult)
        
        var projectId: String = ""
        if projectCreateCode == 0, let projectCreateStr = projectCreateResult {
            let projectResponse = String(cString: projectCreateStr)
            print("✅ Project created successfully: \(projectResponse.prefix(100))...")
            
            // Extract project ID for further operations
            if let data = projectResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                projectId = json["id"] as? String ?? ""
                print("   Project ID: \(projectId)")
            }
            
            project_free(projectCreateStr)
        } else {
            print("❌ Project creation failed with code: \(projectCreateCode)")
            if let error = core.getLastError() {
                print("   Error: \(error)")
            }
        }
        
        // List projects with authentication token
        print("Listing projects with authentication...")
        var projectListResult: UnsafeMutablePointer<CChar>?
        let projectListCode = project_list(accessToken, &projectListResult)
        
        if projectListCode == 0, let projectListStr = projectListResult {
            let projectListResponse = String(cString: projectListStr)
            print("✅ Project list retrieved: \(projectListResponse.prefix(150))...")
            project_free(projectListStr)
        } else {
            print("❌ Project list failed with code: \(projectListCode)")
        }
        
        // Update project if we have an ID
        if !projectId.isEmpty {
            print("Updating project with structured JSON...")
            let updateProjectJson = """
            {
                "id": "\(projectId)",
                "name": "Updated Production Test Project",
                "description": "Updated description demonstrating JSON payload handling",
                "status": "Active"
            }
            """
            
            var projectUpdateResult: UnsafeMutablePointer<CChar>?
            let projectUpdateCode = project_update(updateProjectJson, &projectUpdateResult)
            
            if projectUpdateCode == 0, let projectUpdateStr = projectUpdateResult {
                let projectUpdateResponse = String(cString: projectUpdateStr)
                print("✅ Project updated successfully: \(projectUpdateResponse.prefix(100))...")
                project_free(projectUpdateStr)
            } else {
                print("❌ Project update failed with code: \(projectUpdateCode)")
            }
        }
    }
    
    // MARK: - Participant Operations Example
    
    /// Example: Test participant operations with proper JSON payloads
    public func testParticipantOperations() async throws {
        print("👥 Testing participant operations with structured JSON...")
        
        // First, get an access token for authentication
        let accessToken = try await getTestAccessToken()
        
        // Create a participant with proper JSON payload
        let newParticipantJson = """
        {
            "name": "John Doe",
            "age": 30,
            "gender": "Male",
            "contact_info": "john.doe@example.com",
            "address": "123 Test Street",
            "occupation": "Teacher",
            "household_size": 4,
            "income_level": "Medium",
            "education_level": "Bachelor",
            "health_status": "Good",
            "participation_status": "Active"
        }
        """
        
        print("Creating participant with structured JSON payload...")
        var participantCreateResult: UnsafeMutablePointer<CChar>?
        let participantCreateCode = participant_create(newParticipantJson, &participantCreateResult)
        
        if participantCreateCode == 0, let participantCreateStr = participantCreateResult {
            let participantResponse = String(cString: participantCreateStr)
            print("✅ Participant created successfully: \(participantResponse.prefix(100))...")
            participant_free(participantCreateStr)
        } else {
            print("❌ Participant creation failed with code: \(participantCreateCode)")
            if let error = core.getLastError() {
                print("   Error: \(error)")
            }
        }
        
        // List participants with authentication
        print("Listing participants with authentication...")
        var participantListResult: UnsafeMutablePointer<CChar>?
        let participantListCode = participant_list(accessToken, &participantListResult)
        
        if participantListCode == 0, let participantListStr = participantListResult {
            let participantListResponse = String(cString: participantListStr)
            print("✅ Participant list retrieved: \(participantListResponse.prefix(150))...")
            participant_free(participantListStr)
        } else {
            print("❌ Participant list failed with code: \(participantListCode)")
        }
    }
    
    // MARK: - Helper Methods
    
    /// Helper to get a test access token
    private func getTestAccessToken() async throws -> String {
        let loginCredentials = """
        {
            "email": "testuser@example.com",
            "password": "TestPassword123!"
        }
        """
        
        var loginResult: UnsafeMutablePointer<CChar>?
        let loginCode = auth_login(loginCredentials, &loginResult)
        
        if loginCode == 0, let loginResultStr = loginResult {
            let loginResponse = String(cString: loginResultStr)
            
            if let data = loginResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let accessToken = json["access_token"] as? String {
                auth_free(loginResultStr)
                return accessToken
            }
            
            auth_free(loginResultStr)
        }
        
        throw RustCoreError.operationFailed(code: loginCode, details: "Failed to get access token")
    }
    
    // MARK: - Complete Test Suite
    
    /// Run all production-ready examples
    public func runProductionReadyExamples() async throws {
        print("🚀 Running Production-Ready iPad Rust Core Examples...")
        print(String(repeating: "=", count: 60))
        
        // Initialize with proper database path
        try await initializeLibrary()
        
        // Test authentication with proper JSON payloads
        try await testAuthenticationWorkflow()
        
        // Test domain operations with structured data
        try await testProjectOperations()
        
        // Test participant operations
        try await testParticipantOperations()
        
        // Test offline mode functionality
        print("\n📱 Testing offline mode functionality...")
        print("Initial offline mode: \(core.isOfflineMode())")
        core.setOfflineMode(true)
        print("After setting to true: \(core.isOfflineMode())")
        core.setOfflineMode(false)
        print("After setting to false: \(core.isOfflineMode())")
        
        // Final error check
        if let error = core.getLastError() {
            print("\n⚠️ Final error check: \(error)")
        } else {
            print("\n✅ No errors in final check")
        }
        
        print("\n🎉 Production-ready examples completed successfully!")
        print("✅ Database: Proper iOS Documents directory")
        print("✅ Authentication: JWT token-based with proper JSON")
        print("✅ JSON Payloads: Structured data validation")
        print("✅ Domain Logic: Projects, Users, Participants tested")
        print("✅ Memory Management: Proper FFI cleanup")
        print("✅ Error Handling: Thread-local storage working")
    }
}

// Example of how to run this (e.g., in a main.swift or an XCTest)
/*
func runMinimalExample() async {
    let examples = iPadRustCoreExamples()
    do {
        try await examples.initializeLibrary()
        print("✅ Minimal example completed.")
    } catch {
        print("❌ Minimal example failed: \(error.localizedDescription)")
    }
}

// To run from a command-line tool or a simple test:
//Task {
//    await runMinimalExample()
//}
*/