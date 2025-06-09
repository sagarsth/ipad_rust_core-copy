//
//  FFIUserDeclarations.swift
//  SwiftUI_ActionAid
//
//  This file contains the Swift declarations for the FFI functions
//  defined in the Rust `user` module (`src/ffi/user.rs`).
//

import Foundation

// MARK: - User FFI Declarations

// MARK: - Memory Management
@_silgen_name("user_free")
func user_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - User CRUD Functions
@_silgen_name("user_create")
func user_create(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_get")
func user_get(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_get_all")
func user_get_all(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_update")
func user_update(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_hard_delete")
func user_hard_delete(_ payload_json: UnsafePointer<CChar>) -> CInt

// MARK: - User Utility Functions
@_silgen_name("user_is_email_unique")
func user_is_email_unique(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_get_stats")
func user_get_stats(
    _ payload_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("user_change_password")
func user_change_password(_ payload_json: UnsafePointer<CChar>) -> CInt 