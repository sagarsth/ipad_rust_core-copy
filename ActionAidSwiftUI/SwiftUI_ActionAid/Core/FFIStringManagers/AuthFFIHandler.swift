//
//  AuthFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

// MARK: - AuthFFIHandler

/// A handler class that provides a Swift-friendly interface to the Rust `auth` FFI functions.
///
/// This class encapsulates the complexity of FFI calls, including:
/// - Running operations on a background thread to avoid blocking the UI.
/// - Bridging C-style string handling and memory management with Swift's `String` and ARC.
/// - Converting JSON payloads to and from Swift `Codable` types.
/// - Providing modern `async/await` interfaces for asynchronous Rust operations.
/// - Centralized error handling that surfaces Rust errors as Swift `Error` types.
class AuthFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.auth.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    // MARK: - Initialization & Setup
    
    /// Initializes default user accounts if they don't already exist.
    /// - Parameter token: A setup token, typically "init_setup".
    /// - Returns: A `Result` indicating success or failure.
    func initializeDefaultAccounts(token: String) async -> Result<Void, Error> {
        await executeVoidOperation {
            token.withCString { cToken in
                auth_initialize_default_accounts(cToken)
            }
        }
    }
    
    /// Initializes a comprehensive set of test data for development and testing.
    /// - Parameter token: A setup token, typically "init_setup".
    /// - Returns: A `Result` indicating success or failure.
    func initializeTestData(token: String) async -> Result<Void, Error> {
        await executeVoidOperation {
            token.withCString { cToken in
                auth_initialize_test_data(cToken)
            }
        }
    }
    
    // MARK: - Core Authentication
    
    /// Authenticates a user with their email and password.
    /// - Parameter credentials: The user's login credentials.
    /// - Returns: A `Result` containing the `AuthLoginResponse` on success, or an `Error` on failure.
    func login(credentials: Credentials) async -> Result<AuthLoginResponse, Error> {
        do {
            let jsonPayload = try encode(credentials)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    auth_login(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Verifies the validity of an access token.
    /// - Parameter token: The access token to verify.
    /// - Returns: A `Result` containing the `AuthContextResponse` on success, or an `Error` on failure.
    func verifyToken(token: String) async -> Result<AuthContextResponse, Error> {
        await executeOperation { resultPtr in
            token.withCString { cToken in
                auth_verify_token(cToken, resultPtr)
            }
        }
    }
    
    /// Refreshes a user session using a refresh token.
    /// - Parameter refreshToken: The refresh token.
    /// - Returns: A `Result` containing the new `RefreshedTokenResponse` on success, or an `Error` on failure.
    func refreshToken(refreshToken: String) async -> Result<RefreshedTokenResponse, Error> {
        await executeOperation { resultPtr in
            refreshToken.withCString { cToken in
                auth_refresh_token(cToken, resultPtr)
            }
        }
    }
    
    /// Logs out a user by invalidating their tokens.
    /// - Parameters:
    ///   - accessToken: The user's current access token.
    ///   - refreshToken: The user's refresh token (optional).
    /// - Returns: A `Result` indicating success or failure.
    func logout(accessToken: String, refreshToken: String?) async -> Result<Void, Error> {
        let payload: [String: String?] = [
            "access_token": accessToken,
            "refresh_token": refreshToken
        ]
        do {
            let jsonPayload = try encode(payload)
            return await executeVoidOperation {
                jsonPayload.withCString { cJson in
                    auth_logout(cJson)
                }
            }
        } catch {
            return .failure(error)
        }
    }

    /// Hashes a password using Argon2.
    /// - Parameter password: The plaintext password to hash.
    /// - Returns: A `Result` containing the hashed password string on success, or an `Error` on failure.
    func hashPassword(password: String) async -> Result<String, Error> {
        await executeStringOperation { resultPtr in
            password.withCString { cPassword in
                auth_hash_password(cPassword, resultPtr)
            }
        }
    }

    // MARK: - User Management
    
    /// Retrieves the profile of the currently authenticated user.
    /// - Parameter token: The user's access token.
    /// - Returns: A `Result` containing the `UserResponse` on success, or an `Error` on failure.
    func getCurrentUser(token: String) async -> Result<UserResponse, Error> {
        await executeOperation { resultPtr in
            token.withCString { cToken in
                auth_get_current_user(cToken, resultPtr)
            }
        }
    }
    
    /// Updates the profile of the currently authenticated user.
    /// - Parameters:
    ///   - update: An `UpdateUser` object with the fields to update.
    ///   - token: The user's access token.
    /// - Returns: A `Result` containing the updated `UserResponse` on success, or an `Error` on failure.
    func updateCurrentUser(update: UpdateUser, token: String) async -> Result<UserResponse, Error> {
        do {
            let jsonPayload = try encode(update)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    token.withCString { cToken in
                        auth_update_current_user(cJson, cToken, resultPtr)
                    }
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Changes the current user's password.
    /// - Parameters:
    ///   - oldPassword: The user's current password.
    ///   - newPassword: The desired new password.
    ///   - token: The user's access token.
    /// - Returns: A `Result` indicating success or failure.
    func changePassword(oldPassword: String, newPassword: String, token: String) async -> Result<Void, Error> {
        let payload = [
            "old_password": oldPassword,
            "new_password": newPassword
        ]
        do {
            let jsonPayload = try encode(payload)
            return await executeVoidOperation {
                jsonPayload.withCString { cJson in
                    token.withCString { cToken in
                        auth_change_password(cJson, cToken)
                    }
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    // MARK: - Admin User Management
    
    /// Creates a new user. (Admin-only)
    /// - Parameters:
    ///   - newUser: A `NewUser` object containing the new user's details.
    ///   - token: An admin's access token.
    /// - Returns: A `Result` containing the created `UserResponse` on success, or an `Error` on failure.
    func createUser(_ newUser: NewUser, token: String) async -> Result<UserResponse, Error> {
        do {
            let jsonPayload = try encode(newUser)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    token.withCString { cToken in
                        auth_create_user(cJson, cToken, resultPtr)
                    }
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Retrieves a specific user by their ID. (Admin-only)
    /// - Parameters:
    ///   - userId: The UUID of the user to retrieve.
    ///   - token: An admin's access token.
    /// - Returns: A `Result` containing the `UserResponse` on success, or an `Error` on failure.
    func getUser(userId: String, token: String) async -> Result<UserResponse, Error> {
        await executeOperation { resultPtr in
            userId.withCString { cId in
                token.withCString { cToken in
                    auth_get_user(cId, cToken, resultPtr)
                }
            }
        }
    }
    
    /// Retrieves a list of all users. (Admin-only)
    /// - Parameter token: An admin's access token.
    /// - Returns: A `Result` containing an array of `UserResponse` on success, or an `Error` on failure.
    func getAllUsers(token: String) async -> Result<[UserResponse], Error> {
        await executeOperation { resultPtr in
            token.withCString { cToken in
                auth_get_all_users(cToken, resultPtr)
            }
        }
    }
    
    /// Updates a user's details. (Admin-only)
    /// - Parameters:
    ///   - userId: The UUID of the user to update.
    ///   - update: An `UpdateUser` object with the fields to update.
    ///   - token: An admin's access token.
    /// - Returns: A `Result` containing the updated `UserResponse` on success, or an `Error` on failure.
    func updateUser(userId: String, update: UpdateUser, token: String) async -> Result<UserResponse, Error> {
        do {
            let jsonPayload = try encode(update)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    userId.withCString { cId in
                        token.withCString { cToken in
                            auth_update_user(cId, cJson, cToken, resultPtr)
                        }
                    }
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Permanently deletes a user from the system. (Admin-only)
    /// - Parameters:
    ///   - userId: The UUID of the user to delete.
    ///   - token: An admin's access token.
    /// - Returns: A `Result` indicating success or failure.
    func hardDeleteUser(userId: String, token: String) async -> Result<Void, Error> {
        await executeVoidOperation {
            userId.withCString { cId in
                token.withCString { cToken in
                    auth_hard_delete_user(cId, cToken)
                }
            }
        }
    }
    
    /// Checks if an email address is already in use.
    /// - Parameters:
    ///   - email: The email to check.
    ///   - excludeId: An optional user ID to exclude from the check (used when updating an existing user's email).
    /// - Returns: A `Result` containing `EmailUniquenessResponse` on success, or an `Error` on failure.
    func isEmailUnique(email: String, excludeId: String?) async -> Result<EmailUniquenessResponse, Error> {
        let payload: [String: String?] = [
            "email": email,
            "exclude_id": excludeId
        ]
        do {
            let jsonPayload = try encode(payload)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    auth_is_email_unique(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    // MARK: - Private Helpers
    
    private func encode<T: Encodable>(_ value: T) throws -> String {
        let data = try jsonEncoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw FFIError.stringConversionFailed
        }
        return string
    }

    /// Executes an FFI operation that is expected to return a JSON string, which is then decoded into a `Decodable` type.
    private func executeOperation<T: Decodable>(
        _ operation: @escaping (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    ) async -> Result<T, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                let ffiResult = FFIHelper.execute(
                    call: operation,
                    parse: { responseString in
                        guard let data = responseString.data(using: .utf8) else {
                            throw FFIError.stringConversionFailed
                        }
                        return try self.jsonDecoder.decode(T.self, from: data)
                    },
                    free: auth_free
                )
                
                if let value = ffiResult.value {
                    continuation.resume(returning: .success(value))
                } else if let error = ffiResult.error {
                    continuation.resume(returning: .failure(FFIError.rustError(error)))
                } else {
                    continuation.resume(returning: .failure(FFIError.unknown))
                }
            }
        }
    }
    
    /// Executes an FFI operation that returns a raw string (e.g., a hashed password).
    private func executeStringOperation(
        _ operation: @escaping (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    ) async -> Result<String, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                let ffiResult = FFIHelper.execute(
                    call: operation,
                    parse: { $0 }, // Return the raw string
                    free: auth_free
                )
                
                if let value = ffiResult.value {
                    continuation.resume(returning: .success(value))
                } else if let error = ffiResult.error {
                    continuation.resume(returning: .failure(FFIError.rustError(error)))
                } else {
                    continuation.resume(returning: .failure(FFIError.unknown))
                }
            }
        }
    }

    /// Executes an FFI operation that does not return any data.
    private func executeVoidOperation(
        _ operation: @escaping () -> CInt
    ) async -> Result<Void, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                let status = operation()
                if status == 0 {
                    continuation.resume(returning: .success(()))
                } else {
                    let error = FFIHelper.getLastError()
                    continuation.resume(returning: .failure(FFIError.rustError(error)))
                }
            }
        }
    }
}

enum FFIError: Error, LocalizedError {
    case rustError(String)
    case stringConversionFailed
    case unknown
    
    var errorDescription: String? {
        switch self {
        case .rustError(let message):
            return "Rust FFI Error: \(message)"
        case .stringConversionFailed:
            return "Failed to convert data to UTF-8 string."
        case .unknown:
            return "An unknown FFI error occurred."
        }
    }
} 