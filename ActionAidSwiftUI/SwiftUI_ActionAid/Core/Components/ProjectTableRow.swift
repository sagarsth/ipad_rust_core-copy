//
//  ProjectTableRow.swift
//  SwiftUI_ActionAid
//
//  Table row component for projects in table view mode
//

import SwiftUI

struct ProjectTableRow: View {
    let project: ProjectResponse
    let columns: [TableColumn]
    
    private var statusInfo: (text: String, color: Color) {
        switch project.statusId {
        case 1: return ("On Track", .green)
        case 2: return ("At Risk", .orange)
        case 3: return ("Delayed", .red)
        case 4: return ("Completed", .blue)
        default: return ("Unknown", .gray)
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
                Text(project.name)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                
                if let objective = project.objective, !objective.isEmpty {
                    Text(objective)
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            
        case "status":
            Badge(text: statusInfo.text, color: statusInfo.color)
            
        case "responsible_team":
            if let team = project.responsibleTeam {
                HStack(spacing: 4) {
                    Image(systemName: "person.2")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Text(team)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "strategic_goal":
            if let strategicGoalName = project.effectiveStrategicGoalName {
                HStack(spacing: 4) {
                    Image(systemName: "flag")
                        .font(.system(size: 11))
                        .foregroundColor(.blue)
                    Text(strategicGoalName)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "timeline":
            if let timeline = project.timeline {
                HStack(spacing: 4) {
                    Image(systemName: "calendar")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Text(timeline)
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "documents":
            HStack(spacing: 4) {
                Image(systemName: "doc")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                
                Text("\(project.documentCount ?? 0)")
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .fontWeight(.medium)
            }
            
        case "updated_at":
            Text(formatDate(project.updatedAt))
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