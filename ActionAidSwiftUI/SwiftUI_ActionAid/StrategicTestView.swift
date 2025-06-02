//  StrategicTestView.swift
//  ActionAid SwiftUI Strategic Domain Test - Enhanced with Compression
//

import SwiftUI
import UniformTypeIdentifiers
import QuickLook

// MARK: - Identifiable URL wrapper
struct IdentifiableURL: Identifiable {
    let id = UUID()
    let url: URL
}

// MARK: - Codable Payloads for FFI (Ensure these match your Rust DTOs)
struct AuthContextPayload: Codable {
    let user_id: String
    let role: String
    let device_id: String
    let offline_mode: Bool
}

struct DocumentTypeDetailsPayload: Codable {
    let name: String
    let description: String
    let allowed_extensions: String // Comma-separated, e.g., "pdf,docx"
    let max_size: Int // Bytes
    let compression_level: Int // e.g., 0-9
    let compression_method: String // e.g., "lossless", "zstd"
    let min_size_for_compression: Int // Bytes
    let default_priority: String // e.g., "normal", "high"
    let icon: String // e.g., "pdf_icon" or URL
    let related_tables: String // JSON string array, e.g., "[\"strategic_goals\"]"
}

struct DocumentTypeCreateFFIPayload: Codable {
    let document_type: DocumentTypeDetailsPayload
    let auth: AuthContextPayload
}

// MARK: - Additional Codable Payloads for Compression
struct CompressionQueueStatus: Codable {
    let pending_count: Int64
    let processing_count: Int64
    let completed_count: Int64
    let failed_count: Int64
    let skipped_count: Int64
}

struct CompressionStats: Codable {
    let total_original_size: Int64
    let total_compressed_size: Int64
    let space_saved: Int64
    let compression_ratio: Double
    let total_files_compressed: Int64
    let total_files_pending: Int64
    let total_files_failed: Int64
    let total_files_skipped: Int64
    let last_compression_date: String?
}

struct CompressionResult: Codable {
    let document_id: String
    let original_size: Int64
    let compressed_size: Int64
    let compressed_file_path: String
    let space_saved_bytes: Int64
    let space_saved_percentage: Double
    let method_used: String
    let quality_level: Int
    let duration_ms: Int64
}

// MARK: - Enhanced Strategic Test View

struct StrategicTestView: View {
    @State private var statusMessage = "Ready to test Strategic Domain with Compression"
    @State private var testResults = ""
    @ObservedObject var authState = authenticationState

    // Goal and document type IDs
    @State private var adminGoalId: String?
    @State private var strategicPlanDocTypeId: String?
    @State private var photoEvidenceDocTypeId: String?
    @State private var generalReportDocTypeId: String?

    // Goal details
    @State private var currentGoalDetails: StrategicGoalDetails?
    
    // Document tracking
    @State private var uploadedDocuments: [DocumentInfo] = []
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Compression tracking
    @State private var compressionQueueStatus: CompressionQueueStatus?
    @State private var compressionStats: CompressionStats?
    @State private var documentCompressionStatuses: [String: String] = [:] // docId -> status
    
    // Delete options
    @State private var showDeleteOptions = false
    @State private var selectedDeleteOption: DeleteOption = .softDelete
    
    enum DeleteOption: String, CaseIterable {
        case softDelete = "Soft Delete"
        case hardDelete = "Hard Delete"
        case forceDelete = "Force Delete"
        
        var description: String {
            switch self {
            case .softDelete: return "Mark as deleted but keep data"
            case .hardDelete: return "Permanently remove data"
            case .forceDelete: return "Force delete with dependencies"
            }
        }
    }
    
    struct StrategicGoalDetails: Codable {
        let id: String
        let objective_code: String
        let outcome: String
        let kpi: String
        let target_value: Double
        let actual_value: Double
        let status_id: Int
        let responsible_team: String
        let sync_priority: String
        let created_by_user_id: String
        let created_at: String
        let updated_at: String
    }
    
    struct DocumentInfo: Identifiable, Equatable {
        let id: String
        let filename: String
        let linkedField: String?
        var localPath: String?
        var temporaryLocalURLForPreview: URL?
        var compressionStatus: String?
        var originalSize: Int64?
        var compressedSize: Int64?
        var isAvailableLocally: Bool?
    }
    
    enum TestPhase {
        case idle, setupComplete, goalCreated, documentsUploaded, compressionInProgress, 
             compressionComplete, readyToDelete
    }
    @State private var currentTestPhase: TestPhase = .idle

    var body: some View {
        ScrollView(.vertical, showsIndicators: true) {
            VStack(spacing: 15) {
                Text("🎯 Strategic Goal & Compression Tests")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text(statusMessage)
                    .font(.headline)
                    .foregroundColor(currentTestPhase != .idle ? .orange : .primary)

                // Debug info
                VStack(alignment: .leading) {
                    Text("Debug Info:")
                        .font(.caption).fontWeight(.bold)
                    Text("Current Phase: \(String(describing: currentTestPhase))")
                        .font(.caption)
                    Text("DocType IDs Set: \(strategicPlanDocTypeId != nil && photoEvidenceDocTypeId != nil && generalReportDocTypeId != nil ? "✅" : "❌")")
                        .font(.caption)
                    Text("Goal ID: \(adminGoalId?.prefix(8) ?? "None")")
                        .font(.caption)
                }
                .padding()
                .background(Color.yellow.opacity(0.1))
                .cornerRadius(8)
                .padding(.horizontal)

                // MARK: - Test Flow Buttons
                Group {
                    Button("1️⃣ Setup: Create DocTypes") {
                        Task { await setupDocumentTypes() }
                    }
                    .testButtonStyle(currentTestPhase == .idle ? .blue : .gray)
                    .disabled(currentTestPhase != .idle && currentTestPhase != .setupComplete) // Allow re-run if already setup

                    Button("2️⃣ Create Strategic Goal with Details") {
                        Task { await createStrategicGoalWithDetails() }
                    }
                    .testButtonStyle(currentTestPhase == .setupComplete ? .blue : .gray)
                    .disabled(strategicPlanDocTypeId == nil || photoEvidenceDocTypeId == nil || generalReportDocTypeId == nil)

                    Button("3️⃣ Upload Documents with Different Priorities") {
                        Task { await uploadDocumentsWithPriorities() }
                    }
                    .testButtonStyle(currentTestPhase == .goalCreated ? .blue : .gray)
                    .disabled(adminGoalId == nil)

                    Button("4️⃣ Check Compression Queue & Stats") {
                        Task { await checkCompressionStatus() }
                    }
                    .testButtonStyle(.green)
                    .disabled(uploadedDocuments.isEmpty)

                    Button("5️⃣ Test Compression Operations") {
                        Task { await testCompressionOperations() }
                    }
                    .testButtonStyle(.purple)
                    .disabled(uploadedDocuments.isEmpty)
                    
                    Button("6️⃣ Delete Goal (Test Options)") {
                        showDeleteOptions = true
                    }
                    .testButtonStyle(.red)
                    .disabled(adminGoalId == nil)
                    
                    Button("🔄 Reset Test") {
                        resetTest()
                    }
                    .testButtonStyle(.orange)
                }
                .padding(.horizontal)

                // MARK: - Goal Details Display
                if let goalDetails = currentGoalDetails {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("📊 Strategic Goal Details")
                            .font(.headline)
                        
                        Group {
                            HStack {
                                Text("ID:").fontWeight(.medium)
                                Text(goalDetails.id.prefix(8) + "...")
                                    .font(.system(.caption, design: .monospaced))
                            }
                            HStack {
                                Text("Code:").fontWeight(.medium)
                                Text(goalDetails.objective_code)
                            }
                            HStack {
                                Text("Sync Priority:").fontWeight(.medium)
                                Text(goalDetails.sync_priority)
                                    .foregroundColor(goalDetails.sync_priority == "High" ? .red : .blue)
                            }
                            HStack {
                                Text("Target:").fontWeight(.medium)
                                Text("\(goalDetails.target_value, specifier: "%.1f")")
                            }
                            HStack {
                                Text("Actual:").fontWeight(.medium)
                                Text("\(goalDetails.actual_value, specifier: "%.1f")")
                            }
                            HStack {
                                Text("Team:").fontWeight(.medium)
                                Text(goalDetails.responsible_team)
                            }
                        }
                        .font(.caption)
                    }
                    .padding()
                    .background(Color(.systemGray6))
                    .cornerRadius(8)
                    .padding(.horizontal)
                }

                // MARK: - Document List with Compression Info
                if !uploadedDocuments.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("📄 Documents (\(uploadedDocuments.count))")
                            .font(.headline)
                        
                        ForEach(uploadedDocuments) { doc in
                            DocumentRowEnhanced(
                                document: doc,
                                compressionStatus: documentCompressionStatuses[doc.id],
                                onOpen: { openDocument(doc) },
                                onDelete: { Task { await deleteDocument(doc) } },
                                onCompress: { Task { await compressDocument(doc) } },
                                onCancelCompression: { Task { await cancelCompression(doc) } }
                            )
                        }
                    }
                    .padding()
                    .background(Color(.systemGray5))
                    .cornerRadius(10)
                    .padding(.horizontal)
                }

                // MARK: - Compression Stats Display
                if let queueStatus = compressionQueueStatus {
                    CompressionStatusView(
                        queueStatus: queueStatus,
                        stats: compressionStats
                    )
                    .padding(.horizontal)
                }

                // MARK: - Test Results Log
                TestResultsLog(testResults: $testResults)
                    .padding(.horizontal)
            }
            .padding(.vertical)
        }
        .sheet(item: $selectedDocumentURL) { identifiableURL in
            QuickLookView(url: identifiableURL.url) {
                if let doc = uploadedDocuments.first(where: { 
                    $0.temporaryLocalURLForPreview == identifiableURL.url 
                }) {
                    Task { await unregisterDocumentUsage(doc.id) }
                }
            }
        }
        .sheet(isPresented: $showDeleteOptions) {
            DeleteOptionsSheet(
                selectedOption: $selectedDeleteOption,
                onDelete: { option in
                    Task { await deleteGoalWithOption(option) }
                    showDeleteOptions = false
                }
            )
        }
        .onAppear {
            updateResults("🚀 Strategic Test View Ready. Real-world flow demonstration.", clearPrevious: true)
        }
    }

    // MARK: - Test Implementation Functions

    func resetTest() {
        adminGoalId = nil
        strategicPlanDocTypeId = nil
        photoEvidenceDocTypeId = nil
        generalReportDocTypeId = nil
        currentGoalDetails = nil
        uploadedDocuments.removeAll()
        documentCompressionStatuses.removeAll()
        compressionQueueStatus = nil
        compressionStats = nil
        currentTestPhase = .idle
        statusMessage = "Ready to test Strategic Domain with Compression"
        updateResults("🔄 Test reset complete", clearPrevious: true)
    }

    func setupDocumentTypes() async {
        currentTestPhase = .idle
        statusMessage = "Setting up document types..."
        updateResults("\n--- 1️⃣ Setting up Document Types ---", clearPrevious: true)
        
        // Document types with compression settings
        let documentTypes = [
            (name: "Strategic Plan", extensions: "pdf,doc,docx", maxSize: 10485760, compressionLevel: 7, compressionMethod: "lossless", minSizeForCompression: 102400, priority: "high"),
            (name: "Photo Evidence", extensions: "jpg,jpeg,png,heic", maxSize: 5242880, compressionLevel: 8, compressionMethod: "lossy", minSizeForCompression: 51200, priority: "normal"),
            (name: "General Report", extensions: "pdf,xlsx,csv,docx,txt", maxSize: 10485760, compressionLevel: 6, compressionMethod: "lossless", minSizeForCompression: 102400, priority: "normal")
        ]
        
        // First, try to find existing document types
        updateResults("\n🔍 Step 1: Checking for existing document types...")
        for docType in documentTypes {
            await findDocumentTypeByName(name: docType.name)
        }
        
        // Log what we found
        updateResults("\n📋 Found existing types:")
        updateResults("   Strategic Plan: \(strategicPlanDocTypeId != nil ? "✅" : "❌")")
        updateResults("   Photo Evidence: \(photoEvidenceDocTypeId != nil ? "✅" : "❌")")
        updateResults("   General Report: \(generalReportDocTypeId != nil ? "✅" : "❌")")
        
        // Create any missing document types
        if strategicPlanDocTypeId == nil || photoEvidenceDocTypeId == nil || generalReportDocTypeId == nil {
            updateResults("\n🏗 Step 2: Creating missing document types...")
            
            for docType in documentTypes {
                // Skip if already found
                if (docType.name == "Strategic Plan" && strategicPlanDocTypeId != nil) ||
                   (docType.name == "Photo Evidence" && photoEvidenceDocTypeId != nil) ||
                   (docType.name == "General Report" && generalReportDocTypeId != nil) {
                    updateResults("ℹ️ \(docType.name) type already exists, skipping")
                    continue
                }
                
                // Build payload with safe defaults
                let details = DocumentTypeDetailsPayload(
                    name: docType.name,
                    description: "Document type for \(docType.name)",
                    allowed_extensions: docType.extensions,
                    max_size: docType.maxSize,
                    compression_level: docType.compressionLevel,
                    compression_method: docType.compressionMethod,
                    min_size_for_compression: docType.minSizeForCompression,
                    default_priority: docType.priority,
                    icon: "doc_icon",
                    related_tables: "[\"strategic_goals\"]" // Simplified from old code
                )
                
                let payload = DocumentTypeCreateFFIPayload(
                    document_type: details, 
                    auth: getAuthContext()
                )
                
                do {
                    let jsonData = try JSONEncoder().encode(payload)
                    guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                        updateResults("❌ Failed to create JSON for \(docType.name)")
                        continue
                    }
                    
                    updateResults("\n🔨 Creating \(docType.name)...")
                    updateResults("📄 Payload: \(jsonString.prefix(200))...")
                    
                    // FFI CALL WITH PROPER ERROR HANDLING
                    var resultPtr: UnsafeMutablePointer<CChar>?
                    let status = document_type_create(jsonString, &resultPtr)
                    
                    // 1. Check FFI status first
                    guard status == 0 else {
                        if let errorPtr = get_last_error() {
                            let error = String(cString: errorPtr)
                            updateResults("❌ FFI Error creating \(docType.name): \(error)")
                            
                            // If it exists, try to find it
                            if error.localizedCaseInsensitiveContains("unique constraint") || 
                               error.localizedCaseInsensitiveContains("already exists") {
                                updateResults("ℹ️ Document Type '\(docType.name)' already exists. Attempting to find it...")
                                await findDocumentTypeByName(name: docType.name)
                            }
                        } else {
                            updateResults("❌ Unknown FFI error creating \(docType.name) (status: \(status))")
                        }
                        continue
                    }
                    
                    // 2. Check if we got a result
                    guard let resultStr = resultPtr else {
                        updateResults("❌ document_type_create returned null for \(docType.name)")
                        continue
                    }
                    defer { document_free(resultStr) }
                    
                    let response = String(cString: resultStr)
                    updateResults("✅ Creation response: \(response.prefix(200))...")
                    
                    // 3. Parse response
                    if let data = response.data(using: .utf8),
                       let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                       let id = json["id"] as? String {
                        
                        switch docType.name {
                        case "Strategic Plan": 
                            strategicPlanDocTypeId = id
                            updateResults("✅ Created Strategic Plan type: \(id.prefix(8))...")
                        case "Photo Evidence": 
                            photoEvidenceDocTypeId = id
                            updateResults("✅ Created Photo Evidence type: \(id.prefix(8))...")
                        case "General Report": 
                            generalReportDocTypeId = id
                            updateResults("✅ Created General Report type: \(id.prefix(8))...")
                        default: break
                        }
                    } else {
                        updateResults("⚠️ Created \(docType.name) but couldn't parse ID. Response: \(response)")
                        // Fallback to lookup if creation succeeded
                        updateResults("ℹ️ Attempting to find newly created \(docType.name)...")
                        await findDocumentTypeByName(name: docType.name)
                    }
                } catch {
                    updateResults("❌ JSON Error for \(docType.name): \(error.localizedDescription)")
                }
            }
        }
        
        // Final check
        updateResults("\n📊 Final Status:")
        updateResults("   Strategic Plan ID: \(strategicPlanDocTypeId?.prefix(8) ?? "NOT SET")")
        updateResults("   Photo Evidence ID: \(photoEvidenceDocTypeId?.prefix(8) ?? "NOT SET")")
        updateResults("   General Report ID: \(generalReportDocTypeId?.prefix(8) ?? "NOT SET")")
        
        if strategicPlanDocTypeId != nil && photoEvidenceDocTypeId != nil && generalReportDocTypeId != nil {
            currentTestPhase = .setupComplete
            statusMessage = "✅ Document types ready. Create goal next."
            updateResults("\n✅ All document types are ready!")
        } else {
            updateResults("\n❌ Failed to set up all document types.")
            updateResults("   Missing IDs:")
            if strategicPlanDocTypeId == nil { updateResults("   - Strategic Plan") }
            if photoEvidenceDocTypeId == nil { updateResults("   - Photo Evidence") }
            if generalReportDocTypeId == nil { updateResults("   - General Report") }
            statusMessage = "❌ Setup incomplete. Check logs."
            currentTestPhase = .idle
        }
    }

    func createStrategicGoalWithDetails() async {
        statusMessage = "Creating strategic goal..."
        updateResults("\n--- 2️⃣ Creating Strategic Goal ---")
        
        // Generate a unique objective code with timestamp
        let timestamp = Int(Date().timeIntervalSince1970)
        let objectiveCode = "STRAT-\(timestamp)"
        
        let createPayload = """
        {
            "goal": {
                "objective_code": "\(objectiveCode)",
                "outcome": "Improve community health outcomes through digital transformation",
                "kpi": "Number of digital health services deployed",
                "target_value": 25.0,
                "actual_value": 5.0,
                "status_id": 1,
                "responsible_team": "Digital Innovation Team",
                "sync_priority": "High",
                "created_by_user_id": "\(authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000")"
            },
            "auth": \(getAuthContextAsJSONString())
        }
        """
        
        var result: UnsafeMutablePointer<CChar>?
        let status = strategic_goal_create(createPayload, &result)
        
        if let resultStr = result {
            defer { strategic_goal_free(resultStr) }
            
            if status == 0, let data = String(cString: resultStr).data(using: .utf8) {
                do {
                    if let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] {
                        adminGoalId = json["id"] as? String
                        
                        // Parse and store goal details
                        currentGoalDetails = StrategicGoalDetails(
                            id: json["id"] as? String ?? "",
                            objective_code: json["objective_code"] as? String ?? "",
                            outcome: json["outcome"] as? String ?? "",
                            kpi: json["kpi"] as? String ?? "",
                            target_value: json["target_value"] as? Double ?? 0.0,
                            actual_value: json["actual_value"] as? Double ?? 0.0,
                            status_id: json["status_id"] as? Int ?? 0,
                            responsible_team: json["responsible_team"] as? String ?? "",
                            sync_priority: json["sync_priority"] as? String ?? "",
                            created_by_user_id: json["created_by_user_id"] as? String ?? "",
                            created_at: json["created_at"] as? String ?? "",
                            updated_at: json["updated_at"] as? String ?? ""
                        )
                        
                        updateResults("✅ Created Strategic Goal:")
                        updateResults("   ID: \(currentGoalDetails?.id.prefix(8) ?? "")...")
                        updateResults("   Code: \(currentGoalDetails?.objective_code ?? "")")
                        updateResults("   Sync Priority: \(currentGoalDetails?.sync_priority ?? "")")
                        updateResults("   Progress: \((currentGoalDetails?.actual_value ?? 0) / (currentGoalDetails?.target_value ?? 1) * 100)%")
                        
                        currentTestPhase = .goalCreated
                        statusMessage = "Goal created. Upload documents next."
                    }
                } catch {
                    updateResults("❌ Error parsing goal response: \(error)")
                }
            } else {
                if let errorPtr = get_last_error() {
                    let error = String(cString: errorPtr)
                    updateResults("❌ Failed to create goal: \(error)")
                } else {
                    updateResults("❌ Failed to create goal: Unknown error")
                }
            }
        }
    }

    func uploadDocumentsWithPriorities() async {
        guard let goalId = adminGoalId else { return }
        
        statusMessage = "Uploading documents with compression priorities..."
        updateResults("\n--- 3️⃣ Uploading Documents with Compression Priorities ---")
        
        // Upload different documents with different priorities and linked fields
        let documentsToUpload = [
            (name: "test_strategic_plan", ext: "pdf", typeId: strategicPlanDocTypeId!, 
             title: "Q4 Strategic Plan", linkedField: "actual_value", priority: "High", compressionPriority: "High"),
            (name: "test_report", ext: "pdf", typeId: generalReportDocTypeId!, 
             title: "Monthly Report", linkedField: "target_value", priority: "NORMAL", compressionPriority: "NORMAL"),
            (name: "test_image", ext: "jpg", typeId: photoEvidenceDocTypeId!, 
             title: "Field Photo", linkedField: nil, priority: "LOW", compressionPriority: "BACKGROUND")
        ]
        
        for docInfo in documentsToUpload {
            guard let data = loadTestDocument(named: docInfo.name, type: docInfo.ext) else { continue }
            
            let uploadPayload = """
            {
                "goal_id": "\(goalId)",
                "file_data": "\(data.base64EncodedString())",
                "original_filename": "\(docInfo.name).\(docInfo.ext)",
                "title": "\(docInfo.title)",
                "document_type_id": "\(docInfo.typeId)",
                \(docInfo.linkedField.map { "\"linked_field\": \"\($0)\"," } ?? "")
                "sync_priority": "\(docInfo.priority)",
                "compression_priority": "\(docInfo.compressionPriority)",
                "auth": \(getAuthContextAsJSONString())
            }
            """
            
            var result: UnsafeMutablePointer<CChar>?
            let status = strategic_goal_upload_document(uploadPayload, &result)
            
            if let resultStr = result {
                defer { strategic_goal_free(resultStr) }
                
                if status == 0, let responseData = String(cString: resultStr).data(using: .utf8),
                   let json = try? JSONSerialization.jsonObject(with: responseData) as? [String: Any] {
                    
                    let doc = DocumentInfo(
                        id: json["id"] as? String ?? "",
                        filename: json["original_filename"] as? String ?? "",
                        linkedField: json["field_identifier"] as? String,
                        localPath: json["file_path"] as? String,
                        temporaryLocalURLForPreview: nil,
                        compressionStatus: json["compression_status"] as? String,
                        originalSize: json["size_bytes"] as? Int64,
                        compressedSize: json["compressed_size_bytes"] as? Int64,
                        isAvailableLocally: json["is_available_locally"] as? Bool
                    )
                    
                    uploadedDocuments.append(doc)
                    documentCompressionStatuses[doc.id] = doc.compressionStatus ?? "PENDING"
                    
                    updateResults("✅ Uploaded '\(doc.filename)':")
                    updateResults("   Linked to: \(doc.linkedField ?? "general")")
                    updateResults("   Compression Priority: \(docInfo.compressionPriority)")
                    updateResults("   Size: \(formatFileSize(Int(doc.originalSize ?? 0)))")
                }
            }
        }
        
        if !uploadedDocuments.isEmpty {
            currentTestPhase = .documentsUploaded
            statusMessage = "Documents uploaded. Check compression status."
            
            // Start monitoring compression
            Task {
                await checkCompressionStatus()
            }
        }
    }

    func checkCompressionStatus() async {
        updateResults("\n--- 4️⃣ Checking Compression Status ---")
        
        // Get queue status
        var queueResult: UnsafeMutablePointer<CChar>?
        let queueStatus = compression_get_queue_status(&queueResult)
        
        if let resultStr = queueResult {
            defer { compression_free(resultStr) }
            
            if queueStatus == 0, let data = String(cString: resultStr).data(using: .utf8) {
                do {
                    compressionQueueStatus = try JSONDecoder().decode(CompressionQueueStatus.self, from: data)
                    updateResults("📊 Compression Queue:")
                    updateResults("   Pending: \(compressionQueueStatus?.pending_count ?? 0)")
                    updateResults("   Processing: \(compressionQueueStatus?.processing_count ?? 0)")
                    updateResults("   Completed: \(compressionQueueStatus?.completed_count ?? 0)")
                } catch {
                    updateResults("❌ Error parsing queue status: \(error)")
                }
            }
        }
        
        // Get compression stats
        var statsResult: UnsafeMutablePointer<CChar>?
        let statsStatus = compression_get_stats(&statsResult)
        
        if let resultStr = statsResult {
            defer { compression_free(resultStr) }
            
            if statsStatus == 0, let data = String(cString: resultStr).data(using: .utf8) {
                do {
                    compressionStats = try JSONDecoder().decode(CompressionStats.self, from: data)
                    updateResults("💾 Compression Statistics:")
                    updateResults("   Space Saved: \(formatFileSize(Int(compressionStats?.space_saved ?? 0)))")
                    updateResults("   Compression Ratio: \(String(format: "%.1f%%", compressionStats?.compression_ratio ?? 0))")
                } catch {
                    updateResults("❌ Error parsing stats: \(error)")
                }
            }
        }
        
        // Check individual document statuses
        for doc in uploadedDocuments {
            await checkDocumentCompressionStatus(doc)
        }
        
        if compressionQueueStatus?.processing_count ?? 0 > 0 {
            currentTestPhase = .compressionInProgress
            statusMessage = "Compression in progress..."
        } else if compressionQueueStatus?.completed_count ?? 0 > 0 {
            currentTestPhase = .compressionComplete
            statusMessage = "Compression complete. Ready for next steps."
        }
    }

    func checkDocumentCompressionStatus(_ doc: DocumentInfo) async {
        let payload = """
        {"document_id": "\(doc.id)"}
        """
        
        var result: UnsafeMutablePointer<CChar>?
        let status = compression_get_document_status(payload, &result)
        
        if let resultStr = result {
            defer { compression_free(resultStr) }
            
            if status == 0, let data = String(cString: resultStr).data(using: .utf8),
               let statusStr = String(data: data, encoding: .utf8) {
                documentCompressionStatuses[doc.id] = statusStr.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\"", with: "")
            }
        }
    }

    func testCompressionOperations() async {
        updateResults("\n--- 5️⃣ Testing Compression Operations ---")
        
        // Test 1: Update compression priority
        if let firstDoc = uploadedDocuments.first {
            updateResults("🔄 Updating compression priority for \(firstDoc.filename)...")
            
            let updatePayload = """
            {"document_id": "\(firstDoc.id)", "priority": "HIGH"}
            """
            
            var result: UnsafeMutablePointer<CChar>?
            let status = compression_update_priority(updatePayload, &result)
            
            if let resultStr = result {
                defer { compression_free(resultStr) }
                
                if status == 0 {
                    updateResults("✅ Updated compression priority to HIGH")
                }
            }
        }
        
        // Test 2: Cancel compression for a document
        if let pendingDoc = uploadedDocuments.first(where: { documentCompressionStatuses[$0.id] == "PENDING" }) {
            updateResults("❌ Canceling compression for \(pendingDoc.filename)...")
            
            let cancelPayload = """
            {"document_id": "\(pendingDoc.id)"}
            """
            
            var result: UnsafeMutablePointer<CChar>?
            let status = compression_cancel(cancelPayload, &result)
            
            if let resultStr = result {
                defer { compression_free(resultStr) }
                
                if status == 0 {
                    updateResults("✅ Cancelled compression")
                }
            }
        }
        
        // Test 3: Manual compression trigger
        if let uncompressedDoc = uploadedDocuments.first(where: { 
            documentCompressionStatuses[$0.id] == "PENDING" || documentCompressionStatuses[$0.id] == "SKIPPED" 
        }) {
            updateResults("🗜 Manually compressing \(uncompressedDoc.filename)...")
            
            let compressPayload = """
            {
                "document_id": "\(uncompressedDoc.id)",
                "config": {
                    "method": "lossless",
                    "quality_level": 75,
                    "min_size_bytes": 1024
                }
            }
            """
            
            var result: UnsafeMutablePointer<CChar>?
            let status = compression_compress_document(compressPayload, &result)
            
            if let resultStr = result {
                defer { compression_free(resultStr) }
                
                if status == 0, let data = String(cString: resultStr).data(using: .utf8) {
                    do {
                        let compressionResult = try JSONDecoder().decode(CompressionResult.self, from: data)
                        updateResults("✅ Compression completed:")
                        updateResults("   Original: \(formatFileSize(Int(compressionResult.original_size)))")
                        updateResults("   Compressed: \(formatFileSize(Int(compressionResult.compressed_size)))")
                        updateResults("   Saved: \(compressionResult.space_saved_percentage)%")
                    } catch {
                        updateResults("❌ Error parsing compression result: \(error)")
                    }
                }
            }
        }
        
        // Refresh status after operations
        await checkCompressionStatus()
    }

    func compressDocument(_ doc: DocumentInfo) async {
        updateResults("🗜 Queuing \(doc.filename) for compression...")
        
        let payload = """
        {"document_id": "\(doc.id)", "priority": "High"}
        """
        
        let status = compression_queue_document(payload)
        
        if status == 0 {
            updateResults("✅ Document queued for compression")
            documentCompressionStatuses[doc.id] = "QUEUED"
            await checkCompressionStatus()
        }
    }

    func cancelCompression(_ doc: DocumentInfo) async {
        updateResults("❌ Canceling compression for \(doc.filename)...")
        
        let payload = """
        {"document_id": "\(doc.id)"}
        """
        
        var result: UnsafeMutablePointer<CChar>?
        let status = compression_cancel(payload, &result)
        
        if let resultStr = result {
            defer { compression_free(resultStr) }
            
            if status == 0 {
                updateResults("✅ Compression cancelled")
                documentCompressionStatuses[doc.id] = "CANCELLED"
            }
        }
    }

    func deleteGoalWithOption(_ option: DeleteOption) async {
        guard let goalId = adminGoalId else { return }
        
        updateResults("\n--- 6️⃣ Deleting Strategic Goal (\(option.rawValue)) ---")
        statusMessage = "Deleting goal with \(option.rawValue)..."
        
        let deletePayload: String
        switch option {
        case .softDelete:
            deletePayload = """
            {"id": "\(goalId)", "hard_delete": false, "auth": \(getAuthContextAsJSONString())}
            """
        case .hardDelete:
            deletePayload = """
            {"id": "\(goalId)", "hard_delete": true, "auth": \(getAuthContextAsJSONString())}
            """
        case .forceDelete:
            // First try regular hard delete, then force if needed
            deletePayload = """
            {"id": "\(goalId)", "hard_delete": true, "auth": \(getAuthContextAsJSONString())}
            """
        }
        
        var result: UnsafeMutablePointer<CChar>?
        let status = strategic_goal_delete(deletePayload, &result)
        
        if let resultStr = result {
            defer { strategic_goal_free(resultStr) }
            
            if status == 0 {
                let response = String(cString: resultStr)
                updateResults("✅ Goal deleted successfully: \(response)")
                adminGoalId = nil
                currentGoalDetails = nil
                uploadedDocuments.removeAll()
                currentTestPhase = .idle
                statusMessage = "Goal deleted. Test complete."
            } else if option == .forceDelete {
                // Try force delete with dependencies
                updateResults("⚠️ Regular delete failed, attempting force delete...")
                // You would need a force delete FFI function here
                updateResults("❌ Force delete not implemented in FFI")
            }
        }
        
        if status != 0 {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Delete failed: \(error)")
                
                // Parse error for dependency info
                if error.contains("dependencies") {
                    updateResults("ℹ️ Goal has dependencies. Use force delete or remove dependencies first.")
                }
            }
        }
    }

    func deleteDocument(_ doc: DocumentInfo) async {
        updateResults("🗑 Deleting document: \(doc.filename)")
        
        let payload = """
        {"id": "\(doc.id)", "auth": \(getAuthContextAsJSONString())}
        """
        
        let status = document_delete(payload)
        
        if status == 0 {
            uploadedDocuments.removeAll { $0.id == doc.id }
            documentCompressionStatuses.removeValue(forKey: doc.id)
            updateResults("✅ Document deleted")
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Delete failed: \(error)")
            }
        }
    }

    // MARK: - Helper Functions
    
    private func loadTestDocument(named name: String, type ext: String) -> Data? {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext) else {
            updateResults("❌ Could not find \(name).\(ext) in bundle")
            return nil
        }
        do {
            return try Data(contentsOf: url)
        } catch {
            updateResults("❌ Failed to load \(name).\(ext): \(error)")
            return nil
        }
    }
    
    private func openDocument(_ doc: DocumentInfo) {
        // Same as original implementation
        updateResults("👁 Opening \(doc.filename)...")
        // Implementation remains the same
    }
    
    private func unregisterDocumentUsage(_ documentId: String) async {
        let payload = """
        {
            "document_id": "\(documentId)",
            "user_id": "\(authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000")",
            "device_id": "\(AuthenticationState.getDeviceId())",
            "auth": \(getAuthContextAsJSONString())
        }
        """
        
        let status = document_unregister_in_use(payload)
        if status == 0 {
            updateResults("✅ Unregistered document usage for \(documentId.prefix(8))...")
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("⚠️ Failed to unregister document usage: \(error)")
            }
        }
    }
    
    private func getAuthContextAsJSONString(roleOverride: String? = nil) -> String {
        let authData = AuthContextPayload(
            user_id: authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000",
            role: roleOverride ?? authState.lastLoggedInUser?.role ?? "admin",
            device_id: AuthenticationState.getDeviceId(),
            offline_mode: false
        )
        guard let jsonData = try? JSONEncoder().encode(authData), 
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            return "{}"
        }
        return jsonString
    }
    
    private func updateResults(_ message: String, clearPrevious: Bool = false) {
        DispatchQueue.main.async {
            let timestamp = DateFormatter.localizedString(from: Date(), dateStyle: .none, timeStyle: .medium)
            let logMessage = "\(timestamp): \(message)"
            if clearPrevious {
                self.testResults = logMessage + "\n"
            } else {
                self.testResults = logMessage + "\n" + self.testResults
            }
            print("📄 \(logMessage)")
        }
    }
    
    private func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB, .useBytes]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
    
    private func findDocumentTypeByName(name: String) async {
        updateResults("🔍 Looking for document type: \(name)...")
        
        let payload = """
        {
            "name": "\(name)",
            "auth": \(getAuthContextAsJSONString())
        }
        """
        
        var resultPtr: UnsafeMutablePointer<CChar>?
        let status = document_type_find_by_name(payload, &resultPtr)

        guard status == 0 else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Error finding '\(name)': \(error)")
            } else {
                updateResults("❌ Error finding '\(name)': Unknown error (status: \(status))")
            }
            return
        }
        
        guard let resultStr = resultPtr else {
            updateResults("❌ document_type_find_by_name returned null for \(name)")
            return
        }
        defer { document_free(resultStr) }

        let response = String(cString: resultStr)
        updateResults("📄 Find response: \(response.prefix(200))...")
        
        // Handle different response formats
        if response.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "null" || 
           response.trimmingCharacters(in: .whitespacesAndNewlines) == "{}" ||
           response.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            updateResults("❌ Document Type '\(name)' not found")
            return
        }
        
        if let data = response.data(using: .utf8) {
            do {
                // Try parsing as a single object
                if let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let id = json["id"] as? String {
                    // Set the ID based on the document type name
                    switch name {
                    case "Strategic Plan":
                        strategicPlanDocTypeId = id
                        updateResults("✅ Found Strategic Plan with ID: \(id.prefix(8))...")
                    case "Photo Evidence":
                        photoEvidenceDocTypeId = id
                        updateResults("✅ Found Photo Evidence with ID: \(id.prefix(8))...")
                    case "General Report":
                        generalReportDocTypeId = id
                        updateResults("✅ Found General Report with ID: \(id.prefix(8))...")
                    default:
                        updateResults("⚠️ Found document type '\(name)' but don't know where to store ID")
                    }
                } else {
                    // Try parsing as an array (in case the API returns an array)
                    if let jsonArray = try JSONSerialization.jsonObject(with: data) as? [[String: Any]],
                       let firstItem = jsonArray.first,
                       let id = firstItem["id"] as? String {
                        switch name {
                        case "Strategic Plan":
                            strategicPlanDocTypeId = id
                            updateResults("✅ Found Strategic Plan (from array) with ID: \(id.prefix(8))...")
                        case "Photo Evidence":
                            photoEvidenceDocTypeId = id
                            updateResults("✅ Found Photo Evidence (from array) with ID: \(id.prefix(8))...")
                        case "General Report":
                            generalReportDocTypeId = id
                            updateResults("✅ Found General Report (from array) with ID: \(id.prefix(8))...")
                        default:
                            break
                        }
                    } else {
                        updateResults("⚠️ Could not parse response for '\(name)': \(response.prefix(100))...")
                    }
                }
            } catch {
                updateResults("❌ Error parsing response for '\(name)': \(error)")
            }
        }
    }

    private func getAuthContext() -> AuthContextPayload {
        return AuthContextPayload(
            user_id: authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000",
            role: authState.lastLoggedInUser?.role ?? "admin",
            device_id: AuthenticationState.getDeviceId(),
            offline_mode: false
        )
    }
}

// MARK: - Enhanced Document Row
struct DocumentRowEnhanced: View {
    let document: StrategicTestView.DocumentInfo
    let compressionStatus: String?
    let onOpen: () -> Void
    let onDelete: () -> Void
    let onCompress: () -> Void
    let onCancelCompression: () -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: documentIconName(for: document.filename))
                    .font(.title2)
                    .frame(width: 30)
                
                VStack(alignment: .leading, spacing: 2) {
                    Text(document.filename)
                        .font(.caption)
                        .fontWeight(.medium)
                        .lineLimit(1)
                    
                    if let field = document.linkedField {
                        HStack(spacing: 4) {
                            Image(systemName: "link")
                                .font(.caption2)
                            Text("Linked to: \(field)")
                                .font(.caption2)
                                .foregroundColor(.secondary)
                        }
                    }
                    
                    HStack(spacing: 8) {
                        if let originalSize = document.originalSize {
                            Text("Original: \(formatFileSize(Int(originalSize)))")
                                .font(.caption2)
                                .foregroundColor(.gray)
                        }
                        
                        if let compressedSize = document.compressedSize {
                            Text("Compressed: \(formatFileSize(Int(compressedSize)))")
                                .font(.caption2)
                                .foregroundColor(.green)
                        }
                    }
                }
                
                Spacer()
                
                // Compression status indicator
                if let status = compressionStatus {
                    CompressionStatusBadge(status: status)
                }
                
                // Action buttons
                HStack(spacing: 8) {
                    Button(action: onOpen) {
                        Image(systemName: "eye")
                            .foregroundColor(.blue)
                    }
                    .buttonStyle(BorderlessButtonStyle())
                    
                    if compressionStatus == "PENDING" || compressionStatus == "SKIPPED" {
                        Button(action: onCompress) {
                            Image(systemName: "wand.and.rays")
                                .foregroundColor(.purple)
                        }
                        .buttonStyle(BorderlessButtonStyle())
                    }
                    
                    if compressionStatus == "IN_PROGRESS" || compressionStatus == "QUEUED" {
                        Button(action: onCancelCompression) {
                            Image(systemName: "xmark.circle")
                                .foregroundColor(.orange)
                        }
                        .buttonStyle(BorderlessButtonStyle())
                    }
                    
                    Button(action: onDelete) {
                        Image(systemName: "trash")
                            .foregroundColor(.red)
                    }
                    .buttonStyle(BorderlessButtonStyle())
                }
            }
            
            if document.isAvailableLocally == true {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                        .font(.caption2)
                    Text("Available locally")
                        .font(.caption2)
                        .foregroundColor(.green)
                }
            }
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 8)
        .background(Color(.systemGray6))
        .cornerRadius(6)
    }
    
    private func documentIconName(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png", "heic": return "photo.fill"
        default: return "doc.fill"
        }
    }
    
    private func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
}

// MARK: - Compression Status Badge
struct CompressionStatusBadge: View {
    let status: String
    
    var statusColor: Color {
        switch status {
        case "PENDING": return .gray
        case "QUEUED", "IN_PROGRESS": return .orange
        case "COMPLETED": return .green
        case "FAILED", "ERROR": return .red
        case "SKIPPED": return .blue
        case "CANCELLED": return .purple
        default: return .gray
        }
    }
    
    var statusIcon: String {
        switch status {
        case "PENDING": return "clock"
        case "QUEUED": return "hourglass"
        case "IN_PROGRESS": return "arrow.triangle.2.circlepath"
        case "COMPLETED": return "checkmark.circle.fill"
        case "FAILED", "ERROR": return "exclamationmark.triangle.fill"
        case "SKIPPED": return "forward.fill"
        case "CANCELLED": return "xmark.circle.fill"
        default: return "questionmark.circle"
        }
    }
    
    var body: some View {
        HStack(spacing: 2) {
            Image(systemName: statusIcon)
                .font(.caption2)
            Text(status)
                .font(.caption2)
        }
        .foregroundColor(statusColor)
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(statusColor.opacity(0.15))
        .cornerRadius(4)
    }
}

// MARK: - Compression Status View
struct CompressionStatusView: View {
    let queueStatus: CompressionQueueStatus
    let stats: CompressionStats?
    
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("🗜 Compression Status")
                .font(.headline)
            
            HStack(spacing: 15) {
                StatusItem(title: "Pending", count: queueStatus.pending_count, color: .gray)
                StatusItem(title: "Processing", count: queueStatus.processing_count, color: .orange)
                StatusItem(title: "Completed", count: queueStatus.completed_count, color: .green)
                StatusItem(title: "Failed", count: queueStatus.failed_count, color: .red)
                StatusItem(title: "Skipped", count: queueStatus.skipped_count, color: .blue)
            }
            
            if let stats = stats {
                Divider()
                
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Space Saved:")
                        Text(formatFileSize(Int(stats.space_saved)))
                            .fontWeight(.medium)
                            .foregroundColor(.green)
                    }
                    .font(.caption)
                    
                    HStack {
                        Text("Compression Ratio:")
                        Text("\(String(format: "%.1f", stats.compression_ratio))%")
                            .fontWeight(.medium)
                    }
                    .font(.caption)
                    
                    HStack {
                        Text("Total Files:")
                        Text("\(stats.total_files_compressed)")
                            .fontWeight(.medium)
                    }
                    .font(.caption)
                }
            }
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(8)
    }
    
    private func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB, .useGB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
}

struct StatusItem: View {
    let title: String
    let count: Int64
    let color: Color
    
    var body: some View {
        VStack(spacing: 2) {
            Text("\(count)")
                .font(.headline)
                .foregroundColor(color)
            Text(title)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }
}

// MARK: - Delete Options Sheet
struct DeleteOptionsSheet: View {
    @Binding var selectedOption: StrategicTestView.DeleteOption
    let onDelete: (StrategicTestView.DeleteOption) -> Void
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            VStack(spacing: 20) {
                Text("Choose Delete Method")
                    .font(.headline)
                    .padding(.top)
                
                ForEach(StrategicTestView.DeleteOption.allCases, id: \.self) { option in
                    Button(action: {
                        selectedOption = option
                        onDelete(option)
                    }) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(option.rawValue)
                                .font(.headline)
                            Text(option.description)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding()
                        .background(selectedOption == option ? Color.blue.opacity(0.1) : Color(.systemGray6))
                        .cornerRadius(10)
                    }
                    .buttonStyle(PlainButtonStyle())
                }
                
                Spacer()
            }
            .padding()
            .navigationTitle("Delete Goal")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}

// MARK: - Test Results Log Component
struct TestResultsLog: View {
    @Binding var testResults: String
    
    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical, showsIndicators: true) {
                Text(testResults)
                    .font(.system(size: 9, design: .monospaced))
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .id("resultsLog")
            }
            .frame(height: 300)
            .background(Color(.systemGray6))
            .cornerRadius(8)
            .onChange(of: testResults) { oldValue, newValue in
                withAnimation {
                    proxy.scrollTo("resultsLog", anchor: .top)
                }
            }
        }
    }
}

// MARK: - QuickLook Support
struct QuickLookView: UIViewControllerRepresentable {
    let url: URL
    let onDismiss: (() -> Void)?  // Add callback
    
    init(url: URL, onDismiss: (() -> Void)? = nil) {
        self.url = url
        self.onDismiss = onDismiss
    }
    
    func makeUIViewController(context: Context) -> QLPreviewController {
        let controller = QLPreviewController()
        controller.dataSource = context.coordinator
        controller.delegate = context.coordinator  // Add delegate
        return controller
    }
    
    func updateUIViewController(_ uiViewController: QLPreviewController, context: Context) {
        // Check if the URL has changed and needs reload, though IdentifiableURL should handle this.
        if context.coordinator.url != url {
            context.coordinator.url = url
            uiViewController.reloadData()
        }
    }
    
    func makeCoordinator() -> Coordinator {
        Coordinator(url: url, onDismiss: onDismiss)
    }
    
    class Coordinator: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
        var url: URL
        let onDismiss: (() -> Void)?
        
        init(url: URL, onDismiss: (() -> Void)? = nil) {
            self.url = url
            self.onDismiss = onDismiss
        }
        
        func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }
        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
            // Ensure the file exists before attempting to preview
            guard FileManager.default.fileExists(atPath: url.path) else {
                print("File does not exist at path for QuickLook: \(url.path)")
                // Return a placeholder or handle the error.
                // For simplicity, we'll still return the URL, but QL might show an error.
                return url as QLPreviewItem
            }
            return url as QLPreviewItem
        }
        
        // Add delegate method
        func previewControllerWillDismiss(_ controller: QLPreviewController) {
            onDismiss?()
        }
    }
}

// MARK: - FFI Function Declarations (if not already in bridging header)
@_silgen_name("get_last_error")
func get_last_error() -> UnsafeMutablePointer<CChar>?

// Add these if not already declared:
@_silgen_name("compression_get_queue_status")
func compression_get_queue_status(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_get_stats")
func compression_get_stats(_ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_get_document_status")
func compression_get_document_status(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_queue_document")
func compression_queue_document(_ payload: UnsafePointer<CChar>) -> Int32

@_silgen_name("compression_cancel")
func compression_cancel(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_update_priority")
func compression_update_priority(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_compress_document")
func compression_compress_document(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("compression_free")
func compression_free(_ ptr: UnsafeMutablePointer<CChar>)

// Add missing FFI declaration
@_silgen_name("document_type_list")
func document_type_list(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("document_type_find_by_name")
func document_type_find_by_name(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("strategic_goal_list")
func strategic_goal_list(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("strategic_goal_create")
func strategic_goal_create(_ payload: UnsafePointer<CChar>, _ result: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("strategic_goal_free")
func strategic_goal_free(_ ptr: UnsafeMutablePointer<CChar>)

@_silgen_name("document_unregister_in_use")
func document_unregister_in_use(_ payload: UnsafePointer<CChar>) -> Int32

// Extension for button styling remains the same
extension Button {
    func testButtonStyle(_ color: Color = .blue) -> some View {
        self
            .frame(maxWidth: .infinity)
            .padding(.vertical, 10)
            .background(color)
            .foregroundColor(.white)
            .cornerRadius(8)
            .shadow(color: color.opacity(0.3), radius: 3, x: 0, y: 2)
    }
}

// QuickLookView remains the same as in original
