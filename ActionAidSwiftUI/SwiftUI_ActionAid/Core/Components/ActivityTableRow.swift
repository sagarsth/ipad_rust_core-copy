//
//  ActivityTableRow.swift
//  SwiftUI_ActionAid
//
//  Table row component for activities in table view mode
//

import SwiftUI

struct ActivityTableRow: View {
    let activity: ActivityResponse
    let columns: [TableColumn]
    
    private var statusInfo: (text: String, color: Color) {
        switch activity.statusId {
        case 1: return ("Completed", .green)
        case 2: return ("In Progress", .blue)
        case 3: return ("Pending", .orange)
        case 4: return ("Blocked", .red)
        default: return ("Unknown", .gray)
        }
    }
    
    private var progressColor: Color {
        guard let progress = activity.progressPercentage else { return .gray }
        if progress >= 80 { return .green }
        else if progress >= 50 { return .blue }
        else if progress > 0 { return .orange }
        else { return .red }
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
        case "description":
            VStack(alignment: .leading, spacing: 2) {
                Text(activity.description ?? "Untitled Activity")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                
                if let kpi = activity.kpi, !kpi.isEmpty {
                    Text(kpi)
                        .font(.system(size: 12))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            
        case "kpi":
            if let kpi = activity.kpi {
                Text(kpi)
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .lineLimit(1)
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "progress":
            VStack(spacing: 4) {
                if let progress = activity.progressPercentage {
                    // Ensure progress is a valid number and clamp it properly
                    let safeProgress = progress.isNaN || progress.isInfinite ? 0.0 : progress
                    let normalizedProgress = max(0.0, min(safeProgress / 100.0, 1.0))
                    
                    ProgressView(value: normalizedProgress, total: 1.0)
                        .progressViewStyle(LinearProgressViewStyle())
                        .tint(progressColor)
                        .frame(height: 6)
                    
                    Text(String(format: "%.0f%%", max(0, min(safeProgress, 100))))
                        .font(.system(size: 11))
                        .foregroundColor(progressColor)
                        .fontWeight(.medium)
                } else {
                    Text("—")
                        .font(.system(size: 13))
                        .foregroundColor(.secondary)
                }
            }
            
        case "target":
            if let target = activity.targetValue {
                Text(formatNumber(target))
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .fontWeight(.medium)
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "actual":
            if let actual = activity.actualValue {
                HStack(spacing: 2) {
                    Text(formatNumber(actual))
                        .font(.system(size: 13))
                        .foregroundColor(.primary)
                        .fontWeight(.medium)
                    
                    // Show trend indicator if we have both actual and target
                    if let target = activity.targetValue, target > 0 {
                        let percentage = (actual / target) * 100
                        Image(systemName: percentage >= 100 ? "checkmark.circle.fill" : "circle.dashed")
                            .font(.system(size: 11))
                            .foregroundColor(percentage >= 100 ? .green : .secondary)
                    }
                }
            } else {
                Text("—")
                    .font(.system(size: 13))
                    .foregroundColor(.secondary)
            }
            
        case "status":
            Badge(text: statusInfo.text, color: statusInfo.color)
            
        case "project":
            if let projectName = activity.projectName {
                HStack(spacing: 4) {
                    Image(systemName: "folder")
                        .font(.system(size: 11))
                        .foregroundColor(.blue)
                    Text(projectName)
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
                
                Text("\(activity.documentCount ?? 0)")
                    .font(.system(size: 13))
                    .foregroundColor(.primary)
                    .fontWeight(.medium)
            }
            
        case "updated_at":
            Text(formatDate(activity.updatedAt))
                .font(.system(size: 13))
                .foregroundColor(.secondary)
                .lineLimit(1)
            
        default:
            Text("—")
                .font(.system(size: 13))
                .foregroundColor(.secondary)
        }
    }
    
    private func formatNumber(_ value: Double) -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        formatter.maximumFractionDigits = value.truncatingRemainder(dividingBy: 1) == 0 ? 0 : 2
        formatter.minimumFractionDigits = 0
        return formatter.string(from: NSNumber(value: value)) ?? String(value)
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