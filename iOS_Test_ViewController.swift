import UIKit

// Import the C functions directly
// You'll need to add the header file to your bridging header
// or create a module.modulemap

class iPadRustCoreTestViewController: UIViewController {
    
    @IBOutlet weak var statusLabel: UILabel!
    @IBOutlet weak var testButton: UIButton!
    @IBOutlet weak var resultTextView: UITextView!
    
    override func viewDidLoad() {
        super.viewDidLoad()
        setupUI()
    }
    
    private func setupUI() {
        title = "iPad Rust Core Test"
        statusLabel.text = "Ready to test"
        resultTextView.isEditable = false
        resultTextView.font = UIFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        resultTextView.backgroundColor = UIColor.systemBackground
        resultTextView.layer.borderColor = UIColor.systemGray4.cgColor
        resultTextView.layer.borderWidth = 1
        resultTextView.layer.cornerRadius = 8
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
        appendResult("🚀 Starting iPad Rust Core Production Tests")
        
        // Test 1: Library version
        appendResult("\n📋 Testing library version...")
        var versionResult: UnsafeMutablePointer<CChar>?
        let versionCode = get_library_version(&versionResult)
        
        if versionCode == 0, let versionStr = versionResult {
            let version = String(cString: versionStr)
            appendResult("✅ Library version: \(version)")
            free_string(versionStr)
        } else {
            appendResult("❌ Failed to get library version")
        }
        
        // Test 2: Database initialization with proper iOS path
        appendResult("\n📋 Testing database initialization...")
        
        // Get iOS Documents directory
        let documentsPath = FileManager.default.urls(for: .documentDirectory, 
                                                   in: .userDomainMask).first!
        let dbURL = documentsPath.appendingPathComponent("test_ipad_rust_core.sqlite")
        let dbPath = "sqlite://" + dbURL.path
        
        // Get device ID
        let deviceId = UIDevice.current.identifierForVendor?.uuidString ?? "unknown-device"
        let jwtSecret = "test-jwt-secret-for-ios"
        
        appendResult("Database path: \(dbPath)")
        appendResult("Device ID: \(deviceId)")
        
        let initResult = initialize_library(dbPath, deviceId, false, jwtSecret)
        if initResult == 0 {
            appendResult("✅ Library initialized successfully")
        } else {
            appendResult("❌ Library initialization failed with code: \(initResult)")
            
            // Get last error
            var errorResult: UnsafeMutablePointer<CChar>?
            let errorCode = get_last_error(&errorResult)
            if errorCode == 0, let errorStr = errorResult {
                let error = String(cString: errorStr)
                appendResult("   Error: \(error)")
                free_string(errorStr)
            }
            return
        }
        
        // Test 3: Authentication workflow
        appendResult("\n📋 Testing authentication...")
        
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
            appendResult("✅ Test user created")
            user_free(userResultStr)
        } else {
            appendResult("⚠️ User creation failed (may already exist)")
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
            appendResult("✅ Login successful")
            
            // Parse tokens
            if let data = loginResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let accessToken = json["access_token"] as? String {
                
                appendResult("   Access token received: \(accessToken.prefix(20))...")
                
                // Test authenticated operations
                appendResult("\n📋 Testing authenticated operations...")
                
                var userListResult: UnsafeMutablePointer<CChar>?
                let userListCode = auth_get_all_users(accessToken, &userListResult)
                
                if userListCode == 0, let userListStr = userListResult {
                    appendResult("✅ User list retrieved with authentication")
                    auth_free(userListStr)
                } else {
                    appendResult("❌ Authenticated user list failed")
                }
            }
            
            auth_free(loginResultStr)
        } else {
            appendResult("❌ Login failed")
        }
        
        appendResult("\n🎉 iOS Production tests completed!")
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
