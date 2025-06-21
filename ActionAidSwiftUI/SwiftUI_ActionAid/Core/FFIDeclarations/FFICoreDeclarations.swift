import Foundation
import UIKit

// MARK: - Core FFI Declarations

// General purpose memory management
@_silgen_name("free_string")
func free_string(_ ptr: UnsafeMutablePointer<CChar>?)

// General purpose error handling
@_silgen_name("get_last_error")
func get_last_error() -> UnsafeMutablePointer<CChar>?

// MARK: - Initialization Functions
@_silgen_name("initialize_library")
func initialize_library(
    _ db_url: UnsafePointer<CChar>,
    _ device_id: UnsafePointer<CChar>,
    _ is_sync_disabled: Bool,
    _ jwt_secret: UnsafePointer<CChar>
) -> CInt

// MARK: - Storage Functions
@_silgen_name("set_ios_storage_path")
func set_ios_storage_path(_ path: UnsafePointer<CChar>) -> CInt

// MARK: - iOS System Integration Functions

/// Begin a background task safely with callback support
@_cdecl("ios_begin_background_task_safe")
func iosBeginBackgroundTaskSafe(
    identifier: UnsafePointer<CChar>?,
    expirationHandler: @escaping @convention(c) (UnsafeMutableRawPointer?) -> Void,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    let taskName = identifier != nil ? String(cString: identifier!) : "Export Task"
    
    let taskIdentifier = UIApplication.shared.beginBackgroundTask(withName: taskName) {
        // Call the Rust expiration handler
        expirationHandler(context)
    }
    
    return taskIdentifier == .invalid ? -1 : Int32(Int(taskIdentifier.rawValue))
}

/// End a background task safely
@_cdecl("ios_end_background_task_safe")
func iosEndBackgroundTaskSafe(taskId: Int32) {
    guard taskId != -1 else { return }
    
    let identifier = UIBackgroundTaskIdentifier(rawValue: Int(taskId))
    UIApplication.shared.endBackgroundTask(identifier)
}

/// Get remaining background time
@_cdecl("ios_background_time_remaining")
func iosBackgroundTimeRemaining() -> Double {
    return UIApplication.shared.backgroundTimeRemaining
}

// MARK: - iOS Keychain Functions

/// Save data to iOS Keychain
@_cdecl("ios_keychain_save")
func iosKeychainSave(key: UnsafePointer<CChar>, data: UnsafePointer<UInt8>, length: Int) -> Int32 {
    guard let keyString = String(cString: key, encoding: .utf8) else {
        return -1
    }
    
    let dataToSave = Data(bytes: data, count: length)
    
    // Create keychain query
    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: keyString,
        kSecAttrService as String: "ActionAidExport",
        kSecValueData as String: dataToSave,
        kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
    ]
    
    // Delete existing item first
    SecItemDelete(query as CFDictionary)
    
    // Add new item
    let status = SecItemAdd(query as CFDictionary, nil)
    
    return status == errSecSuccess ? 0 : Int32(status)
}

/// Load data from iOS Keychain
@_cdecl("ios_keychain_load")
func iosKeychainLoad(key: UnsafePointer<CChar>, dataPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, lengthPtr: UnsafeMutablePointer<Int>) -> Int32 {
    guard let keyString = String(cString: key, encoding: .utf8) else {
        return -1
    }
    
    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: keyString,
        kSecAttrService as String: "ActionAidExport",
        kSecReturnData as String: true,
        kSecMatchLimit as String: kSecMatchLimitOne
    ]
    
    var result: AnyObject?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    
    guard status == errSecSuccess, let data = result as? Data else {
        return Int32(status)
    }
    
    // Allocate memory for the data
    let allocatedPtr = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
    data.withUnsafeBytes { bytes in
        allocatedPtr.initialize(from: bytes.bindMemory(to: UInt8.self).baseAddress!, count: data.count)
    }
    
    dataPtr.pointee = allocatedPtr
    lengthPtr.pointee = data.count
    
    return 0
}

/// Delete item from iOS Keychain
@_cdecl("ios_keychain_delete")
func iosKeychainDelete(key: UnsafePointer<CChar>) {
    guard let keyString = String(cString: key, encoding: .utf8) else {
        return
    }
    
    let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrAccount as String: keyString,
        kSecAttrService as String: "ActionAidExport"
    ]
    
    SecItemDelete(query as CFDictionary)
}

/// Free memory allocated by keychain operations
@_cdecl("ios_keychain_free")
func iosKeychainFree(dataPtr: UnsafeMutablePointer<UInt8>) {
    dataPtr.deallocate()
}

// MARK: - Memory Pressure Monitoring

private var memoryPressureCallback: (@convention(c) (Int32, UnsafeMutableRawPointer?) -> Void)?
private var memoryPressureContext: UnsafeMutableRawPointer?
private var memoryPressureSource: DispatchSourceMemoryPressure?

/// Register memory pressure handler
@_cdecl("ios_register_memory_pressure_handler")
func iosRegisterMemoryPressureHandler(
    callback: @escaping @convention(c) (Int32, UnsafeMutableRawPointer?) -> Void,
    context: UnsafeMutableRawPointer?
) {
    memoryPressureCallback = callback
    memoryPressureContext = context
    
    // Create dispatch source for memory pressure
    memoryPressureSource = DispatchSource.makeMemoryPressureSource(
        eventMask: [.warning, .critical],
        queue: .global(qos: .utility)
    )
    
    memoryPressureSource?.setEventHandler {
        let event = memoryPressureSource?.mask
        
        let level: Int32
        if event?.contains(.critical) == true {
            level = 2 // Critical
        } else if event?.contains(.warning) == true {
            level = 1 // Warning
        } else {
            level = 0 // Normal
        }
        
        callback(level, context)
    }
    
    memoryPressureSource?.resume()
}

/// Get current thermal state
@_cdecl("ios_get_thermal_state")
func iosGetThermalState() -> Int32 {
    if #available(iOS 11.0, *) {
        switch ProcessInfo.processInfo.thermalState {
        case .nominal:
            return 0
        case .fair:
            return 1
        case .serious:
            return 2
        case .critical:
            return 3
        @unknown default:
            return 3
        }
    } else {
        return 0 // Assume nominal on older iOS versions
    }
}

/// Request critical memory release
@_cdecl("ios_request_critical_memory_release")
func iosRequestCriticalMemoryRelease() {
    // Post memory warning notification
    NotificationCenter.default.post(name: UIApplication.didReceiveMemoryWarningNotification, object: nil)
    
    // Force garbage collection if possible
    autoreleasepool {
        // This autoreleasepool helps release temporary objects
    }
}

/// Trim memory at specified level
@_cdecl("ios_trim_memory")
func iosTrimMemory(level: Int32) {
    switch level {
    case 0: // Light trim
        // Clear non-essential caches
        URLCache.shared.removeAllCachedResponses()
        
    case 1: // Moderate trim
        // Clear image caches and other moderate memory usage
        URLCache.shared.removeAllCachedResponses()
        
    case 2: // Aggressive trim
        // Clear all possible caches and release memory aggressively
        URLCache.shared.removeAllCachedResponses()
        iosRequestCriticalMemoryRelease()
        
    default:
        break
    }
}

// Legacy background task functions (kept for compatibility)
@_cdecl("ios_begin_background_task")
func iosBeginBackgroundTask(name: UnsafePointer<CChar>) -> Int32 {
    let taskName = String(cString: name)
    let taskIdentifier = UIApplication.shared.beginBackgroundTask(withName: taskName) {
        // Default expiration handler - just log
        print("Background task '\(taskName)' expired")
    }
    
    return taskIdentifier == .invalid ? -1 : Int32(Int(taskIdentifier.rawValue))
}

@_cdecl("ios_end_background_task")
func iosEndBackgroundTask(taskId: Int32) {
    guard taskId != -1 else { return }
    
    let identifier = UIBackgroundTaskIdentifier(rawValue: Int(taskId))
    UIApplication.shared.endBackgroundTask(identifier)
} 