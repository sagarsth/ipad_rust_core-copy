//
//  UploadModels.swift
//  ActionAid SwiftUI
//
//  Models for document upload results and tracking
//

import Foundation

// MARK: - Upload Result Model

/// Result of a document upload operation
struct UploadResult: Identifiable {
    let id = UUID()
    let filename: String
    let success: Bool
    let message: String
} 