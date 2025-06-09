import Foundation

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