//
//  DocumentUploadSheet.swift
//  ActionAid SwiftUI
//
//  Generic document upload sheet that works with any DocumentIntegratable entity
//

import SwiftUI
import UniformTypeIdentifiers
import PhotosUI

// MARK: - Generic Document Upload Sheet

/// Generic document upload sheet that can work with any entity supporting documents
struct DocumentUploadSheet<Entity: DocumentUploadable>: View {
    let entity: Entity
    let config: DocumentUploadConfig
    let onUploadComplete: () -> Void
    
    @Environment(\.dismiss) var dismiss
    @EnvironmentObject var authManager: AuthenticationManager
    
    @State internal var documentTitle = ""
    @State internal var linkedField = ""
    @State internal var priority: SyncPriority = .normal
    @StateObject internal var fileManager = DocumentFileManager()
    @State internal var showFilePicker = false
    @State internal var selectedPhotos: [PhotosPickerItem] = []
    @State internal var isUploading = false
    @State internal var uploadResults: [UploadResult] = []
    @State internal var errorMessage: String?
    
    // FIXED: Add photo processing state to prevent UI refreshes
    @State private var isProcessingPhotos = false
    @State private var processedPhotoCount = 0
    @State private var totalPhotosToProcess = 0
    
    // FIXED: Add Face ID authentication state tracking
    @State private var isPhotoPickerAuthenticating = false
    @State private var photoPickerAuthStartTime: Date?
    
    // MARK: - Computed Properties
    
    internal var isSingleUpload: Bool {
        fileManager.count == 1
    }
    
    internal var isBulkUpload: Bool {
        fileManager.count > 1
    }
    
    internal var uploadModeDescription: String {
        if fileManager.isEmpty {
            return "No files selected"
        } else if isSingleUpload {
            return "Single file upload"
        } else {
            return "Bulk upload (\(fileManager.count) files) - \(fileManager.getSizeDescription())"
        }
    }
    
    internal var isUploadDisabled: Bool {
        return fileManager.isEmpty || isUploading || isProcessingPhotos || (isSingleUpload && config.allowFieldLinking && linkedField.isEmpty)
    }
    
    var body: some View {
        NavigationView {
            Form {
                documentInformationSection
                fileSelectionSection
                uploadResultsSection
                helpSection
            }
            .navigationTitle("Upload Documents")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Upload") {
                        uploadDocuments()
                    }
                    .disabled(isUploadDisabled)
                }
            }
            .fileImporter(
                isPresented: $showFilePicker,
                allowedContentTypes: allowedFileTypes,
                allowsMultipleSelection: true
            ) { result in
                handleFileSelection(result)
            }
            .disabled(isUploading)
            .onChange(of: fileManager.count) { oldCount, newCount in
                // Clear linked field when switching from single to bulk mode
                if oldCount == 1 && newCount > 1 {
                    linkedField = ""
                }
            }
            .overlay {
                if isUploading {
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    VStack {
                        ProgressView()
                        Text("Uploading documents...")
                            .foregroundColor(.white)
                    }
                } else if isProcessingPhotos {
                    // FIXED: Show photo processing progress
                    Color.black.opacity(0.3)
                        .ignoresSafeArea()
                    VStack {
                        ProgressView(value: Double(processedPhotoCount), total: Double(totalPhotosToProcess))
                            .progressViewStyle(CircularProgressViewStyle())
                        Text("Processing photos... (\(processedPhotoCount)/\(totalPhotosToProcess))")
                            .foregroundColor(.white)
                    }
                }
            }
            .onDisappear {
                // Clean up temp files when view is dismissed
                fileManager.clearAll()
            }
        }
    }
    
    // MARK: - View Sections
    
    private var documentInformationSection: some View {
        Section("Document Information") {
            TextField("Shared Title (Optional)", text: $documentTitle)
                .help("This title will be applied to all selected documents")
            
            // Upload mode indicator
            if !fileManager.isEmpty {
                HStack {
                    Image(systemName: isSingleUpload ? "doc" : "doc.on.doc")
                        .foregroundColor(isSingleUpload ? .blue : .green)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(uploadModeDescription)
                            .font(.caption)
                            .foregroundColor(.secondary)
                        
                        // Show size warning if approaching limits
                        if fileManager.totalSize > 500_000_000 { // 500MB warning
                            Text("⚠️ Approaching size limit")
                                .font(.caption2)
                                .foregroundColor(.orange)
                        }
                    }
                }
            }
            
            // Linked field - only for single uploads if config allows, disabled for bulk
            if config.allowFieldLinking {
                if isSingleUpload {
                    Picker("Link to Field", selection: $linkedField) {
                        ForEach(entity.linkableFields, id: \.0) { field in
                            Text(field.1).tag(field.0)
                        }
                    }
                    .help("Single uploads can be linked to specific \(entity.entityTypeName.lowercased()) fields")
                } else if isBulkUpload {
                    HStack {
                        Text("Link to Field")
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("Disabled for bulk upload")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
            }
            
            Picker("Priority", selection: $priority) {
                Text("Low").tag(SyncPriority.low)
                Text("Normal").tag(SyncPriority.normal)
                Text("High").tag(SyncPriority.high)
            }
        }
    }
    
    private var fileSelectionSection: some View {
        Section("File Selection") {
            HStack(spacing: 16) {
                Button(action: { showFilePicker = true }) {
                    VStack(spacing: 4) {
                        Image(systemName: "doc.badge.plus")
                            .font(.title2)
                        Text("Documents")
                            .font(.caption)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(Color(.systemGray6))
                    .cornerRadius(8)
                }
                .buttonStyle(PlainButtonStyle())
                
                // FIXED: Use configurable selection limit and improved handling
                PhotosPicker(
                    selection: $selectedPhotos,
                    maxSelectionCount: config.maxPhotoSelectionCount,
                    matching: .any(of: [.images, .videos])
                ) {
                    VStack(spacing: 4) {
                        Image(systemName: isPhotoPickerAuthenticating ? "faceid" : "photo.badge.plus")
                            .font(.title2)
                            .foregroundColor(isPhotoPickerAuthenticating ? .orange : .primary)
                        Text(isPhotoPickerAuthenticating ? "Authenticating..." : "Photos/Videos")
                            .font(.caption)
                            .foregroundColor(isPhotoPickerAuthenticating ? .orange : .primary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(Color(.systemGray6))
                    .cornerRadius(8)
                    .opacity(isPhotoPickerAuthenticating ? 0.8 : 1.0)
                }
                .buttonStyle(PlainButtonStyle())
                .disabled(isPhotoPickerAuthenticating || isProcessingPhotos)
                .onTapGesture {
                    // FIXED: Track Face ID authentication start
                    if !isPhotoPickerAuthenticating && !isProcessingPhotos {
                        isPhotoPickerAuthenticating = true
                        photoPickerAuthStartTime = Date()
                        
                        // Auto-reset authentication state if it takes too long (Face ID timeout)
                        DispatchQueue.main.asyncAfter(deadline: .now() + 10.0) {
                            if self.isPhotoPickerAuthenticating {
                                self.isPhotoPickerAuthenticating = false
                                self.photoPickerAuthStartTime = nil
                            }
                        }
                    }
                }
                .onChange(of: selectedPhotos) { oldPhotos, newPhotos in
                    // FIXED: Clear authentication state when photos are actually selected
                    if isPhotoPickerAuthenticating {
                        isPhotoPickerAuthenticating = false
                        photoPickerAuthStartTime = nil
                    }
                    
                    // FIXED: Only process if we actually have new photos to avoid refresh loops
                    if !newPhotos.isEmpty && newPhotos != oldPhotos {
                        handlePhotoSelection(newPhotos)
                    }
                }
                .onAppear {
                    // FIXED: Reset any stuck photo selection state when view appears
                    if selectedPhotos.count != fileManager.optimizedFiles.filter({ $0.name.contains("photo_") || $0.name.contains("video_") }).count {
                        selectedPhotos = []
                    }
                    
                    // FIXED: Reset authentication state on appear
                    isPhotoPickerAuthenticating = false
                    photoPickerAuthStartTime = nil
                }
            }
            
            // FIXED: Add helpful information about Face ID and selection limits
            if config.maxPhotoSelectionCount < 50 || isPhotoPickerAuthenticating {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        if isPhotoPickerAuthenticating {
                            HStack(spacing: 8) {
                                ProgressView()
                                    .scaleEffect(0.8)
                                Text("Waiting for Face ID authentication...")
                                    .font(.caption)
                                    .foregroundColor(.orange)
                            }
                        }
                        
                        if config.maxPhotoSelectionCount < 50 {
                            Text("📸 You can select up to \(config.maxPhotoSelectionCount) photos/videos at once")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        
                        if isPhotoPickerAuthenticating || photoPickerAuthStartTime != nil {
                            Text("💡 If Face ID is required on your device, photo selection may pause briefly for authentication")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                    .padding(.vertical, 4)
                } header: {
                    Text("Photo Selection")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            
            // Display selected files
            if !fileManager.isEmpty {
                ForEach(fileManager.allFiles, id: \.id) { file in
                    fileRowView(file: file)
                }
            }
        }
    }
    
    private var uploadResultsSection: some View {
        Group {
            if !uploadResults.isEmpty {
                Section("Upload Results") {
                    ForEach(uploadResults) { result in
                        HStack {
                            Image(systemName: result.success ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                                .foregroundColor(result.success ? .green : .red)
                            
                            VStack(alignment: .leading, spacing: 2) {
                                Text(result.filename)
                                    .font(.subheadline)
                                Text(result.message)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                            
                            Spacer()
                        }
                    }
                }
            }
        }
    }
    
    private var helpSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 8) {
                if isSingleUpload {
                    Text("Document type is automatically detected from file extension. Field linking allows you to associate this document with a specific \(entity.entityTypeName.lowercased()) field. Photos and videos from your photo library are supported.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else if isBulkUpload {
                    Text("Document types are automatically detected from file extensions. Bulk uploads are processed efficiently but cannot be linked to specific fields. Photos and videos from your photo library are supported.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else {
                    Text("Document types are automatically detected from file extensions. You can select files from Documents or photos/videos from your photo library.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Divider()
                
                HStack {
                    Image(systemName: "info.circle")
                        .foregroundColor(.blue)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Size Limits:")
                            .font(.caption)
                            .fontWeight(.medium)
                        Text("• Maximum file size: \(formatFileSize(config.maxFileSize))")
                        Text("• Maximum total size: \(formatFileSize(Int(config.maxTotalSize)))")
                        Text("• Photo/Video limit: \(config.maxPhotoSelectionCount) at a time")
                        Text("• Blocked file types: .dmg, .iso, .app, .pkg")
                    }
                    .font(.caption2)
                    .foregroundColor(.secondary)
                }
            }
        }
    }
    
    private func fileRowView(file: (id: UUID, name: String, size: Int, detectedType: String)) -> some View {
        HStack {
            Image(systemName: fileIcon(for: file.name))
                .foregroundColor(isSingleUpload ? .blue : .green)
            
            VStack(alignment: .leading, spacing: 2) {
                Text(file.name)
                    .font(.subheadline)
                    .lineLimit(1)
                HStack {
                    Text("\(formatFileSize(file.size)) • \(file.detectedType)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    if isSingleUpload && !linkedField.isEmpty {
                        Text("• Will link to \(getFieldDisplayName(for: linkedField))")
                            .font(.caption2)
                            .foregroundColor(.blue)
                    }
                }
                
                // Show file size warning
                if file.size > 20_000_000 { // 20MB
                    Text("⚠️ Large file - may take time to upload")
                        .font(.caption2)
                        .foregroundColor(.orange)
                }
                
                // Show optimization indicator
                if fileManager.optimizedFiles.contains(where: { $0.id == file.id }) {
                    Text("⚡ iOS Optimized (No Base64)")
                        .font(.caption2)
                        .foregroundColor(.green)
                }
            }
            
            Spacer()
            
            Button(action: {
                fileManager.removeFile(withId: file.id)
            }) {
                Image(systemName: "minus.circle.fill")
                    .foregroundColor(.red)
            }
        }
    }
    
    // MARK: - File Selection Methods
    
    private func handleFileSelection(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            for url in urls {
                // Security check: only allow files from temp directories or user's Documents
                guard url.startAccessingSecurityScopedResource() else { continue }
                defer { url.stopAccessingSecurityScopedResource() }
                
                do {
                    // iOS Optimized Path-Based Approach (no data loading!)
                    let resourceValues = try url.resourceValues(forKeys: [.fileSizeKey, .contentTypeKey])
                    let fileSize = resourceValues.fileSize ?? 0
                    let contentType = resourceValues.contentType
                    
                    // Check if file is allowed
                    let fileName = url.lastPathComponent
                    let fileExtension = (fileName as NSString).pathExtension.lowercased()
                    
                    if config.blockedExtensions.contains(fileExtension) {
                        print("⚠️ [FILE_SELECTION] Blocked file type: \(fileName)")
                        continue
                    }
                    
                    // Check file size limits
                    if fileSize > config.maxFileSize {
                        print("⚠️ [FILE_SELECTION] File too large: \(fileName) (\(fileSize) bytes)")
                        continue
                    }
                    
                    // Create optimized file (no data copy!) with temporary path
                    let tempDir = FileManager.default.temporaryDirectory
                    let tempPath = tempDir.appendingPathComponent(UUID().uuidString + "_" + fileName)
                    try FileManager.default.copyItem(at: url, to: tempPath)
                    
                    let detectedType = contentType?.description ?? fileExtension
                    let optimizedFile = OptimizedDocumentFile(
                        name: fileName,
                        tempPath: tempPath.path,
                        size: fileSize,
                        detectedType: detectedType
                    )
                    
                    print("⚡ [FILE_SELECTION] Created optimized file: \(fileName) at path: \(tempPath.path)")
                    
                    if fileManager.addOptimizedFile(optimizedFile) {
                        print("✅ [FILE_SELECTION] Added optimized file: \(fileName)")
                    } else {
                        // Clean up if couldn't add
                        optimizedFile.cleanup()
                        print("❌ [FILE_SELECTION] Failed to add optimized file: \(fileName) (size limits)")
                        errorMessage = "File '\(fileName)' exceeds size limits"
                    }
                    
                } catch {
                    print("❌ [FILE_SELECTION] Error processing file \(url.lastPathComponent): \(error)")
                    errorMessage = "Failed to process file: \(url.lastPathComponent)"
                }
            }
            
        case .failure(let error):
            print("❌ [FILE_SELECTION] File selection failed: \(error)")
            errorMessage = "Failed to select files: \(error.localizedDescription)"
        }
    }
    
    // FIXED: Improved photo selection handling to prevent refresh issues
    private func handlePhotoSelection(_ photos: [PhotosPickerItem]) {
        guard !photos.isEmpty else { return }
        
        print("🏞️ [PHOTO_SELECTION] Starting photo selection with \(photos.count) items")
        
        // Set processing state
        isProcessingPhotos = true
        totalPhotosToProcess = photos.count
        processedPhotoCount = 0
        
        // FIXED: Use DispatchGroup to track completion of all photos
        let photoProcessingGroup = DispatchGroup()
        
        for (index, photo) in photos.enumerated() {
            photoProcessingGroup.enter()
            
            // FIXED: Use improved filename generation with better type detection
            let baseTimestamp = Date().timeIntervalSince1970
            let generatedFilename = generatePhotoFilename(for: photo, baseTimestamp: baseTimestamp, sequenceIndex: index)
            
            photo.loadTransferable(type: Data.self) { result in
                defer { 
                    DispatchQueue.main.async {
                        self.processedPhotoCount += 1
                        photoProcessingGroup.leave()
                    }
                }
                
                switch result {
                case .success(let data?):
                    // iOS Optimized Path-Based Approach for photos too!
                    DispatchQueue.main.async {
                        let fileSize = data.count
                        
                        // FIXED: Better type detection from PhotosPickerItem
                        let detectedType = self.detectTypeFromPhoto(photo) ?? "image/jpeg"
                        
                        // Check file size limits
                        if fileSize > self.config.maxFileSize {
                            print("⚠️ [PHOTO_SELECTION] Photo too large: \(generatedFilename) (\(fileSize) bytes)")
                            self.errorMessage = "Photo '\(generatedFilename)' exceeds size limits"
                            return
                        }
                        
                        // Store in temp file
                        let tempDir = FileManager.default.temporaryDirectory
                        let tempPath = tempDir.appendingPathComponent(UUID().uuidString + "_" + generatedFilename)
                        
                        do {
                            try data.write(to: tempPath)
                            
                            let optimizedFile = OptimizedDocumentFile(
                                name: generatedFilename,
                                tempPath: tempPath.path,
                                size: fileSize,
                                detectedType: detectedType
                            )
                            
                            print("📱 [PHOTO_SELECTION] Created optimized photo: \(generatedFilename) at path: \(tempPath.path)")
                            
                            if self.fileManager.addOptimizedFile(optimizedFile) {
                                print("✅ [PHOTO_SELECTION] Added optimized photo: \(generatedFilename)")
                            } else {
                                // Clean up if couldn't add
                                optimizedFile.cleanup()
                                print("❌ [PHOTO_SELECTION] Failed to add optimized photo: \(generatedFilename) (size limits)")
                                self.errorMessage = "Photo '\(generatedFilename)' exceeds total size limits"
                            }
                            
                        } catch {
                            print("❌ [PHOTO_SELECTION] Error creating temp file for photo: \(error)")
                            self.errorMessage = "Failed to process photo: \(generatedFilename)"
                        }
                    }
                    
                case .success(nil):
                    print("⚠️ [PHOTO_SELECTION] No data for photo: \(photo.itemIdentifier ?? "unknown")")
                case .failure(let error):
                    print("❌ [PHOTO_SELECTION] Failed to load photo: \(error)")
                }
            }
        }
        
        // FIXED: Clear selection only AFTER all photos are processed to avoid refresh issues
        photoProcessingGroup.notify(queue: .main) {
            self.isProcessingPhotos = false
            
            // FIXED: Clear selection with a small delay to ensure UI stability
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                self.selectedPhotos = []
                print("✅ [PHOTO_SELECTION] Completed processing \(photos.count) photos, selection cleared")
            }
        }
    }
    
    // FIXED: Add helper method for better photo type detection
    private func detectTypeFromPhoto(_ photo: PhotosPickerItem) -> String? {
        for supportedType in photo.supportedContentTypes {
            let identifier = supportedType.identifier
            
            // Video types
            if identifier.contains("mpeg-4") || identifier.contains("mp4") {
                return "video/mp4"
            }
            if identifier.contains("quicktime") || identifier.contains("mov") {
                return "video/quicktime"
            }
            if identifier.contains("video") {
                return "video/mp4" // Default video type
            }
            
            // Image types
            if identifier.contains("heif") || identifier.contains("heic") {
                return "image/heic"
            }
            if identifier.contains("jpeg") || identifier.contains("jpg") {
                return "image/jpeg"
            }
            if identifier.contains("png") {
                return "image/png"
            }
        }
        
        return "image/jpeg" // Default fallback
    }
    
    // Note: Helper methods formatFileSize, fileIcon, getFieldDisplayName, and generatePhotoFilename are defined in DocumentUploadSheetExtension.swift
} 