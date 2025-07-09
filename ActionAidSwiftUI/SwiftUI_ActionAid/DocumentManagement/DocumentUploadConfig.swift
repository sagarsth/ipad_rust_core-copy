//
//  DocumentUploadConfig.swift
//  ActionAid SwiftUI
//
//  Configuration for the generic DocumentUploadSheet
//

import Foundation

// MARK: - Document Upload Configuration

/// Configuration for document upload functionality
struct DocumentUploadConfig {
    /// Maximum file size in bytes (default: 500MB)
    let maxFileSize: Int
    
    /// Maximum total size across all files in bytes (default: 2GB)
    let maxTotalSize: Int64
    
    /// Whether field linking is allowed for this entity type
    let allowFieldLinking: Bool
    
    /// Maximum number of files that can be uploaded at once
    let maxFileCount: Int
    
    /// Maximum number of photos/videos that can be selected at once from PhotosPicker
    let maxPhotoSelectionCount: Int
    
    /// File extensions that are blocked
    let blockedExtensions: [String]
    
    // MARK: - Initializers
    
    init(
        maxFileSize: Int = 500_000_000,  // 500MB
        maxTotalSize: Int64 = 2_000_000_000,  // 2GB
        allowFieldLinking: Bool = true,
        maxFileCount: Int = 50,
        maxPhotoSelectionCount: Int = 25,  // FIXED: Increased from hardcoded 10 and made configurable
        blockedExtensions: [String] = ["dmg", "iso", "app", "pkg", "exe", "msi"]
    ) {
        self.maxFileSize = maxFileSize
        self.maxTotalSize = maxTotalSize
        self.allowFieldLinking = allowFieldLinking
        self.maxFileCount = maxFileCount
        self.maxPhotoSelectionCount = maxPhotoSelectionCount
        self.blockedExtensions = blockedExtensions
    }
    
    // MARK: - Predefined Configurations
    
    /// Standard configuration for most entity types
    static let standard = DocumentUploadConfig()
    
    /// Configuration for entities that don't support field linking
    static let noFieldLinking = DocumentUploadConfig(allowFieldLinking: false)
    
    /// Configuration with smaller file size limits
    static let restricted = DocumentUploadConfig(
        maxFileSize: 100_000_000,  // 100MB
        maxTotalSize: 500_000_000,  // 500MB
        maxFileCount: 10,
        maxPhotoSelectionCount: 10  // Keep lower limit for restricted config
    )
    
    /// Configuration for high-capacity uploads
    static let highCapacity = DocumentUploadConfig(
        maxFileSize: 1_000_000_000,  // 1GB
        maxTotalSize: 10_000_000_000,  // 10GB
        maxFileCount: 100,
        maxPhotoSelectionCount: 50  // Higher photo limit for high-capacity
    )
    
    /// Configuration specifically optimized for photo/video uploads
    static let photoVideo = DocumentUploadConfig(
        maxFileSize: 2_000_000_000,  // 2GB for large videos
        maxTotalSize: 10_000_000_000,  // 10GB total
        allowFieldLinking: false,  // Usually not needed for bulk photo uploads
        maxFileCount: 100,
        maxPhotoSelectionCount: 100  // High limit for photo/video focused uploads
    )
} 