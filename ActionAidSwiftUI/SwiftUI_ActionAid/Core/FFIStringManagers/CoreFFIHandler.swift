import Foundation
import UIKit

/// Handles core FFI functions like library initialization and storage setup.
class CoreFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.core.ffi", qos: .utility)

    /// Prepares the necessary storage directories for the app.
    /// - Returns: The path to the main storage directory.
    /// - Throws: An error if the directory could not be created.
    func prepareStorage() throws -> String {
        let documentsPath = getDocumentsDirectory()
        let storagePath = "\(documentsPath)/ActionAid/storage"
        
        try FileManager.default.createDirectory(atPath: storagePath, withIntermediateDirectories: true, attributes: nil)
        
        let setResult = storagePath.withCString { set_ios_storage_path($0) }
        guard setResult == 0 else {
            throw FFIError.rustError("Failed to set iOS storage path (code: \(setResult))")
        }
        
        return storagePath
    }

    /// Initializes the core Rust library.
    /// - Parameter storagePath: The path to the app's storage directory.
    /// - Throws: An FFIError if initialization fails.
    func initializeLibrary(storagePath: String) async throws {
        let dbPath = getDatabasePath(storagePath: storagePath)
        let deviceId = await UIDevice.current.identifierForVendor?.uuidString ?? "unknown-device"
        let jwtSecret = "preview-jwt-secret-for-a-real-app"
        let sqliteUrl = "sqlite://\(dbPath)?mode=rwc"

        let status = await withCheckedContinuation { continuation in
            queue.async {
                sqliteUrl.withCString { cUrl in
                    deviceId.withCString { cDeviceId in
                        jwtSecret.withCString { cJwtSecret in
                            let result = initialize_library(cUrl, cDeviceId, false, cJwtSecret)
                            continuation.resume(returning: result)
                        }
                    }
                }
            }
        }
        
        if status != 0 {
            let errorDetails = FFIHelper.getLastError()
            throw FFIError.rustError("Database initialization failed (code: \(status)). Details: \(errorDetails)")
        }
    }
    
    // MARK: - Private Helpers

    private func getDocumentsDirectory() -> String {
        let paths = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
        return paths[0].path
    }
    
    private func getDatabasePath(storagePath: String) -> String {
        let dbDir = (storagePath as NSString).deletingLastPathComponent
        return "\(dbDir)/actionaid_core.sqlite"
    }
} 