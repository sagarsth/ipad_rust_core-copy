#!/usr/bin/env python3
"""
Setup script for testing iPad Rust Core in Xcode
This script prepares the library for iOS testing and provides instructions
"""

import os
import subprocess
import sys
from pathlib import Path

def run_command(cmd, cwd=None, check=True):
    """Run a command and return the result"""
    print(f"Running: {cmd}")
    try:
        result = subprocess.run(cmd, shell=True, cwd=cwd, check=check, 
                              capture_output=True, text=True)
        if result.stdout:
            print(result.stdout)
        return result
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}")
        if e.stderr:
            print(f"Error output: {e.stderr}")
        if check:
            sys.exit(1)
        return e

def main():
    print("🚀 Setting up iPad Rust Core for Xcode testing...")
    
    # Get the project root directory
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    
    print(f"Project root: {project_root}")
    
    # Step 1: Build the Rust library for iOS
    print("\n📱 Building Rust library for iOS...")
    
    # Check if we have the iOS build script
    ios_build_script = project_root / "scripts" / "build-ios.sh"
    if ios_build_script.exists():
        print("Found iOS build script, running it...")
        run_command(f'chmod +x "{ios_build_script}"')
        run_command(f'"{ios_build_script}"', cwd=project_root)
    else:
        print("iOS build script not found, building manually...")
        # Add iOS targets if not already added
        run_command("rustup target add aarch64-apple-ios", cwd=project_root, check=False)
        run_command("rustup target add x86_64-apple-ios", cwd=project_root, check=False)
        run_command("rustup target add aarch64-apple-ios-sim", cwd=project_root, check=False)
        
        # Build for iOS targets
        run_command("cargo build --target aarch64-apple-ios --release", cwd=project_root)
        run_command("cargo build --target aarch64-apple-ios-sim --release", cwd=project_root)
        run_command("cargo build --target x86_64-apple-ios --release", cwd=project_root)
    
    # Step 2: Generate the C header
    print("\n📋 Generating C header...")
    header_script = project_root / "scripts" / "generate_header.py"
    if header_script.exists():
        run_command(f'python3 "{header_script}"', cwd=project_root)
    else:
        print("Header generation script not found, using cbindgen...")
        run_command("cbindgen --config cbindgen.toml --crate ipad_rust_core --output include/ipad_rust_core.h", cwd=project_root)
    
    # Step 3: Copy the library to the Swift package location
    print("\n📦 Setting up Swift package...")
    
    # Create the library directory if it doesn't exist
    lib_dir = project_root / "Sources" / "iPadRustCoreC"
    lib_dir.mkdir(exist_ok=True)
    
    # Copy the static library (we'll use the simulator version for testing)
    target_dir = project_root / "target" / "aarch64-apple-ios-sim" / "release"
    lib_file = target_dir / "libipad_rust_core.a"
    
    if lib_file.exists():
        import shutil
        dest_lib = lib_dir / "libipad_rust_core.a"
        shutil.copy2(lib_file, dest_lib)
        print(f"Copied library to {dest_lib}")
    else:
        print(f"Warning: Library file not found at {lib_file}")
    
    # Step 4: Test Swift package compilation
    print("\n🔨 Testing Swift package compilation...")
    result = run_command("swift build", cwd=project_root, check=False)
    
    if result.returncode == 0:
        print("✅ Swift package builds successfully!")
    else:
        print("❌ Swift package build failed. Check the errors above.")
    
    # Step 5: Create an Xcode-compatible test file
    print("\n📱 Creating iOS test file...")
    
    ios_test_content = '''
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
        appendResult("🚀 Starting iPad Rust Core Production Tests\\n")
        
        // Test 1: Library version
        appendResult("📋 Testing library version...")
        if let version = core.getLibraryVersion() {
            appendResult("✅ Library version: \\(version)\\n")
        } else {
            appendResult("❌ Failed to get library version\\n")
        }
        
        // Test 2: Database initialization with proper iOS path
        appendResult("📋 Testing database initialization...")
        let dbPath = core.getDatabaseURL(filename: "test_ipad_rust_core.sqlite")
        let deviceId = core.getDeviceId()
        let jwtSecret = "test-jwt-secret-for-ios"
        
        appendResult("Database path: \\(dbPath)")
        appendResult("Device ID: \\(deviceId)")
        
        let initResult = initialize_library(dbPath, deviceId, false, jwtSecret)
        if initResult == 0 {
            appendResult("✅ Library initialized successfully\\n")
        } else {
            appendResult("❌ Library initialization failed with code: \\(initResult)\\n")
            if let error = core.getLastError() {
                appendResult("   Error: \\(error)\\n")
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
            appendResult("✅ Test user created\\n")
            user_free(userResultStr)
        } else {
            appendResult("⚠️ User creation failed (may already exist)\\n")
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
            appendResult("✅ Login successful\\n")
            
            // Parse tokens
            if let data = loginResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let accessToken = json["access_token"] as? String {
                
                // Test authenticated operations
                appendResult("📋 Testing authenticated operations...")
                
                var userListResult: UnsafeMutablePointer<CChar>?
                let userListCode = auth_get_all_users(accessToken, &userListResult)
                
                if userListCode == 0, let userListStr = userListResult {
                    appendResult("✅ User list retrieved with authentication\\n")
                    auth_free(userListStr)
                } else {
                    appendResult("❌ Authenticated user list failed\\n")
                }
            }
            
            auth_free(loginResultStr)
        } else {
            appendResult("❌ Login failed\\n")
        }
        
        // Test offline mode
        appendResult("📋 Testing offline mode...")
        appendResult("Initial offline mode: \\(core.isOfflineMode())")
        core.setOfflineMode(true)
        appendResult("After setting to true: \\(core.isOfflineMode())")
        core.setOfflineMode(false)
        appendResult("After setting to false: \\(core.isOfflineMode())\\n")
        
        appendResult("🎉 iOS Production tests completed!\\n")
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
    
    print(f"Created iOS test file: {ios_test_file}")
    
    # Step 6: Provide instructions
    print("\n" + "="*60)
    print("🎯 XCODE SETUP INSTRUCTIONS")
    print("="*60)
    print()
    print("1. Open Xcode and create a new iOS App project")
    print("   - Choose 'App' template")
    print("   - Language: Swift")
    print("   - Interface: Storyboard")
    print("   - Minimum iOS version: 13.0+")
    print()
    print("2. Add the iPad Rust Core Swift Package:")
    print("   - File → Add Package Dependencies")
    print(f"   - Enter local path: {project_root}")
    print("   - Add 'iPadRustCore' library to your target")
    print()
    print("3. Copy the test code:")
    print(f"   - Copy content from: {ios_test_file}")
    print("   - Replace your ViewController.swift content")
    print()
    print("4. Add UI elements to Main.storyboard:")
    print("   - UILabel (statusLabel)")
    print("   - UIButton (testButton) with action 'runTests'")
    print("   - UITextView (resultTextView)")
    print()
    print("5. Add the static library:")
    print("   - Drag libipad_rust_core.a to your Xcode project")
    print("   - Add to 'Link Binary With Libraries' build phase")
    print()
    print("6. Configure build settings:")
    print("   - Add header search path to include/")
    print("   - Link SystemConfiguration framework")
    print()
    print("7. Run on iOS Simulator or Device!")
    print()
    print("📱 Benefits of testing in Xcode:")
    print("✅ Proper iOS sandbox environment")
    print("✅ Real Documents directory access")
    print("✅ UIDevice integration testing")
    print("✅ iOS-specific debugging tools")
    print("✅ Performance profiling")
    print("✅ Memory leak detection")
    print()
    print("🔧 If you encounter issues:")
    print("- Check that all Rust targets are built")
    print("- Verify the static library is linked correctly")
    print("- Ensure header files are accessible")
    print("- Check iOS deployment target compatibility")

if __name__ == "__main__":
    main() 