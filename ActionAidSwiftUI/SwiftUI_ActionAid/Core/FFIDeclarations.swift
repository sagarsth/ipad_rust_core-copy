//
//  FFIDeclarations.swift
//  ActionAid SwiftUI
//
//  Legacy FFI function declarations - contains functions not yet moved to domain-specific files
//  Most functions have been moved to domain-specific files in Core/FFIDeclarations/
//

import Foundation

// MARK: - Livelihood Functions (not yet moved to domain-specific files)
@_silgen_name("livelihood_create")
func livelihood_create(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_list")
func livelihood_list(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_get")
func livelihood_get(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_update")
func livelihood_update(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_delete")
func livelihood_delete(_ payload: UnsafePointer<CChar>) -> Int32

@_silgen_name("livelihood_add_outcome")
func livelihood_add_outcome(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_get_stats")
func livelihood_get_stats(
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

@_silgen_name("livelihood_free")
func livelihood_free(_ ptr: UnsafeMutablePointer<CChar>)

@_silgen_name("livelihood_activity_create")
func livelihood_activity_create(
    _ payload: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int32

// MARK: - Helper Extensions
extension String {
    /// Convert String to C string and execute a closure with it
    func withCString<Result>(_ body: (UnsafePointer<CChar>) throws -> Result) rethrows -> Result {
        try self.utf8CString.withUnsafeBufferPointer { buffer in
            try body(buffer.baseAddress!)
        }
    }
}

// MARK: - Result Handling
struct FFIResult<T> {
    let value: T?
    let error: String?
    
    var isSuccess: Bool { value != nil }
    var isFailure: Bool { value == nil }
}

// MARK: - Generic FFI Helper
class FFIHelper {
    /// Execute FFI call with automatic error handling
    static func execute<T>(
        call: (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32,
        parse: (String) throws -> T,
        free: (UnsafeMutablePointer<CChar>) -> Void
    ) -> FFIResult<T> {
        var resultPtr: UnsafeMutablePointer<CChar>?
        let status = call(&resultPtr)
        
        guard status == 0 else {
            let error = getLastError()
            return FFIResult(value: nil, error: error)
        }
        
        guard let ptr = resultPtr else {
            return FFIResult(value: nil, error: "Null response from FFI")
        }
        
        let response = String(cString: ptr)
        defer { free(ptr) }
        
        do {
            let value = try parse(response)
            return FFIResult(value: value, error: nil)
        } catch {
            return FFIResult(value: nil, error: error.localizedDescription)
        }
    }
    
    static func getLastError() -> String {
        if let errorPtr = get_last_error() {
            let error = String(cString: errorPtr)
            free_string(errorPtr)
            return error.isEmpty ? "Unknown error" : error
        }
        return "Unknown error"
    }
}