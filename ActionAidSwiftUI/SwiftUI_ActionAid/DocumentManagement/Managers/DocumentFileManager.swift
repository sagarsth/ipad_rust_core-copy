//
//  DocumentFileManager.swift
//  ActionAid SwiftUI
//
//  Manages document file selection, storage, and size limits
//

import Foundation
import SwiftUI

// MARK: - Document File Manager

/// Manages document file selection and storage with size limits
class DocumentFileManager: ObservableObject {
    @Published var selectedFiles: [DocumentFile] = []
    @Published var optimizedFiles: [OptimizedDocumentFile] = []  // New: Path-based files
    @Published var totalSize: Int64 = 0
    
    private let maxFileSize: Int = 500_000_000 // 500MB per file
    private let maxTotalSize: Int64 = 2000_000_000 // 2000MB total
    
    // MARK: - File Management Methods
    
    /// Add a legacy document file (data-based)
    func addFile(_ file: DocumentFile) -> Bool {
        // Check individual file size
        if file.size > maxFileSize {
            return false
        }
        
        // Check total size
        let newTotalSize = totalSize + Int64(file.size)
        if newTotalSize > maxTotalSize {
            return false
        }
        
        selectedFiles.append(file)
        totalSize = newTotalSize
        return true
    }
    
    /// Add an optimized document file (path-based, no memory overhead!)
    func addOptimizedFile(_ file: OptimizedDocumentFile) -> Bool {
        // Check individual file size
        if file.size > maxFileSize {
            return false
        }
        
        // Check total size
        let newTotalSize = totalSize + Int64(file.size)
        if newTotalSize > maxTotalSize {
            return false
        }
        
        optimizedFiles.append(file)
        totalSize = newTotalSize
        return true
    }
    
    /// Remove a file by ID
    func removeFile(withId id: UUID) {
        // Check legacy files
        if let index = selectedFiles.firstIndex(where: { $0.id == id }) {
            let file = selectedFiles[index]
            file.cleanup() // Clean up temp file
            totalSize -= Int64(file.size)
            selectedFiles.remove(at: index)
        }
        // Check optimized files
        else if let index = optimizedFiles.firstIndex(where: { $0.id == id }) {
            let file = optimizedFiles[index]
            file.cleanup() // Clean up temp file
            totalSize -= Int64(file.size)
            optimizedFiles.remove(at: index)
        }
    }
    
    /// Clear all files and cleanup temp storage
    func clearAll() {
        for file in selectedFiles {
            file.cleanup()
        }
        for file in optimizedFiles {
            file.cleanup()
        }
        selectedFiles.removeAll()
        optimizedFiles.removeAll()
        totalSize = 0
    }
    
    // MARK: - Computed Properties
    
    var count: Int { selectedFiles.count + optimizedFiles.count }
    var isEmpty: Bool { selectedFiles.isEmpty && optimizedFiles.isEmpty }
    
    /// Combined files for UI display
    var allFiles: [(id: UUID, name: String, size: Int, detectedType: String)] {
        var combined: [(id: UUID, name: String, size: Int, detectedType: String)] = []
        
        for file in selectedFiles {
            combined.append((id: file.id, name: file.name, size: file.size, detectedType: file.detectedType))
        }
        
        for file in optimizedFiles {
            combined.append((id: file.id, name: file.name, size: file.size, detectedType: file.detectedType))
        }
        
        return combined
    }
    
    /// Get human-readable size description
    func getSizeDescription() -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: totalSize)
    }
    
    // MARK: - Cleanup
    
    deinit {
        clearAll()
    }
} 