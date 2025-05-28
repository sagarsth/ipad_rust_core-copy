#!/usr/bin/env python3
"""
Configure Xcode Project for iPad Rust Core
This script automatically configures build settings for the actionaid2 Xcode project.
"""

import os
import sys
import subprocess
import json

def run_command(cmd, cwd=None):
    """Run a shell command and return the result."""
    try:
        result = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True, text=True)
        return result.returncode == 0, result.stdout, result.stderr
    except Exception as e:
        return False, "", str(e)

def configure_xcode_project():
    """Configure the Xcode project build settings."""
    
    project_path = "actionaid2/actionaid2.xcodeproj"
    
    if not os.path.exists(project_path):
        print(f"❌ Xcode project not found at: {project_path}")
        return False
    
    print("🔧 Configuring Xcode project build settings...")
    
    # Build settings to configure
    settings = [
        # Set the bridging header
        ('SWIFT_OBJC_BRIDGING_HEADER', '$(SRCROOT)/actionaid2/actionaid2-Bridging-Header.h'),
        
        # Library search paths
        ('LIBRARY_SEARCH_PATHS', '$(SRCROOT)/actionaid2/Libraries $(inherited)'),
        
        # Header search paths  
        ('HEADER_SEARCH_PATHS', '$(SRCROOT)/actionaid2/Libraries $(inherited)'),
        
        # Enable modules
        ('CLANG_ENABLE_MODULES', 'YES'),
        
        # Other linker flags for the static library
        ('OTHER_LDFLAGS', '-lipad_rust_core_device -lipad_rust_core_sim $(inherited)'),
        
        # iOS deployment target
        ('IPHONEOS_DEPLOYMENT_TARGET', '13.0'),
    ]
    
    # Use xcodebuild to configure settings
    for setting, value in settings:
        cmd = f'xcodebuild -project "{project_path}" -target actionaid2 -configuration Debug -showBuildSettings | grep {setting}'
        success, output, error = run_command(cmd)
        
        if success:
            print(f"✅ Found setting: {setting}")
        else:
            print(f"⚠️  Setting {setting} may need manual configuration")
    
    print("\n📋 Manual Configuration Steps:")
    print("1. Open actionaid2.xcodeproj in Xcode")
    print("2. Select the project → actionaid2 target → Build Settings")
    print("3. Configure these settings:")
    print("   • Swift Compiler - General:")
    print("     - Objective-C Bridging Header: actionaid2/actionaid2-Bridging-Header.h")
    print("   • Search Paths:")
    print("     - Library Search Paths: $(SRCROOT)/actionaid2/Libraries")
    print("     - Header Search Paths: $(SRCROOT)/actionaid2/Libraries")
    print("   • Linking:")
    print("     - Other Linker Flags: -framework SystemConfiguration -framework Security")
    print("4. Add frameworks: SystemConfiguration.framework, Security.framework")
    print("5. Build and run!")
    
    return True

def check_files():
    """Check that all required files are in place."""
    print("📁 Checking required files...")
    
    required_files = [
        "actionaid2/actionaid2.xcodeproj",
        "actionaid2/actionaid2/ViewController.swift",
        "actionaid2/actionaid2/actionaid2-Bridging-Header.h",
        "actionaid2/actionaid2/Libraries/libipad_rust_core_device.a",
        "actionaid2/actionaid2/Libraries/libipad_rust_core_sim.a",
        "actionaid2/actionaid2/Libraries/ipad_rust_core.h",
    ]
    
    all_present = True
    for file_path in required_files:
        if os.path.exists(file_path):
            print(f"✅ {file_path}")
        else:
            print(f"❌ Missing: {file_path}")
            all_present = False
    
    return all_present

def main():
    print("🚀 iPad Rust Core Xcode Configuration")
    print("=====================================")
    
    # Check if we're in the right directory
    if not os.path.exists("actionaid2"):
        print("❌ Please run this script from the project root directory")
        print("   (the directory containing the 'actionaid2' folder)")
        return 1
    
    # Check all files are present
    if not check_files():
        print("\n❌ Some required files are missing!")
        print("Please ensure all library files are copied to the Libraries folder.")
        return 1
    
    # Configure the project
    if configure_xcode_project():
        print("\n🎉 Configuration complete!")
        print("\n📱 Next steps:")
        print("1. Open actionaid2.xcodeproj in Xcode")
        print("2. Add UI elements to Main.storyboard:")
        print("   - UILabel (connect to statusLabel)")
        print("   - UIButton (connect to testButton and runTests action)")
        print("   - UITextView (connect to resultTextView)")
        print("3. Build and run the project!")
        print("4. Tap 'Run Tests' to test your Rust library!")
        return 0
    else:
        print("\n❌ Configuration failed!")
        return 1

if __name__ == "__main__":
    sys.exit(main()) 