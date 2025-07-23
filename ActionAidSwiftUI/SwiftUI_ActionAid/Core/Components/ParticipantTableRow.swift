//
//  ParticipantTableRow.swift
//  SwiftUI_ActionAid
//
//  Table row component for participants in table view mode
//

import SwiftUI

struct ParticipantTableRow: View {
    let participant: ParticipantResponse
    let columns: [TableColumn]
    let documentCounts: [String: Int]
    
    private var genderIcon: String {
        switch participant.parsedGender {
        case .male: return "person.fill"
        case .female: return "person.fill"
        case .other: return "person.2.fill"
        case .preferNotToSay, .none: return "person.fill"
        }
    }
    
    private var genderColor: Color {
        switch participant.parsedGender {
        case .male: return .blue
        case .female: return .pink
        case .other: return .purple
        case .preferNotToSay, .none: return .gray
        }
    }
    
    private var ageGroupIcon: String {
        switch participant.parsedAgeGroup {
        case .child: return "figure.child"
        case .youth: return "figure.walk"
        case .adult: return "figure.stand"
        case .elderly: return "figure.walk.motion"
        case .none: return "person"
        }
    }
    
    var body: some View {
        HStack(spacing: 0) {
            ForEach(columns, id: \.key) { column in
                columnContent(for: column)
                    .frame(maxWidth: column.width ?? .infinity, alignment: Alignment(horizontal: column.alignment, vertical: .center))
                    .padding(.horizontal, 12) // Increased from 8 to 12 for better margins
                    .padding(.vertical, 16)   // Increased from 12 to 16 for better vertical spacing
                
                if column.key != columns.last?.key {
                    Divider()
                        .frame(height: 30)
                }
            }
        }
        .background(Color(.systemBackground))
    }
    
    @ViewBuilder
    private func columnContent(for column: TableColumn) -> some View {
        switch column.key {
        case "name":
            VStack(alignment: .leading, spacing: 2) {
                Text(participant.name)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                
                if let location = participant.location, !location.isEmpty {
                    HStack(spacing: 4) {
                        Image(systemName: "location")
                            .font(.system(size: 10))
                        Text(location)
                            .font(.system(size: 12))
                    }
                    .foregroundColor(.secondary)
                    .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            
        case "gender":
            if let _ = participant.parsedGender {
                HStack(spacing: 4) {
                    Image(systemName: genderIcon)
                        .font(.system(size: 12))
                        .foregroundColor(genderColor)
                    Text(participant.genderDisplayName)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "age_group":
            if let _ = participant.parsedAgeGroup {
                HStack(spacing: 4) {
                    Image(systemName: ageGroupIcon)
                        .font(.system(size: 12))
                        .foregroundColor(.blue)
                    Text(participant.ageGroupDisplayName)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "location":
            if let location = participant.location {
                HStack(spacing: 4) {
                    Image(systemName: "map")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Text(location)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "disability":
            VStack(spacing: 2) {
                HStack(spacing: 4) {
                    Image(systemName: participant.disability ? "checkmark.circle.fill" : "xmark.circle")
                        .font(.system(size: 12))
                        .foregroundColor(participant.disability ? .green : .secondary)
                    
                    Text(participant.disability ? "Yes" : "No")
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .fontWeight(participant.disability ? .medium : .regular)
                }
                
                if participant.disability, let disabilityType = participant.disabilityType {
                    Text(disabilityType.capitalized)
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
            }
            
        case "workshops":
            HStack(spacing: 4) {
                Image(systemName: "person.3")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                
                VStack(alignment: .leading, spacing: 1) {
                    Text("\(participant.workshopCount ?? 0)")
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .fontWeight(.medium)
                    
                    if let completed = participant.completedWorkshopCount,
                       let total = participant.workshopCount,
                       total > 0 {
                        Text("\(completed) done")
                            .font(.system(size: 10))
                            .foregroundColor(.secondary)
                    }
                }
            }
            
        case "documents":
            HStack(spacing: 4) {
                Image(systemName: "doc")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                
                Text("\(documentCounts[participant.id] ?? 0)")
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .fontWeight(.medium)
            }
            
        case "updated_at":
            Text(formatDate(participant.updatedAt))
                .font(.system(size: 13))
                .foregroundColor(.secondary)
                .lineLimit(1)
            
        default:
            Text("—")
                .font(.system(size: 13))
                .foregroundColor(.secondary)
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateStyle = .short
            displayFormatter.timeStyle = .none
            return displayFormatter.string(from: date)
        }
        return dateString
    }
}