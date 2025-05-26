# iPad Rust Core - Swift Integration Guide

This guide explains how to integrate the iPad Rust Core static library into your Swift/iOS projects.

## 🚀 Quick Start

### 1. Build the Static Library

Choose your target platform and run the appropriate build script:

```bash
# For iOS (creates universal libraries for device and simulator)
./scripts/build-ios.sh

# For macOS (creates universal library)
./scripts/build-macos.sh
```

### 2. Integration Options

You have three options for integrating the library:

#### Option A: Direct Integration (Recommended for existing projects)
#### Option B: Swift Package Manager
#### Option C: CocoaPods (if you create a podspec)

---

## 📱 Option A: Direct Integration

### Step 1: Add Static Libraries to Xcode

1. **Add the static library files to your Xcode project:**
   - For iOS: Add both `libipad_rust_core_device.a` and `libipad_rust_core_sim.a`
   - For macOS: Add `libipad_rust_core.a`

2. **Configure Build Settings:**
   - Go to your target's **Build Settings**
   - Under **Library Search Paths**, add the path to your library files
   - Under **Header Search Paths**, add the path to the header file

### Step 2: Add the Header File

1. **Create or update your bridging header:**
   ```c
   // YourProject-Bridging-Header.h
   #import "ipad_rust_core.h"
   ```

2. **Set the bridging header path:**
   - Go to **Build Settings** → **Swift Compiler - General**
   - Set **Objective-C Bridging Header** to your bridging header file

### Step 3: Configure Target-Specific Libraries

For iOS projects that support both device and simulator:

1. **Select your target** → **Build Phases** → **Link Binary With Libraries**

2. **Add conditional linking:**
   - Add `libipad_rust_core_device.a` for iOS device builds
   - Add `libipad_rust_core_sim.a` for iOS simulator builds

3. **Use build configurations or scripts to handle this automatically**

### Step 4: Link System Frameworks

Add these system frameworks to your project:
- `Foundation.framework`
- `Security.framework`
- For iOS: `UIKit.framework`
- For macOS: `AppKit.framework`

---

## 📦 Option B: Swift Package Manager

### Step 1: Add Package Dependency

Add this to your `Package.swift`:

```swift
dependencies: [
    .package(path: "/path/to/ipad_rust_core")
]
```

Or add via Xcode:
1. **File** → **Add Package Dependencies**
2. Enter the local path or repository URL
3. Select **iPadRustCore** package

### Step 2: Import and Use

```swift
import iPadRustCore

// Initialize the library
try iPadRustCore.shared.initialize(
    dbPath: "/path/to/database.db",
    deviceId: "unique-device-id",
    offlineMode: false,
    jwtSecret: "your-jwt-secret"
)

// Use export functions
let options = ExportOptions(includeBlobs: true)
let summary = try iPadRustCore.shared.exportProjectsAll(
    options: options,
    token: "your-auth-token"
)
```

---

## 🔧 Usage Examples

### Basic Initialization

```swift
import iPadRustCore

class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        
        do {
            // Get documents directory for database
            let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
            let dbPath = documentsPath.appendingPathComponent("app_database.db").path
            
            // Initialize the Rust core
            try iPadRustCore.shared.initialize(
                dbPath: dbPath,
                deviceId: UIDevice.current.identifierForVendor?.uuidString ?? "unknown",
                offlineMode: false,
                jwtSecret: "your-super-secret-jwt-key"
            )
            
            print("✅ iPad Rust Core initialized successfully")
            
        } catch {
            print("❌ Failed to initialize iPad Rust Core: \(error)")
        }
        
        return true
    }
}
```

### Export Data

```swift
class ExportManager {
    
    func exportAllProjects(authToken: String) async throws -> ExportJobSummary {
        let options = ExportOptions(
            includeBlobs: true,
            targetPath: nil // Use default path
        )
        
        return try iPadRustCore.shared.exportProjectsAll(
            options: options,
            token: authToken
        )
    }
    
    func exportStrategicGoalsWithStatus(statusId: Int64, authToken: String) async throws -> ExportJobSummary {
        let options = ExportOptions(
            includeBlobs: false,
            targetPath: "/custom/export/path",
            statusId: statusId
        )
        
        return try iPadRustCore.shared.exportStrategicGoalsAll(
            options: options,
            token: authToken
        )
    }
    
    func checkExportStatus(jobId: String) async throws -> ExportJobSummary {
        return try iPadRustCore.shared.getExportStatus(jobId: jobId)
    }
}
```

### Custom Export with Multiple Filters

```swift
func createCustomExport(authToken: String) async throws -> ExportJobSummary {
    let request = ExportRequest(
        filters: [
            .strategicGoals(statusId: 1),
            .projectsAll,
            .activitiesAll,
            .unifiedAllDomains(includeTypeTags: true)
        ],
        includeBlobs: true,
        targetPath: "/custom/export/path"
    )
    
    return try iPadRustCore.shared.createExport(
        request: request,
        token: authToken
    )
}
```

### Error Handling

```swift
func handleRustCoreOperations() {
    do {
        let deviceId = try iPadRustCore.shared.getDeviceId()
        print("Device ID: \(deviceId)")
        
        let isOffline = iPadRustCore.shared.isOfflineMode()
        print("Offline mode: \(isOffline)")
        
    } catch RustCoreError.initializationFailed(let code) {
        print("Initialization failed with code: \(code)")
    } catch RustCoreError.operationFailed(let code) {
        print("Operation failed with code: \(code)")
    } catch RustCoreError.nullPointer {
        print("Unexpected null pointer from Rust")
    } catch {
        print("Unknown error: \(error)")
    }
}
```

---

## 🔧 Build Configuration

### Xcode Build Settings

For optimal performance and compatibility, configure these build settings:

```
ENABLE_BITCODE = NO
SWIFT_OBJC_BRIDGING_HEADER = YourProject/YourProject-Bridging-Header.h
LIBRARY_SEARCH_PATHS = $(PROJECT_DIR)/path/to/rust/libraries
HEADER_SEARCH_PATHS = $(PROJECT_DIR)/path/to/rust/headers
```

### Conditional Compilation

For projects supporting multiple platforms:

```swift
#if os(iOS)
    // iOS-specific code
#elseif os(macOS)
    // macOS-specific code
#endif
```

---

## 🐛 Troubleshooting

### Common Issues

1. **"Library not found" error:**
   - Verify library search paths are correct
   - Ensure you're using the right library for your target (device vs simulator)

2. **"Header not found" error:**
   - Check bridging header path
   - Verify header search paths

3. **Linking errors:**
   - Ensure all required system frameworks are linked
   - Check that the static library architecture matches your target

4. **Runtime crashes:**
   - Verify library initialization is called before any other operations
   - Check that all required parameters are provided

### Debug Tips

1. **Enable verbose logging:**
   ```swift
   // Add this to see more detailed error information
   print("Library version: \(String(cString: get_library_version()))")
   ```

2. **Check library architecture:**
   ```bash
   lipo -info libipad_rust_core.a
   ```

3. **Verify symbols:**
   ```bash
   nm -D libipad_rust_core.a | grep export_
   ```

---

## 📚 API Reference

### Core Functions

- `initialize(dbPath:deviceId:offlineMode:jwtSecret:)` - Initialize the library
- `setOfflineMode(_:)` - Toggle offline mode
- `getDeviceId()` - Get current device ID
- `isOfflineMode()` - Check offline status

### Export Functions

- `createExport(request:token:)` - Create custom export
- `getExportStatus(jobId:)` - Check export status
- `exportStrategicGoalsAll(options:token:)` - Export strategic goals
- `exportProjectsAll(options:token:)` - Export projects
- `exportActivitiesAll(options:token:)` - Export activities

### Data Models

- `ExportRequest` - Custom export configuration
- `ExportOptions` - Simple export options
- `ExportJobSummary` - Export job response
- `EntityFilter` - Filter types for exports

---

## 🔄 Migration Guide

If you're upgrading from a previous version:

1. **Update static libraries** with new builds
2. **Update header file** if the API has changed
3. **Review breaking changes** in the changelog
4. **Test thoroughly** on all target platforms

---

## 🤝 Support

For issues and questions:

1. Check the troubleshooting section above
2. Review the API documentation
3. Create an issue in the project repository
4. Provide detailed error messages and system information

---

## 📄 License

This library is distributed under the same license as the main project. 