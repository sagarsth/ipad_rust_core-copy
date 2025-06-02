//
//  StrategicTestView.swift
//  ActionAid SwiftUI Strategic Domain Test
//
//  Strategic Domain Test Interface - SwiftUI
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

// MARK: - Strategic Test View

struct StrategicTestView: View {
    @State private var statusMessage = "Ready to test Strategic Domain"
    @State private var testResults = ""
    @ObservedObject var authState = authenticationState // Ensure this is properly initialized globally

    @State private var adminGoalId: String?
    @State private var strategicPlanDocTypeId: String?
    @State private var photoEvidenceDocTypeId: String?
    @State private var generalReportDocTypeId: String?


    @State private var uploadedDocuments: [DocumentInfo] = []
    @State private var selectedDocumentURL: IdentifiableURL?
    
    // Track document info
    struct DocumentInfo: Identifiable, Equatable { // Added Equatable
        let id: String
        let filename: String
        let linkedField: String?
        var localPath: String? // Path where file might be stored/cached locally by Rust
        var temporaryLocalURLForPreview: URL? // URL for QuickLook from temp dir if downloaded
    }
    
    enum TestPhase {
        case idle, settingUpData, singleDocUpload, singleDocListed, singleDocDeleted,
             multiDocUpload, multiDocListed, multiDocOneDeleted, multiDocAllDeleted
    }
    @State private var currentTestPhase: TestPhase = .idle

    var body: some View {
        ScrollView(.vertical, showsIndicators: true) {
            VStack(spacing: 15) {
                Text("🎯 Strategic Goal & Document Tests")
                    .font(.largeTitle)
                    .fontWeight(.bold)
                
                Text(statusMessage)
                    .font(.headline)
                    .foregroundColor(currentTestPhase != .idle ? .orange : .primary)

                // --- Test Execution Buttons ---
                Group {
                    Button("1️⃣ Setup: Create DocTypes & Admin Goal") {
                        Task { await setupInitialData() }
                    }
                    .testButtonStyle(currentTestPhase == .idle ? .blue : .gray)
                    .disabled(currentTestPhase != .idle && currentTestPhase != .settingUpData)

                    Button("2️⃣ Upload Single Linked Document (Plan)") {
                        Task { await uploadSingleStrategicPlanDocument() }
                    }
                    .testButtonStyle(currentTestPhase == .settingUpData ? .blue : .gray)
                    .disabled(adminGoalId == nil || strategicPlanDocTypeId == nil || currentTestPhase != .settingUpData)

                    Button("3️⃣ Delete Single Linked Document") {
                        Task { await deleteFirstUploadedDocument() }
                    }
                    .testButtonStyle(currentTestPhase == .singleDocListed ? .red : .gray)
                    .disabled(currentTestPhase != .singleDocListed || uploadedDocuments.filter { $0.filename == "test_strategic_plan.pdf" }.isEmpty)

                    Button("4️⃣ Upload Multiple Documents (Support Files)") {
                        Task { await uploadMultipleSupportDocuments() }
                    }
                    .testButtonStyle(currentTestPhase == .singleDocDeleted || currentTestPhase == .settingUpData ? .blue : .gray) // Allow if single was skipped or after deletion
                    .disabled(adminGoalId == nil || photoEvidenceDocTypeId == nil || generalReportDocTypeId == nil || (currentTestPhase != .singleDocDeleted && currentTestPhase != .settingUpData))
                    
                    Button("5️⃣ Delete One from Multiple (test_word.docx)") {
                        Task { await deleteSpecificDocument(filename: "test_word.docx") }
                    }
                    .testButtonStyle(currentTestPhase == .multiDocListed ? .red : .gray)
                    .disabled(currentTestPhase != .multiDocListed || uploadedDocuments.filter { $0.filename == "test_word.docx" }.isEmpty)

                    Button("🗑️ Delete All Documents") {
                        Task {
                            for doc in uploadedDocuments {
                                await deleteDocumentById(doc.id, filename: doc.filename)
                            }
                        }
                    }
                    .testButtonStyle(currentTestPhase == .multiDocOneDeleted || currentTestPhase == .multiDocListed ? .red : .gray)
                    .disabled((currentTestPhase != .multiDocOneDeleted && currentTestPhase != .multiDocListed) || uploadedDocuments.isEmpty)
                    
                    Button("🔓 Unregister All Documents") {
                        Task {
                            for doc in uploadedDocuments {
                                await unregisterDocumentUsage(doc.id)
                            }
                            updateResults("✅ Unregistered all documents")
                        }
                    }
                    .testButtonStyle(.orange)
                    .disabled(uploadedDocuments.isEmpty)
                    
                     Button("🔄 Reset All Test Data & UI") {
                        resetAllTestData()
                    }
                    .testButtonStyle(.orange)
                }
                .padding(.horizontal)

                // --- Document List ---
                if !uploadedDocuments.isEmpty {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("📄 Documents for Goal ID: \(adminGoalId?.prefix(8) ?? "N/A") (\(uploadedDocuments.count))")
                            .font(.headline)
                        ForEach(uploadedDocuments) { doc in
                            DocumentRow(
                                document: doc,
                                onOpen: { openDocument(doc) },
                                onDelete: { Task { await deleteDocumentById(doc.id, filename: doc.filename) } }
                            )
                        }
                    }
                    .padding()
                    .background(Color(.systemGray5))
                    .cornerRadius(10)
                    .padding(.horizontal)
                } else if adminGoalId != nil {
                     Text("No documents currently associated with goal \(adminGoalId?.prefix(8) ?? "N/A").")
                        .padding()
                }

                // --- Test Results Log ---
                ScrollViewReader { proxy in
                    ScrollView(.vertical, showsIndicators: true) {
                        Text(testResults)
                            .font(.system(size: 9, design: .monospaced))
                            .padding()
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id("resultsLog")
                    }
                    .frame(height: 250)
                    .background(Color(.systemGray6))
                    .cornerRadius(8)
                    .padding(.horizontal)
                    .onChange(of: testResults) { _ in
                        withAnimation {
                            proxy.scrollTo("resultsLog", anchor: .top)
                        }
                    }
                }
            }
            .padding(.vertical)
        }
        .sheet(item: $selectedDocumentURL) { identifiableURL in
            QuickLookView(url: identifiableURL.url) {
                // Unregister document when QuickLook is dismissed
                if let doc = uploadedDocuments.first(where: { 
                    $0.temporaryLocalURLForPreview == identifiableURL.url 
                }) {
                    Task {
                        await unregisterDocumentUsage(doc.id)
                    }
                }
            }
        }
        .onAppear {
             updateResults("StrategicTestView appeared. Ensure Core Tests for authentication have run.", clearPrevious: true)
        }
    }

    // MARK: - Test Step Functions

    func resetAllTestData() {
        updateResults("🔄 Resetting test data and UI...", clearPrevious: true)
        // Note: This doesn't clear backend data. For full reset, backend functions would be needed.
        adminGoalId = nil
        strategicPlanDocTypeId = nil
        photoEvidenceDocTypeId = nil
        generalReportDocTypeId = nil
        uploadedDocuments.removeAll()
        currentTestPhase = .idle
        statusMessage = "Ready to test Strategic Domain"
        updateResults("UI Reset. Please run database cleanup if needed and restart tests.")
    }

    func setupInitialData() async {
        currentTestPhase = .settingUpData
        statusMessage = "Setting up initial data..."
        updateResults("--- 1️⃣ Setting up Document Types & Admin Goal ---", clearPrevious: true)
        await initializeDocumentTypes()
        await ensureAdminGoalExists()
        if adminGoalId != nil && strategicPlanDocTypeId != nil && photoEvidenceDocTypeId != nil && generalReportDocTypeId != nil {
            updateResults("✅ Initial data setup complete.")
            statusMessage = "Setup complete. Ready for document tests."
        } else {
            updateResults("❌ Initial data setup failed. Check logs.")
            statusMessage = "Setup failed. Cannot proceed."
            currentTestPhase = .idle // Reset phase if setup fails
        }
    }
    
    private func initializeDocumentTypes() async {
        updateResults("Initializing document types...")
        
        let documentTypesToCreate = [
            (name: "Strategic Plan", extensions: "pdf,doc,docx", maxSize: 10485760, priority: "high", typeIdState: \Self.strategicPlanDocTypeId),
            (name: "Photo Evidence", extensions: "jpg,jpeg,png,heic", maxSize: 5242880, priority: "normal", typeIdState: \Self.photoEvidenceDocTypeId),
            (name: "General Report", extensions: "pdf,xlsx,csv,docx,txt", maxSize: 10485760, priority: "normal", typeIdState: \Self.generalReportDocTypeId)
        ]
        
        let authPayload = AuthContextPayload(
            user_id: authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000", // Fallback admin
            role: authState.lastLoggedInUser?.role ?? "admin", // Fallback admin
            device_id: AuthenticationState.getDeviceId(),
            offline_mode: false
        )
        
        for docTypeInfo in documentTypesToCreate {
            let relatedTablesJsonString = "[\"strategic_goals\",\"projects\",\"activities\"]"
            let docTypeDetails = DocumentTypeDetailsPayload(
                name: docTypeInfo.name,
                description: "System-defined document type for \(docTypeInfo.name)",
                allowed_extensions: docTypeInfo.extensions, max_size: docTypeInfo.maxSize,
                compression_level: 6, compression_method: "lossless",
                min_size_for_compression: 1024 * 1024, // 1MB
                default_priority: docTypeInfo.priority, icon: "doc_icon_default",
                related_tables: relatedTablesJsonString
            )
            let ffiPayload = DocumentTypeCreateFFIPayload(document_type: docTypeDetails, auth: authPayload)
            
            do {
                let jsonData = try JSONEncoder().encode(ffiPayload)
                guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                    updateResults("❌ Failed to create JSON string for \(docTypeInfo.name)"); continue
                }
                
                var ccharResult: UnsafeMutablePointer<CChar>?
                let status = document_type_create(jsonString, &ccharResult)
                
                guard let resultStr = ccharResult else {
                    updateResults("❌ document_type_create returned null pointer for \(docTypeInfo.name)")
                    if status != 0 { 
                        if let errorPtr = get_last_error() {
                            let error = String(cString: errorPtr)
                            updateResults("   Error: \(error)")
                        }
                    }
                    continue
                }
                defer { document_free(resultStr) }

                if status == 0 {
                    let response = String(cString: resultStr)
                    if let data = response.data(using: .utf8),
                       let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                       let id = json["id"] as? String {
                        // Set the ID based on the document type name
                        switch docTypeInfo.name {
                        case "Strategic Plan":
                            strategicPlanDocTypeId = id
                        case "Photo Evidence":
                            photoEvidenceDocTypeId = id
                        case "General Report":
                            generalReportDocTypeId = id
                        default:
                            break
                        }
                        updateResults("✅ Created/Verified Document Type '\(docTypeInfo.name)' with ID: \(id.prefix(8))...")
                    } else {
                        updateResults("⚠️ Created \(docTypeInfo.name) but couldn't parse ID from response: \(response.prefix(100))")
                    }
                } else {
                    if let errorPtr = get_last_error() {
                        let error = String(cString: errorPtr)
                        // Check if it's a "unique constraint" error, meaning it likely already exists
                        if error.localizedCaseInsensitiveContains("unique constraint") || error.localizedCaseInsensitiveContains("already exists") {
                             updateResults("ℹ️ Document Type '\(docTypeInfo.name)' likely already exists. Attempting to find it...")
                            // Try to find it by name if creation failed due to uniqueness
                            await findDocumentTypeByName(name: docTypeInfo.name)
                        } else {
                            updateResults("❌ Failed to create Document Type '\(docTypeInfo.name)': \(error). JSON: \(jsonString.prefix(200))")
                        }
                    } else {
                        updateResults("❌ Failed to create Document Type '\(docTypeInfo.name)': Unknown error. JSON: \(jsonString.prefix(200))")
                    }
                }
            } catch {
                updateResults("❌ Error encoding JSON for \(docTypeInfo.name): \(error)")
            }
        }
        updateResults("🏁 Document types initialization/verification process completed.")
    }

    private func findDocumentTypeByName(name: String) async {
        let payload = """
        {
            "name": "\(name)",
            "auth": \(getAuthContextAsJSONString())
        }
        """
        var ccharResult: UnsafeMutablePointer<CChar>?
        let status = document_type_find_by_name(payload, &ccharResult)

        guard let resultStr = ccharResult else {
            updateResults("❌ document_type_find_by_name returned null for \(name)"); return
        }
        defer { document_free(resultStr) }

        if status == 0 {
            let response = String(cString: resultStr)
            if let data = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any], // Should be a single object or null
               let id = json["id"] as? String {
                // Set the ID based on the document type name
                switch name {
                case "Strategic Plan":
                    strategicPlanDocTypeId = id
                case "Photo Evidence":
                    photoEvidenceDocTypeId = id
                case "General Report":
                    generalReportDocTypeId = id
                default:
                    break
                }
                updateResults("✅ Found existing Document Type '\(name)' with ID: \(id.prefix(8))...")
            } else if response.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "null" || response.trimmingCharacters(in: .whitespacesAndNewlines) == "{}" {
                 updateResults("❌ Document Type '\(name)' not found by name after creation attempt failed.")
            }
            else {
                updateResults("⚠️ Could not parse ID for existing Document Type '\(name)' from response: \(response.prefix(100))")
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Error finding Document Type '\(name)' by name: \(error)")
            } else {
                updateResults("❌ Error finding Document Type '\(name)' by name: Unknown error")
            }
        }
    }

    private func ensureAdminGoalExists() async {
        updateResults("Ensuring 'admin-001' goal exists...")
        let listPayload = """
        {
            "pagination": {"page": 1, "per_page": 10}, 
            "auth": \(getAuthContextAsJSONString())
        }
        """
        var listResult: UnsafeMutablePointer<CChar>?
        let listStatus = strategic_goal_list(listPayload, &listResult)

        guard let listResultStr = listResult else {
            updateResults("❌ strategic_goal_list returned null pointer while checking for admin-001")
            if listStatus != 0 { 
                if let errorPtr = get_last_error() {
                    let err = String(cString: errorPtr)
                    updateResults("   Error: \(err)")
                }
            }
            return
        }
        defer { strategic_goal_free(listResultStr) }

        var foundGoalId: String? = nil
        if listStatus == 0 {
            let response = String(cString: listResultStr)
            if let data = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                if let adminGoal = items.first(where: { $0["objective_code"] as? String == "ADMIN-001" }) {
                    foundGoalId = adminGoal["id"] as? String
                }
            }
        }

        if let id = foundGoalId {
            adminGoalId = id
            updateResults("✅ Found existing 'ADMIN-001' goal with ID: \(id.prefix(8))...")
        } else {
            updateResults("ℹ️ 'ADMIN-001' goal not found, attempting to create...")
            let createPayload = """
            {
                "goal": {
                    "objective_code": "ADMIN-001",
                    "outcome": "Primary goal for document testing by admin user.",
                    "kpi": "Document operations successful",
                    "target_value": 10.0,
                    "actual_value": 0.0,
                    "status_id": 1, 
                    "responsible_team": "Test Team",
                    "sync_priority": "Normal",
                    "created_by_user_id": "\(authState.lastLoggedInUser?.userId ?? "00000000-0000-0000-0000-000000000000")"
                },
                "auth": \(getAuthContextAsJSONString(roleOverride: "admin")) 
            }
            """
            var createResult: UnsafeMutablePointer<CChar>?
            let createStatus = strategic_goal_create(createPayload, &createResult)
            
            guard let createResultStr = createResult else {
                updateResults("❌ strategic_goal_create returned null for ADMIN-001")
                if createStatus != 0 { 
                    if let errorPtr = get_last_error() {
                        let err = String(cString: errorPtr)
                        updateResults("   Error: \(err)")
                    }
                }
                return
            }
            defer { strategic_goal_free(createResultStr) }

            if createStatus == 0 {
                let response = String(cString: createResultStr)
                 if let data = response.data(using: .utf8),
                   let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let id = json["id"] as? String {
                    adminGoalId = id
                    updateResults("✅ Created 'ADMIN-001' goal with ID: \(id.prefix(8))...")
                } else {
                     updateResults("❌ Created 'ADMIN-001' but failed to parse ID from response: \(response.prefix(100))")
                }
            } else {
                if let errorPtr = get_last_error() {
                    let error = String(cString: errorPtr)
                    updateResults("❌ Failed to create 'ADMIN-001' goal: \(error). Payload: \(createPayload.prefix(200))")
                } else {
                    updateResults("❌ Failed to create 'ADMIN-001' goal: Unknown error. Payload: \(createPayload.prefix(200))")
                }
            }
        }
    }

    func uploadSingleStrategicPlanDocument() async {
        guard let goalId = adminGoalId, let docTypeId = strategicPlanDocTypeId else {
            updateResults("❌ Cannot upload single plan: Admin Goal ID or Strategic Plan DocType ID missing.")
            return
        }
        currentTestPhase = .singleDocUpload
        statusMessage = "Uploading single linked document..."
        updateResults("\n--- 2️⃣ Uploading Single Document (test_strategic_plan.pdf) ---")
        
        guard let pdfData = loadTestDocument(named: "test_strategic_plan", type: "pdf") else {
            statusMessage = "Failed to load test PDF."
            currentTestPhase = .settingUpData // Revert phase
            return
        }
        let base64Data = pdfData.base64EncodedString()
        
        let payload = """
        {
            "goal_id": "\(goalId)",
            "file_data": "\(base64Data)",
            "original_filename": "test_strategic_plan.pdf",
            "title": "Official Strategic Plan Document",
            "document_type_id": "\(docTypeId)",
            "linked_field": "actual_value", 
            "sync_priority": "high",
            "auth": \(getAuthContextAsJSONString())
        }
        """
        
        var ccharResult: UnsafeMutablePointer<CChar>?
        let status = strategic_goal_upload_document(payload, &ccharResult)
        
        guard let resultStr = ccharResult else {
            updateResults("❌ strategic_goal_upload_document returned null pointer.")
            if status != 0 { 
                if let errorPtr = get_last_error() {
                    let err = String(cString: errorPtr)
                    updateResults("   Error: \(err)")
                }
            }
            statusMessage = "Upload failed."
            currentTestPhase = .settingUpData // Revert phase
            return
        }
        defer { strategic_goal_free(resultStr) }

        if status == 0 {
            let response = String(cString: resultStr)
            if let data = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let docId = json["id"] as? String {
                let newDoc = DocumentInfo(
                    id: docId,
                    filename: json["original_filename"] as? String ?? "test_strategic_plan.pdf",
                    linkedField: json["field_identifier"] as? String, // This should be "actual_value"
                    localPath: json["file_path"] as? String
                )
                if !uploadedDocuments.contains(where: { $0.id == newDoc.id }) {
                    uploadedDocuments.append(newDoc)
                }
                updateResults("✅ Uploaded '\(newDoc.filename)' to field '\(newDoc.linkedField ?? "none")'. ID: \(newDoc.id.prefix(8))... Path: \(newDoc.localPath ?? "N/A")")
                await listGoalDocuments() // Refresh list
                currentTestPhase = .singleDocListed
                statusMessage = "Single document uploaded."
            } else {
                updateResults("❌ Uploaded single doc but failed to parse response: \(response.prefix(100)).")
                statusMessage = "Upload parsing failed."
                currentTestPhase = .settingUpData
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Single document upload failed: \(error)")
            } else {
                updateResults("❌ Single document upload failed: Unknown error")
            }
            statusMessage = "Upload error."
            currentTestPhase = .settingUpData
        }
    }

    func deleteFirstUploadedDocument() async {
        guard let docToDelete = uploadedDocuments.first(where: { $0.filename == "test_strategic_plan.pdf" }) else {
            updateResults("ℹ️ No 'test_strategic_plan.pdf' found to delete.")
            currentTestPhase = .singleDocDeleted // Assume it was already deleted or never uploaded
            statusMessage = "Plan document not found for deletion."
            return
        }
        statusMessage = "Deleting single linked document..."
        updateResults("\n--- 3️⃣ Deleting Single Linked Document (\(docToDelete.filename)) ---")
        await deleteDocumentById(docToDelete.id, filename: docToDelete.filename)
        if !uploadedDocuments.contains(where: {$0.id == docToDelete.id}) {
             currentTestPhase = .singleDocDeleted
             statusMessage = "Single document deleted."
        } else {
            // Deletion failed, stay in current phase or revert
            statusMessage = "Failed to delete single document."
        }
    }
    
    func uploadMultipleSupportDocuments() async {
        guard let goalId = adminGoalId, 
              let photoDocTypeId = photoEvidenceDocTypeId,
              let reportDocTypeId = generalReportDocTypeId else {
            updateResults("❌ Cannot upload multiple docs: Admin Goal ID or relevant DocType IDs missing.")
            return
        }
        currentTestPhase = .multiDocUpload
        statusMessage = "Uploading multiple documents..."
        updateResults("\n--- 4️⃣ Uploading Multiple Support Documents ---")
        
        var filesToUpload: [(data: Data, filename: String, docTypeId: String, title: String)] = []
        
        if let reportData = loadTestDocument(named: "test_report", type: "pdf") {
            filesToUpload.append((reportData, "test_report.pdf", reportDocTypeId, "Monthly Progress Report"))
        }
        if let wordData = loadTestDocument(named: "test_word", type: "docx") {
            filesToUpload.append((wordData, "test_word.docx", reportDocTypeId, "Field Visit Notes"))
        }
        if let imageData = loadTestDocument(named: "test_image", type: "jpg") {
            filesToUpload.append((imageData, "test_image.jpg", photoDocTypeId, "Site Photo Evidence"))
        }

        guard !filesToUpload.isEmpty else {
            updateResults("❌ No test documents loaded for bulk upload.");
            statusMessage = "No files for bulk upload."
            currentTestPhase = .singleDocDeleted // Revert phase
            return
        }

        var filesPayloadArray: [[String: String]] = []
        for fileInfo in filesToUpload {
            filesPayloadArray.append([
                "file_data": fileInfo.data.base64EncodedString(),
                "filename": fileInfo.filename
                // "title" and "document_type_id" are per-batch in this FFI, not per-file.
                // If your FFI `strategic_goal_bulk_upload_documents` supports per-file types/titles, adjust here.
                // For now, we assume it uses one docTypeId for the batch. We'll use generalReportDocTypeId.
            ])
        }
        
        let filesJsonData = try! JSONSerialization.data(withJSONObject: filesPayloadArray)
        let filesJsonStr = String(data: filesJsonData, encoding: .utf8)!
        
        // Using generalReportDocTypeId for the batch. If different types are needed per file in a batch,
        // the FFI `strategic_goal_bulk_upload_documents` or the upload strategy needs adjustment.
        // Or, upload them in separate batches if the FFI is batch-type specific.
        // For this test, let's assume the FFI is designed for a batch of similar-typed general documents.
        let payload = """
        {
            "goal_id": "\(goalId)",
            "files": \(filesJsonStr),
            "title": "General Supporting Documents Batch", 
            "document_type_id": "\(reportDocTypeId)", 
            "sync_priority": "normal",
            "auth": \(getAuthContextAsJSONString())
        }
        """
        
        var ccharResult: UnsafeMutablePointer<CChar>?
        let status = strategic_goal_bulk_upload_documents(payload, &ccharResult)
        
        guard let resultStr = ccharResult else {
            updateResults("❌ strategic_goal_bulk_upload_documents returned null pointer.")
            if status != 0 { 
                if let errorPtr = get_last_error() {
                    let err = String(cString: errorPtr)
                    updateResults("   Error: \(err)")
                }
            }
            statusMessage = "Bulk upload failed."
            currentTestPhase = .singleDocDeleted // Revert phase
            return
        }
        defer { strategic_goal_free(resultStr) }

        if status == 0 {
            let response = String(cString: resultStr)
            if let data = response.data(using: .utf8),
               let docsArray = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] {
                var newDocsCount = 0
                for docJson in docsArray {
                    if let docId = docJson["id"] as? String {
                        let newDoc = DocumentInfo(
                            id: docId,
                            filename: docJson["original_filename"] as? String ?? "unknown_file",
                            linkedField: docJson["field_identifier"] as? String, // Likely nil for bulk uploads via this FFI
                            localPath: docJson["file_path"] as? String
                        )
                        if !uploadedDocuments.contains(where: { $0.id == newDoc.id }) {
                            uploadedDocuments.append(newDoc)
                        }
                        newDocsCount += 1
                        updateResults("✅ Bulk Uploaded: \(newDoc.filename). ID: \(newDoc.id.prefix(8))...")
                    }
                }
                updateResults("✅ Bulk upload processed \(newDocsCount) of \(docsArray.count) documents from response.")
                await listGoalDocuments()
                currentTestPhase = .multiDocListed
                statusMessage = "Multiple documents uploaded."
            } else {
                 updateResults("❌ Bulk upload succeeded but failed to parse response: \(response.prefix(100)).")
                 statusMessage = "Bulk upload parsing error."
                 currentTestPhase = .singleDocDeleted
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Bulk upload failed: \(error)")
            } else {
                updateResults("❌ Bulk upload failed: Unknown error")
            }
            statusMessage = "Bulk upload error."
            currentTestPhase = .singleDocDeleted
        }
    }

    func deleteSpecificDocument(filename: String) async {
        guard let docToDelete = uploadedDocuments.first(where: { $0.filename == filename }) else {
            updateResults("ℹ️ Document '\(filename)' not found to delete.")
             statusMessage = "'\(filename)' not found for deletion."
            // Consider current phase: if it was expected, maybe change phase
            if currentTestPhase == .multiDocListed { currentTestPhase = .multiDocOneDeleted }
            return
        }
        statusMessage = "Deleting '\(filename)'..."
        updateResults("\n--- 5️⃣ Deleting Specific Document (\(filename)) ---")
        await deleteDocumentById(docToDelete.id, filename: filename)
        if !uploadedDocuments.contains(where: {$0.id == docToDelete.id}) {
            currentTestPhase = .multiDocOneDeleted
            statusMessage = "'\(filename)' deleted."
        } else {
            statusMessage = "Failed to delete '\(filename)'."
        }
    }

    func deleteAllGoalDocuments() async {
        guard let goalId = adminGoalId, !uploadedDocuments.isEmpty else {
            updateResults("ℹ️ No documents to delete for goal or goal ID missing.")
            currentTestPhase = .multiDocAllDeleted
            statusMessage = "No documents to delete."
            return
        }
        statusMessage = "Deleting all goal documents..."
        updateResults("\n--- 6️⃣ Deleting All Documents for Goal \(goalId.prefix(8))... ---")
        
        // Create a copy of IDs to iterate over, as `deleteDocumentById` modifies `uploadedDocuments`
        let idsToDelete = uploadedDocuments.map { $0.id }
        let filenamesToDelete = uploadedDocuments.map { $0.filename }

        for (index, docId) in idsToDelete.enumerated() {
            let filename = filenamesToDelete[index]
            updateResults("Attempting to delete \(filename) (ID: \(docId.prefix(8)))...")
            await deleteDocumentById(docId, filename: filename)
        }
        
        if uploadedDocuments.isEmpty {
            updateResults("✅ All documents for the goal should now be deleted.")
            currentTestPhase = .multiDocAllDeleted
            statusMessage = "All documents deleted."
        } else {
            updateResults("⚠️ Some documents might not have been deleted. \(uploadedDocuments.count) remaining.")
            statusMessage = "Not all documents deleted."
            // currentTestPhase might remain .multiDocOneDeleted or similar depending on exact flow desired
        }
        await listGoalDocuments() // Final verification
    }
    
    // MARK: - Core Document Operations (Helper)

    private func loadTestDocument(named name: String, type ext: String) -> Data? {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext) else {
            updateResults("❌ Could not find \(name).\(ext) in bundle")
            return nil
        }
        do {
            let data = try Data(contentsOf: url)
            updateResults("✅ Loaded \(name).\(ext) - \(formatFileSize(data.count))")
            return data
        } catch {
            updateResults("❌ Failed to load \(name).\(ext): \(error)")
            return nil
        }
    }

    private func listGoalDocuments() async {
        guard let goalId = adminGoalId else {
            updateResults("ℹ️ Admin Goal ID missing, cannot list documents.")
            uploadedDocuments.removeAll() // Clear UI list if no goal
            return
        }
        updateResults("🔄 Refreshing document list for goal \(goalId.prefix(8))...")
        
        // Fix: Change from ["document_type"] to [{"type": "DocumentType"}]
        let includeArray = [["type": "DocumentType"]]
        let includeJSON = try! JSONSerialization.data(withJSONObject: includeArray)
        let includeStr = String(data: includeJSON, encoding: .utf8)!
        
        let payload = """
        {
            "related_table": "strategic_goals",
            "related_id": "\(goalId)",
            "pagination": {"page": 1, "per_page": 50},
            "include": \(includeStr),
            "auth": \(getAuthContextAsJSONString())
        }
        """
        var ccharResult: UnsafeMutablePointer<CChar>?
        let status = document_list_by_entity(payload, &ccharResult)
        
        guard let resultStr = ccharResult else {
            updateResults("❌ document_list_by_entity returned null pointer.")
            if status != 0 { 
                if let errorPtr = get_last_error() {
                    let err = String(cString: errorPtr)
                    updateResults("   Error: \(err)")
                }
            }
            return
        }
        defer { document_free(resultStr) }

        if status == 0 {
            let response = String(cString: resultStr)
            if let data = response.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let items = json["items"] as? [[String: Any]] {
                updateResults("Found \(items.count) documents:")
                
                let newDocumentList = items.compactMap { docData -> DocumentInfo? in
                    guard let id = docData["id"] as? String else { return nil }
                    return DocumentInfo(
                        id: id,
                        filename: docData["original_filename"] as? String ?? "Unknown Filename",
                        linkedField: docData["field_identifier"] as? String,
                        localPath: docData["file_path"] as? String
                    )
                }
                // Efficiently update the list to avoid flicker if possible, or just replace
                if uploadedDocuments != newDocumentList {
                     uploadedDocuments = newDocumentList
                }

                if items.isEmpty {
                    updateResults("   No documents found for this goal.")
                } else {
                    items.forEach { docData in
                        let filename = docData["original_filename"] as? String ?? "N/A"
                        let fieldId = docData["field_identifier"] as? String ?? "general"
                        let isLocal = docData["is_available_locally"] as? Bool ?? false
                        let size = docData["size_bytes"] as? Int ?? 0
                        updateResults("  📄 '\(filename)' (Linked: \(fieldId), Local: \(isLocal), Size: \(formatFileSize(size)))")
                    }
                }
            } else {
                updateResults("❌ Failed to parse document list response: \(response.prefix(100))")
                if uploadedDocuments.isEmpty == false { uploadedDocuments.removeAll() } // Clear if parsing fails
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Failed to list documents: \(error)")
            } else {
                updateResults("❌ Failed to list documents: Unknown error")
            }
            if uploadedDocuments.isEmpty == false { uploadedDocuments.removeAll() }
        }
    }
    
    private func openDocument(_ doc: DocumentInfo) {
        updateResults("Attempting to open: \(doc.filename) (ID: \(doc.id.prefix(8)))...")
        
        // If we already have a temporary local URL from a previous download for preview, use it.
        if let tempURL = doc.temporaryLocalURLForPreview, FileManager.default.fileExists(atPath: tempURL.path) {
            updateResults("✅ Opening previously downloaded temporary file: \(tempURL.lastPathComponent)")
            
            // IMPORTANT: Unregister the document before opening with QuickLook
            Task {
                await unregisterDocumentUsage(doc.id)
                await MainActor.run {
                    selectedDocumentURL = IdentifiableURL(url: tempURL)
                }
            }
            return
        }
        
        // Otherwise, try to get the path from Rust's cache via `document_open`
        let openPayload = """
        {"id": "\(doc.id)", "auth": \(getAuthContextAsJSONString())}
        """
        var openResultPtr: UnsafeMutablePointer<CChar>?
        let openStatus = document_open(openPayload, &openResultPtr)

        guard let openResultStr = openResultPtr else {
            updateResults("❌ document_open returned null pointer for \(doc.filename). Attempting download...")
            Task { await downloadAndOpenDocument(doc) }
            return
        }
        defer { document_free(openResultStr) }

        if openStatus == 0 {
            let openResponse = String(cString: openResultStr)
            if let data = openResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let filePath = json["file_path"] as? String, !filePath.isEmpty {
                
                // Construct full path. NOTE: The file_path from Rust is relative to the base storage path.
                // Based on the Rust LocalFileStorageService, the structure is:
                // {base_storage_path}/{relative_path}
                // where base_storage_path was set via set_ios_storage_path() during initialization
                // and relative_path follows the pattern: "original/entity_type/entity_id/unique_filename.ext"
                
                // The base storage path in our Swift app is: {Documents}/ActionAid/storage
                let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
                let storageDirectory = documentsPath.appendingPathComponent("ActionAid/storage")
                let fullPath = storageDirectory.appendingPathComponent(filePath)

                if FileManager.default.fileExists(atPath: fullPath.path) {
                    updateResults("✅ Opening local file from Rust cache: \(fullPath.path)")
                    // Unregister before opening
                    Task {
                        await unregisterDocumentUsage(doc.id)
                        await MainActor.run {
                            selectedDocumentURL = IdentifiableURL(url: fullPath)
                        }
                    }
                } else {
                    updateResults("⚠️ Local file path from Rust ('\(fullPath.path)') not found. Attempting download...")
                    Task { await downloadAndOpenDocument(doc) }
                }
            } else {
                updateResults("ℹ️ Document '\(doc.filename)' not available locally via `document_open` (Path: \(openResponse.prefix(100))). Attempting download...")
                Task { await downloadAndOpenDocument(doc) }
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Error calling `document_open` for \(doc.filename): \(error). Attempting download...")
            } else {
                updateResults("❌ Error calling `document_open` for \(doc.filename): Unknown error. Attempting download...")
            }
            Task { await downloadAndOpenDocument(doc) }
        }
    }
    
    private func downloadAndOpenDocument(_ doc: DocumentInfo) async {
        updateResults("Downloading \(doc.filename) for preview...")
        let downloadPayload = """
        {"id": "\(doc.id)", "auth": \(getAuthContextAsJSONString())}
        """
        var downloadResultPtr: UnsafeMutablePointer<CChar>?
        let downloadStatus = document_download(downloadPayload, &downloadResultPtr)

        guard let downloadResultStr = downloadResultPtr else {
            updateResults("❌ document_download returned null for \(doc.filename)")
            if downloadStatus != 0 { 
                if let errorPtr = get_last_error() {
                    let err = String(cString: errorPtr)
                    updateResults("   Error: \(err)")
                }
            }
            return
        }
        defer { document_free(downloadResultStr) }

        if downloadStatus == 0 {
            let downloadResponse = String(cString: downloadResultStr)
            if let data = downloadResponse.data(using: .utf8),
               let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let base64Data = json["data"] as? String,
               let fileData = Data(base64Encoded: base64Data) {
                
                let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + "-" + doc.filename) // Unique temp name
                do {
                    try fileData.write(to: tempURL)
                    // Update the document info with the temporary URL for potential re-opening without re-download
                    if let index = uploadedDocuments.firstIndex(where: { $0.id == doc.id }) {
                        uploadedDocuments[index].temporaryLocalURLForPreview = tempURL
                    }
                    await MainActor.run { selectedDocumentURL = IdentifiableURL(url: tempURL) }
                    updateResults("✅ Downloaded '\(doc.filename)' to temporary location for preview: \(tempURL.lastPathComponent)")
                } catch {
                    updateResults("❌ Failed to save downloaded file '\(doc.filename)' to temporary location: \(error)")
                }
            } else {
                updateResults("❌ No 'data' field or invalid base64 in download response for \(doc.filename). Response: \(downloadResponse.prefix(100))")
            }
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Download failed for \(doc.filename): \(error)")
            } else {
                updateResults("❌ Download failed for \(doc.filename): Unknown error")
            }
        }
    }
    
    private func deleteDocumentById(_ docId: String, filename: String) async {
        updateResults("Attempting to delete document: '\(filename)' (ID: \(docId.prefix(8)))...")
        let payload = """
        {"id": "\(docId)", "auth": \(getAuthContextAsJSONString())}
        """
        // Assuming document_delete performs a hard delete or the type of delete appropriate for testing
        let status = document_delete(payload) 
        
        if status == 0 {
            uploadedDocuments.removeAll { $0.id == docId }
            updateResults("✅ Document '\(filename)' (ID: \(docId.prefix(8))) deleted successfully from backend.")
            // Optionally, clean up any temporary local file if it exists
            // This requires tracking temporaryLocalURLForPreview if set during download/open
        } else {
            if let errorPtr = get_last_error() {
                let error = String(cString: errorPtr)
                updateResults("❌ Failed to delete document '\(filename)' (ID: \(docId.prefix(8))): \(error)")
            } else {
                updateResults("❌ Failed to delete document '\(filename)' (ID: \(docId.prefix(8))): Unknown error")
            }
        }
        await listGoalDocuments() // Refresh list from backend
    }
    
    // MARK: - Helpers
    private func getAuthContextAsJSONString(roleOverride: String? = nil) -> String {
        let currentUser = authState.lastLoggedInUser
        // Fallback to a default admin if no user is logged in, common for tests
        let userId = currentUser?.userId ?? "00000000-0000-0000-0000-000000000000" // Default Admin User ID
        let userRole = roleOverride ?? currentUser?.role ?? "admin" // Default Admin Role or override

        let authData = AuthContextPayload(
            user_id: userId,
            role: userRole,
            device_id: AuthenticationState.getDeviceId(),
            offline_mode: false
        )
        let encoder = JSONEncoder()
        guard let jsonData = try? encoder.encode(authData), 
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            updateResults("❌ Critical error: Could not create auth context JSON.")
            return "{}" // Should ideally not happen
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
            print("📄 \(logMessage)") // Also print to console for easier debugging
        }
    }

    private func formatFileSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useKB, .useMB, .useBytes]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }

     // Helper to get last error from Rust FFI
    private func get_last_error() -> UnsafeMutablePointer<CChar>? {
        // This function should be globally available if your bridging header exposes it
        // For this example, assuming it's named get_last_error in Rust FFI
        // and you have a way to call it.
        // If it's part of a specific module, adjust the call.
        // e.g. error_handling_get_last_error()
        // This is a placeholder; ensure you have the actual FFI function.
        return अर्थाद्get_last_error() // Replace with your actual FFI error function
    }

    // Add this helper function
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
                free_string(errorPtr)
            }
        }
    }
}


// MARK: - Document Row View (Ensure this is appropriate for your UI)
struct DocumentRow: View {
    let document: StrategicTestView.DocumentInfo
    let onOpen: () -> Void
    let onDelete: () -> Void
    
    var body: some View {
        HStack {
            Image(systemName: documentIconName(for: document.filename))
                .font(.title2)
                .frame(width: 30)

            VStack(alignment: .leading) {
                Text(document.filename)
                    .font(.caption)
                    .fontWeight(.medium)
                    .lineLimit(1)
                if let field = document.linkedField {
                    Text("Linked to: \(field)")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                if let path = document.localPath {
                    Text("Cache Path: \(path.suffix(30))") // Show only last part of path
                        .font(.caption2)
                        .foregroundColor(.gray)
                        .lineLimit(1)
                } else if document.temporaryLocalURLForPreview != nil {
                     Text("Preview available (temp)")
                        .font(.caption2)
                        .foregroundColor(.green)
                }
            }
            Spacer()
            Button(action: onOpen) { Image(systemName: "eye").foregroundColor(.blue) }
                .buttonStyle(BorderlessButtonStyle())
            Button(action: onDelete) { Image(systemName: "trash").foregroundColor(.red) }
                .buttonStyle(BorderlessButtonStyle())
        }
        .padding(.vertical, 4)
    }

    private func documentIconName(for filename: String) -> String {
        let ext = (filename as NSString).pathExtension.lowercased()
        switch ext {
        case "pdf": return "doc.text.fill"
        case "doc", "docx": return "doc.richtext.fill"
        case "jpg", "jpeg", "png", "heic": return "photo.fill"
        case "zip", "rar": return "doc.zipper"
        default: return "doc.fill"
        }
    }
}

// MARK: - Button Style
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

// MARK: - Global Error Function (Placeholder - ensure you have this FFI)
// You need to ensure this function is correctly bridged from your Rust FFI
// and returns a pointer to the last error string.
// This is a placeholder declaration.
@_silgen_name("get_last_error") // Or whatever your Rust FFI function is named
private func अर्थाद्get_last_error() -> UnsafeMutablePointer<CChar>?
