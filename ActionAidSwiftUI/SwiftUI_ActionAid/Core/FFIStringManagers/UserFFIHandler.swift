//
//  UserFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/21/23.
//

import Foundation

// MARK: - UserFFIHandler

/// Provides a Swift-friendly interface to the Rust `user` FFI functions.
/// This class handles the complexity of FFI calls, including JSON serialization,
/// background thread execution, and memory management for the user domain.
class UserFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.user.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    init() {
        // Note: Do NOT use convertToSnakeCase because AuthContextPayload already uses snake_case field names
        // Using convertToSnakeCase would corrupt field names like "user_id" -> "user__id"
        // jsonEncoder.keyEncodingStrategy = .convertToSnakeCase
        // jsonDecoder.keyDecodingStrategy = .convertFromSnakeCase
        
        // Set up date formatting to match backend RFC3339 format (only applies to actual Date types)
        jsonEncoder.dateEncodingStrategy = .iso8601
        
        // For decoding, all our models use String fields for dates, so this shouldn't interfere
        jsonDecoder.dateDecodingStrategy = .iso8601
    }
    
    // MARK: - User Management
    
    /// Creates a new user.
    /// - Parameters:
    ///   - user: The `NewUser` object containing the details of the user to create.
    ///   - auth: The authentication context of the administrator performing the action.
    /// - Returns: A `Result` containing the created `UserResponse` on success, or an `Error` on failure.
    func createUser(user: NewUser, auth: AuthContextPayload) async -> Result<UserResponse, Error> {
        struct CreateUserPayload: Codable {
            let user: NewUser
            let auth: AuthContextPayload
        }
        let payload = CreateUserPayload(user: user, auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_create)
    }
    
    /// Retrieves a specific user by their ID.
    /// - Parameters:
    ///   - userId: The UUID of the user to retrieve.
    ///   - auth: The authentication context.
    /// - Returns: A `Result` containing the `UserResponse` on success, or an `Error` on failure.
    func getUser(userId: String, auth: AuthContextPayload) async -> Result<UserResponse, Error> {
        struct GetUserPayload: Codable {
            let id: String
            let auth: AuthContextPayload
        }
        let payload = GetUserPayload(id: userId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_get)
    }

    /// Retrieves a list of all users.
    /// - Parameter auth: The authentication context.
    /// - Returns: A `Result` containing an array of `UserResponse` on success, or an `Error` on failure.
    func getAllUsers(auth: AuthContextPayload) async -> Result<[UserResponse], Error> {
        struct GetAllUsersPayload: Codable {
            let auth: AuthContextPayload
        }
        let payload = GetAllUsersPayload(auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_get_all)
    }

    /// Updates a user's details.
    /// - Parameters:
    ///   - userId: The UUID of the user to update.
    ///   - update: An `UpdateUser` object with the fields to change.
    ///   - auth: The authentication context.
    /// - Returns: A `Result` containing the updated `UserResponse` on success, or an `Error` on failure.
    func updateUser(userId: String, update: UpdateUser, auth: AuthContextPayload) async -> Result<UserResponse, Error> {
        struct UpdateUserPayload: Codable {
            let id: String
            let update: UpdateUser
            let auth: AuthContextPayload
        }
        let payload = UpdateUserPayload(id: userId, update: update, auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_update)
    }
    
    /// Permanently deletes a user.
    /// - Parameters:
    ///   - userId: The UUID of the user to delete.
    ///   - auth: The authentication context.
    /// - Returns: A `Result` indicating success or failure.
    func hardDeleteUser(userId: String, auth: AuthContextPayload) async -> Result<Void, Error> {
        struct DeleteUserPayload: Codable {
            let id: String
            let auth: AuthContextPayload
        }
        let payload = DeleteUserPayload(id: userId, auth: auth)
        return await executeVoidOperation(payload: payload, ffiCall: user_hard_delete)
    }
    
    // MARK: - Utility Functions
    
    /// Checks if an email is unique.
    /// - Parameters:
    ///   - email: The email to check.
    ///   - excludeId: An optional user ID to exclude from the check.
    ///   - auth: The authentication context.
    /// - Returns: A `Result` containing an `EmailUniquenessResponse` on success, or an `Error` on failure.
    func isEmailUnique(email: String, excludeId: String?, auth: AuthContextPayload) async -> Result<EmailUniquenessResponse, Error> {
        struct EmailUniquenessPayload: Codable {
            let email: String
            let exclude_id: String?
            let auth: AuthContextPayload
        }
        let payload = EmailUniquenessPayload(email: email, exclude_id: excludeId, auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_is_email_unique)
    }

    /// Retrieves a summary of user statistics.
    /// - Parameter auth: The authentication context.
    /// - Returns: A `Result` containing the `UserStats` on success, or an `Error` on failure.
    func getUserStats(auth: AuthContextPayload) async -> Result<UserStats, Error> {
        struct GetUserStatsPayload: Codable {
            let auth: AuthContextPayload
        }
        let payload = GetUserStatsPayload(auth: auth)
        return await executeOperation(payload: payload, ffiCall: user_get_stats)
    }

    /// Changes the password for the user in the provided auth context.
    /// - Parameters:
    ///   - oldPassword: The user's current password.
    ///   - newPassword: The desired new password.
    ///   - auth: The authentication context of the user changing their password.
    /// - Returns: A `Result` indicating success or failure.
    func changePassword(oldPassword: String, newPassword: String, auth: AuthContextPayload) async -> Result<Void, Error> {
        struct ChangePasswordPayload: Codable {
            let old_password: String
            let new_password: String
            let auth: AuthContextPayload
        }
        let payload = ChangePasswordPayload(old_password: oldPassword, new_password: newPassword, auth: auth)
        return await executeVoidOperation(payload: payload, ffiCall: user_change_password)
    }

    // MARK: - Private Helpers
    
    private func encode<T: Encodable>(_ value: T) throws -> String {
        let data = try jsonEncoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw FFIError.stringConversionFailed
        }
        return string
    }

    private func executeOperation<P: Encodable, R: Decodable>(
        payload: P,
        ffiCall: @escaping (UnsafePointer<CChar>, UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    ) async -> Result<R, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonPayload = try self.encode(payload)
                    let ffiResult = FFIHelper.execute(
                        call: { resultPtr in
                            jsonPayload.withCString { cJson in
                                ffiCall(cJson, resultPtr)
                            }
                        },
                        parse: { responseString in
                            guard let data = responseString.data(using: .utf8) else {
                                throw FFIError.stringConversionFailed
                            }
                            return try self.jsonDecoder.decode(R.self, from: data)
                        },
                        free: user_free
                    )
                    
                    if let value = ffiResult.value {
                        continuation.resume(returning: .success(value))
                    } else if let error = ffiResult.error {
                        continuation.resume(returning: .failure(FFIError.rustError(error)))
                    } else {
                        continuation.resume(returning: .failure(FFIError.unknown))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }
    
    private func executeVoidOperation<P: Encodable>(
        payload: P,
        ffiCall: @escaping (UnsafePointer<CChar>) -> CInt
    ) async -> Result<Void, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                do {
                    let jsonPayload = try self.encode(payload)
                    let status = jsonPayload.withCString { cJson in
                        ffiCall(cJson)
                    }
                    if status == 0 {
                        continuation.resume(returning: .success(()))
                    } else {
                        let error = FFIHelper.getLastError()
                        continuation.resume(returning: .failure(FFIError.rustError(error)))
                    }
                } catch {
                    continuation.resume(returning: .failure(error))
                }
            }
        }
    }
} 