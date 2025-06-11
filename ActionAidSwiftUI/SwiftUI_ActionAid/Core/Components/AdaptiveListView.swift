import SwiftUI
import Foundation

// MARK: - List View Style
enum ListViewStyle: String, CaseIterable {
    case cards = "cards"
    case table = "table"
    
    var icon: String {
        switch self {
        case .cards: return "rectangle.grid.1x2"
        case .table: return "list.bullet"
        }
    }
    
    var displayName: String {
        switch self {
        case .cards: return "Cards"
        case .table: return "Table"
        }
    }
}

// MARK: - Groupable Protocol
protocol MonthGroupable {
    var createdAt: String { get }
    var updatedAt: String { get }
}

// MARK: - Table Column Configuration
struct TableColumn {
    let key: String
    let title: String
    let width: CGFloat?
    let alignment: HorizontalAlignment
    let isVisible: (UIDevice) -> Bool
    let isCustomizable: Bool
    let isRequired: Bool
    
    init(
        key: String, 
        title: String, 
        width: CGFloat? = nil, 
        alignment: HorizontalAlignment = .leading, 
        isVisible: @escaping (UIDevice) -> Bool = { _ in true },
        isCustomizable: Bool = true,
        isRequired: Bool = false
    ) {
        self.key = key
        self.title = title
        self.width = width
        self.alignment = alignment
        self.isVisible = isVisible
        self.isCustomizable = isCustomizable
        self.isRequired = isRequired
    }
}

// MARK: - Column Preference Manager
class ColumnPreferenceManager: ObservableObject {
    private let userDefaults = UserDefaults.standard
    
    func getHiddenColumns(for domain: String) -> Set<String> {
        let hidden = userDefaults.stringArray(forKey: "hiddenColumns_\(domain)") ?? []
        return Set(hidden)
    }
    
    func setHiddenColumns(_ columns: Set<String>, for domain: String) {
        userDefaults.set(Array(columns), forKey: "hiddenColumns_\(domain)")
    }
    
    func toggleColumn(_ columnKey: String, for domain: String) {
        var hidden = getHiddenColumns(for: domain)
        if hidden.contains(columnKey) {
            hidden.remove(columnKey)
        } else {
            hidden.insert(columnKey)
        }
        setHiddenColumns(hidden, for: domain)
    }
}

// MARK: - Adaptive List View
struct AdaptiveListView<Item: Identifiable & MonthGroupable & Equatable, CardContent: View, RowContent: View>: View {
    let items: [Item]
    let viewStyle: ListViewStyle
    let onViewStyleChange: (ListViewStyle) -> Void
    let onItemTap: (Item) -> Void
    let cardContent: (Item) -> CardContent
    let tableColumns: [TableColumn]
    let rowContent: (Item, [TableColumn]) -> RowContent
    let domainName: String
    let userRole: String? // Add user role for button visibility
    let onFilterBasedSelectAll: (() -> Void)? // Callback for backend filter selection
    
    @State private var groupedItems: [(monthYear: String, items: [Item])] = []
    @State private var showColumnCustomizer = false
    @StateObject private var columnPreferenceManager = ColumnPreferenceManager()
    @State private var hiddenColumns: Set<String> = []
    
    // Selection state from parent
    @Binding var isInSelectionMode: Bool
    @Binding var selectedItems: Set<String>
    
    // Multi-selection state (only for table view)
    @State private var isSelectAllExplicitlyPressed = false // Track explicit Select All
    
    // Advanced year/month selection
    @State private var selectedYears: Set<Int> = []
    @State private var selectedMonths: Set<Int> = [] // 1-12
    @State private var showYearPicker = false
    @State private var showMonthPicker = false
    
    private let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMMM yyyy"
        return formatter
    }()
    
    private let isoFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
    
    // Available years and months from data
    private var availableYears: [Int] {
        let years = groupedItems.compactMap { group in
            dateFormatter.date(from: group.monthYear)?.year
        }
        return Array(Set(years)).sorted(by: >)
    }
    
    private var availableMonths: [Int] {
        Array(1...12)
    }
    
    private func monthName(for month: Int) -> String {
        DateFormatter().monthSymbols[month - 1]
    }
    
    init(
        items: [Item],
        viewStyle: ListViewStyle,
        onViewStyleChange: @escaping (ListViewStyle) -> Void,
        onItemTap: @escaping (Item) -> Void,
        cardContent: @escaping (Item) -> CardContent,
        tableColumns: [TableColumn],
        rowContent: @escaping (Item, [TableColumn]) -> RowContent,
        domainName: String,
        userRole: String?,
        isInSelectionMode: Binding<Bool>,
        selectedItems: Binding<Set<String>>,
        onFilterBasedSelectAll: (() -> Void)? = nil
    ) {
        self.items = items
        self.viewStyle = viewStyle
        self.onViewStyleChange = onViewStyleChange
        self.onItemTap = onItemTap
        self.cardContent = cardContent
        self.tableColumns = tableColumns
        self.rowContent = rowContent
        self.domainName = domainName
        self.userRole = userRole
        self._isInSelectionMode = isInSelectionMode
        self._selectedItems = selectedItems
        self.onFilterBasedSelectAll = onFilterBasedSelectAll
    }
    
    var body: some View {
        VStack(spacing: 0) {
            // View Style Toggle with Column Customization
            HStack {
                Spacer()
                
                // Custom segmented control with column customization
                HStack(spacing: 0) {
                    ForEach(ListViewStyle.allCases, id: \.self) { style in
                        Button(action: {
                            if style == viewStyle && style == .table {
                                // If already in table view, show column customizer
                                showColumnCustomizer = true
                            } else {
                                // Switch view style
                                onViewStyleChange(style)
                                // Clear selection when switching views
                                selectedItems.removeAll()
                                isInSelectionMode = false
                            }
                        }) {
                            HStack(spacing: 4) {
                                Image(systemName: style.icon)
                                    .font(.caption)
                                Text(style.displayName)
                                    .font(.caption)
                                
                                // Show settings icon for table when selected
                                if style == .table && viewStyle == .table {
                                    Image(systemName: "gearshape.fill")
                                        .font(.caption2)
                                        .foregroundColor(.secondary)
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(viewStyle == style ? Color.blue : Color.clear)
                            .foregroundColor(viewStyle == style ? .white : .blue)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
                        }
                    }
                }
                .background(Color(.systemGray6))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .padding(.horizontal)
            .padding(.bottom, 8)
            
            // Selection Controls (only show in table view when in selection mode)
            if viewStyle == .table && isInSelectionMode {
                advancedSelectionControlsView
            }
            
            // Content
            if groupedItems.isEmpty {
                emptyStateView
            } else {
                switch viewStyle {
                case .cards:
                    cardListView
                case .table:
                    tableView
                }
            }
        }
        .onAppear {
            hiddenColumns = columnPreferenceManager.getHiddenColumns(for: domainName)
            groupItemsByMonth()
        }
        .onChange(of: items) { oldItems, newItems in
            // Group the new items immediately
            groupItemsByMonth()
            
            // Keep selection mode active but filter out invalid selections
            let newItemIds = Set(newItems.map { String(describing: $0.id) })
            selectedItems = selectedItems.intersection(newItemIds)
            
            // If we had date filters active, reapply them to the new filtered items
            if !selectedYears.isEmpty || !selectedMonths.isEmpty {
                updateSelectionBasedOnDateFilters()
            }
            
            // Don't automatically exit selection mode - let user explicitly exit with clear button
            // Reset explicit select all if no items are selected
            if selectedItems.isEmpty {
                isSelectAllExplicitlyPressed = false
            }
        }
        .sheet(isPresented: $showColumnCustomizer) {
            ColumnCustomizerSheet(
                columns: tableColumns,
                hiddenColumns: hiddenColumns,
                domainName: domainName,
                onSave: { newHiddenColumns in
                    hiddenColumns = newHiddenColumns
                    columnPreferenceManager.setHiddenColumns(newHiddenColumns, for: domainName)
                }
            )
        }
    }
    
    // MARK: - Advanced Selection Controls
    private var advancedSelectionControlsView: some View {
        VStack(spacing: 8) {
            // Main controls row
            HStack(spacing: 8) {
                // Select All / Filter Selection button
                Button(action: {
                    if isSelectAllExplicitlyPressed {
                        // If select all was explicitly pressed, clear everything
                        selectedItems.removeAll()
                        selectedYears.removeAll()
                        selectedMonths.removeAll()
                        isSelectAllExplicitlyPressed = false
                    } else {
                        // Check if we have filters - if so, delegate to parent for backend filtering
                        if hasActiveFilters() {
                            // Trigger backend filter selection via callback
                            onFilterBasedSelectAll?()
                        } else {
                            // When selecting all without filters, select everything visible
                            selectedYears.removeAll()
                            selectedMonths.removeAll()
                            let allItemIds = groupedItems.flatMap { $0.items.map { String(describing: $0.id) } }
                            selectedItems = Set(allItemIds)
                            isSelectAllExplicitlyPressed = true
                        }
                    }
                }) {
                    VStack(spacing: 2) {
                        Image(systemName: isSelectAllExplicitlyPressed ? "checkmark.circle.fill" : "circle")
                            .font(.caption)
                            .foregroundColor(isSelectAllExplicitlyPressed ? .blue : .secondary)
                        Text("Select All")
                            .font(.caption2)
                            .fontWeight(.medium)
                    }
                    .frame(height: 50) // Fixed height
                    .frame(maxWidth: .infinity)
                    .background(isSelectAllExplicitlyPressed ? Color.blue.opacity(0.1) : Color(.systemGray6))
                    .foregroundColor(isSelectAllExplicitlyPressed ? .blue : .primary)
                    .cornerRadius(8)
                }
                
                // Years button
                Button(action: { 
                    if !isSelectAllExplicitlyPressed {
                        showYearPicker = true 
                    }
                }) {
                    VStack(spacing: 2) {
                        Image(systemName: selectedYears.isEmpty ? "calendar" : "calendar.badge.checkmark")
                            .font(.caption)
                            .foregroundColor(isSelectAllExplicitlyPressed ? .gray : (selectedYears.isEmpty ? .secondary : .blue))
                        Text("Years")
                            .font(.caption2)
                            .fontWeight(.medium)
                        if !selectedYears.isEmpty {
                            Text("\(selectedYears.count)")
                                .font(.caption2)
                                .foregroundColor(isSelectAllExplicitlyPressed ? .gray : .blue)
                        } else {
                            // Invisible text to maintain consistent height
                            Text(" ")
                                .font(.caption2)
                        }
                    }
                    .frame(height: 50) // Fixed height
                    .frame(maxWidth: .infinity)
                    .background(isSelectAllExplicitlyPressed ? Color(.systemGray5) : (selectedYears.isEmpty ? Color(.systemGray6) : Color.blue.opacity(0.1)))
                    .foregroundColor(isSelectAllExplicitlyPressed ? .gray : (selectedYears.isEmpty ? .primary : .blue))
                    .cornerRadius(8)
                }
                .disabled(isSelectAllExplicitlyPressed)
                
                // Months button
                Button(action: { 
                    if !isSelectAllExplicitlyPressed {
                        showMonthPicker = true 
                    }
                }) {
                    VStack(spacing: 2) {
                        Image(systemName: selectedMonths.isEmpty ? "calendar.day.timeline.leading" : "calendar.day.timeline.leading.filled")
                            .font(.caption)
                            .foregroundColor(isSelectAllExplicitlyPressed ? .gray : (selectedMonths.isEmpty ? .secondary : .blue))
                        Text("Months")
                            .font(.caption2)
                            .fontWeight(.medium)
                        if !selectedMonths.isEmpty {
                            Text("\(selectedMonths.count)")
                                .font(.caption2)
                                .foregroundColor(isSelectAllExplicitlyPressed ? .gray : .blue)
                        } else {
                            // Invisible text to maintain consistent height
                            Text(" ")
                                .font(.caption2)
                        }
                    }
                    .frame(height: 50) // Fixed height
                    .frame(maxWidth: .infinity)
                    .background(isSelectAllExplicitlyPressed ? Color(.systemGray5) : (selectedMonths.isEmpty ? Color(.systemGray6) : Color.blue.opacity(0.1)))
                    .foregroundColor(isSelectAllExplicitlyPressed ? .gray : (selectedMonths.isEmpty ? .primary : .blue))
                    .cornerRadius(8)
                }
                .disabled(isSelectAllExplicitlyPressed)
                
                // Clear button - smaller with X
                Button(action: {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        selectedItems.removeAll()
                        selectedYears.removeAll()
                        selectedMonths.removeAll()
                        isSelectAllExplicitlyPressed = false
                        isInSelectionMode = false
                    }
                }) {
                    Image(systemName: "xmark")
                        .font(.caption)
                        .fontWeight(.semibold)
                        .foregroundColor(.red)
                }
                .frame(width: 50, height: 50) // Square button
                .background(Color.red.opacity(0.1))
                .cornerRadius(8)
            }
            .padding(.horizontal)
            
            // Action buttons row (only show when items are selected)
            if !selectedItems.isEmpty {
                HStack(spacing: 12) {
                    // Export button for admin and tl
                    if let role = userRole, (role.lowercased() == "admin" || role.lowercased() == "tl") {
                        Button(action: {
                            // TODO: Handle export action
                            print("Export \(selectedItems.count) items")
                        }) {
                            HStack(spacing: 4) {
                                Image(systemName: "square.and.arrow.up")
                                    .font(.caption)
                                Text("Export")
                                    .font(.caption2)
                                    .fontWeight(.medium)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Color.blue.opacity(0.1))
                            .foregroundColor(.blue)
                            .cornerRadius(6)
                        }
                    }
                    
                    // Soft delete button for officer and field tl
                    if let role = userRole, (role.lowercased() == "officer" || role.lowercased() == "field_tl" || role.lowercased() == "fieldtl") {
                        Button(action: {
                            // TODO: Handle soft delete action
                            print("Soft delete \(selectedItems.count) items")
                        }) {
                            HStack(spacing: 4) {
                                Image(systemName: "trash")
                                    .font(.caption)
                                Text("Archive")
                                    .font(.caption2)
                                    .fontWeight(.medium)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Color.orange.opacity(0.1))
                            .foregroundColor(.orange)
                            .cornerRadius(6)
                        }
                    }
                    
                    // Hard delete button for admin only
                    if let role = userRole, role.lowercased() == "admin" {
                        Button(action: {
                            // TODO: Handle hard delete action
                            print("Hard delete \(selectedItems.count) items")
                        }) {
                            HStack(spacing: 4) {
                                Image(systemName: "trash.fill")
                                    .font(.caption)
                                Text("Delete")
                                    .font(.caption2)
                                    .fontWeight(.medium)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 6)
                            .background(Color.red.opacity(0.1))
                            .foregroundColor(.red)
                            .cornerRadius(6)
                        }
                    }
                    
                    Spacer()
                }
                .padding(.horizontal)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
            
            // Selected count
            Text("\(selectedItems.count) selected")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(.vertical, 8)
        .background(Color(.systemGray6).opacity(0.8))
        .sheet(isPresented: $showYearPicker) {
            YearPickerSheet(
                availableYears: availableYears,
                selectedYears: $selectedYears,
                onSelectionChange: {
                    isSelectAllExplicitlyPressed = false // Reset explicit select all
                    updateSelectionBasedOnDateFilters()
                    // Enter selection mode if not already active and we have selections
                    if !isInSelectionMode && (!selectedYears.isEmpty || !selectedMonths.isEmpty) {
                        isInSelectionMode = true
                    }
                }
            )
        }
        .sheet(isPresented: $showMonthPicker) {
            MonthPickerSheet(
                selectedMonths: $selectedMonths,
                onSelectionChange: {
                    isSelectAllExplicitlyPressed = false // Reset explicit select all
                    updateSelectionBasedOnDateFilters()
                    // Enter selection mode if not already active and we have selections
                    if !isInSelectionMode && (!selectedYears.isEmpty || !selectedMonths.isEmpty) {
                        isInSelectionMode = true
                    }
                }
            )
        }
    }
    
    // MARK: - Helper Methods for Advanced Selection
    
    /// Check if any filters are currently active (UI-based or backend-based)
    private func hasActiveFilters() -> Bool {
        // Check for date filters
        return !selectedYears.isEmpty || !selectedMonths.isEmpty
        // Note: Backend filters (search, status, etc.) will be handled by the parent callback
    }
    
    private func getFilteredItemIds() -> [String] {
        var filteredItems: [Item] = []
        let currentYear = Calendar.current.component(.year, from: Date())
        
        // Use current items directly if groupedItems might be stale
        let currentGroupedItems = getCurrentGroupedItems()
        
        for group in currentGroupedItems {
            if let date = dateFormatter.date(from: group.monthYear) {
                let year = date.year
                let month = date.month
                
                let shouldInclude: Bool
                
                if selectedYears.isEmpty && selectedMonths.isEmpty {
                    // No filters set - include nothing
                    shouldInclude = false
                } else if selectedYears.isEmpty && !selectedMonths.isEmpty {
                    // Only months selected - default to current year
                    shouldInclude = (year == currentYear) && selectedMonths.contains(month)
                } else if !selectedYears.isEmpty && selectedMonths.isEmpty {
                    // Only years selected - include all months for those years
                    shouldInclude = selectedYears.contains(year)
                } else {
                    // Both years and months selected - both must match
                    shouldInclude = selectedYears.contains(year) && selectedMonths.contains(month)
                }
                
                if shouldInclude {
                    filteredItems.append(contentsOf: group.items)
                }
            }
        }
        
        return filteredItems.map { String(describing: $0.id) }
    }
    
    private func getCurrentGroupedItems() -> [(monthYear: String, items: [Item])] {
        let calendar = Calendar.current
        
        // Group current items by month-year
        let grouped = Dictionary(grouping: items) { item in
            let dateString = item.createdAt.isEmpty ? item.updatedAt : item.createdAt
            
            if let date = isoFormatter.date(from: dateString) {
                return calendar.dateInterval(of: .month, for: date)?.start ?? Date()
            }
            return Date()
        }
        
        // Sort by month (newest first) and items within month (newest first)
        return grouped.map { (monthStart, items) in
            let sortedItems = items.sorted { item1, item2 in
                let date1String = item1.createdAt.isEmpty ? item1.updatedAt : item1.createdAt
                let date2String = item2.createdAt.isEmpty ? item2.updatedAt : item2.createdAt
                
                let date1 = isoFormatter.date(from: date1String) ?? Date.distantPast
                let date2 = isoFormatter.date(from: date2String) ?? Date.distantPast
                
                return date1 > date2
            }
            
            return (
                monthYear: dateFormatter.string(from: monthStart),
                items: sortedItems
            )
        }.sorted { group1, group2 in
            let date1 = dateFormatter.date(from: group1.monthYear) ?? Date.distantPast
            let date2 = dateFormatter.date(from: group2.monthYear) ?? Date.distantPast
            return date1 > date2
        }
    }
    
    private func updateSelectionBasedOnDateFilters() {
        let filteredIds = getFilteredItemIds()
        
        // If we have year or month filters active, automatically select matching items
        if !selectedYears.isEmpty || !selectedMonths.isEmpty {
            selectedItems = Set(filteredIds)
        } else {
            // If no filters, clear all selections but keep selection mode active
            selectedItems.removeAll()
            // Don't exit selection mode - let user continue selecting
        }
        
        // Reset explicit select all if no items are selected
        if selectedItems.isEmpty {
            isSelectAllExplicitlyPressed = false
        }
    }
    
    // MARK: - Card List View
    private var cardListView: some View {
        ScrollView {
            LazyVStack(spacing: 16) {
                ForEach(groupedItems, id: \.monthYear) { group in
                    VStack(alignment: .leading, spacing: 12) {
                        // Month Header
                        HStack {
                            Text(group.monthYear)
                                .font(.headline)
                                .fontWeight(.semibold)
                            
                            Spacer()
                            
                            Text("\(group.items.count) items")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .padding(.horizontal)
                        
                        // Cards
                        LazyVStack(spacing: 12) {
                            ForEach(group.items) { item in
                                Button(action: { onItemTap(item) }) {
                                    cardContent(item)
                                }
                                .buttonStyle(PlainButtonStyle())
                            }
                        }
                        .padding(.horizontal)
                    }
                }
            }
            .padding(.vertical)
        }
    }
    
    // MARK: - Table View
    private var tableView: some View {
        ScrollView {
            LazyVStack(spacing: 20) {
                ForEach(groupedItems, id: \.monthYear) { group in
                    VStack(alignment: .leading, spacing: 8) {
                        // Month Header
                        HStack {
                            Text(group.monthYear)
                                .font(.headline)
                                .fontWeight(.semibold)
                            
                            Spacer()
                            
                            Text("\(group.items.count) items")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .padding(.horizontal)
                        
                        // Table
                        VStack(spacing: 0) {
                            // Header
                            tableHeader
                            
                            // Rows
                            ForEach(group.items) { item in
                                let itemId = String(describing: item.id)
                                let isSelected = selectedItems.contains(itemId)
                                
                                // Fixed row with grey internal highlighting
                                VStack(spacing: 0) {
                                    HStack {
                                        rowContent(item, visibleColumns)
                                    }
                                    .padding(.vertical, 8)
                                    .padding(.horizontal, 8)
                                    .background(isSelected ? Color.gray.opacity(0.15) : Color.clear)
                                    .animation(.easeInOut(duration: 0.15), value: isSelected)
                                    .contentShape(Rectangle())
                                    .onTapGesture {
                                        if isInSelectionMode {
                                            withAnimation(.easeInOut(duration: 0.15)) {
                                                toggleSelection(for: itemId)
                                            }
                                        } else {
                                            onItemTap(item)
                                        }
                                    }
                                    .onLongPressGesture(minimumDuration: 0.5) {
                                        if !isInSelectionMode {
                                            withAnimation(.easeInOut(duration: 0.2)) {
                                                isInSelectionMode = true
                                                selectedItems.insert(itemId)
                                            }
                                            
                                            // Haptic feedback
                                            let impactFeedback = UIImpactFeedbackGenerator(style: .medium)
                                            impactFeedback.impactOccurred()
                                        }
                                    }
                                    
                                    if item.id != group.items.last?.id {
                                        Divider()
                                            .padding(.horizontal, 8)
                                    }
                                }
                            }
                        }
                        .background(Color(.systemBackground))
                        .cornerRadius(12)
                        .shadow(color: Color.black.opacity(0.05), radius: 2, x: 0, y: 1)
                        .padding(.horizontal)
                    }
                }
            }
            .padding(.vertical)
        }
    }
    
    // MARK: - Table Header
    private var tableHeader: some View {
        HStack(spacing: 0) {
            ForEach(visibleColumns, id: \.key) { column in
                Text(column.title)
                    .font(.caption)
                    .fontWeight(.semibold)
                    .foregroundColor(.secondary)
                    .frame(maxWidth: column.width ?? .infinity, alignment: Alignment(horizontal: column.alignment, vertical: .center))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 12)
                
                if column.key != visibleColumns.last?.key {
                    Divider()
                        .frame(height: 20)
                }
            }
        }
        .background(Color(.systemGray6))
    }
    
    // MARK: - Empty State
    private var emptyStateView: some View {
        VStack(spacing: 16) {
            Image(systemName: "tray")
                .font(.system(size: 50))
                .foregroundColor(.secondary)
            Text("No items found")
                .font(.headline)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
    
    // MARK: - Helper Properties
    private var visibleColumns: [TableColumn] {
        tableColumns.filter { column in
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
    }
    
    // MARK: - Helper Methods
    private func toggleSelection(for itemId: String) {
        if selectedItems.contains(itemId) {
            selectedItems.remove(itemId)
            // Don't automatically exit selection mode - let user explicitly exit with clear button
            if selectedItems.isEmpty {
                isSelectAllExplicitlyPressed = false
            }
        } else {
            selectedItems.insert(itemId)
        }
    }
    
    private func groupItemsByMonth() {
        let calendar = Calendar.current
        
        // Group items by month-year
        let grouped = Dictionary(grouping: items) { item in
            // Try to parse the updated date, fallback to created date
            let dateString = item.updatedAt.isEmpty ? item.createdAt : item.updatedAt
            
            if let date = isoFormatter.date(from: dateString) {
                return calendar.dateInterval(of: .month, for: date)?.start ?? Date()
            }
            return Date() // Fallback for invalid dates
        }
        
        // Sort by month (newest first) and items within month (newest first)
        groupedItems = grouped.map { (monthStart, items) in
            let sortedItems = items.sorted { item1, item2 in
                let date1String = item1.updatedAt.isEmpty ? item1.createdAt : item1.updatedAt
                let date2String = item2.updatedAt.isEmpty ? item2.createdAt : item2.updatedAt
                
                let date1 = isoFormatter.date(from: date1String) ?? Date.distantPast
                let date2 = isoFormatter.date(from: date2String) ?? Date.distantPast
                
                return date1 > date2 // Newest first within the month
            }
            
            return (
                monthYear: dateFormatter.string(from: monthStart),
                items: sortedItems
            )
        }.sorted { group1, group2 in
            // Sort months newest first
            let date1 = dateFormatter.date(from: group1.monthYear) ?? Date.distantPast
            let date2 = dateFormatter.date(from: group2.monthYear) ?? Date.distantPast
            return date1 > date2
        }
    }
}

// MARK: - Date Extensions
extension Date {
    var year: Int {
        Calendar.current.component(.year, from: self)
    }
    
    var month: Int {
        Calendar.current.component(.month, from: self)
    }
}

// MARK: - Year Picker Sheet
struct YearPickerSheet: View {
    let availableYears: [Int]
    @Binding var selectedYears: Set<Int>
    let onSelectionChange: () -> Void
    @Environment(\.dismiss) var dismiss
    
    var body: some View {
        NavigationView {
            List {
                ForEach(availableYears, id: \.self) { year in
                    HStack {
                        Text("\(year)")
                            .font(.subheadline)
                        
                        Spacer()
                        
                        if selectedYears.contains(year) {
                            Image(systemName: "checkmark")
                                .foregroundColor(.blue)
                        }
                    }
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if selectedYears.contains(year) {
                            selectedYears.remove(year)
                        } else {
                            selectedYears.insert(year)
                        }
                        onSelectionChange()
                    }
                }
            }
            .navigationTitle("Select Years")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: - Month Picker Sheet
struct MonthPickerSheet: View {
    @Binding var selectedMonths: Set<Int>
    let onSelectionChange: () -> Void
    @Environment(\.dismiss) var dismiss
    
    private let monthNames = DateFormatter().monthSymbols!
    
    var body: some View {
        NavigationView {
            List {
                ForEach(1...12, id: \.self) { month in
                    HStack {
                        Text(monthNames[month - 1])
                            .font(.subheadline)
                        
                        Spacer()
                        
                        if selectedMonths.contains(month) {
                            Image(systemName: "checkmark")
                                .foregroundColor(.blue)
                        }
                    }
                    .contentShape(Rectangle())
                    .onTapGesture {
                        if selectedMonths.contains(month) {
                            selectedMonths.remove(month)
                        } else {
                            selectedMonths.insert(month)
                        }
                        onSelectionChange()
                    }
                }
            }
            .navigationTitle("Select Months")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: - Column Customizer Sheet
struct ColumnCustomizerSheet: View {
    let columns: [TableColumn]
    @State private var localHiddenColumns: Set<String>
    let domainName: String
    let onSave: (Set<String>) -> Void
    @Environment(\.dismiss) var dismiss
    
    init(columns: [TableColumn], hiddenColumns: Set<String>, domainName: String, onSave: @escaping (Set<String>) -> Void) {
        self.columns = columns
        self.domainName = domainName
        self.onSave = onSave
        self._localHiddenColumns = State(initialValue: hiddenColumns)
    }
    
    var body: some View {
        NavigationView {
            List {
                Section {
                    Text("Customize which columns to display in table view. Required columns cannot be hidden.")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                
                Section("Available Columns") {
                    ForEach(columns.filter(\.isCustomizable), id: \.key) { column in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(column.title)
                                    .font(.subheadline)
                                
                                if !column.isVisible(UIDevice.current) {
                                    Text("Hidden on \(UIDevice.current.userInterfaceIdiom == .pad ? "iPhone" : "iPad")")
                                        .font(.caption2)
                                        .foregroundColor(.secondary)
                                }
                            }
                            
                            Spacer()
                            
                            Toggle("", isOn: Binding(
                                get: { !localHiddenColumns.contains(column.key) },
                                set: { isVisible in
                                    if isVisible {
                                        localHiddenColumns.remove(column.key)
                                    } else {
                                        localHiddenColumns.insert(column.key)
                                    }
                                }
                            ))
                        }
                    }
                }
                
                Section("Always Visible") {
                    ForEach(columns.filter { $0.isRequired || !$0.isCustomizable }, id: \.key) { column in
                        HStack {
                            Text(column.title)
                                .font(.subheadline)
                            Spacer()
                            Text("Required")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                }
            }
            .navigationTitle("Customize Columns")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Save") {
                        onSave(localHiddenColumns)
                        dismiss()
                    }
                }
            }
        }
    }
}

// MARK: - View Style Preference Manager
class ViewStylePreferenceManager: ObservableObject {
    private let userDefaults = UserDefaults.standard
    
    func getViewStyle(for domain: String) -> ListViewStyle {
        let rawValue = userDefaults.string(forKey: "viewStyle_\(domain)") ?? ListViewStyle.cards.rawValue
        return ListViewStyle(rawValue: rawValue) ?? .cards
    }
    
    func setViewStyle(_ style: ListViewStyle, for domain: String) {
        userDefaults.set(style.rawValue, forKey: "viewStyle_\(domain)")
    }
} 