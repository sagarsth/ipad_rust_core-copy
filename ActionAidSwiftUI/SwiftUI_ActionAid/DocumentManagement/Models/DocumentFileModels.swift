//
//  DocumentFileModels.swift
//  ActionAid SwiftUI
//
//  Data models for document file management
//

import Foundation

// MARK: - Document File Models

/// Legacy document file model (stores data in memory)
struct DocumentFile: Identifiable {
    let id: UUID
    let name: String
    let size: Int
    let detectedType: String
    var tempURL: URL? // Store file in temp directory instead of memory
    
    // Computed property to get data when needed
    var data: Data? {
        guard let tempURL = tempURL else { return nil }
        return try? Data(contentsOf: tempURL)
    }
    
    init(name: String, data: Data, size: Int, detectedType: String) {
        self.id = UUID()
        self.name = name
        self.size = size
        self.detectedType = detectedType
        
        // Store data in temporary file to avoid memory issues
        let tempDir = FileManager.default.temporaryDirectory
        let tempURL = tempDir.appendingPathComponent(self.id.uuidString)
        
        do {
            try data.write(to: tempURL)
            self.tempURL = tempURL
        } catch {
            print("❌ Failed to write temp file: \(error)")
            self.tempURL = nil
        }
    }
    
    func cleanup() {
        guard let tempURL = tempURL else { return }
        try? FileManager.default.removeItem(at: tempURL)
    }
}

// MARK: - iOS Optimized Document File (Path-Based, No Memory Copy)

/// Optimized document file model (uses file paths, not memory storage)
struct OptimizedDocumentFile: Identifiable {
    let id: UUID
    let name: String
    let tempPath: String              // Direct path to temp file (no Data loading!)
    let size: Int
    let detectedType: String
    
    init(name: String, tempPath: String, size: Int, detectedType: String) {
        self.id = UUID()
        self.name = name
        self.tempPath = tempPath
        self.size = size
        self.detectedType = detectedType
    }
    
    func cleanup() {
        try? FileManager.default.removeItem(atPath: tempPath)
    }
} 