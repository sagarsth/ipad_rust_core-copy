//
//  UserModels.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/20/23.
//

import Foundation

/// DTO for user login credentials.
struct Credentials: Codable {
    let email: String
    let password: String
}

/// DTO for creating a new user.
struct NewUser: Codable {
    let email: String
    let password: String
    let name: String
    let role: String
    let active: Bool
    
    // This field is optional in Rust, so we'll mark it optional here too.
    // It will be set on the Rust side if needed.
    let created_by_user_id: String?
    
    enum CodingKeys: String, CodingKey {
        case email, password, name, role, active
        case created_by_user_id = "created_by_user_id"
    }
    
    // Convenience initializer with default active = true
    init(email: String, password: String, name: String, role: String, active: Bool = true, created_by_user_id: String? = nil) {
        self.email = email
        self.password = password
        self.name = name
        self.role = role
        self.active = active
        self.created_by_user_id = created_by_user_id
    }
}

/// DTO for updating an existing user. All fields are optional.
struct UpdateUser: Codable {
    var email: String?
    var password: String?
    var name: String?
    var role: String?
    var active: Bool?
    
    // This is set on the Rust side and not sent from Swift,
    // so we can exclude it from encoding.
    // let updated_by_user_id: String?
    
    enum CodingKeys: String, CodingKey {
        case email, password, name, role, active
    }
}

/// Response DTO for user operations that return user data.
struct UserResponse: Codable {
    let id: String
    let email: String
    let name: String
    let role: String
    let active: Bool
    let last_login: String?
    let created_at: String
    let updated_at: String?
    
    // Audit fields
    let created_by_user_id: String?
    let updated_by_user_id: String?
    let created_by_device_id: String?
    let updated_by_device_id: String?
    
    // Enriched fields (usernames)
    let created_by: String?
    let updated_by: String?
    
    enum CodingKeys: String, CodingKey {
        case id, email, name, role, active
        case last_login = "last_login"
        case created_at = "created_at"
        case updated_at = "updated_at"
        case created_by_user_id = "created_by_user_id"
        case updated_by_user_id = "updated_by_user_id"
        case created_by_device_id = "created_by_device_id"
        case updated_by_device_id = "updated_by_device_id"
        case created_by = "created_by"
        case updated_by = "updated_by"
    }
}

/// Response DTO for email uniqueness checks.
struct EmailUniquenessResponse: Codable {
    let is_unique: Bool
    
    enum CodingKeys: String, CodingKey {
        case is_unique = "is_unique"
    }
}

/// Auth context payload for operations requiring authentication.
struct AuthContextPayload: Codable {
    let user_id: String
    let role: String
    let device_id: String
    let offline_mode: Bool
    
    enum CodingKeys: String, CodingKey {
        case user_id = "user_id"
        case role
        case device_id = "device_id"
        case offline_mode = "offline_mode"
    }
}

/// A struct that holds the summary of user counts by role and status.
/// This mirrors the `UserStats` struct in Rust.
struct UserStats: Codable {
    let total: Int
    let active: Int
    let inactive: Int
    let admin: Int
    let fieldTl: Int
    let field: Int
} 