import SwiftUI
import Foundation
import Combine

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

// MARK: - Device Orientation Detection
@MainActor
class OrientationDetector: ObservableObject {
    @Published var isLandscape: Bool = false
    private var updateTimer: Timer?
    
    init() {
        updateOrientation()
        NotificationCenter.default.addObserver(
            forName: UIDevice.orientationDidChangeNotification,
            object: nil,
            queue: .main
        ) { _ in
            // Debounce orientation changes to prevent glitching
            self.scheduleOrientationUpdate()
        }
    }
    
    private func scheduleOrientationUpdate() {
        updateTimer?.invalidate()
        updateTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: false) { _ in
            self.updateOrientation()
        }
    }
    
    private func updateOrientation() {
        let newIsLandscape: Bool
        
        // Use screen bounds as primary indicator, device orientation as secondary
        let screenBounds = UIScreen.main.bounds
        let screenIsLandscape = screenBounds.width > screenBounds.height
        
        let deviceOrientation = UIDevice.current.orientation
        if deviceOrientation.isValidInterfaceOrientation {
            newIsLandscape = deviceOrientation.isLandscape
        } else {
            newIsLandscape = screenIsLandscape
        }
        
        // Only update if there's actually a change to prevent unnecessary UI updates
        if newIsLandscape != isLandscape {
            isLandscape = newIsLandscape
            print("📱 [ORIENTATION] Changed to \(isLandscape ? "landscape" : "portrait")")
        }
    }
    
    deinit {
        updateTimer?.invalidate()
        NotificationCenter.default.removeObserver(self)
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
    @Binding var showColumnCustomizer: Bool // Binding to control column customizer sheet
    
    // Authentication manager for export functionality
    @EnvironmentObject var authManager: AuthenticationManager
    
    @State private var groupedItems: [(monthYear: String, items: [Item])] = []
    @StateObject private var columnPreferenceManager = ColumnPreferenceManager()
    @StateObject private var orientationDetector = OrientationDetector()
    @State private var hiddenColumns: Set<String> = []
    
    // Selection state from parent
    @Binding var isInSelectionMode: Bool
    @Binding var selectedItems: Set<String>
    
    // Multi-selection state (only for table view)
    @State private var isSelectAllExplicitlyPressed = false // Track explicit Select All
    
    // Track previous filter state to detect changes
    @State private var previousFilterState: String = ""
    
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
        onFilterBasedSelectAll: (() -> Void)? = nil,
        showColumnCustomizer: Binding<Bool>
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
        self._showColumnCustomizer = showColumnCustomizer
    }

    
    var body: some View {
        VStack(spacing: 0) {
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
            
            let newItemIds = Set(newItems.map { String(describing: $0.id) })
            
            // Apply selection logic based on current mode
            if isSelectAllExplicitlyPressed {
                // "Select All" mode: select all new items (backend filters already applied)
                selectedItems = newItemIds
            } else if !selectedYears.isEmpty || !selectedMonths.isEmpty {
                // Date filter mode: reapply date filters to new items
                updateSelectionBasedOnDateFilters()
            } else {
                // Individual selection mode: filter out invalid selections
                selectedItems = selectedItems.intersection(newItemIds)
                // Exit selection mode if no items selected
                if selectedItems.isEmpty {
                    isInSelectionMode = false
                }
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
                        // OVERRIDE: Select All clears date filters and selects everything
                        selectedYears.removeAll()
                        selectedMonths.removeAll()
                        
                        // Check if we have backend filters (search, status, etc.) - if so, delegate to parent
                        if hasBackendFilters() {
                            // Trigger backend filter selection via callback which will use selectAllItems()
                            onFilterBasedSelectAll?()
                        } else {
                            // When selecting all without any filters, select everything visible using selectAllItems()
                            let allItemIds = groupedItems.flatMap { $0.items.map { String(describing: $0.id) } }
                            selectedItems = Set(allItemIds)
                        }
                        
                        isSelectAllExplicitlyPressed = true
                    }
                }) {
                    VStack(spacing: 2) {
                        Image(systemName: isSelectAllExplicitlyPressed ? "checkmark.circle.fill" : "circle")
                            .font(.caption)
                            .foregroundColor(isSelectAllExplicitlyPressed ? .blue : .secondary)
                        Text("Select All")
                            .font(.caption2)
                            .fontWeight(.medium)
                        // Invisible text to maintain consistent height with Years/Months
                        Text(" ")
                            .font(.caption2)
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
            }
            .padding(.horizontal)
            

        }
        .padding(.vertical, 8)
        .background(Color(.systemGray6).opacity(0.8))
        .sheet(isPresented: $showYearPicker) {
            YearPickerSheet(
                availableYears: availableYears,
                selectedYears: $selectedYears,
                onSelectionChange: {
                    // OVERRIDE: Date filter selection clears "Select All" state
                    isSelectAllExplicitlyPressed = false
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
                    // OVERRIDE: Date filter selection clears "Select All" state
                    isSelectAllExplicitlyPressed = false
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
    
    /// Check if any date filters (year/month) are currently active
    private func hasActiveFilters() -> Bool {
        // Check for date filters
        return !selectedYears.isEmpty || !selectedMonths.isEmpty
    }
    
    /// Check if any backend filters (search, status, etc.) are currently active
    /// This delegates to the parent callback which handles backend filter detection
    private func hasBackendFilters() -> Bool {
        // Always delegate to parent for backend filter-aware selection
        // The parent callback (onFilterBasedSelectAll) will handle the actual backend filter detection
        // and either select based on backend filters or select all visible items
        return onFilterBasedSelectAll != nil
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
            let dateString = item.updatedAt.isEmpty ? item.createdAt : item.updatedAt
            
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
        // Simple logic: if date filters are active, select matching items; otherwise clear selection
        if !selectedYears.isEmpty || !selectedMonths.isEmpty {
            let filteredIds = getFilteredItemIds()
            selectedItems = Set(filteredIds)
        } else {
            // No date filters active - clear selection (user removed all date filters)
            selectedItems.removeAll()
            // Exit selection mode if no items selected and no "Select All" active
            if !isSelectAllExplicitlyPressed {
                isInSelectionMode = false
            }
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
                                
                                // Modern row highlighting
                                VStack(spacing: 0) {
                                    HStack {
                                        rowContent(item, visibleColumns)
                                    }
                                    .background(
                                        isSelected ? 
                                        Color.blue.opacity(0.08) : Color.clear
                                    )
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 10)
                                            .stroke(isSelected ? Color.blue.opacity(0.2) : Color.clear, lineWidth: 1.5)
                                    )
                                    .cornerRadius(10)
                                    .scaleEffect(isSelected ? 0.995 : 1.0)
                                    .animation(.spring(response: 0.3, dampingFraction: 0.8), value: isSelected)
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
                    .padding(.horizontal, 12) // Increased from 8 to 12 to match row padding
                    .padding(.vertical, 16)   // Increased from 12 to 16 to match row padding
                
                if column.key != visibleColumns.last?.key {
                    Divider()
                        .frame(height: 30)
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
        // Use dynamic column configuration for strategic goals if available
        let columnsToUse: [TableColumn]
        if domainName == "strategic_goals" {
            // Use dynamic configuration for strategic goals
            columnsToUse = StrategicGoalTableConfig.columns(hiddenColumns: hiddenColumns)
        } else {
            // Use static configuration for other domains
            columnsToUse = tableColumns
        }
        
        // Filter columns based on user preferences and device visibility
        let filteredColumns = columnsToUse.filter { column in
            // Hide columns that user has explicitly hidden
            if hiddenColumns.contains(column.key) {
                return false
            }
            
            // Apply device-specific visibility (this is now consistent for all devices)
            return column.isVisible(UIDevice.current)
        }
        
        // Apply column count limits based on orientation and device
        let maxColumns = getMaxColumnsForCurrentContext()
        
        print("🔄 [COLUMNS] Domain: \(domainName), Filtered: \(filteredColumns.count), Max: \(maxColumns), Landscape: \(orientationDetector.isLandscape)")
        print("🔄 [COLUMNS] Available columns: \(filteredColumns.map { "\($0.title)(\($0.isRequired ? "req" : "opt"))" }.joined(separator: ", "))")
        
        // If we have more columns than the limit, prioritize them
        if filteredColumns.count > maxColumns {
            // Always include required columns first
            let requiredColumns = filteredColumns.filter { $0.isRequired }
            let optionalColumns = filteredColumns.filter { !$0.isRequired }
            
            // Take as many optional columns as we can fit
            let remainingSlots = maxColumns - requiredColumns.count
            let selectedOptionalColumns = Array(optionalColumns.prefix(max(0, remainingSlots)))
            
            let finalColumns = requiredColumns + selectedOptionalColumns
            print("🔄 [COLUMNS] Limited to: \(finalColumns.map { $0.title }.joined(separator: ", "))")
            return finalColumns
        }
        
        print("🔄 [COLUMNS] No limit applied, showing all \(filteredColumns.count) columns")
        return filteredColumns
    }
    
    /// Get maximum number of columns based on current device and orientation
    private func getMaxColumnsForCurrentContext() -> Int {
        let currentDevice = UIDevice.current
        
        if currentDevice.userInterfaceIdiom == .phone {
            return orientationDetector.isLandscape ? 6 : 4  // iPhone: 6 landscape, 4 portrait
        } else {
            return orientationDetector.isLandscape ? 10 : 8  // iPad: 7 landscape, 4 portrait  
        }
    }
    

    
    // MARK: - Helper Methods
    
    private func openInFilesApp(path: String) {
        let fileURL = URL(fileURLWithPath: path)
        
        // Try to open Files app to the specific folder
        if #available(iOS 14.0, *) {
            // Modern approach - this will open Files app and navigate to the folder
            let activityViewController = UIActivityViewController(
                activityItems: [fileURL],
                applicationActivities: nil
            )
            
            // Get the root view controller to present from
            if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
               let window = windowScene.windows.first,
               let rootViewController = window.rootViewController {
                
                // For iPad - configure popover
                if let popover = activityViewController.popoverPresentationController {
                    popover.sourceView = rootViewController.view
                    popover.sourceRect = CGRect(x: rootViewController.view.bounds.midX, 
                                              y: rootViewController.view.bounds.midY, 
                                              width: 0, height: 0)
                    popover.permittedArrowDirections = []
                }
                
                rootViewController.present(activityViewController, animated: true)
            }
        } else {
            // Fallback for older iOS versions
            let documentsURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
            UIApplication.shared.open(documentsURL)
        }
    }
    
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
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Customize which columns to display in table view. Required columns cannot be hidden.")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        
                        if UIDevice.current.userInterfaceIdiom == .phone {
                            Text("📱 iPhone: Portrait shows 4 columns max, Landscape shows 6 columns max")
                                .font(.caption2)
                                .foregroundColor(.blue)
                        } else {
                            Text("📱 iPad: Portrait shows 8 columns max, Landscape shows 10 columns max")
                                .font(.caption2)
                                .foregroundColor(.blue)
                        }
                    }
                }
                
                Section("Available Columns") {
                    ForEach(columns.filter(\.isCustomizable), id: \.key) { column in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(column.title)
                                    .font(.subheadline)
                                
                                if !column.isVisible(UIDevice.current) {
                                    Text("Not available on \(UIDevice.current.userInterfaceIdiom == .phone ? "iPhone" : "iPad")")
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

// MARK: - Export Options Sheet
struct ExportOptionsSheet: View {
    let selectedItemCount: Int
    let onExport: (Bool, ExportFormat) -> Void
    @Binding var isExporting: Bool
    @Binding var exportError: String?
    @Environment(\.dismiss) var dismiss
    
    @State private var includeBlobs = false
    @State private var selectedFormat: ExportFormat = .default
    @State private var showFormatRecommendation = false
    
    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 24) {
                    // Smart recommendation banner
                    if showFormatRecommendation {
                        recommendationBanner
                    }
                    
                    exportFormatSelection
                    exportOptionsSection
                    exportInfoSection
                    
                    if let error = exportError {
                        errorSection(error)
                    }
                }
                .padding()
            }
            .navigationTitle("Export Options")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Export") {
                        onExport(includeBlobs, selectedFormat)
                    }
                    .disabled(isExporting)
                }
            }
            .onAppear {
                // Set recommended format based on selected item count
                let recommendedFormat = ExportFormat.recommended(for: selectedItemCount)
                if selectedFormat.id == ExportFormat.default.id {
                    selectedFormat = recommendedFormat
                    showFormatRecommendation = true
                }
            }
        }
    }
    
    private var exportFormatSelection: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Export Format")
                .font(.headline)
                .foregroundColor(.primary)
            
            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
                GridItem(.flexible())
            ], spacing: 12) {
                ForEach(ExportFormat.allCases, id: \.id) { format in
                    FormatCard(
                        format: format,
                        isSelected: selectedFormat.id == format.id,
                        onSelect: { selectedFormat = format }
                    )
                }
            }
            
            // Format-specific options
            formatSpecificOptions
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    @ViewBuilder
    private var formatSpecificOptions: some View {
        switch selectedFormat {
        case .csv(let options):
            csvOptionsView(options)
        case .parquet(let options):
            parquetOptionsView(options)
        case .jsonLines:
            EmptyView()
        }
    }
    
    private func csvOptionsView(_ options: CsvOptions) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("CSV Options")
                .font(.subheadline)
                .fontWeight(.medium)
                .foregroundColor(.secondary)
            
            Toggle("Compress output file", isOn: Binding(
                get: { options.compress },
                set: { newValue in
                    selectedFormat = .csv(CsvOptions(
                        delimiter: options.delimiter,
                        quoteChar: options.quoteChar,
                        escapeChar: options.escapeChar,
                        compress: newValue
                    ))
                }
            ))
            
            Text(options.compress ? 
                "Compressed CSV saves space but needs decompression to view" : 
                "Uses comma delimiter and double quotes")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(.top, 8)
    }
    
    private func parquetOptionsView(_ options: ParquetOptions) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Parquet Options")
                .font(.subheadline)
                .fontWeight(.medium)
                .foregroundColor(.secondary)
            
            HStack {
                Text("Compression:")
                    .font(.caption)
                Spacer()
                Text(options.compression.displayName)
                    .font(.caption)
                    .fontWeight(.medium)
            }
            
            HStack {
                Text("Row Group Size:")
                    .font(.caption)
                Spacer()
                Text("\(options.rowGroupSize)")
                    .font(.caption)
                    .fontWeight(.medium)
            }
            
            Text("Optimized for large datasets and analytics")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(.top, 8)
    }
    
    private var exportOptionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Additional Options")
                .font(.headline)
                .foregroundColor(.primary)
            
            Toggle("Include file attachments", isOn: $includeBlobs)
                .font(.subheadline)
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    private var exportInfoSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Export Information")
                .font(.headline)
                .foregroundColor(.primary)
            
            Text("This will export all strategic goals that match your current filter settings.")
                .font(.subheadline)
                .foregroundColor(.secondary)
            
            // Format-specific info
            formatInfoView
        }
        .padding()
        .background(Color(.systemGray6))
        .cornerRadius(12)
    }
    
    @ViewBuilder
    private var formatInfoView: some View {
        switch selectedFormat {
        case .jsonLines:
            Label("Best for data processing and APIs", systemImage: "gearshape.2")
                .font(.caption)
                .foregroundColor(.blue)
        case .csv:
            Label("Opens in Excel, Google Sheets, and most tools", systemImage: "tablecells")
                .font(.caption)
                .foregroundColor(.green)
        case .parquet:
            Label("Smallest file size, fastest for large datasets", systemImage: "speedometer")
                .font(.caption)
                .foregroundColor(.purple)
        }
    }
    
    private var recommendationBanner: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Smart Recommendation", systemImage: "lightbulb.fill")
                    .font(.headline)
                    .foregroundColor(.orange)
                
                Spacer()
                
                Button("Dismiss") {
                    withAnimation {
                        showFormatRecommendation = false
                    }
                }
                .font(.caption)
                .foregroundColor(.secondary)
            }
            
            Text("For \(selectedItemCount) items, we recommend \(selectedFormat.displayName) format")
                .font(.subheadline)
                .foregroundColor(.primary)
            
            Text(selectedFormat.description)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding()
        .background(Color.orange.opacity(0.1))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.orange.opacity(0.3), lineWidth: 1)
        )
    }
    
    private func errorSection(_ error: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("Export Error", systemImage: "exclamationmark.triangle")
                .font(.headline)
                .foregroundColor(.red)
            
            Text(error)
                .font(.subheadline)
                .foregroundColor(.red)
        }
        .padding()
        .background(Color.red.opacity(0.1))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.red.opacity(0.3), lineWidth: 1)
        )
    }
}

// MARK: - Format Card Component
struct FormatCard: View {
    let format: ExportFormat
    let isSelected: Bool
    let onSelect: () -> Void
    
    var body: some View {
        Button(action: onSelect) {
            VStack(spacing: 8) {
                Image(systemName: format.icon)
                    .font(.title2)
                    .foregroundColor(isSelected ? .white : .primary)
                
                Text(format.displayName)
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundColor(isSelected ? .white : .primary)
                
                Text(format.fileExtension.uppercased())
                    .font(.caption2)
                    .foregroundColor(isSelected ? .white.opacity(0.8) : .secondary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            .background(isSelected ? Color.blue : Color(.systemBackground))
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? Color.clear : Color(.systemGray4), lineWidth: 1)
            )
            .cornerRadius(8)
        }
        .buttonStyle(PlainButtonStyle())
    }
}

// MARK: - View Style Switcher Component
struct ViewStyleSwitcher: View {
    let currentViewStyle: ListViewStyle
    let onViewStyleChange: (ListViewStyle) -> Void
    let onShowColumnCustomizer: () -> Void
    
    var body: some View {
        Image(systemName: currentViewStyle.icon)
            .font(.caption)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color.blue)
            .foregroundColor(.white)
            .clipShape(RoundedRectangle(cornerRadius: 4))
            .background(Color(.systemGray6))
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .onTapGesture {
                // Toggle between view styles
                let nextStyle: ListViewStyle = currentViewStyle == .cards ? .table : .cards
                onViewStyleChange(nextStyle)
            }
            .onLongPressGesture {
                // Show column customizer on long press (only meaningful for table view)
                if currentViewStyle == .table {
                    onShowColumnCustomizer()
                }
            }
    }
} 