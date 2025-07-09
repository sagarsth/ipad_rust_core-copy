//
//  DocumentRow.swift
//  ActionAid SwiftUI
//
//  Reusable document row component for displaying document information
//

import SwiftUI

// MARK: - Document Row Component

/// A reusable row component for displaying document information
struct DocumentRow: View {
    let document: MediaDocumentResponse
    let onTap: () -> Void
    
    var body: some View {
        HStack {
            Image(systemName: fileIcon(for: document.originalFilename))
                .font(.title3)
                .foregroundColor(document.isAvailableLocally ?? false ? .blue : .gray)
                .frame(width: 40)
            
            VStack(alignment: .leading, spacing: 2) {
                Text(document.title ?? document.originalFilename)
                    .font(.subheadline)
                    .lineLimit(1)
                
                HStack(spacing: 8) {
                    Text(document.typeName ?? "Document")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if let field = document.fieldIdentifier {
                        Text("• Linked to \(field)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                    
                    Text("• \(formatFileSize(document.sizeBytes))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if !(document.isAvailableLocally ?? false) {
                        Text("• Cloud")
                            .font(.caption2)
                            .foregroundColor(.orange)
                    }
                }
            }
            
            Spacer()
            
            CompressionBadge(status: document.compressionStatus)
        }
        .padding(.vertical, 8)
        .opacity((document.hasError == true) ? 0.5 : 1.0)
        .contentShape(Rectangle()) // Make entire row tappable
        .onTapGesture {
            onTap()
        }
    }
    
    // MARK: - Helper Methods
    
    private func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png": return "photo.fill"
        case "xls", "xlsx": return "tablecells.fill"
        case "mp4", "mov": return "video.fill"
        case "mp3", "m4a": return "music.note"
        default: return "doc.fill"
        }
    }
    
    private func formatFileSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }
}

// MARK: - Media Document Row (Legacy - for backward compatibility)
struct MediaDocumentRow: View {
    let document: MediaDocumentResponse
    let onTap: () -> Void
    
    var body: some View {
        HStack {
            Image(systemName: fileIcon(for: document.originalFilename))
                .font(.title3)
                .foregroundColor(document.isAvailableLocally ?? false ? .blue : .gray)
                .frame(width: 40)
            
            VStack(alignment: .leading, spacing: 2) {
                Text(document.title ?? document.originalFilename)
                    .font(.subheadline)
                    .lineLimit(1)
                
                HStack(spacing: 8) {
                    Text(document.typeName ?? "Document")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if let field = document.fieldIdentifier {
                        Text("• Linked to \(field)")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                    
                    Text("• \(DocumentFileUtils.formatFileSize(document.sizeBytes))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    
                    if !(document.isAvailableLocally ?? false) {
                        Text("• Cloud")
                            .font(.caption2)
                            .foregroundColor(.orange)
                    }
                }
            }
            
            Spacer()
            
            CompressionBadge(status: document.compressionStatus)
        }
        .padding(.vertical, 8)
        .opacity((document.hasError == true) ? 0.5 : 1.0)
        .contentShape(Rectangle()) // Make entire row tappable
        .onTapGesture {
            onTap()
        }
    }
    
    private func fileIcon(for filename: String) -> String {
        return DocumentFileUtils.fileIcon(for: filename)
    }
}

// MARK: - Document File Utilities
struct DocumentFileUtils {
    static func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png": return "photo.fill"
        case "xls", "xlsx": return "tablecells.fill"
        case "mp4", "mov": return "video.fill"
        case "mp3", "m4a": return "music.note"
        default: return "doc.fill"
        }
    }
    
    static func formatFileSize(_ bytes: Int64) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: bytes)
    }
} 