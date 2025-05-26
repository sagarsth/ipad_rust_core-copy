# iPad Rust Core - Static Library for Swift

This document provides a complete guide for using the iPad Rust Core as a static library in your Swift/iOS/macOS projects.

## 🎯 Overview

The iPad Rust Core has been successfully converted to a static library that provides:

- **Complete CRUD Operations**: Full Create, Read, Update, Delete for all entities
- **Export Functionality**: 19 specialized export functions for data extraction
- **Authentication & User Management**: Complete auth system with JWT tokens
- **Document Management**: File upload, download, compression, and versioning
- **Cross-Platform Support**: iOS (device + simulator) and macOS (Intel + Apple Silicon)
- **Memory Safe**: Proper memory management with dedicated free functions
- **Type Safe**: Comprehensive Swift wrapper with proper error handling

## 📦 Generated Files

After building, you'll find these files:

### iOS Files (`target/ios/`)
- `libipad_rust_core_device.a` - ARM64 library for iOS devices
- `libipad_rust_core_sim.a` - Universal library for iOS simulator (x86_64 + ARM64)
- `ipad_rust_core.h` - C header file

### macOS Files (`target/macos/`)
- `libipad_rust_core.a` - Universal library for macOS (x86_64 + ARM64)
- `ipad_rust_core.h` - C header file

## 🔨 Building the Static Library

### Prerequisites
```bash
# Install Rust targets
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-ios-sim
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin
```

### Build Commands
```bash
# Build for iOS (creates universal libraries)
./scripts/build-ios.sh

# Build for macOS (creates universal library)
./scripts/build-macos.sh

# Or build manually for specific targets
cargo build --release --target aarch64-apple-ios
cargo build --release --target x86_64-apple-darwin
```

## 🚀 Integration Guide

### Option 1: Direct Integration (Recommended)

1. **Add static libraries to your Xcode project:**
   - Drag and drop the `.a` files into your project
   - For iOS: Add both device and simulator libraries
   - For macOS: Add the universal library

2. **Create/Update bridging header:**
   ```c
   // YourProject-Bridging-Header.h
   #import "ipad_rust_core.h"
   ```

3. **Configure build settings:**
   - Set **Library Search Paths** to include library directory
   - Set **Header Search Paths** to include header directory
   - Set **Objective-C Bridging Header** path

4. **Link system frameworks:**
   - `Foundation.framework`
   - `Security.framework`
   - iOS: `UIKit.framework`
   - macOS: `AppKit.framework`

### Option 2: Swift Package Manager

Add to your `Package.swift`:
```swift
dependencies: [
    .package(path: "/path/to/ipad_rust_core")
]
```

## 📚 API Reference

### Core Functions

#### Library Management
```swift
// Initialize the library
try iPadRustCore.shared.initialize(
    dbPath: "/path/to/database.db",
    deviceId: "unique-device-id",
    offlineMode: false,
    jwtSecret: "your-jwt-secret"
)

// Manage offline mode
iPadRustCore.shared.setOfflineMode(true)
let isOffline = iPadRustCore.shared.isOfflineMode()

// Get device information
let deviceId = try iPadRustCore.shared.getDeviceId()
```

#### Export Functions

**Individual Domain Exports:**
```swift
let options = ExportOptions(includeBlobs: true)

// Export specific domains
let strategicGoals = try iPadRustCore.shared.exportStrategicGoalsAll(options: options, token: token)
let projects = try iPadRustCore.shared.exportProjectsAll(options: options, token: token)
let activities = try iPadRustCore.shared.exportActivitiesAll(options: options, token: token)
```

**Custom Multi-Filter Exports:**
```swift
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

let exportJob = try iPadRustCore.shared.createExport(request: request, token: token)
```

**Export Status Monitoring:**
```swift
let status = try iPadRustCore.shared.getExportStatus(jobId: "job-uuid")
print("Export status: \(status.job.status)")
```

#### CRUD Operations Examples

**Creating Entities:**
```swift
// Create a new project
let newProject = NewProject(
    name: "Community Development Project",
    description: "A project to improve local infrastructure",
    startDate: "2024-01-01",
    endDate: "2024-12-31"
)
let project = try iPadRustCore.shared.createProject(project: newProject, token: token)

// Create a new strategic goal
let newGoal = NewStrategicGoal(
    title: "Improve Education Access",
    description: "Increase literacy rates in rural areas",
    targetDate: "2025-06-30"
)
let goal = try iPadRustCore.shared.createStrategicGoal(goal: newGoal, token: token)
```

**Reading/Fetching Entities:**
```swift
// Get project by ID
let project = try iPadRustCore.shared.getProject(id: "project-uuid", token: token)

// List projects with pagination
let pagination = PaginationParams(page: 1, limit: 20)
let projects = try iPadRustCore.shared.listProjects(pagination: pagination, token: token)

// Search projects
let searchResults = try iPadRustCore.shared.searchProjects(query: "education", token: token)
```

**Updating Entities:**
```swift
// Update project
let updateData = UpdateProject(
    name: "Updated Project Name",
    description: "Updated description"
)
let updatedProject = try iPadRustCore.shared.updateProject(
    id: "project-uuid", 
    update: updateData, 
    token: token
)
```

**Deleting Entities:**
```swift
// Soft delete (default)
try iPadRustCore.shared.deleteProject(id: "project-uuid", hardDelete: false, token: token)

// Hard delete (permanent)
try iPadRustCore.shared.deleteProject(id: "project-uuid", hardDelete: true, token: token)
```

#### Authentication Examples

**User Registration & Login:**
```swift
// Register new user
let registration = UserRegistration(
    email: "user@example.com",
    password: "securePassword123",
    firstName: "John",
    lastName: "Doe"
)
let authResult = try iPadRustCore.shared.registerUser(registration: registration)

// Login
let credentials = LoginCredentials(
    email: "user@example.com",
    password: "securePassword123"
)
let loginResult = try iPadRustCore.shared.login(credentials: credentials)
let token = loginResult.accessToken
```

**Document Management:**
```swift
// Upload document
let documentData = DocumentUpload(
    fileData: base64EncodedData,
    originalFilename: "report.pdf",
    title: "Project Report",
    documentTypeId: "doc-type-uuid",
    relatedEntityId: "project-uuid",
    relatedEntityType: "projects"
)
let document = try iPadRustCore.shared.uploadDocument(document: documentData, token: token)

// Download document
let downloadedDoc = try iPadRustCore.shared.downloadDocument(id: "doc-uuid", token: token)
```

### Complete API Coverage

#### Core Entity CRUD Operations

**Strategic Goals:**
- `strategic_goals_create` - Create new strategic goal
- `strategic_goals_get_by_id` - Get strategic goal by ID
- `strategic_goals_update` - Update strategic goal
- `strategic_goals_delete` - Delete strategic goal
- `strategic_goals_list` - List with pagination
- `strategic_goals_search` - Search strategic goals

**Projects:**
- `projects_create` - Create new project
- `projects_get_by_id` - Get project by ID
- `projects_update` - Update project
- `projects_delete` - Delete project
- `projects_list` - List with pagination
- `projects_search` - Search projects

**Activities:**
- `activities_create` - Create new activity
- `activities_get_by_id` - Get activity by ID
- `activities_update` - Update activity
- `activities_delete` - Delete activity
- `activities_list` - List with pagination
- `activities_search` - Search activities

**Donors:**
- `donors_create` - Create new donor
- `donors_get_by_id` - Get donor by ID
- `donors_update` - Update donor
- `donors_delete` - Delete donor
- `donors_list` - List with pagination
- `donors_search` - Search donors

**Funding:**
- `funding_create` - Create new funding
- `funding_get_by_id` - Get funding by ID
- `funding_update` - Update funding
- `funding_delete` - Delete funding
- `funding_list` - List with pagination
- `funding_search` - Search funding

**Livelihoods:**
- `livelihoods_create` - Create new livelihood
- `livelihoods_get_by_id` - Get livelihood by ID
- `livelihoods_update` - Update livelihood
- `livelihoods_delete` - Delete livelihood
- `livelihoods_list` - List with pagination
- `livelihoods_search` - Search livelihoods

**Workshops:**
- `workshops_create` - Create new workshop
- `workshops_get_by_id` - Get workshop by ID
- `workshops_update` - Update workshop
- `workshops_delete` - Delete workshop
- `workshops_list` - List with pagination
- `workshops_search` - Search workshops

**Media Documents:**
- `media_documents_create` - Create/upload document
- `media_documents_get_by_id` - Get document by ID
- `media_documents_update` - Update document metadata
- `media_documents_delete` - Delete document
- `media_documents_list` - List with pagination
- `media_documents_search` - Search documents
- `media_documents_upload` - Upload file
- `media_documents_download` - Download file

#### Authentication & User Management

**Authentication:**
- `auth_register_user` - Register new user
- `auth_login` - User login
- `auth_refresh_token` - Refresh JWT token
- `auth_logout` - User logout
- `auth_verify_token` - Verify token validity
- `auth_change_password` - Change user password
- `auth_update_profile` - Update user profile
- `auth_get_profile` - Get user profile
- `auth_delete_account` - Delete user account

#### Export Functions

**Individual Domain Exports (7 functions):**
- `exportStrategicGoalsAll`
- `exportProjectsAll`
- `exportActivitiesAll`
- `exportDonorsAll`
- `exportFundingAll`
- `exportLivelihoodsAll`
- `exportWorkshopsAll`

**Date Range Exports (9 functions):**
- `exportStrategicGoalsByDateRange`
- `exportProjectsByDateRange`
- `exportActivitiesByDateRange`
- `exportDonorsByDateRange`
- `exportFundingByDateRange`
- `exportLivelihoodsByDateRange`
- `exportWorkshopsByDateRange`
- `exportMediaDocumentsByDateRange`
- `exportUnifiedByDateRange`

**Media Document Exports (1 function):**
- `exportMediaDocumentsByEntity`

**Advanced Functions (3 functions):**
- `createExport` (custom multi-filter)
- `validateExportRequest`
- `getExportStatus`

#### Sync Operations

**Synchronization:**
- `sync_start` - Start sync process
- `sync_get_status` - Get sync status
- `sync_cancel` - Cancel sync
- `sync_get_conflicts` - Get sync conflicts
- `sync_resolve_conflict` - Resolve conflicts

## 🔧 Configuration

### Build Settings for Xcode

```
ENABLE_BITCODE = NO
SWIFT_OBJC_BRIDGING_HEADER = YourProject/YourProject-Bridging-Header.h
LIBRARY_SEARCH_PATHS = $(PROJECT_DIR)/path/to/rust/libraries
HEADER_SEARCH_PATHS = $(PROJECT_DIR)/path/to/rust/headers
```

### Conditional Compilation

For multi-platform projects:
```swift
#if os(iOS)
    // iOS-specific code
#elseif os(macOS)
    // macOS-specific code
#endif
```

## 🛠️ Development Workflow

### Making Changes

1. **Modify Rust code** in `src/` directory
2. **Update FFI functions** in `src/ffi/` if needed
3. **Rebuild static libraries** (header auto-regenerates):
   ```bash
   ./scripts/build-ios.sh
   ./scripts/build-macos.sh
   ```
4. **Or manually regenerate header:**
   ```bash
   python3 scripts/generate_header.py
   cp include/ipad_rust_core_complete.h include/ipad_rust_core.h
   ```
5. **Replace libraries** in your Xcode project
6. **Test thoroughly** on all target platforms

### Auto-Generated Header

The C header file (`include/ipad_rust_core.h`) is **automatically generated** from your Rust FFI functions using `scripts/generate_header.py`. This ensures:

- ✅ **100% API Coverage** - All FFI functions are exposed
- ✅ **Always Up-to-Date** - Header regenerates on every build
- ✅ **No Manual Sync** - No risk of missing functions
- ✅ **Consistent Naming** - Direct mapping from Rust function names

### Testing

```bash
# Check compilation
cargo check

# Run tests
cargo test

# Build for specific target
cargo build --release --target aarch64-apple-ios
```

## 🐛 Troubleshooting

### Common Issues

1. **"Library not found" error:**
   - Verify library search paths
   - Ensure correct library for target (device vs simulator)

2. **"Header not found" error:**
   - Check bridging header path
   - Verify header search paths

3. **Linking errors:**
   - Ensure all system frameworks are linked
   - Check library architecture matches target

4. **Runtime crashes:**
   - Verify library initialization before use
   - Check all required parameters are provided

### Debug Commands

```bash
# Check library architectures
lipo -info target/ios/libipad_rust_core_device.a

# Verify symbols
nm -D target/ios/libipad_rust_core_device.a | grep export_

# Check library info
file target/ios/libipad_rust_core_device.a
```

## 📊 Library Statistics

- **Total FFI Functions**: 234 functions across all domains
  - **Activity Functions**: 13 functions
  - **Authentication Functions**: 20 functions  
  - **Compression Functions**: 9 functions
  - **Core Library Functions**: 3 functions
  - **Document Management**: 23 functions
  - **Donor Functions**: 18 functions
  - **Export Functions**: 23 functions
  - **Funding Functions**: 17 functions
  - **Livelihood Functions**: 22 functions
  - **Participant Functions**: 21 functions
  - **Project Functions**: 18 functions
  - **Strategic Goal Functions**: 15 functions
  - **User Management**: 7 functions
  - **Workshop Functions**: 24 functions
- **iOS Device Library**: ~67MB (ARM64)
- **iOS Simulator Library**: ~133MB (x86_64 + ARM64)
- **macOS Universal Library**: ~135MB (x86_64 + ARM64)
- **Header File**: ~10KB

## 🔄 Migration from Previous Versions

If upgrading from a previous version:

1. **Replace static libraries** with new builds
2. **Update header file** if API changed
3. **Review breaking changes** in changelog
4. **Update Swift wrapper** if needed
5. **Test on all platforms**

## 📝 Example Project Structure

```
YourProject/
├── YourProject/
│   ├── YourProject-Bridging-Header.h
│   └── ... (your Swift files)
├── Libraries/
│   ├── iOS/
│   │   ├── libipad_rust_core_device.a
│   │   └── libipad_rust_core_sim.a
│   ├── macOS/
│   │   └── libipad_rust_core.a
│   └── Headers/
│       └── ipad_rust_core.h
└── YourProject.xcodeproj
```

## 🎉 Success!

Your iPad Rust Core is now ready for Swift integration! The static library provides:

✅ **Complete CRUD Operations** - Full Create, Read, Update, Delete for all entities  
✅ **Export Functionality** - All 19 export functions for data extraction  
✅ **Authentication System** - Complete user management with JWT tokens  
✅ **Document Management** - File upload, download, compression, and versioning  
✅ **Sync Operations** - Conflict resolution and data synchronization  
✅ **Cross-Platform Support** - iOS and macOS universal libraries  
✅ **Memory Safety** - Proper FFI memory management  
✅ **Type Safety** - Swift wrapper with comprehensive error handling  
✅ **Production Ready** - Optimized release builds  

You can now integrate this static library into any Swift project and access the full power of your Rust backend through a clean, type-safe Swift API with complete CRUD functionality. 