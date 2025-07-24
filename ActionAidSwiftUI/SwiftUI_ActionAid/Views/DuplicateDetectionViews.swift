//
//  DuplicateDetectionViews.swift
//  SwiftUI_ActionAid
//
//  Smart duplicate detection UI components for participants
//

import SwiftUI
import QuickLook

// MARK: - Main Duplicate Detection Popup

struct DuplicateDetectionPopup: View {
    let duplicates: [ParticipantDuplicateInfo]
    let onContinue: () -> Void
    let onCancel: () -> Void
    @Environment(\.dismiss) var dismiss
    
    @State private var currentIndex = 0
    @State private var selectedDocumentURL: IdentifiableURL?
    
    var body: some View {
        VStack(spacing: 0) {
            // Modern Header with Gradient
            VStack(spacing: 16) {
                // Handle bar
                RoundedRectangle(cornerRadius: 2.5)
                    .fill(Color(.systemGray4))
                    .frame(width: 36, height: 5)
                    .padding(.top, 8)
                
                VStack(spacing: 12) {
                    HStack {
                        ZStack {
                            Circle()
                                .fill(.orange.opacity(0.15))
                                .frame(width: 44, height: 44)
                            
                            Image(systemName: "person.2.badge.key")
                                .font(.title2)
                                .foregroundStyle(.orange)
                        }
                        
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Potential Duplicate")
                                .font(.title2)
                                .fontWeight(.bold)
                            
                            Text("Found \(duplicates.count) matching participant\(duplicates.count == 1 ? "" : "s")")
                                .font(.subheadline)
                                .foregroundColor(.secondary)
                        }
                        
                        Spacer()
                    }
                    
                    Text("Review existing records to avoid creating duplicates")
                        .font(.footnote)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 20)
            .background(
                LinearGradient(
                    colors: [Color(.systemBackground), Color(.systemGray6).opacity(0.3)],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            
            // Modern Page Indicator
            if duplicates.count > 1 {
                HStack(spacing: 8) {
                    ForEach(0..<duplicates.count, id: \.self) { index in
                        Capsule()
                            .fill(index == currentIndex ? Color.accentColor : Color(.systemGray5))
                            .frame(width: index == currentIndex ? 24 : 8, height: 4)
                            .animation(.spring(response: 0.3, dampingFraction: 0.7), value: currentIndex)
                    }
                }
                .padding(.bottom, 16)
            }
            
            // Modern Slidable Cards
            TabView(selection: $currentIndex) {
                ForEach(Array(duplicates.enumerated()), id: \.element.id) { index, duplicate in
                    ModernDuplicateCard(
                        participant: duplicate,
                        onDocumentTap: { url in
                            selectedDocumentURL = IdentifiableURL(url: url)
                        }
                    )
                    .tag(index)
                    .padding(.horizontal, 20)
                }
            }
            .tabViewStyle(PageTabViewStyle(indexDisplayMode: .never))
            
            // Modern Action Buttons
            VStack(spacing: 12) {
                Button {
                    UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                    onContinue()
                    dismiss()
                } label: {
                    HStack {
                        Image(systemName: "plus.circle.fill")
                        Text("Create Anyway")
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 50)
                    .background(.blue)
                    .foregroundColor(.white)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
                
                Button {
                    UIImpactFeedbackGenerator(style: .light).impactOccurred()
                    onCancel()
                    dismiss()
                } label: {
                    HStack {
                        Image(systemName: "xmark.circle")
                        Text("Cancel Creation")
                            .fontWeight(.medium)
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 50)
                    .background(Color(.systemGray6))
                    .foregroundColor(.primary)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                }
            }
            .padding(.horizontal, 24)
            .padding(.bottom, max(UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .first?.windows.first?.safeAreaInsets.bottom ?? 0, 16))
            .background(Color(.systemBackground))
        }
        .background(Color(.systemBackground))
        .sheet(item: $selectedDocumentURL) { identifiableURL in
            QuickLookView(url: identifiableURL.url)
        }
    }
}

// MARK: - Modern Duplicate Card

struct ModernDuplicateCard: View {
    let participant: ParticipantDuplicateInfo
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        ScrollView(showsIndicators: false) {
            VStack(spacing: 24) {
                // Modern Profile Section
                ModernProfileSection(participant: participant)
                
                // Modern Info Section
                ModernInfoSection(participant: participant)
                
                // Modern Documents Section
                if participant.hasDocuments {
                    ModernDocumentsSection(
                        participant: participant,
                        onDocumentTap: onDocumentTap
                    )
                }
            }
            .padding(24)
        }
        .background(
            RoundedRectangle(cornerRadius: 20)
                .fill(Color(.systemBackground))
                .shadow(color: .black.opacity(0.05), radius: 20, x: 0, y: 10)
                .overlay(
                    RoundedRectangle(cornerRadius: 20)
                        .stroke(Color(.systemGray6), lineWidth: 0.5)
                )
        )
    }
}

// MARK: - Modern Profile Section

struct ModernProfileSection: View {
    let participant: ParticipantDuplicateInfo
    
    var body: some View {
        VStack(spacing: 16) {
            // Modern profile photo with status indicator
            ZStack {
                // Photo background
                Circle()
                    .fill(
                        LinearGradient(
                            colors: [.blue.opacity(0.1), .purple.opacity(0.1)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .frame(width: 120, height: 120)
                    .overlay(
                        Circle()
                            .stroke(
                                LinearGradient(
                                    colors: [.blue.opacity(0.3), .purple.opacity(0.3)],
                                    startPoint: .topLeading,
                                    endPoint: .bottomTrailing
                                ),
                                lineWidth: 2
                            )
                    )
                
                if let photoURL = participant.profilePhotoUrl {
                    AsyncImage(url: createFileURL(from: photoURL)) { image in
                        image
                            .resizable()
                            .aspectRatio(contentMode: .fill)
                    } placeholder: {
                        ZStack {
                            Circle()
                                .fill(.ultraThinMaterial)
                            
                            ProgressView()
                                .progressViewStyle(CircularProgressViewStyle(tint: .blue))
                                .scaleEffect(0.8)
                        }
                    }
                    .frame(width: 116, height: 116)
                    .clipShape(Circle())
                } else {
                    VStack(spacing: 8) {
                        Image(systemName: "person.fill")
                            .font(.system(size: 36, weight: .light))
                            .foregroundStyle(.blue.opacity(0.6))
                        
                        Text("No Photo")
                            .font(.caption2)
                            .fontWeight(.medium)
                            .foregroundColor(.secondary)
                    }
                }
                
                // Document count badge
                if participant.hasDocuments {
                    VStack {
                        HStack {
                            Spacer()
                            ZStack {
                                Circle()
                                    .fill(.blue)
                                    .frame(width: 28, height: 28)
                                
                                Text("\(participant.totalDocumentCount)")
                                    .font(.caption2)
                                    .fontWeight(.bold)
                                    .foregroundColor(.white)
                            }
                        }
                        Spacer()
                    }
                    .frame(width: 120, height: 120)
                }
            }
            
            // Name and subtitle
            VStack(spacing: 6) {
                Text(participant.name)
                    .font(.title)
                    .fontWeight(.bold)
                    .multilineTextAlignment(.center)
                
                HStack(spacing: 8) {
                    if !participant.genderDisplayName.isEmpty && participant.genderDisplayName != "Not specified" {
                        ModernChip(text: participant.genderDisplayName, color: .blue)
                    }
                    
                    if !participant.ageGroupDisplayName.isEmpty && participant.ageGroupDisplayName != "Not specified" {
                        ModernChip(text: participant.ageGroupDisplayName, color: .green)
                    }
                }
            }
        }
    }
}

// MARK: - Modern Info Section

struct ModernInfoSection: View {
    let participant: ParticipantDuplicateInfo
    
    var body: some View {
        VStack(spacing: 16) {
            // Section Header
            HStack {
                Image(systemName: "person.text.rectangle")
                    .foregroundStyle(.blue)
                    .font(.title3)
                
                Text("Basic Information")
                    .font(.headline)
                    .fontWeight(.semibold)
                
                Spacer()
            }
            
            // Info Cards
            VStack(spacing: 12) {
                if let location = participant.location, !location.isEmpty {
                    ModernInfoCard(
                        icon: "location.fill",
                        label: "Location",
                        value: location,
                        color: .green
                    )
                }
                
                ModernInfoCard(
                    icon: "accessibility",
                    label: "Disability Status",
                    value: participant.disabilityDescription,
                    color: participant.disability ? .orange : .gray
                )
                
                HStack(spacing: 12) {
                    ModernDateCard(
                        label: "Created",
                        date: formatDate(participant.createdAt),
                        icon: "calendar.badge.plus",
                        color: .blue
                    )
                    
                    ModernDateCard(
                        label: "Updated",
                        date: formatDate(participant.updatedAt),
                        icon: "calendar.badge.clock",
                        color: .purple
                    )
                }
            }
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            displayFormatter.timeStyle = .none
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}

// MARK: - Modern Documents Section

struct ModernDocumentsSection: View {
    let participant: ParticipantDuplicateInfo
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        VStack(spacing: 16) {
            // Section Header
            HStack {
                Image(systemName: "doc.on.doc")
                    .foregroundStyle(.blue)
                    .font(.title3)
                
                Text("Documents")
                    .font(.headline)
                    .fontWeight(.semibold)
                
                Spacer()
                
                Text("\(participant.totalDocumentCount)")
                    .font(.caption)
                    .fontWeight(.bold)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(.blue.opacity(0.1))
                    .foregroundColor(.blue)
                    .clipShape(Capsule())
            }
            
            VStack(spacing: 12) {
                // Identification documents (priority)
                if !participant.identificationDocuments.isEmpty {
                    ModernDocumentGroup(
                        title: "ID Documents",
                        documents: participant.identificationDocuments,
                        color: .green,
                        icon: "person.text.rectangle.fill",
                        onDocumentTap: onDocumentTap
                    )
                }
                
                // Other documents
                if !participant.otherDocuments.isEmpty {
                    ModernDocumentGroup(
                        title: "Other Files",
                        documents: participant.otherDocuments,
                        color: .blue,
                        icon: "folder.fill",
                        onDocumentTap: onDocumentTap
                    )
                }
            }
        }
    }
}

// MARK: - Modern Document Group

struct ModernDocumentGroup: View {
    let title: String
    let documents: [DuplicateDocumentInfo]
    let color: Color
    let icon: String
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        VStack(spacing: 12) {
            HStack {
                Image(systemName: icon)
                    .foregroundStyle(color)
                    .font(.subheadline)
                
                Text(title)
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                Spacer()
                
                Text("\(documents.count)")
                    .font(.caption2)
                    .fontWeight(.bold)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(color.opacity(0.15))
                    .foregroundColor(color)
                    .clipShape(Capsule())
            }
            
            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
                GridItem(.flexible())
            ], spacing: 12) {
                ForEach(documents.prefix(6)) { document in
                    ModernDocumentThumbnail(document: document, onTap: onDocumentTap)
                }
                
                if documents.count > 6 {
                    VStack(spacing: 4) {
                        Image(systemName: "plus.circle.fill")
                            .font(.title2)
                            .foregroundStyle(color.opacity(0.6))
                        
                        Text("+\(documents.count - 6)")
                            .font(.caption2)
                            .fontWeight(.medium)
                            .foregroundColor(.secondary)
                    }
                    .frame(height: 70)
                    .frame(maxWidth: .infinity)
                    .background(color.opacity(0.05))
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(color.opacity(0.2), lineWidth: 1)
                    )
                }
            }
        }
        .padding(16)
        .background(color.opacity(0.03))
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(
            RoundedRectangle(cornerRadius: 16)
                .stroke(color.opacity(0.1), lineWidth: 1)
        )
    }
}

// MARK: - Modern Document Thumbnail

struct ModernDocumentThumbnail: View {
    let document: DuplicateDocumentInfo
    let onTap: (URL) -> Void
    
    var body: some View {
        Button {
            if let url = createFileURL(from: document.filePath) {
                onTap(url)
            }
        } label: {
            VStack(spacing: 6) {
                ZStack {
                    RoundedRectangle(cornerRadius: 8)
                        .fill(.blue.opacity(0.1))
                        .frame(width: 40, height: 40)
                    
                    Image(systemName: documentIcon)
                        .font(.title3)
                        .foregroundStyle(.blue)
                }
                
                Text(document.originalFilename)
                    .font(.caption2)
                    .fontWeight(.medium)
                    .lineLimit(2)
                    .multilineTextAlignment(.center)
                    .foregroundColor(.primary)
            }
            .frame(height: 70)
            .frame(maxWidth: .infinity)
            .background(.ultraThinMaterial)
            .clipShape(RoundedRectangle(cornerRadius: 12))
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(.blue.opacity(0.2), lineWidth: 0.5)
            )
        }
        .buttonStyle(.plain)
    }
    
    private var documentIcon: String {
        let filename = document.originalFilename.lowercased()
        
        if filename.contains("jpg") || filename.contains("jpeg") || filename.contains("png") || filename.contains("gif") || filename.contains("heic") {
            return "photo.fill"
        } else if filename.contains("pdf") {
            return "doc.richtext.fill"
        } else if filename.contains("doc") || filename.contains("docx") {
            return "doc.text.fill"
        } else if filename.contains("mp4") || filename.contains("mov") {
            return "video.fill"
        } else {
            return "doc.fill"
        }
    }
}

// MARK: - Modern Helper Components

struct ModernChip: View {
    let text: String
    let color: Color
    
    var body: some View {
        Text(text)
            .font(.caption)
            .fontWeight(.medium)
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(color.opacity(0.15))
            .foregroundColor(color)
            .clipShape(Capsule())
    }
}

struct ModernInfoCard: View {
    let icon: String
    let label: String
    let value: String
    let color: Color
    
    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 8)
                    .fill(color.opacity(0.15))
                    .frame(width: 36, height: 36)
                
                Image(systemName: icon)
                    .font(.subheadline)
                    .foregroundStyle(color)
            }
            
            VStack(alignment: .leading, spacing: 2) {
                Text(label)
                    .font(.caption)
                    .foregroundColor(.secondary)
                
                Text(value)
                    .font(.subheadline)
                    .fontWeight(.medium)
            }
            
            Spacer()
        }
        .padding(12)
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

struct ModernDateCard: View {
    let label: String
    let date: String
    let icon: String
    let color: Color
    
    var body: some View {
        VStack(spacing: 8) {
            ZStack {
                Circle()
                    .fill(color.opacity(0.15))
                    .frame(width: 32, height: 32)
                
                Image(systemName: icon)
                    .font(.caption)
                    .foregroundStyle(color)
            }
            
            VStack(spacing: 2) {
                Text(label)
                    .font(.caption2)
                    .foregroundColor(.secondary)
                
                Text(date)
                    .font(.caption2)
                    .fontWeight(.medium)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 12)
        .background(color.opacity(0.05))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }
}

// MARK: - Helper Functions

/// Convert relative file path to proper file URL for local storage
private func createFileURL(from relativePath: String) -> URL? {
    // Get the documents directory path
    let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
    let storagePath = "\(documentsPath)/ActionAid/storage"
    
    // Create full file path
    let fullPath = "\(storagePath)/\(relativePath)"
    
    // Check if file exists before creating URL
    if FileManager.default.fileExists(atPath: fullPath) {
        print("📸 [DUPLICATE_DETECTION] Loading image from: \(fullPath)")
        return URL(fileURLWithPath: fullPath)
    } else {
        print("⚠️ [DUPLICATE_DETECTION] Image file not found: \(fullPath)")
        return nil
    }
} 