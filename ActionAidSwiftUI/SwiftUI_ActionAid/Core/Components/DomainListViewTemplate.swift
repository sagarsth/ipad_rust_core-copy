import SwiftUI

// MARK: - Example Template for Other Domains
// This shows how to integrate AdaptiveListView into other domain views

/*
// MARK: - Example: Livelihood Items (assuming you have a LivelihoodResponse model)

// 1. Make your model conform to MonthGroupable & Equatable
extension LivelihoodResponse: MonthGroupable, Equatable {
    // Already has createdAt and updatedAt strings
    // Equatable conformance is automatic if all properties are Equatable
    // No additional implementation needed
}

// 2. Create table columns configuration
extension LivelihoodView {
    static var tableColumns: [TableColumn] {
        [
            TableColumn(
                key: "name",
                title: "Name",
                alignment: .leading,
                isRequired: true // Always visible
            ),
            TableColumn(
                key: "type",
                title: "Type",
                width: 100,
                alignment: .leading,
                isCustomizable: true
            ),
            TableColumn(
                key: "status",
                title: "Status",
                width: 80,
                alignment: .center,
                isRequired: true // Always visible
            ),
            TableColumn(
                key: "beneficiaries",
                title: "Beneficiaries",
                width: 90,
                alignment: .trailing,
                isVisible: { $0.userInterfaceIdiom == .pad },
                isCustomizable: true
            ),
            TableColumn(
                key: "location",
                title: "Location",
                width: 120,
                alignment: .leading,
                isVisible: { $0.userInterfaceIdiom == .pad },
                isCustomizable: true
            ),
            TableColumn(
                key: "updated",
                title: "Updated",
                width: 70,
                alignment: .leading,
                isCustomizable: true
            )
        ]
    }
}

// 3. Create a table row component
struct LivelihoodTableRow: View {
    let item: LivelihoodResponse
    let columns: [TableColumn]
    
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
        case "name":
            Text(item.name ?? "N/A")
                .font(.caption)
                .fontWeight(.medium)
                .lineLimit(1)
                
        case "type":
            Text(item.livelihoodType ?? "N/A")
                .font(.caption2)
                .foregroundColor(.secondary)
                
        case "status":
            // Your status badge logic here
            Text(item.status ?? "N/A")
                .font(.caption2)
                
        case "beneficiaries":
            Text("\(item.beneficiaryCount ?? 0)")
                .font(.caption2)
                .fontWeight(.medium)
                
        case "location":
            Text(item.location ?? "N/A")
                .font(.caption2)
                .foregroundColor(.secondary)
                
        case "updated":
            Text(formatDate(item.updatedAt))
                .font(.caption2)
                .foregroundColor(.secondary)
                
        default:
            Text("N/A")
                .font(.caption2)
                .foregroundColor(.secondary)
        }
    }
    
    private func formatDate(_ dateString: String) -> String {
        // Your date formatting logic
        return ""
    }
}

// 4. In your main view, replace the list section with:
struct LivelihoodView: View {
    @StateObject private var viewStyleManager = ViewStylePreferenceManager()
    @State private var currentViewStyle: ListViewStyle = .cards
    @State private var items: [LivelihoodResponse] = []
    // ... other properties
    
    var body: some View {
        VStack {
            // ... your existing search/filter UI
            
            // Replace your existing list with:
            AdaptiveListView(
                items: filteredItems,
                viewStyle: currentViewStyle,
                onViewStyleChange: { newStyle in
                    currentViewStyle = newStyle
                    viewStyleManager.setViewStyle(newStyle, for: "livelihood")
                },
                onItemTap: { item in
                    selectedItem = item
                },
                cardContent: { item in
                    LivelihoodCard(item: item) { }
                },
                tableColumns: Self.tableColumns,
                rowContent: { item, columns in
                    LivelihoodTableRow(item: item, columns: columns)
                },
                domainName: "livelihood", // Important: unique name for each domain
                userRole: authManager.currentUser?.role // Pass user role for action buttons
            )
        }
        .onAppear {
            currentViewStyle = viewStyleManager.getViewStyle(for: "livelihood")
            // ... load data
        }
    }
}

// MARK: - Domain-Specific Sorting Extensions
// You can add domain-specific sorting methods as extensions

extension LivelihoodView {
    enum LivelihoodSortOption: String, CaseIterable {
        case nameAsc = "name_asc"
        case nameDesc = "name_desc"
        case beneficiariesDesc = "beneficiaries_desc"
        case locationAsc = "location_asc"
        case recentlyUpdated = "recently_updated"
        
        var displayName: String {
            switch self {
            case .nameAsc: return "Name A-Z"
            case .nameDesc: return "Name Z-A"
            case .beneficiariesDesc: return "Most Beneficiaries"
            case .locationAsc: return "Location A-Z"
            case .recentlyUpdated: return "Recently Updated"
            }
        }
    }
    
    func sortedItems(by option: LivelihoodSortOption) -> [LivelihoodResponse] {
        switch option {
        case .nameAsc:
            return items.sorted { ($0.name ?? "") < ($1.name ?? "") }
        case .nameDesc:
            return items.sorted { ($0.name ?? "") > ($1.name ?? "") }
        case .beneficiariesDesc:
            return items.sorted { ($0.beneficiaryCount ?? 0) > ($1.beneficiaryCount ?? 0) }
        case .locationAsc:
            return items.sorted { ($0.location ?? "") < ($1.location ?? "") }
        case .recentlyUpdated:
            return items.sorted { item1, item2 in
                // Sort by updatedAt date
                let formatter = ISO8601DateFormatter()
                let date1 = formatter.date(from: item1.updatedAt) ?? Date.distantPast
                let date2 = formatter.date(from: item2.updatedAt) ?? Date.distantPast
                return date1 > date2
            }
        }
    }
}

*/

// MARK: - Usage Instructions
/*
To use this adaptive list view in any domain:

1. Make your response model conform to MonthGroupable & Equatable protocols
2. Create table columns configuration as a static property
   - Set isRequired: true for columns that should always be visible
   - Set isCustomizable: false for columns that users shouldn't be able to hide
   - Use isVisible to control device-specific visibility (iPad vs iPhone)
3. Create a table row component for your domain
4. Replace your existing list with AdaptiveListView
   - Include domainName parameter with a unique identifier for your domain
5. Add domain-specific sorting options as needed

Features:
- View style preference automatically saved and restored per domain
- List grouped by month with newest items first
- Table view shows more columns on iPad, fewer on iPhone
- Long press on table header to customize visible columns
- Column preferences saved per domain
- Required columns always visible, customizable columns can be toggled

Note: Equatable conformance is usually automatic if all properties are Equatable.

Column Customization:
- Long press on any table header to open column customizer
- Toggle visibility of customizable columns
- Required columns cannot be hidden
- Preferences saved separately for each domain
- Device-specific columns (iPad-only) clearly indicated
*/ 