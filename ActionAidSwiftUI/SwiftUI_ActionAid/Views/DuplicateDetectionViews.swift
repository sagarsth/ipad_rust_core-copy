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
        NavigationView {
            VStack(spacing: 0) {
                // Header
                VStack(spacing: 8) {
                    HStack {
                        Image(systemName: "person.2.fill")
                            .foregroundColor(.orange)
                            .font(.title2)
                        
                        Text("Potential Duplicates Found")
                            .font(.headline)
                            .fontWeight(.semibold)
                        
                        Spacer()
                    }
                    
                    Text("Found \(duplicates.count) participant\(duplicates.count == 1 ? "" : "s") with the same name. Please review to avoid duplicates.")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding()
                .background(Color(.systemGroupedBackground))
                
                // Page indicator
                if duplicates.count > 1 {
                    HStack {
                        ForEach(0..<duplicates.count, id: \.self) { index in
                            Circle()
                                .fill(index == currentIndex ? Color.accentColor : Color.gray.opacity(0.3))
                                .frame(width: 8, height: 8)
                        }
                    }
                    .padding(.vertical, 8)
                    .background(Color(.systemGroupedBackground))
                }
                
                // Slidable cards
                TabView(selection: $currentIndex) {
                    ForEach(Array(duplicates.enumerated()), id: \.element.id) { index, duplicate in
                        DuplicateParticipantCard(
                            participant: duplicate,
                            onDocumentTap: { url in
                                selectedDocumentURL = IdentifiableURL(url: url)
                            }
                        )
                        .tag(index)
                        .padding()
                    }
                }
                .tabViewStyle(PageTabViewStyle(indexDisplayMode: .never))
                .background(Color(.systemBackground))
                
                // Action buttons
                HStack(spacing: 16) {
                    Button("Cancel Creation") {
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        onCancel()
                        dismiss()
                    }
                    .buttonStyle(.bordered)
                    .foregroundColor(.red)
                    
                    Button("Create Anyway") {
                        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                        onContinue()
                        dismiss()
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding()
                .background(Color(.systemGroupedBackground))
            }
            .navigationBarHidden(true)
        }
        .sheet(item: $selectedDocumentURL) { identifiableURL in
            QuickLookView(url: identifiableURL.url)
        }
    }
}

// MARK: - Individual Duplicate Card

struct DuplicateParticipantCard: View {
    let participant: ParticipantDuplicateInfo
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                // Profile section with photo
                ProfilePhotoSection(participant: participant)
                
                // Basic info section
                BasicInfoSection(participant: participant)
                
                // Documents section
                DocumentsSection(
                    participant: participant,
                    onDocumentTap: onDocumentTap
                )
                
                // Activity summary
                ActivitySummarySection(participant: participant)
            }
            .padding()
        }
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color(.secondarySystemGroupedBackground))
                .shadow(color: .black.opacity(0.1), radius: 8, x: 0, y: 4)
        )
    }
}

// MARK: - Profile Photo Section

struct ProfilePhotoSection: View {
    let participant: ParticipantDuplicateInfo
    
    var body: some View {
        VStack(spacing: 12) {
            // Profile photo or placeholder
            ZStack {
                Circle()
                    .fill(LinearGradient(
                        colors: [.blue.opacity(0.3), .purple.opacity(0.3)],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    ))
                    .frame(width: 100, height: 100)
                
                if let photoURL = participant.profilePhotoUrl {
                    AsyncImage(url: URL(string: photoURL)) { image in
                        image
                            .resizable()
                            .aspectRatio(contentMode: .fill)
                    } placeholder: {
                        ProgressView()
                            .progressViewStyle(CircularProgressViewStyle(tint: .white))
                    }
                    .frame(width: 100, height: 100)
                    .clipShape(Circle())
                } else {
                    VStack {
                        Image(systemName: "person.fill")
                            .font(.system(size: 40))
                            .foregroundColor(.white)
                        
                        Text("No Photo")
                            .font(.caption2)
                            .foregroundColor(.white)
                    }
                }
            }
            
            // Name and basic details
            VStack(spacing: 4) {
                Text(participant.name)
                    .font(.title2)
                    .fontWeight(.semibold)
                
                HStack(spacing: 16) {
                    if !participant.genderDisplayName.isEmpty && participant.genderDisplayName != "Not specified" {
                        Label(participant.genderDisplayName, systemImage: "person")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    
                    if !participant.ageGroupDisplayName.isEmpty && participant.ageGroupDisplayName != "Not specified" {
                        Label(participant.ageGroupDisplayName, systemImage: "calendar")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
            }
        }
    }
}

// MARK: - Basic Info Section

struct BasicInfoSection: View {
    let participant: ParticipantDuplicateInfo
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionHeader(title: "Basic Information", icon: "info.circle")
            
            VStack(spacing: 6) {
                if let location = participant.location, !location.isEmpty {
                    InfoRow(label: "Location", value: location, icon: "location")
                }
                
                InfoRow(label: "Disability", value: participant.disabilityDescription, icon: "accessibility")
                
                InfoRow(label: "Created", value: formatDate(participant.createdAt), icon: "calendar.badge.plus")
                
                InfoRow(label: "Last Updated", value: formatDate(participant.updatedAt), icon: "calendar.badge.clock")
            }
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .medium
            displayFormatter.timeStyle = .short
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}

// MARK: - Documents Section

struct DocumentsSection: View {
    let participant: ParticipantDuplicateInfo
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionHeader(title: "Documents (\(participant.totalDocumentCount))", icon: "doc.fill")
            
            if participant.hasDocuments {
                VStack(spacing: 12) {
                    // Identification documents (priority)
                    if !participant.identificationDocuments.isEmpty {
                        DocumentGroup(
                            title: "Identification Documents",
                            documents: participant.identificationDocuments,
                            color: .green,
                            icon: "person.text.rectangle",
                            onDocumentTap: onDocumentTap
                        )
                    }
                    
                    // Other documents
                    if !participant.otherDocuments.isEmpty {
                        DocumentGroup(
                            title: "Other Documents",
                            documents: participant.otherDocuments,
                            color: .blue,
                            icon: "doc.richtext",
                            onDocumentTap: onDocumentTap
                        )
                    }
                }
            } else {
                Text("No documents attached")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .italic()
            }
        }
    }
}

// MARK: - Document Group

struct DocumentGroup: View {
    let title: String
    let documents: [DuplicateDocumentInfo]
    let color: Color
    let icon: String
    let onDocumentTap: (URL) -> Void
    
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(color)
                    .font(.caption)
                
                Text(title)
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundColor(color)
                
                Spacer()
                
                Text("\(documents.count)")
                    .font(.caption2)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(color.opacity(0.2))
                    .foregroundColor(color)
                    .clipShape(Capsule())
            }
            
            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible())
            ], spacing: 8) {
                ForEach(documents.prefix(4)) { document in
                    DocumentThumbnail(document: document, onTap: onDocumentTap)
                }
                
                if documents.count > 4 {
                    VStack {
                        Image(systemName: "ellipsis")
                            .font(.title2)
                            .foregroundColor(.secondary)
                        
                        Text("+\(documents.count - 4) more")
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                    .frame(height: 60)
                    .frame(maxWidth: .infinity)
                    .background(Color(.tertiarySystemFill))
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
            }
        }
        .padding(.vertical, 8)
    }
}

// MARK: - Document Thumbnail

struct DocumentThumbnail: View {
    let document: DuplicateDocumentInfo
    let onTap: (URL) -> Void
    
    var body: some View {
        Button {
            if let url = URL(string: document.filePath) {
                onTap(url)
            }
        } label: {
            VStack(spacing: 4) {
                Image(systemName: documentIcon)
                    .font(.title2)
                    .foregroundColor(.primary)
                
                Text(document.originalFilename)
                    .font(.caption2)
                    .lineLimit(2)
                    .multilineTextAlignment(.center)
                    .foregroundColor(.primary)
            }
            .frame(height: 60)
            .frame(maxWidth: .infinity)
            .background(Color(.secondarySystemFill))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
    }
    
    private var documentIcon: String {
        let filename = document.originalFilename.lowercased()
        
        if filename.contains("jpg") || filename.contains("jpeg") || filename.contains("png") || filename.contains("gif") {
            return "photo"
        } else if filename.contains("pdf") {
            return "doc.richtext"
        } else if filename.contains("doc") || filename.contains("docx") {
            return "doc.text"
        } else {
            return "doc"
        }
    }
}

// MARK: - Activity Summary Section

struct ActivitySummarySection: View {
    let participant: ParticipantDuplicateInfo
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionHeader(title: "Activity Summary", icon: "chart.bar")
            
            HStack(spacing: 16) {
                ActivityMetric(
                    title: "Workshops",
                    value: "\(participant.workshopCount)",
                    icon: "person.3",
                    color: .blue
                )
                
                ActivityMetric(
                    title: "Livelihoods",
                    value: "\(participant.livelihoodCount)",
                    icon: "briefcase",
                    color: .green
                )
                
                ActivityMetric(
                    title: "Documents",
                    value: "\(participant.totalDocumentCount)",
                    icon: "doc.stack",
                    color: .orange
                )
            }
        }
    }
}

// MARK: - Helper Views

struct InfoRow: View {
    let label: String
    let value: String
    let icon: String
    
    var body: some View {
        HStack {
            Image(systemName: icon)
                .foregroundColor(.secondary)
                .font(.caption)
                .frame(width: 16)
            
            Text(label)
                .font(.caption)
                .foregroundColor(.secondary)
                .frame(width: 80, alignment: .leading)
            
            Text(value)
                .font(.caption)
                .fontWeight(.medium)
            
            Spacer()
        }
    }
}

struct ActivityMetric: View {
    let title: String
    let value: String
    let icon: String
    let color: Color
    
    var body: some View {
        VStack(spacing: 4) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(color)
            
            Text(value)
                .font(.headline)
                .fontWeight(.bold)
            
            Text(title)
                .font(.caption2)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(color.opacity(0.1))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
} 