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
    private func cellContent(for column: TableColumn) -> some View {
        switch column.key {
        case "code", "objective_code":
            HStack(spacing: 4) {
                Text(goal.objectiveCode)
                    .font(.caption)
                    .fontWeight(.medium)
                    .lineLimit(1)
                    .foregroundColor(.primary)
                
                if (documentCounts[goal.id] ?? 0) > 0 {
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

// MARK: - Strategic Goal Table Configuration (Shared Architecture Pattern)
struct StrategicGoalTableConfig {
    static let columns: [TableColumn] = [
        TableColumn(
            key: "objective_code",
            title: "Code",
            width: nil, // Remove fixed width to allow expansion
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
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "target_value",
            title: "Target",
            width: 100,
            alignment: .trailing,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "actual_value",
            title: "Actual",
            width: 100,
            alignment: .trailing,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        ),
        TableColumn(
            key: "updated_at",
            title: "Updated",
            width: 120,
            alignment: .center,
            isVisible: { _ in true } // Available on all devices - orientation logic controls visibility
        )
    ]
    
    /// Returns columns with dynamic widths based on which columns are hidden
    /// Applies 3:7 ratio for code:outcome when only those two columns are visible
    static func columns(hiddenColumns: Set<String>) -> [TableColumn] {
        // Filter to get visible columns
        let visibleColumns = columns.filter { column in
            // Always show required columns
            if column.isRequired {
                return column.isVisible(UIDevice.current)
            }
            
            // Hide columns that user has hidden
            if hiddenColumns.contains(column.key) {
                return false
            }
            
            // Apply device-specific visibility
            return column.isVisible(UIDevice.current)
        }
        
        // Check if only code and outcome columns are visible
        let visibleKeys = Set(visibleColumns.map(\.key))
        let isOnlyCodeAndOutcome = visibleKeys == Set(["objective_code", "outcome"])
        
        if isOnlyCodeAndOutcome {
            // Apply 3:7 ratio for code:outcome
            let screenWidth = UIScreen.main.bounds.width - 32 // Account for padding
            let codeWidth = screenWidth * 0.3  // 30% for code
            let outcomeWidth = screenWidth * 0.7  // 70% for outcome
            
            return columns.map { column in
                switch column.key {
                case "objective_code":
                    return TableColumn(
                        key: column.key,
                        title: column.title,
                        width: codeWidth,
                        alignment: column.alignment,
                        isVisible: column.isVisible,
                        isCustomizable: column.isCustomizable,
                        isRequired: column.isRequired
                    )
                case "outcome":
                    return TableColumn(
                        key: column.key,
                        title: column.title,
                        width: outcomeWidth,
                        alignment: column.alignment,
                        isVisible: column.isVisible,
                        isCustomizable: column.isCustomizable,
                        isRequired: column.isRequired
                    )
                default:
                    return column
                }
            }
        }
        
        // Return default columns for all other cases
        return columns
    }
}

// MARK: - Strategic Goal Table Columns Configuration (Extension for compatibility)
extension StrategicGoalsView {
    static var tableColumns: [TableColumn] {
        return StrategicGoalTableConfig.columns
    }
}