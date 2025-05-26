# iPad Rust Core - Swift Wrapper

A comprehensive Swift wrapper for the iPad Rust Core static library, providing type-safe access to all 234 FFI functions across 16 domains.

## Overview

The Swift wrapper provides a clean, type-safe interface to the Rust backend functionality, including:

- **Authentication & User Management** (20 functions)
- **CRUD Operations** for all domains (150+ functions)
- **Export System** with 23 export functions
- **Document Management** (23 functions)
- **Analytics & Reporting** (30+ functions)
- **Compression System** (9 functions)
- **Core Library Management** (5 functions)

## Features

### ✅ Complete FFI Coverage
- **234 FFI functions** across all domains
- **Type-safe Swift interfaces** with proper error handling
- **Comprehensive data models** for all entities
- **Memory management** handled automatically

### 🏗️ Architecture
- **Singleton pattern** for easy access (`iPadRustCore.shared`)
- **Generic helper methods** for common operations
- **Structured error handling** with custom error types
- **JSON-based communication** with automatic encoding/decoding

### 📊 Domains Covered
1. **Strategic Goals** (15 functions) - Goal management and tracking
2. **Projects** (17 functions) - Project lifecycle management
3. **Activities** (13 functions) - Activity tracking and progress
4. **Participants** (21 functions) - Participant management and demographics
5. **Workshops** (24 functions) - Workshop management and evaluation
6. **Donors** (18 functions) - Donor management and statistics
7. **Funding** (17 functions) - Funding tracking and analytics
8. **Livelihoods** (22 functions) - Livelihood grants and outcomes
9. **Documents** (23 functions) - Document upload, download, and management
10. **Authentication** (20 functions) - User authentication and management
11. **Export** (23 functions) - Data export with multiple formats
12. **Compression** (9 functions) - Document compression and optimization
13. **User Management** (7 functions) - User CRUD operations
14. **Core Library** (5 functions) - Library initialization and status

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "path/to/ipad-rust-core", from: "1.0.0")
]
```

### Manual Integration

1. Copy the static libraries to your project:
   - `libipad_rust_core_device.a` (iOS device)
   - `libipad_rust_core_sim.a` (iOS simulator)
   - `libipad_rust_core.a` (macOS universal)

2. Add the header file:
   - `include/ipad_rust_core.h`

3. Add the Swift wrapper:
   - `Sources/iPadRustCore/iPadRustCore.swift`

## Quick Start

### 1. Initialize the Library

```swift
import iPadRustCore

let core = iPadRustCore.shared

// Initialize with database path and configuration
try await core.initialize(
    dbPath: "/path/to/database.db",
    deviceId: "unique-device-id",
    offlineMode: false,
    jwtSecret: "your-jwt-secret"
)
```

### 2. Authentication

```swift
// Login
let credentials = LoginCredentials(
    email: "user@example.com",
    password: "password"
)

let credentialsData = try JSONEncoder().encode(credentials)
let credentialsJson = String(data: credentialsData, encoding: .utf8)!

let responseJson = try core.login(credentials: credentialsJson)
let authResponse = try JSONDecoder().decode(AuthResponse.self, from: responseJson.data(using: .utf8)!)

let token = authResponse.accessToken
```

### 3. CRUD Operations

```swift
// Create a strategic goal
let goalData: [String: Any] = [
    "title": "Improve Education Access",
    "description": "Increase access to quality education",
    "status": "active",
    "priority": 1,
    "target_value": 1000.0
]

let payload: [String: Any] = [
    "data": goalData,
    "token": token
]

let payloadJson = String(data: try JSONSerialization.data(withJSONObject: payload), encoding: .utf8)!
let responseJson = try core.createStrategicGoal(payload: payloadJson)
```

### 4. Export Data

```swift
// Export all strategic goals
let options = ExportOptions(
    includeBlobs: true,
    targetPath: nil,
    statusId: nil
)

let exportJob = try core.exportStrategicGoalsAll(options: options, token: token)
print("Export job created: \(exportJob.job.id)")

// Monitor progress
let status = try core.getExportStatus(jobId: exportJob.job.id)
print("Export status: \(status.job.status)")
```

## API Reference

### Core Functions

```swift
// Library management
func initialize(dbPath: String, deviceId: String, offlineMode: Bool, jwtSecret: String) throws
func setOfflineMode(_ offlineMode: Bool) throws
func isOfflineMode() -> Bool
func getLibraryVersion() -> Int32
func getLastError() -> Int32
```

### Authentication

```swift
// User authentication
func login(credentials: String) throws -> String
func verifyToken(_ token: String) throws -> String
func refreshToken(_ refreshToken: String) throws -> String
func logout(token: String) throws

// User management
func createUser(userData: String, token: String) throws -> String
func getUser(userId: String, token: String) throws -> String
func getAllUsers(token: String) throws -> String
func getCurrentUser(token: String) throws -> String
func updateCurrentUser(userData: String, token: String) throws -> String
func changePassword(passwordData: String, token: String) throws
func isEmailUnique(_ email: String) throws -> String
```

### CRUD Operations

#### Strategic Goals
```swift
func createStrategicGoal(payload: String) throws -> String
func createStrategicGoalWithDocuments(payload: String) throws -> String
func getStrategicGoal(payload: String) throws -> String
func listStrategicGoals(payload: String) throws -> String
func updateStrategicGoal(payload: String) throws -> String
func deleteStrategicGoal(payload: String) throws -> String
```

#### Projects
```swift
func createProject(payload: String) throws -> String
func createProjectWithDocuments(payload: String) throws -> String
func getProject(payload: String) throws -> String
func listProjects(payload: String) throws -> String
func updateProject(payload: String) throws -> String
func deleteProject(payload: String) throws -> String
```

#### Activities
```swift
func createActivity(payload: String) throws -> String
func createActivityWithDocuments(payload: String) throws -> String
func getActivity(payload: String) throws -> String
func updateActivity(payload: String) throws -> String
func deleteActivity(payload: String) throws -> String
```

#### Participants
```swift
func createParticipant(payload: String) throws -> String
func createParticipantWithDocuments(payload: String) throws -> String
func getParticipant(payload: String) throws -> String
func listParticipants(payload: String) throws -> String
func updateParticipant(payload: String) throws -> String
func deleteParticipant(payload: String) throws -> String
func getParticipantDemographics(payload: String) throws -> String
func getParticipantGenderDistribution(payload: String) throws -> String
```

#### Workshops
```swift
func createWorkshop(payload: String) throws -> String
func createWorkshopWithDocuments(payload: String) throws -> String
func getWorkshop(payload: String) throws -> String
func listWorkshops(payload: String) throws -> String
func updateWorkshop(payload: String) throws -> String
func deleteWorkshop(payload: String) throws -> String
func addWorkshopParticipant(payload: String) throws -> String
func getWorkshopStatistics(payload: String) throws -> String
```

#### Donors
```swift
func createDonor(payload: String) throws -> String
func createDonorWithDocuments(payload: String) throws -> String
func getDonor(payload: String) throws -> String
func listDonors(payload: String) throws -> String
func updateDonor(payload: String) throws -> String
func deleteDonor(payload: String) throws -> String
func getDonorStatistics(payload: String) throws -> String
```

#### Funding
```swift
func createFunding(payload: String) throws -> String
func createFundingWithDocuments(payload: String) throws -> String
func getFunding(payload: String) throws -> String
func listFunding(payload: String) throws -> String
func updateFunding(payload: String) throws -> String
func deleteFunding(payload: String) throws -> String
func getFundingAnalytics(payload: String) throws -> String
```

#### Livelihoods
```swift
func createLivelihood(payload: String) throws -> String
func createLivelihoodWithDocuments(payload: String) throws -> String
func getLivelihood(payload: String) throws -> String
func listLivelihoods(payload: String) throws -> String
func updateLivelihood(payload: String) throws -> String
func deleteLivelihood(payload: String) throws -> String
func getLivelihoodStatistics(payload: String) throws -> String
```

### Export Functions

```swift
// Export all domains
func exportStrategicGoalsAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportProjectsAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportActivitiesAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportDonorsAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportFundingAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportLivelihoodsAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportWorkshopsAll(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportUnifiedAllDomains(options: ExportOptions, token: String) throws -> ExportJobSummary

// Export by date range
func exportStrategicGoalsByDateRange(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportProjectsByDateRange(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportActivitiesByDateRange(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportMediaDocumentsByDateRange(options: ExportOptions, token: String) throws -> ExportJobSummary
func exportUnifiedByDateRange(options: ExportOptions, token: String) throws -> ExportJobSummary

// Custom exports
func createCustomExport(request: String, token: String) throws -> ExportJobSummary
func validateExportRequest(request: String, token: String) throws -> String

// Export management
func createExport(request: ExportRequest, token: String) throws -> ExportJobSummary
func getExportStatus(jobId: String) throws -> ExportJobSummary
```

### Document Management

```swift
func uploadDocument(payload: String) throws -> String
func downloadDocument(payload: String) throws -> String
func getDocument(payload: String) throws -> String
func listDocumentsByEntity(payload: String) throws -> String
```

### Compression

```swift
func compressDocument(payload: String) throws -> String
func getCompressionQueueStatus() throws -> String
func getCompressionStats() throws -> String
```

## Data Models

### Core Models

```swift
// Authentication
struct LoginCredentials: Codable
struct AuthResponse: Codable
struct User: Codable

// Domain entities
struct StrategicGoal: Codable
struct Project: Codable
struct Activity: Codable
struct Participant: Codable
struct Workshop: Codable
struct Donor: Codable
struct Funding: Codable
struct Livelihood: Codable
struct Document: Codable

// Export models
struct ExportRequest: Codable
struct ExportOptions: Codable
struct ExportJobSummary: Codable
struct ExportJob: Codable
enum EntityFilter: Codable

// Response models
struct ListResponse<T: Codable>: Codable
struct ApiResponse<T: Codable>: Codable
struct StatisticsResponse: Codable
```

### Entity Filter Types

```swift
enum EntityFilter: Codable {
    case strategicGoals(statusId: Int64?)
    case projectsAll
    case activitiesAll
    case donorsAll
    case fundingAll
    case livelihoodsAll
    case workshopsAll
    case unifiedAllDomains(includeTypeTags: Bool)
}
```

## Error Handling

```swift
enum RustCoreError: Error, LocalizedError {
    case initializationFailed(code: Int32)
    case operationFailed(code: Int32)
    case nullPointer
    case jsonEncodingFailed
    case jsonDecodingFailed
}
```

### Error Handling Example

```swift
do {
    let result = try core.verifyToken(token)
    // Handle success
} catch RustCoreError.initializationFailed(let code) {
    print("Initialization failed: \(code)")
} catch RustCoreError.operationFailed(let code) {
    print("Operation failed: \(code)")
    print("Last error: \(core.getLastError())")
} catch RustCoreError.nullPointer {
    print("Unexpected null pointer")
} catch {
    print("Unexpected error: \(error)")
}
```

## Examples

See `Sources/iPadRustCore/Examples.swift` for comprehensive usage examples including:

- Library initialization
- User authentication
- CRUD operations for all domains
- Export functionality with progress monitoring
- Document upload/download
- Analytics and reporting
- Error handling patterns

### Running Examples

```swift
let examples = iPadRustCoreUsageExample()
await examples.runCompleteExample()
```

## Memory Management

The Swift wrapper handles memory management automatically:

- **Automatic deallocation** of C strings returned from Rust
- **Proper cleanup** in defer blocks
- **Safe pointer handling** with null checks
- **Generic helper methods** for consistent memory management

## Thread Safety

- The underlying Rust library handles thread safety
- Swift wrapper methods can be called from any thread
- Use appropriate Swift concurrency patterns (async/await) for best practices

## Performance Considerations

- **JSON serialization** overhead for complex payloads
- **Memory allocation** for large document operations
- **Export operations** may be long-running (use progress monitoring)
- **Compression operations** are asynchronous

## Build Requirements

- **iOS 13.0+** or **macOS 10.15+**
- **Xcode 12.0+**
- **Swift 5.3+**

## Static Library Files

- `libipad_rust_core_device.a` - iOS device (ARM64) - 67MB
- `libipad_rust_core_sim.a` - iOS simulator (x86_64 + ARM64) - 133MB  
- `libipad_rust_core.a` - macOS universal (Intel + Apple Silicon) - 135MB

## Integration Checklist

- [ ] Add static library to project
- [ ] Include header file in bridging header
- [ ] Add Swift wrapper files
- [ ] Configure build settings
- [ ] Initialize library in app startup
- [ ] Implement authentication flow
- [ ] Test CRUD operations
- [ ] Test export functionality
- [ ] Implement error handling
- [ ] Add logging and monitoring

## Troubleshooting

### Common Issues

1. **Library not found**: Ensure static library is properly linked
2. **Header not found**: Check bridging header configuration
3. **Initialization fails**: Verify database path and permissions
4. **Memory issues**: Check for proper cleanup in error paths
5. **Export timeouts**: Increase timeout for large datasets

### Debug Information

```swift
// Get library information
print("Library version: \(core.getLibraryVersion())")
print("Last error: \(core.getLastError())")
print("Offline mode: \(core.isOfflineMode())")
```

## Contributing

1. Follow Swift coding conventions
2. Add comprehensive documentation
3. Include unit tests for new functionality
4. Update examples for new features
5. Ensure memory safety in all operations

## License

This project is licensed under the same terms as the main iPad Rust Core project. 