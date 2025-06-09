//
//  FFIAuthDeclarations.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

// MARK: - Auth FFI Declarations

// This file contains the Swift declarations for the FFI functions
// defined in the Rust `auth` module (`src/ffi/auth.rs`).
// These declarations allow Swift to call the underlying Rust functions.
// The `@_silgen_name` attribute links the Swift function to the
// corresponding C-ABI compatible function in the Rust library.

// MARK: - Memory Management
@_silgen_name("auth_free")
func auth_free(_ ptr: UnsafeMutablePointer<CChar>?)

// MARK: - Authentication Functions
@_silgen_name("auth_login")
func auth_login(
    _ credentials_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_verify_token")
func auth_verify_token(
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_refresh_token")
func auth_refresh_token(
    _ refresh_token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_logout")
func auth_logout(_ logout_json: UnsafePointer<CChar>) -> CInt

@_silgen_name("auth_hash_password")
func auth_hash_password(
    _ password: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - User Management Functions
@_silgen_name("auth_create_user")
func auth_create_user(
    _ user_json: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_get_user")
func auth_get_user(
    _ user_id: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_get_all_users")
func auth_get_all_users(
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_update_user")
func auth_update_user(
    _ user_id: UnsafePointer<CChar>,
    _ update_json: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_hard_delete_user")
func auth_hard_delete_user(
    _ user_id: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>
) -> CInt

@_silgen_name("auth_get_current_user")
func auth_get_current_user(
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_update_current_user")
func auth_update_current_user(
    _ update_json: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("auth_change_password")
func auth_change_password(
    _ password_change_json: UnsafePointer<CChar>,
    _ token: UnsafePointer<CChar>
) -> CInt

@_silgen_name("auth_is_email_unique")
func auth_is_email_unique(
    _ email_check_json: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

// MARK: - Setup and Test Data Functions
@_silgen_name("auth_initialize_default_accounts")
func auth_initialize_default_accounts(_ token: UnsafePointer<CChar>) -> CInt

@_silgen_name("auth_initialize_test_data")
func auth_initialize_test_data(_ token: UnsafePointer<CChar>) -> CInt

// MARK: - Legacy Compatibility Functions
// These are kept for backward compatibility and should be phased out in new Swift code.

@_silgen_name("login")
func login(
    _ email: UnsafePointer<CChar>,
    _ password: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("verify_token")
func verify_token(
    _ token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("refresh_token")
func refresh_token(
    _ refresh_token: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt

@_silgen_name("logout")
func logout(
    _ token: UnsafePointer<CChar>,
    _ refresh_token: UnsafePointer<CChar>?
) -> CInt

@_silgen_name("hash_password")
func hash_password(
    _ password: UnsafePointer<CChar>,
    _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> CInt 