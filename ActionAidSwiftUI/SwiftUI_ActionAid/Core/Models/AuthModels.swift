//
//  AuthModels.swift
//  SwiftUI_ActionAid
//
//  This file defines the data structures used for authentication processes.
//  These models correspond to the JSON data sent to and received from
//  the Rust core's authentication FFI functions.
//

import Foundation

// MARK: - Authentication Data Models

/// The response object returned after a successful user login.
/// It contains the necessary tokens and basic user information.
struct AuthLoginResponse: Codable {
    let access_token: String
    let access_expiry: String
    let refresh_token: String
    let refresh_expiry: String
    let user_id: String
    let role: String
}

/// The response object returned after successfully refreshing an access token.
struct RefreshedTokenResponse: Codable {
    let access_token: String
    let access_expiry: String
}

/// Represents the verified authentication context for a user session.
/// This data is returned when a token is successfully verified.
struct AuthContextResponse: Codable {
    let user_id: String
    let role: String
    let device_id: String
    let offline_mode: Bool
} 