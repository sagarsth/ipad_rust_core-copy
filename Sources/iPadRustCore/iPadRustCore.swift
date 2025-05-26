import Foundation
import iPadRustCoreC

/// Main interface for the iPad Rust Core library
public class iPadRustCore {
    
    /// Shared instance for singleton access
    public static let shared = iPadRustCore()
    
    private var isInitialized = false
    
    private init() {}
    
    // MARK: - Initialization
    
    public func initialize(
        dbPath: String,
        deviceId: String,
        offlineMode: Bool = false,
        jwtSecret: String
    ) throws {
        let result = initialize_library(dbPath, deviceId, offlineMode, jwtSecret)
        
        if result != 0 {
            throw RustCoreError.initializationFailed(code: result)
        }
        isInitialized = true
    }
    
    public func setOfflineMode(_ offlineMode: Bool) {
        set_offline_mode(offlineMode)
    }
    
    public func isOfflineMode() -> Bool {
        return is_offline_mode()
    }
    
    public func getLibraryVersion() -> String? {
        guard let cString = get_library_version() else {
            return nil
        }
        // Heap-allocated string from Rust (get_library_version in ffi/core.rs), needs to be freed.
        defer {
            free_string(cString)
        }
        return String(cString: cString)
    }
    
    public func getLastError() -> String? {
        // This function in C (get_last_error) should return the actual last error message,
        // which is dynamically allocated by the Rust FFI error handling (e.g., FFIError).
        // Therefore, it needs to be freed by the canonical free_string.
        // The placeholder get_last_error in ffi/core.rs returns a static string,
        // but the Swift wrapper should operate on the assumption that the *actual*
        // C get_last_error it links against will provide a dynamically allocated string.
        guard let cString = get_last_error() else {
            return nil
        }
        defer {
            free_string(cString) // Re-enabled: free the dynamically allocated error string.
        }
        return String(cString: cString)
    }
    
    // MARK: - iOS Directory Helpers
    
    /// Get the proper iOS Documents directory path for database storage
    public func getDocumentsDirectory() -> String {
        let paths = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
        let documentsDirectory = paths[0]
        return documentsDirectory.path
    }
    
    /// Get a proper database path in the iOS Documents directory
    public func getDatabasePath(filename: String = "ipad_rust_core.sqlite") -> String {
        let documentsPath = getDocumentsDirectory()
        return "\(documentsPath)/\(filename)"
    }
    
    /// Get a proper database URL for SQLite connection
    public func getDatabaseURL(filename: String = "ipad_rust_core.sqlite") -> String {
        return "sqlite://\(getDatabasePath(filename: filename))"
    }
    
    // MARK: - Utility Functions
        
    /// Swift wrapper for the C free_string function.
    public func freeSwiftString(_ ptr: UnsafeMutablePointer<CChar>?) {
        if let validPtr = ptr {
            free_string(validPtr) // Calls the C FFI function free_string
        }
    }

    // All other public methods (Export, Auth, CRUD, Compression) are removed from here.
    // All private helper methods like performExport, performStringOperation are removed.
    // All data model structs are removed.
}

// MARK: - Private Helper Method (kept for error parsing if getLastError returns JSON)
/// Helper to extract "details" or "message" from an error JSON string
private func extractDetailsFromErrorJson(_ jsonString: String) -> String? {
    guard let jsonData = jsonString.data(using: .utf8) else { return nil }
    do {
        if let json = try JSONSerialization.jsonObject(with: jsonData, options: []) as? [String: Any] {
            if let details = json["details"] as? String {
                return details
            }
            if let message = json["message"] as? String {
                return message
            }
            // Could check for a generic "error" field too
            if let error = json["error"] as? String {
                return error
            }
        }
    } catch {
        // Not a valid JSON or not the expected structure, ignore
    }
    return nil // If no specific detail found, return nil to use a more generic error from RustCoreError
}

// The global encodeToString helper is removed.

// MARK: - Error Handling Enum
public enum RustCoreError: Error, LocalizedError {
    case initializationFailed(code: Int32)
    case operationFailed(code: Int32, details: String?)
    case nullPointer
    // Removed jsonEncodingFailed and decodingFailed as they were primarily used by
    // the complex operations and data models that have been removed.
    // If any remaining minimal function needs them, they can be re-added.
    // For now, focusing on the most direct FFI errors.
    // Re-adding for potential use by other parts of the app or future minimal tests:
    case jsonEncodingFailed(message: String) 
    case decodingFailed(message: String)


    public var errorDescription: String? {
        switch self {
        case .initializationFailed(let code):
            return "Rust Core initialization failed with code: \(code)."
        case .operationFailed(let code, let details):
            let baseMessage = "Rust Core operation failed with code: \(code)."
            if let details = details, !details.isEmpty {
                return "\(baseMessage) Details: \(details)"
            }
            return baseMessage
        case .nullPointer:
            return "Rust Core returned a null pointer unexpectedly."
        case .jsonEncodingFailed(let message):
            return "JSON encoding failed: \(message)"
        case .decodingFailed(let message):
            return "JSON decoding failed: \(message)"
        }
    }
}

// All data models (ExportRequest, User, Project, etc.) are removed from here.
