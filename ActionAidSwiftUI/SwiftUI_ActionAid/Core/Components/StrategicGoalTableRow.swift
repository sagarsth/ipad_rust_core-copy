import SwiftUI

struct StrategicGoalTableRow: View {
    let goal: StrategicGoalResponse
    let columns: [TableColumn]
    let documentCounts: [String: Int]
    
    private var progress: Double {
        goal.progressPercentage ?? 0.0
    }
    
    private var statusInfo: (text: String, color: Color) {
        switch goal.statusId {
        case 1: return ("On Track", .green)
        case 2: return ("At Risk", .orange)
        case 3: return ("Behind", .red)
        case 4: return ("Completed", .blue)
        default: return ("Unknown", .gray)
        }
    }
    
    var body: some View {
        HStack(spacing: 0) {
            ForEach(columns, id: \.key) { column in
                cellContent(for: column)
                    .frame(maxWidth: column.width ?? .infinity, alignment: Alignment(horizontal: column.alignment, vertical: .center))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 12)
                
                if column.key != columns.last?.key {
                    Divider()
                        .frame(height: 30)
                }
            }
        }
        .background(Color(.systemBackground))
    }
    
    @ViewBuilder
    private func cellContent(for column: TableColumn) -> some View {
        switch column.key {
        case "code", "objective_code":
            HStack(spacing: 4) {
                Text(goal.objectiveCode)
                    .font(.caption)
                    .fontWeight(.medium)
                    .lineLimit(1)
                    .foregroundColor(.primary)
                
                if goal.hasDocumentsTracked(in: documentCounts) {
                    Image(systemName: "paperclip")
                        .font(.caption2)
                        .foregroundColor(.blue)
                }
            }
                
        case "outcome":
            Text(goal.outcome ?? "N/A")
                .font(.caption2)
                .lineLimit(2)
                .foregroundColor(.primary)
                
        case "status":
            Badge(text: statusInfo.text, color: statusInfo.color)
                .font(.caption2)
                
        case "progress":
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text("\(Int(progress))%")
                        .font(.caption2)
                        .fontWeight(.medium)
                        .foregroundColor(progress > 100 ? .purple : .primary)
                    Spacer()
                }
                
                GeometryReader { geometry in
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 2)
                            .fill(Color(.systemGray5))
                            .frame(height: 4)
                        
                        RoundedRectangle(cornerRadius: 2)
                            .fill(progress > 100 ? .purple : statusInfo.color)
                            .frame(width: geometry.size.width * min(progress / 100, 1.0), height: 4)
                    }
                }
                .frame(height: 4)
            }
            
        case "team", "responsible_team":
            Text(goal.responsibleTeam ?? "N/A")
                .font(.caption2)
                .lineLimit(1)
                .foregroundColor(.secondary)
                
        case "kpi":
            Text(goal.kpi ?? "N/A")
                .font(.caption2)
                .lineLimit(1)
                .foregroundColor(.secondary)
                
        case "values", "target_value":
            Text("\(Int(goal.targetValue ?? 0))")
                .font(.caption2)
                .fontWeight(.medium)
                
        case "actual_value":
            Text("\(Int(goal.actualValue ?? 0))")
                .font(.caption2)
                .fontWeight(.medium)
                
        case "values_combined":
            VStack(alignment: .leading, spacing: 1) {
                HStack {
                    Text("T:")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    Text("\(Int(goal.targetValue ?? 0))")
                        .font(.caption2)
                        .fontWeight(.medium)
                }
                HStack {
                    Text("A:")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    Text("\(Int(goal.actualValue ?? 0))")
                        .font(.caption2)
                        .fontWeight(.medium)
                }
            }
            
        case "priority":
            if goal.syncPriority == .high {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.caption)
                    .foregroundColor(.red)
            } else {
                Image(systemName: "minus.circle")
                    .font(.caption)
                    .foregroundColor(.gray)
            }
            
        case "updated", "updated_at":
            Text(formatDate(goal.updatedAt))
                .font(.caption2)
                .foregroundColor(.secondary)
                .lineLimit(1)
                
        default:
            Text("N/A")
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        
        if let date = formatter.date(from: dateString) {
            let displayFormatter = DateFormatter()
            displayFormatter.dateFormat = "MMM d"
            return displayFormatter.string(from: date)
        }
        return ""
    }
}

// MARK: - Strategic Goal Table Columns Configuration
extension StrategicGoalsView {
    static var tableColumns: [TableColumn] {
        [
            TableColumn(
                key: "objective_code",
                title: "Code",
                width: 100,
                alignment: .leading,
                isRequired: true
            ),
            TableColumn(
                key: "outcome",
                title: "Outcome",
                alignment: .leading,
                isRequired: true
            ),
            TableColumn(
                key: "status",
                title: "Status",
                width: 100,
                alignment: .center
            ),
            TableColumn(
                key: "progress",
                title: "Progress",
                width: 120,
                alignment: .center
            ),
            TableColumn(
                key: "responsible_team",
                title: "Team",
                width: 140,
                alignment: .leading,
                isVisible: { $0.userInterfaceIdiom == .pad }
            ),
            TableColumn(
                key: "target_value",
                title: "Target",
                width: 100,
                alignment: .trailing,
                isVisible: { $0.userInterfaceIdiom == .pad }
            ),
            TableColumn(
                key: "actual_value",
                title: "Actual",
                width: 100,
                alignment: .trailing,
                isVisible: { $0.userInterfaceIdiom == .pad }
            ),
            TableColumn(
                key: "updated_at",
                title: "Updated",
                width: 120,
                alignment: .center,
                isVisible: { $0.userInterfaceIdiom == .pad }
            )
        ]
    }
} 