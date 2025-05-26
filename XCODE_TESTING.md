# Testing iPad Rust Core in Xcode

This guide will help you test your production-ready iPad Rust Core library in a proper iOS environment using Xcode.

## 🚀 Quick Start

1. **Run the setup script:**
   ```bash
   python3 scripts/setup_xcode_simple.py
   ```

2. **Follow the instructions printed by the script**

3. **Open Xcode and create a new iOS project**

## 📱 Detailed Setup Instructions

### Step 1: Create New iOS Project

1. Open Xcode
2. Choose "Create a new Xcode project"
3. Select "iOS" → "App"
4. Configure your project:
   - Product Name: `iPadRustCoreTest`
   - Language: `Swift`
   - Interface: `Storyboard`
   - Use Core Data: `No`
   - Include Tests: `Yes` (optional)

### Step 2: Add Static Libraries

1. In Xcode, right-click your project in the navigator
2. Select "Add Files to [ProjectName]"
3. Navigate to your iPad Rust Core project directory
4. Add these files from `target/ios/`:
   - `libipad_rust_core_device.a`
   - `libipad_rust_core_sim.a`
   - `ipad_rust_core.h`
5. Also add from the project root:
   - `iPad-Rust-Core-Bridging-Header.h`

### Step 3: Configure Build Settings

1. Select your project in the navigator
2. Select your target
3. Go to "Build Settings"
4. Search for "Bridging Header"
5. Set "Objective-C Bridging Header" to: `iPad-Rust-Core-Bridging-Header.h`
6. Search for "Header Search Paths"
7. Add the path to where `ipad_rust_core.h` is located
8. Go to "Build Phases" → "Link Binary With Libraries"
9. Add `SystemConfiguration.framework`

### Step 4: Set Up the User Interface

1. Open `Main.storyboard`
2. Select the View Controller
3. Add these UI elements:
   - **UILabel** at the top (for status)
   - **UIButton** in the middle (for running tests)
   - **UITextView** at the bottom (for results)

4. Connect the outlets:
   - Control-drag from the label to your ViewController and connect to `statusLabel`
   - Control-drag from the button to your ViewController and connect to `testButton`
   - Control-drag from the text view to your ViewController and connect to `resultTextView`

5. Connect the action:
   - Control-drag from the button to your ViewController and create an action called `runTests`

### Step 5: Replace ViewController Code

1. Open `ViewController.swift`
2. Replace all content with the code from `iOS_Test_ViewController.swift`
3. Make sure the class name matches your storyboard connection

### Step 6: Build and Run

1. Select an iOS Simulator (iPhone or iPad)
2. Press Cmd+R to build and run
3. Tap the "Run Tests" button
4. Watch the results in the text view

## 🎯 What the Tests Validate

The iOS tests validate all the production-ready improvements:

### ✅ Database Integration
- **Proper iOS Documents Directory**: Uses `FileManager.default.urls(for: .documentDirectory)`
- **Sandbox Compliance**: Database files are stored in the app's sandbox
- **SQLite URL Format**: Proper `sqlite://` URL construction

### ✅ Authentication System
- **JWT Token Generation**: Creates access and refresh tokens
- **Password Hashing**: Secure Argon2 password hashing
- **Token Verification**: Validates token signatures and expiration
- **Authenticated Operations**: Tests user operations with tokens

### ✅ iOS-Specific Features
- **Device ID**: Uses `UIDevice.current.identifierForVendor`
- **iOS File Paths**: Proper iOS Documents directory access
- **Memory Management**: Proper FFI memory cleanup
- **UI Integration**: Real iOS UI with async operations

### ✅ Production JSON Payloads
- **Structured Data**: Valid JSON for all operations
- **Type Safety**: Proper data validation
- **Error Handling**: Comprehensive error reporting

## 🔧 Troubleshooting

### Build Errors

**"Library not found"**
- Ensure both `.a` files are added to "Link Binary With Libraries"
- Check that the files are actually copied to your project

**"Header file not found"**
- Verify the bridging header path in Build Settings
- Ensure `ipad_rust_core.h` is added to your project
- Check header search paths

**"Undefined symbols"**
- Make sure you're linking the correct library for your target (device vs simulator)
- Verify all required frameworks are linked (SystemConfiguration)

### Runtime Errors

**"Database initialization failed"**
- Check that your app has proper Documents directory access
- Verify the database path construction
- Look for permission issues in the iOS simulator

**"Authentication failed"**
- Ensure the JWT secret is properly set
- Check that user creation succeeded before login
- Verify JSON payload formatting

### Performance Issues

**"Slow startup"**
- The first run may be slower due to database initialization
- Subsequent runs should be faster
- Use Instruments to profile performance

## 📊 Expected Results

When you run the tests successfully, you should see:

```
🚀 Starting iPad Rust Core Production Tests

📋 Testing library version...
✅ Library version: 1.0.0

📋 Testing database initialization...
Database path: sqlite:///Users/.../Documents/test_ipad_rust_core.sqlite
Device ID: 12345678-1234-1234-1234-123456789ABC
✅ Library initialized successfully

📋 Testing authentication...
✅ Test user created
✅ Login successful
   Access token received: eyJ0eXAiOiJKV1QiLCJh...

📋 Testing authenticated operations...
✅ User list retrieved with authentication

🎉 iOS Production tests completed!
✅ Database: iOS Documents directory
✅ Authentication: JWT tokens working
✅ Device ID: iOS UIDevice integration
✅ Runtime: Centralized Tokio runtime
```

## 🚀 Next Steps

After successful testing in Xcode:

1. **Performance Testing**: Use Instruments to profile memory and CPU usage
2. **Device Testing**: Test on actual iOS devices
3. **Integration**: Integrate into your main iOS app
4. **App Store**: Prepare for App Store submission with proper entitlements

## 📱 Benefits of Xcode Testing

- **Real iOS Environment**: Proper sandbox, permissions, and file system
- **Debugging Tools**: Xcode debugger, console, and breakpoints
- **Performance Profiling**: Instruments for memory leaks and performance
- **Device Testing**: Test on real iPads and iPhones
- **UI Integration**: Real iOS UI components and interactions
- **Crash Reporting**: Proper crash logs and symbolication

## 🔗 Related Files

- `scripts/setup_xcode_simple.py` - Setup script
- `iOS_Test_ViewController.swift` - Test view controller
- `iPad-Rust-Core-Bridging-Header.h` - Bridging header
- `target/ios/` - iOS build artifacts
- `PRODUCTION_READY.md` - Complete production features documentation 