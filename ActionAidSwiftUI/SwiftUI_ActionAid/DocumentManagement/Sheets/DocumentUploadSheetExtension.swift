//
//  DocumentUploadSheetExtension.swift
//  ActionAid SwiftUI
//
//  Helper methods and file handling logic for DocumentUploadSheet
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI

// MARK: - Document Upload Sheet Extension

extension DocumentUploadSheet {
    
    // MARK: - Computed Properties
    
    /// Allowed file types for upload based on configuration
    var allowedFileTypes: [UTType] {
        var types: [UTType] = []
        
        // Documents
        types.append(contentsOf: [.pdf, .rtf, .plainText, .html])
        
        // Add custom UTTypes for additional document formats
        if let mdType = UTType(filenameExtension: "md") { types.append(mdType) }
        if let pagesType = UTType(filenameExtension: "pages") { types.append(pagesType) }
        if let numbersType = UTType(filenameExtension: "numbers") { types.append(numbersType) }
        if let keynoteType = UTType(filenameExtension: "key") { types.append(keynoteType) }
        
        // Images
        types.append(contentsOf: [.jpeg, .png, .heic, .gif, .webP, .bmp, .tiff, .svg])
        
        // Add custom UTTypes for additional image formats
        if let heifType = UTType(filenameExtension: "heif") { types.append(heifType) }
        if let avifType = UTType(filenameExtension: "avif") { types.append(avifType) }
        
        // Videos
        types.append(contentsOf: [.quickTimeMovie, .mpeg4Movie, .video, .avi])
        
        // Add custom UTTypes for additional video formats
        if let mkvType = UTType(filenameExtension: "mkv") { types.append(mkvType) }
        if let webmType = UTType(filenameExtension: "webm") { types.append(webmType) }
        if let threegpType = UTType(filenameExtension: "3gp") { types.append(threegpType) }
        if let m4vType = UTType(filenameExtension: "m4v") { types.append(m4vType) }
        
        // Audio
        types.append(contentsOf: [.mp3, .wav, .aiff, .audio])
        
        // Add custom UTTypes for additional audio formats
        if let aacType = UTType(filenameExtension: "aac") { types.append(aacType) }
        if let flacType = UTType(filenameExtension: "flac") { types.append(flacType) }
        if let m4aType = UTType(filenameExtension: "m4a") { types.append(m4aType) }
        if let oggType = UTType(filenameExtension: "ogg") { types.append(oggType) }
        if let opusType = UTType(filenameExtension: "opus") { types.append(opusType) }
        if let cafType = UTType(filenameExtension: "caf") { types.append(cafType) }
        
        // Archives
        types.append(contentsOf: [.zip, .gzip])
        
        // Add custom UTTypes for additional archive formats
        if let rarType = UTType(filenameExtension: "rar") { types.append(rarType) }
        if let sevenZipType = UTType(filenameExtension: "7z") { types.append(sevenZipType) }
        if let tarType = UTType(filenameExtension: "tar") { types.append(tarType) }
        if let bz2Type = UTType(filenameExtension: "bz2") { types.append(bz2Type) }
        
        // Office docs
        types.append(contentsOf: [.spreadsheet, .presentation])
        
        // Add custom UTTypes for additional document formats
        if let docType = UTType(filenameExtension: "doc") { types.append(docType) }
        if let docxType = UTType(filenameExtension: "docx") { types.append(docxType) }
        if let xlsType = UTType(filenameExtension: "xls") { types.append(xlsType) }
        if let xlsxType = UTType(filenameExtension: "xlsx") { types.append(xlsxType) }
        if let pptType = UTType(filenameExtension: "ppt") { types.append(pptType) }
        if let pptxType = UTType(filenameExtension: "pptx") { types.append(pptxType) }
        if let odtType = UTType(filenameExtension: "odt") { types.append(odtType) }
        if let odsType = UTType(filenameExtension: "ods") { types.append(odsType) }
        if let odpType = UTType(filenameExtension: "odp") { types.append(odpType) }
        if let csvType = UTType(filenameExtension: "csv") { types.append(csvType) }
        if let tsvType = UTType(filenameExtension: "tsv") { types.append(tsvType) }
        
        // Add custom UTTypes for code files
        let codeExtensions = ["html", "css", "js", "json", "xml", "yaml", "yml", "sql", "py", "rs", "swift", "java", "cpp", "c", "h"]
        for ext in codeExtensions {
            if let codeType = UTType(filenameExtension: ext) {
                types.append(codeType)
            }
        }
        
        // Add custom UTTypes for data files
        let dataExtensions = ["db", "sqlite", "backup"]
        for ext in dataExtensions {
            if let dataType = UTType(filenameExtension: ext) {
                types.append(dataType)
            }
        }
        
        // Fallback for other file types
        types.append(contentsOf: [.data, .item])
        
        return types
    }
    
    // Note: handleFileSelection and handlePhotoSelection are defined in the main DocumentUploadSheet
    
    func generatePhotoFilename(for photo: PhotosPickerItem, baseTimestamp: TimeInterval, sequenceIndex: Int) -> String {
        let timestamp = baseTimestamp + (Double(sequenceIndex) * 0.001) + (Date().timeIntervalSince1970.truncatingRemainder(dividingBy: 1))
        
        let shortId: String
        if let identifier = photo.itemIdentifier {
            shortId = String(identifier.prefix(8))
        } else {
            shortId = "seq\(sequenceIndex)"
        }
        
        let microsecondTimestamp = String(format: "%.6f", timestamp).replacingOccurrences(of: ".", with: "")
        
        // Check for video types first
        for supportedType in photo.supportedContentTypes {
            if supportedType.identifier == "public.mpeg-4" || supportedType.identifier.contains("mpeg-4") {
                return "video_\(shortId)_\(microsecondTimestamp).mp4"
            }
            if supportedType.identifier == "com.apple.quicktime-movie" || supportedType.identifier.contains("quicktime") {
                return "video_\(shortId)_\(microsecondTimestamp).mov"
            }
            if supportedType.identifier.contains("video") {
                return "video_\(shortId)_\(microsecondTimestamp).mp4"
            }
        }
        
        // Check for image types
        if photo.supportedContentTypes.contains(.heif) || photo.supportedContentTypes.contains(.heic) {
            return "photo_\(shortId)_\(microsecondTimestamp).heic"
        } else if photo.supportedContentTypes.contains(.jpeg) {
            return "photo_\(shortId)_\(microsecondTimestamp).jpg"
        } else if photo.supportedContentTypes.contains(.png) {
            return "photo_\(shortId)_\(microsecondTimestamp).png"
        }
        
        return "media_\(microsecondTimestamp).unknown"
    }
    
    func detectDocumentType(from filename: String) -> String {
        let fileExtension = (filename as NSString).pathExtension.lowercased()
        
        // Special handling for generated filenames
        if filename.contains("video_") && (fileExtension == "mp4" || fileExtension == "mov" || fileExtension == "unknown") {
            return "Video"
        }
        if filename.contains("photo_") && (fileExtension == "jpg" || fileExtension == "png" || fileExtension == "heic" || fileExtension == "unknown") {
            return "Image"
        }
        
        // Map extensions to document types
        switch fileExtension {
        case "jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "bmp", "tiff", "svg":
            return "Image"
        case "pdf", "doc", "docx", "rtf", "txt", "md", "pages", "odt":
            return "Document"
        case "xlsx", "xls", "numbers", "csv", "tsv", "ods":
            return "Spreadsheet"
        case "pptx", "ppt", "key", "odp":
            return "Presentation"
        case "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp":
            return "Video"
        case "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "caf":
            return "Audio"
        case "zip", "rar", "7z", "tar", "gz", "bz2":
            return "Archive"
        case "html", "css", "js", "json", "xml", "yaml", "yml", "sql", "py", "rs", "swift", "java", "cpp", "c", "h":
            return "Code"
        case "db", "sqlite", "backup":
            return "Data"
        case "unknown":
            if filename.contains("video_") {
                return "Video"
            } else if filename.contains("photo_") {
                return "Image"
            } else {
                return "Document"
            }
        default:
            return "Unknown (\(fileExtension))"
        }
    }
    
    func fileIcon(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx", "rtf", "pages", "odt": return "doc.richtext.fill"
        case "txt", "md": return "doc.text"
        case "jpg", "jpeg", "png", "heic", "heif", "webp", "gif", "bmp", "tiff": return "photo.fill"
        case "svg": return "photo.artframe"
        case "mp4", "mov", "m4v", "avi", "mkv", "webm", "3gp": return "video.fill"
        case "mp3", "m4a", "wav", "aac", "flac", "ogg", "opus", "caf": return "music.note"
        case "xlsx", "xls", "numbers", "csv", "tsv", "ods": return "tablecells.fill"
        case "pptx", "ppt", "key", "odp": return "rectangle.on.rectangle.fill"
        case "zip", "rar", "7z", "tar", "gz", "bz2": return "archivebox.fill"
        case "html", "css": return "chevron.left.forwardslash.chevron.right"
        case "js", "json", "xml", "yaml", "yml": return "curlybraces"
        case "sql": return "tablecells"
        case "py", "rs", "swift", "java", "cpp", "c", "h": return "chevron.left.forwardslash.chevron.right"
        case "db", "sqlite", "backup": return "externaldrive.fill"
        default: return "doc.fill"
        }
    }
    
    func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
    
    func getFieldDisplayName(for fieldKey: String) -> String {
        return entity.linkableFields.first { $0.0 == fieldKey }?.1 ?? fieldKey
    }
    

    
    // MARK: - Upload Methods
    
    func uploadDocuments() {
        isUploading = true
        uploadResults = []
        errorMessage = nil
        
        Task {
            guard let currentUser = authManager.currentUser else {
                await MainActor.run {
                    self.errorMessage = "User not authenticated."
                    self.isUploading = false
                }
                return
            }
            
            let authContext = AuthContextPayload(
                user_id: currentUser.userId,
                role: currentUser.role,
                device_id: authManager.getDeviceId(),
                offline_mode: false
            )
            
            // Get document type ID
            let firstFileName = fileManager.optimizedFiles.first?.name ?? ""
            let specificDocTypeId = await entity.detectDocumentTypeId(for: firstFileName, auth: authContext)
            let defaultDocTypeId = await entity.getDefaultDocumentTypeId(auth: authContext)
            let finalDocTypeId = specificDocTypeId ?? defaultDocTypeId ?? "00000000-0000-0000-0000-000000000000"
            
            if isSingleUpload {
                // Single upload with field linking
                if let file = fileManager.optimizedFiles.first {
                    let result = await entity.uploadDocument(
                        filePath: file.tempPath,
                        originalFilename: file.name,
                        title: documentTitle.isEmpty ? nil : documentTitle,
                        documentTypeId: finalDocTypeId,
                        linkedField: linkedField.isEmpty ? nil : linkedField,
                        syncPriority: priority,
                        compressionPriority: .normal,
                        auth: authContext
                    )
                    
                    await MainActor.run {
                        self.isUploading = false
                        
                        switch result {
                        case .success(let document):
                            self.uploadResults = [UploadResult(
                                filename: document.originalFilename,
                                success: true,
                                message: "✅ iOS Optimized Upload - \(document.typeName ?? "Document")" +
                                        (linkedField.isEmpty ? "" : " (linked to \(getFieldDisplayName(for: linkedField)))")
                            )]
                            
                            onUploadComplete()
                            
                            // Clean up temp file after successful upload
                            file.cleanup()
                            
                            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                dismiss()
                            }
                            
                        case .failure(let error):
                            self.errorMessage = "Upload failed: \(error.localizedDescription)"
                            self.uploadResults = [UploadResult(
                                filename: file.name,
                                success: false,
                                message: "Failed to upload"
                            )]
                            
                            // Clean up temp file after upload attempt
                            file.cleanup()
                        }
                    }
                }
            } else {
                // Bulk upload - upload each file individually to ensure correct document types
                var allResults: [UploadResult] = []
                var hasAnySuccess = false
                
                for file in fileManager.optimizedFiles {
                    // Detect specific document type for each file
                    let specificDocTypeId = await entity.detectDocumentTypeId(for: file.name, auth: authContext)
                    let defaultDocTypeId = await entity.getDefaultDocumentTypeId(auth: authContext)
                    let fileDocTypeId = specificDocTypeId ?? defaultDocTypeId ?? "00000000-0000-0000-0000-000000000000"
                    
                    // Upload each file individually
                    let result = await entity.uploadDocument(
                        filePath: file.tempPath,
                        originalFilename: file.name,
                        title: documentTitle.isEmpty ? nil : documentTitle,
                        documentTypeId: fileDocTypeId,
                        linkedField: nil, // Bulk uploads don't support field linking
                        syncPriority: priority,
                        compressionPriority: .normal,
                        auth: authContext
                    )
                    
                    switch result {
                    case .success(let document):
                        allResults.append(UploadResult(
                            filename: document.originalFilename,
                            success: true,
                            message: "✅ iOS Optimized Upload - \(document.typeName ?? "Document")"
                        ))
                        hasAnySuccess = true
                        
                    case .failure(let error):
                        allResults.append(UploadResult(
                            filename: file.name,
                            success: false,
                            message: "Failed to upload: \(error.localizedDescription)"
                        ))
                    }
                    
                    // Clean up individual file after upload attempt
                    file.cleanup()
                }
                
                await MainActor.run {
                    self.isUploading = false
                    self.uploadResults = allResults
                    
                    if hasAnySuccess {
                        onUploadComplete()
                        
                        // Clear the file manager since individual files were already cleaned up
                        fileManager.clearAll()
                        
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                            dismiss()
                        }
                    } else {
                        self.errorMessage = "All uploads failed. Please check the files and try again."
                    }
                }
            }
        }
    }
} 