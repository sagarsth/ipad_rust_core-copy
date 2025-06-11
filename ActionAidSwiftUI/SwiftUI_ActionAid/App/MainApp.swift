//
//  MainApp.swift
//  ActionAid SwiftUI
//
//  App entry point with persistent authentication
//

import SwiftUI
import Foundation

@main
struct ActionAidApp: App {
    @StateObject private var authManager = AuthenticationManager.shared
    @State private var isInitialized = false
    @State private var initError: String?
    
    var body: some Scene {
        WindowGroup {
            if !isInitialized {
                InitializationView(
                    isInitialized: $isInitialized,
                    initError: $initError
                )
                .environmentObject(authManager)
                .onAppear {
                    // Ensure initialization happens immediately when app starts
                    Task {
                        await performInitialization()
                    }
                }
            } else if authManager.isAuthenticated {
                MainTabView()
                    .environmentObject(authManager)
            } else {
                LoginView()
                    .environmentObject(authManager)
            }
        }
    }
    
    private func performInitialization() async {
        // Prevent multiple initializations
        guard !isInitialized else { return }
        
        print("🚀 [APP] Starting application initialization...")
        
        // Check if already initialized (from previous app launch)
        let dbPath = getDatabasePath()
        let fileExists = FileManager.default.fileExists(atPath: dbPath)
        
        if fileExists {
            print("📂 [APP] Database file exists, attempting quick verification...")
            // Quick verification - try to access the auth service directly
            // Try a minimal FFI call to verify Rust state
            let testResult = auth_initialize_default_accounts("verification_check")
            if testResult == 0 {
                print("✅ [APP] Existing initialization verified, proceeding...")
                await MainActor.run {
                    self.isInitialized = true
                }
                return
            } else {
                print("⚠️ [APP] Verification failed, will re-initialize")
            }
        }
        
        print("🔧 [APP] Performing full initialization...")
        
        // Set storage path first
        let documentsPath = getDocumentsDirectory()
        let storagePath = "\(documentsPath)/ActionAid/storage"
        
        do {
            try FileManager.default.createDirectory(
                atPath: storagePath,
                withIntermediateDirectories: true,
                attributes: nil
            )
            print("📁 [APP] Storage directory ready: \(storagePath)")
        } catch {
            print("❌ [APP] Failed to create storage directory: \(error)")
            await MainActor.run {
                self.initError = "Failed to create storage directory: \(error.localizedDescription)"
            }
            return
        }
        
        // Set iOS storage path for Rust
        let storageSetResult = set_ios_storage_path(storagePath)
        if storageSetResult != 0 {
            print("❌ [APP] Failed to set storage path, code: \(storageSetResult)")
            await MainActor.run {
                self.initError = "Failed to configure storage path (code: \(storageSetResult))"
            }
            return
        }
        print("✅ [APP] Storage path configured")
        
        // Ensure database directory exists
        let dbDirectory = (dbPath as NSString).deletingLastPathComponent
        do {
            try FileManager.default.createDirectory(
                atPath: dbDirectory,
                withIntermediateDirectories: true,
                attributes: nil
            )
            print("✅ [APP] Database directory ready")
        } catch {
            print("❌ [APP] Failed to create database directory: \(error)")
            await MainActor.run {
                self.initError = "Failed to create database directory: \(error.localizedDescription)"
            }
            return
        }
        
        // Initialize Rust library
        let deviceId = authManager.getDeviceId()
        let jwtSecret = "production_jwt_secret_\(deviceId.prefix(8))"
        let sqliteUrl = "sqlite://\(dbPath)?mode=rwc"
        
        print("🔗 [APP] Initializing Rust library...")
        print("    Database: \(sqliteUrl)")
        print("    Device ID: \(deviceId)")
        
        let initResult = initialize_library(sqliteUrl, deviceId, false, jwtSecret)
        if initResult != 0 {
            print("❌ [APP] Library initialization failed, code: \(initResult)")
            
            // Get detailed error from Rust
            var errorDetails = "Unknown error"
            if let errorPtr = get_last_error() {
                errorDetails = String(cString: errorPtr)
                free_string(errorPtr)
            }
            
            await MainActor.run {
                self.initError = "Failed to initialize library (code: \(initResult))\n\(errorDetails)"
            }
            return
        }
        print("✅ [APP] Rust library initialized successfully")
        
        // Verify AuthService is available by testing it
        print("🔍 [APP] Verifying AuthService availability...")
        let verifyResult = auth_initialize_default_accounts("init_setup")
        if verifyResult != 0 {
            print("❌ [APP] AuthService verification failed, code: \(verifyResult)")
            
            var errorDetails = "AuthService not available"
            if let errorPtr = get_last_error() {
                errorDetails = String(cString: errorPtr)
                free_string(errorPtr)
            }
            
            await MainActor.run {
                self.initError = "AuthService initialization failed (code: \(verifyResult))\n\(errorDetails)"
            }
            return
        }
        print("✅ [APP] AuthService verified and working")
        
        // Initialize test data
        print("🧪 [APP] Setting up test data...")
        let testDataResult = auth_initialize_test_data("init_setup")
        if testDataResult != 0 {
            print("⚠️ [APP] Test data setup failed, code: \(testDataResult) (non-critical)")
        } else {
            print("✅ [APP] Test data initialized")
        }
        
        // Final verification - try a more complex operation
        print("🔬 [APP] Final verification test...")
        let testCredentials = """
        {
            "email": "admin@example.com",
            "password": "Admin123!"
        }
        """
        
        var testLoginResult: UnsafeMutablePointer<CChar>?
        let testLoginCode = auth_login(testCredentials, &testLoginResult)
        
        if testLoginCode == 0 && testLoginResult != nil {
            print("✅ [APP] Final verification successful - AuthService fully operational")
            // Free the test result
            auth_free(testLoginResult)
        } else {
            print("❌ [APP] Final verification failed - AuthService not working properly")
            
            var errorDetails = "Authentication test failed"
            if let errorPtr = get_last_error() {
                errorDetails = String(cString: errorPtr)
                free_string(errorPtr)
            }
            
            await MainActor.run {
                self.initError = "Authentication system not working: \(errorDetails)"
            }
            return
        }
        
        print("🎉 [APP] Initialization completed successfully!")
        await MainActor.run {
            self.isInitialized = true
        }
    }
    
    private func getDatabasePath() -> String {
        let documentsPath = getDocumentsDirectory()
        let dbDir = "\(documentsPath)/ActionAid"
        return "\(dbDir)/actionaid_core.sqlite"
    }
    
    private func getDocumentsDirectory() -> String {
        let paths = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
        return paths[0].path
    }
}

// MARK: - Authentication Manager
class AuthenticationManager: ObservableObject {
    static let shared = AuthenticationManager()
    
    @Published var currentUser: AuthenticatedUser?
    @Published var isAuthenticated: Bool = false
    
    struct AuthenticatedUser: Codable {
        let userId: String
        let email: String
        let role: String
        let token: String
        let loginTime: Date
    }
    
    private let userDefaultsKey = "ActionAidAuthenticatedUser"
    
    private init() {
        loadStoredUser()
    }
    
    func login(userId: String, email: String, role: String, token: String) {
        let user = AuthenticatedUser(
            userId: userId,
            email: email,
            role: role,
            token: token,
            loginTime: Date()
        )
        
        currentUser = user
        isAuthenticated = true
        
        // Store in UserDefaults
        if let encoded = try? JSONEncoder().encode(user) {
            UserDefaults.standard.set(encoded, forKey: userDefaultsKey)
        }
    }
    
    func logout() {
        currentUser = nil
        isAuthenticated = false
        UserDefaults.standard.removeObject(forKey: userDefaultsKey)
    }
    
    private func loadStoredUser() {
        if let data = UserDefaults.standard.data(forKey: userDefaultsKey),
           let user = try? JSONDecoder().decode(AuthenticatedUser.self, from: data) {
            
            // Check if token is still valid (e.g., not older than 30 days)
            let daysSinceLogin = Calendar.current.dateComponents([.day], from: user.loginTime, to: Date()).day ?? 0
            if daysSinceLogin < 30 {
                // Validate token with backend to ensure user is still active
                validateStoredToken(user)
            } else {
                // Token expired, clear it
                logout()
            }
        }
    }
    
    private func validateStoredToken(_ user: AuthenticatedUser) {
        Task {
            do {
                let authHandler = AuthFFIHandler()
                let currentUserResult = try await authHandler.getCurrentUser(token: user.token).get()
                
                await MainActor.run {
                    if currentUserResult.active {
                        // User is still active, keep them logged in
                        self.currentUser = user
                        self.isAuthenticated = true
                        print("✅ Stored token validated - user is active")
                    } else {
                        // User has been deactivated, force logout
                        print("❌ User has been deactivated - forcing logout")
                        self.logout()
                    }
                }
            } catch {
                await MainActor.run {
                    // Token validation failed, force logout
                    print("❌ Token validation failed - forcing logout: \(error)")
                    self.logout()
                }
            }
        }
    }
    
    func getAuthContext() -> [String: Any] {
        guard let user = currentUser else {
            return [:]
        }
        return [
            "user_id": user.userId,
            "role": user.role,
            "device_id": getDeviceId(),
            "offline_mode": false
        ]
    }
    
    func getAuthContextJSON() -> String {
        let context = getAuthContext()
        if let data = try? JSONSerialization.data(withJSONObject: context),
           let json = String(data: data, encoding: .utf8) {
            return json
        }
        return "{}"
    }
    
    func getDeviceId() -> String {
        return UIDevice.current.identifierForVendor?.uuidString ?? "unknown-device"
    }
}

// MARK: - Initialization View
struct InitializationView: View {
    @Binding var isInitialized: Bool
    @Binding var initError: String?
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var initProgress: String = "Initializing system..."
    
    var body: some View {
        VStack(spacing: 30) {
            Image(systemName: "heart.circle.fill")
                .font(.system(size: 80))
                .foregroundColor(.red)
            
            Text("ActionAid")
                .font(.largeTitle)
                .fontWeight(.bold)
            
            if let error = initError {
                VStack(spacing: 15) {
                    Text("Initialization Error")
                        .font(.headline)
                        .foregroundColor(.red)
                    
                    ScrollView {
                        Text(error)
                            .font(.caption)
                            .multilineTextAlignment(.leading)
                            .padding()
                    }
                    .frame(maxHeight: 200)
                    
                    Button("Retry") {
                        initError = nil
                        Task {
                            // Force re-initialization
                            isInitialized = false
                        }
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding()
            } else {
                VStack(spacing: 15) {
                    ProgressView()
                        .scaleEffect(1.2)
                    Text(initProgress)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding()
    }
}

// MARK: - Login View
struct LoginView: View {
    @EnvironmentObject var authManager: AuthenticationManager
    @State private var email = ""
    @State private var password = ""
    @State private var isLoading = false
    @State private var errorMessage: String?
    
    var body: some View {
        VStack(spacing: 0) {
            // Header
            VStack(spacing: 20) {
                Image(systemName: "heart.circle.fill")
                    .font(.system(size: 80))
                    .foregroundColor(.red)
                
                Text("ActionAid")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text("Sign in to your account")
                    .font(.headline)
                    .foregroundColor(.secondary)
            }
            .padding(.top, 60)
            .padding(.bottom, 40)
            
            // Login Form
            VStack(spacing: 20) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Email")
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundColor(.secondary)
                    
                    TextField("admin@example.com", text: $email)
                        .textFieldStyle(.roundedBorder)
                        .keyboardType(.emailAddress)
                        .autocapitalization(.none)
                        .autocorrectionDisabled()
                }
                
                VStack(alignment: .leading, spacing: 8) {
                    Text("Password")
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundColor(.secondary)
                    
                    SecureField("Enter password", text: $password)
                        .textFieldStyle(.roundedBorder)
                }
                
                if let error = errorMessage {
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }
                
                Button(action: login) {
                    if isLoading {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle(tint: .white))
                            .scaleEffect(0.8)
                    } else {
                        Text("Sign In")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(Color.blue)
                .foregroundColor(.white)
                .cornerRadius(10)
                .disabled(isLoading || email.isEmpty || password.isEmpty)
                .opacity((isLoading || email.isEmpty || password.isEmpty) ? 0.6 : 1.0)
            }
            .padding(.horizontal, 30)
            
            Spacer()
            
            // Default Credentials Hint
            VStack(spacing: 8) {
                Text("Default Credentials")
                    .font(.caption2)
                    .fontWeight(.medium)
                    .foregroundColor(.secondary)
                
                VStack(alignment: .leading, spacing: 4) {
                    Text("Admin: admin@example.com / Admin123!")
                        .font(.caption2)
                    Text("Manager: lead@example.com / Lead123!")
                        .font(.caption2)
                    Text("User: officer@example.com / Officer123!")
                        .font(.caption2)
                }
                .foregroundColor(.secondary)
            }
            .padding()
            .background(Color.gray.opacity(0.1))
            .cornerRadius(8)
            .padding(.bottom, 30)
            .padding(.horizontal, 30)
        }
        .onAppear {
            // Pre-fill with admin credentials for easier testing
            email = "admin@example.com"
            password = "Admin123!"
        }
    }
    
    private func login() {
        isLoading = true
        errorMessage = nil
        
        Task {
            let loginPayload = """
            {
                "email": "\(email)",
                "password": "\(password)"
            }
            """
            
            var result: UnsafeMutablePointer<CChar>?
            let status = auth_login(loginPayload, &result)
            
            await MainActor.run {
                isLoading = false
                
                if status == 0, let resultStr = result {
                    let response = String(cString: resultStr)
                    defer { free_string(resultStr) }
                    
                    if let data = response.data(using: .utf8),
                       let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                       let token = json["access_token"] as? String,
                       let userId = json["user_id"] as? String,
                       let role = json["role"] as? String {
                        
                        authManager.login(
                            userId: userId,
                            email: email,
                            role: role,
                            token: token
                        )
                    } else {
                        errorMessage = "Invalid response from server"
                    }
                } else {
                    if let errorPtr = get_last_error() {
                        let error = String(cString: errorPtr)
                        free_string(errorPtr)
                        errorMessage = error
                    } else {
                        errorMessage = "Login failed. Please check your credentials."
                    }
                }
            }
        }
    }
}
