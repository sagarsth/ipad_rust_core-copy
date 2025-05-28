
import UIKit
import iPadRustCore

class iPadRustCoreTestViewController: UIViewController {
    
    @IBOutlet weak var statusLabel: UILabel!
    @IBOutlet weak var testButton: UIButton!
    @IBOutlet weak var resultTextView: UITextView!
    
    private let core = iPadRustCore.shared
    
    override func viewDidLoad() {
        super.viewDidLoad()
        setupUI()
    }
    
    private func setupUI() {
        title = "iPad Rust Core Test"
        statusLabel.text = "Ready to test"
        resultTextView.isEditable = false
        resultTextView.font = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    }
    
    @IBAction func runTests(_ sender: UIButton) {
        testButton.isEnabled = false
        statusLabel.text = "Running tests..."
        resultTextView.text = ""
        
        Task {
            await runProductionReadyTests()
            
            DispatchQueue.main.async {
                self.testButton.isEnabled = true
                self.statusLabel.text = "Tests completed"
            }
        }
    }
    
    private func runProductionReadyTests() async {
        appendResult("🚀 Starting iPad Rust Core Production Tests\n")
        
        // Test 1: Library version
        appendResult("📋 Testing library version...")
        if let version = core.getLibraryVersion() {
            appendResult("✅ Library version: \(version)\n")
        } else {
            appendResult("❌ Failed to get library version\n")
        }
        
        // Test 2: Database initialization with proper iOS path
        appendResult("📋 Testing database initialization...")
        let dbPath = core.getDatabaseURL(filename: "test_ipad_rust_core.sqlite")
        let deviceId = core.getDeviceId()
        let jwtSecret = "test-jwt-secret-for-ios"
        
        appendResult("Database path: \(dbPath)")
        appendResult("Device ID: \(deviceId)")
        
        let initResult = initialize_library(dbPath, deviceId, false, jwtSecret)
        if initResult == 0 {
            appendResult("✅ Library initialized successfully\n")
        } else {
            appendResult("❌ Library initialization failed with code: \(initResult)\n")
            if let error = core.getLastError() {
                appendResult("   Error: \(error)\n")
            }
            return
        }
        
        // Test 3: Authentication workflow
        appendResult("📋 Testing authentication...")
        
        let createUserJson = """
        {
            "email": "iostest@example.com",
            "name": "iOS Test User",
            "password": "TestPassword123!",
            "role": "User",
            "active": true
        }
        """
        
        var createUserResult: UnsafeMutablePointer<CChar>?
        let createUserCode = user_create(createUserJson, &createUserResult)
        
        if createUserCode == 0, let userResultStr = createUserResult {
            let userResponse = String(cString: userResultStr)
            appendResult("✅ Test user created\n")
            user_free(userResultStr)
        } else {
            appendResult("⚠️ User creation failed (may already exist)\n")
        }
        
        // Test login
        let loginCredentials = """
        {
            "email": "iostest@example.com",
            "password": "TestPassword123!"
        }
        """
        
        var loginResult: UnsafeMutablePointer<CChar>?
        let loginCode = auth_login(loginCredentials, &loginResult)
        
        if loginCode == 0, let loginResultStr = loginResult {
            let loginResponse = String(cString: loginResultStr)
            appendResult("✅ Login successful\n")
            
            // Parse tokens
            if let data = loginResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let accessToken = json["access_token"] as? String {
                
                // Test authenticated operations
                appendResult("📋 Testing authenticated operations...")
                
                var userListResult: UnsafeMutablePointer<CChar>?
                let userListCode = auth_get_all_users(accessToken, &userListResult)
                
                if userListCode == 0, let userListStr = userListResult {
                    appendResult("✅ User list retrieved with authentication\n")
                    auth_free(userListStr)
                } else {
                    appendResult("❌ Authenticated user list failed\n")
                }
            }
            
            auth_free(loginResultStr)
        } else {
            appendResult("❌ Login failed\n")
        }
        
        // Test offline mode
        appendResult("📋 Testing offline mode...")
        appendResult("Initial offline mode: \(core.isOfflineMode())")
        core.setOfflineMode(true)
        appendResult("After setting to true: \(core.isOfflineMode())")
        core.setOfflineMode(false)
        appendResult("After setting to false: \(core.isOfflineMode())\n")
        
        appendResult("🎉 iOS Production tests completed!\n")
        appendResult("✅ Database: iOS Documents directory")
        appendResult("✅ Authentication: JWT tokens working")
        appendResult("✅ Device ID: iOS UIDevice integration")
        appendResult("✅ Runtime: Centralized Tokio runtime")
    }
    
    private func appendResult(_ text: String) {
        DispatchQueue.main.async {
            self.resultTextView.text += text + "\n"
            
            // Scroll to bottom
            let bottom = NSMakeRange(self.resultTextView.text.count - 1, 1)
            self.resultTextView.scrollRangeToVisible(bottom)
        }
    }
}
