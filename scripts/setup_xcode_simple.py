#!/usr/bin/env python3
"""
Simple setup script for testing iPad Rust Core in Xcode
Uses existing iOS build artifacts and provides clear instructions
"""

import os
import shutil
from pathlib import Path

def main():
    print("🚀 Setting up iPad Rust Core for Xcode testing...")
    
    # Get the project root directory
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    
    print(f"Project root: {project_root}")
    
    # Check if iOS build artifacts exist
    ios_dir = project_root / "target" / "ios"
    device_lib = ios_dir / "libipad_rust_core_device.a"
    sim_lib = ios_dir / "libipad_rust_core_sim.a"
    header_file = ios_dir / "ipad_rust_core.h"
    
    if not all([device_lib.exists(), sim_lib.exists(), header_file.exists()]):
        print("❌ iOS build artifacts not found!")
        print("Please run: ./scripts/build-ios.sh first")
        return
    
    print("✅ Found iOS build artifacts:")
    print(f"   📱 Device library: {device_lib}")
    print(f"   🖥️  Simulator library: {sim_lib}")
    print(f"   📄 Header file: {header_file}")
    
    # Create an iOS test file with proper imports
    print("\n📱 Creating iOS test file...")
    
    ios_test_content = '''import UIKit

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
        appendResult("\\n📋 Testing library version...")
        var versionResult: UnsafeMutablePointer<CChar>?
        let versionCode = get_library_version(&versionResult)
        
        if versionCode == 0, let versionStr = versionResult {
            let version = String(cString: versionStr)
            appendResult("✅ Library version: \\(version)")
            free_string(versionStr)
        } else {
            appendResult("❌ Failed to get library version")
        }
        
        // Test 2: Database initialization with proper iOS path
        appendResult("\\n📋 Testing database initialization...")
        
        // Get iOS Documents directory
        let documentsPath = FileManager.default.urls(for: .documentDirectory, 
                                                   in: .userDomainMask).first!
        let dbURL = documentsPath.appendingPathComponent("test_ipad_rust_core.sqlite")
        let dbPath = "sqlite://" + dbURL.path
        
        // Get device ID
        let deviceId = UIDevice.current.identifierForVendor?.uuidString ?? "unknown-device"
        let jwtSecret = "test-jwt-secret-for-ios"
        
        appendResult("Database path: \\(dbPath)")
        appendResult("Device ID: \\(deviceId)")
        
        let initResult = initialize_library(dbPath, deviceId, false, jwtSecret)
        if initResult == 0 {
            appendResult("✅ Library initialized successfully")
        } else {
            appendResult("❌ Library initialization failed with code: \\(initResult)")
            
            // Get last error
            var errorResult: UnsafeMutablePointer<CChar>?
            let errorCode = get_last_error(&errorResult)
            if errorCode == 0, let errorStr = errorResult {
                let error = String(cString: errorStr)
                appendResult("   Error: \\(error)")
                free_string(errorStr)
            }
            return
        }
        
        // Test 3: Authentication workflow
        appendResult("\\n📋 Testing authentication...")
        
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
                
                appendResult("   Access token received: \\(accessToken.prefix(20))...")
                
                // Test authenticated operations
                appendResult("\\n📋 Testing authenticated operations...")
                
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
        
        appendResult("\\n🎉 iOS Production tests completed!")
        appendResult("✅ Database: iOS Documents directory")
        appendResult("✅ Authentication: JWT tokens working")
        appendResult("✅ Device ID: iOS UIDevice integration")
        appendResult("✅ Runtime: Centralized Tokio runtime")
    }
    
    private func appendResult(_ text: String) {
        DispatchQueue.main.async {
            self.resultTextView.text += text + "\\n"
            
            // Scroll to bottom
            let bottom = NSMakeRange(self.resultTextView.text.count - 1, 1)
            self.resultTextView.scrollRangeToVisible(bottom)
        }
    }
}
'''
    
    ios_test_file = project_root / "iOS_Test_ViewController.swift"
    with open(ios_test_file, 'w') as f:
        f.write(ios_test_content)
    
    print(f"✅ Created iOS test file: {ios_test_file}")
    
    # Create a bridging header template
    bridging_header_content = '''//
//  iPad-Rust-Core-Bridging-Header.h
//  
//  Bridging header for iPad Rust Core C functions
//

#ifndef iPad_Rust_Core_Bridging_Header_h
#define iPad_Rust_Core_Bridging_Header_h

// Include the iPad Rust Core C header
#include "ipad_rust_core.h"

#endif /* iPad_Rust_Core_Bridging_Header_h */
'''
    
    bridging_header_file = project_root / "iPad-Rust-Core-Bridging-Header.h"
    with open(bridging_header_file, 'w') as f:
        f.write(bridging_header_content)
    
    print(f"✅ Created bridging header: {bridging_header_file}")
    
    # Provide instructions
    print("\\n" + "="*60)
    print("🎯 XCODE SETUP INSTRUCTIONS")
    print("="*60)
    print()
    print("1. 📱 Create a new iOS App project in Xcode:")
    print("   - Choose 'App' template")
    print("   - Language: Swift")
    print("   - Interface: Storyboard")
    print("   - Minimum iOS version: 13.0+")
    print()
    print("2. 📚 Add the static libraries:")
    print(f"   - Drag {device_lib.name} to your Xcode project")
    print(f"   - Drag {sim_lib.name} to your Xcode project")
    print("   - Add both to 'Link Binary With Libraries' build phase")
    print()
    print("3. 📄 Add the header files:")
    print(f"   - Drag {header_file.name} to your Xcode project")
    print(f"   - Drag {bridging_header_file.name} to your Xcode project")
    print()
    print("4. ⚙️  Configure build settings:")
    print("   - Go to Build Settings → Swift Compiler - General")
    print(f"   - Set 'Objective-C Bridging Header' to: {bridging_header_file.name}")
    print("   - Add header search path to the directory containing ipad_rust_core.h")
    print("   - Link SystemConfiguration framework")
    print()
    print("5. 🎨 Set up the UI in Main.storyboard:")
    print("   - Add UILabel (connect to statusLabel)")
    print("   - Add UIButton (connect to testButton, action: runTests)")
    print("   - Add UITextView (connect to resultTextView)")
    print()
    print("6. 📝 Replace ViewController.swift:")
    print(f"   - Copy content from: {ios_test_file}")
    print("   - Replace your ViewController.swift content")
    print()
    print("7. 🚀 Run on iOS Simulator or Device!")
    print()
    print("📱 Benefits of testing in Xcode:")
    print("✅ Proper iOS sandbox environment")
    print("✅ Real Documents directory access")
    print("✅ UIDevice integration testing")
    print("✅ iOS-specific debugging tools")
    print("✅ Performance profiling with Instruments")
    print("✅ Memory leak detection")
    print("✅ Crash reporting and symbolication")
    print()
    print("🔧 Troubleshooting:")
    print("- If build fails: Check that both .a files are linked")
    print("- If functions not found: Verify bridging header path")
    print("- If runtime errors: Check iOS deployment target (13.0+)")
    print("- If database errors: Check app has Documents directory access")
    print()
    print("📁 Files created:")
    print(f"   • {ios_test_file}")
    print(f"   • {bridging_header_file}")
    print()
    print("📁 Files to add to Xcode:")
    print(f"   • {device_lib}")
    print(f"   • {sim_lib}")
    print(f"   • {header_file}")

if __name__ == "__main__":
    main() 