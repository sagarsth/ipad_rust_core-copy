//
//  EntityListView.swift
//  ActionAid SwiftUI
//
//  Generic entity list view with search, filters, view modes, and selection
//

import SwiftUI

// MARK: - Empty State Configuration
struct EmptyStateConfig {
    let icon: String
    let title: String
    let subtitle: String?
    let actionTitle: String?
    let action: (() -> Void)?
    
    init(
        icon: String,
        title: String,
        subtitle: String? = nil,
        actionTitle: String? = nil,
        action: (() -> Void)? = nil
    ) {
        self.icon = icon
        self.title = title
        self.subtitle = subtitle
        self.actionTitle = actionTitle
        self.action = action
    }
    
    static let strategicGoals = EmptyStateConfig(
        icon: "target",
        title: "No Strategic Goals",
        subtitle: "Create your first strategic goal to get started",
        actionTitle: "Create Goal"
    )
    
    static let projects = EmptyStateConfig(
        icon: "folder",
        title: "No Projects",
        subtitle: "Create your first project to get started",
        actionTitle: "Create Project"
    )
    
    static let users = EmptyStateConfig(
        icon: "person.2",
        title: "No Users",
        subtitle: "Add your first user to get started",
        actionTitle: "Add User"
    )
    
    static let documents = EmptyStateConfig(
        icon: "doc",
        title: "No Documents",
        subtitle: "Upload your first document to get started",
        actionTitle: "Upload Document"
    )
}

// MARK: - Entity List View
struct EntityListView<Entity: Identifiable & MonthGroupable & Equatable, CardContent: View, RowContent: View>: View {
    // MARK: - Data
    let entities: [Entity]
    let isLoading: Bool
    let emptyStateConfig: EmptyStateConfig
    
    // MARK: - Search and Filtering
    @Binding var searchText: String
    @Binding var selectedFilters: Set<String>
    let filterOptions: [FilterOption]
    
    // MARK: - View Configuration
    @Binding var currentViewStyle: ListViewStyle
    let onViewStyleChange: (ListViewStyle) -> Void
    
    // MARK: - Selection
    @ObservedObject var selectionManager: SelectionManager
    let onFilterBasedSelectAll: (() -> Void)?
    
    // MARK: - Content Builders
    let onItemTap: (Entity) -> Void
    let cardContent: (Entity) -> CardContent
    let tableColumns: [TableColumn]
    let rowContent: (Entity, [TableColumn]) -> RowContent
    
    // MARK: - Configuration
    let domainName: String
    let userRole: String?
    @Binding var showColumnCustomizer: Bool
    
    // MARK: - Private State
    @State private var isInSelectionMode = false
    @State private var selectedItems: Set<String> = []
    
    // MARK: - Computed Properties
    private var hasActiveFilters: Bool {
        !searchText.isEmpty || !selectedFilters.contains("all")
    }
    
    private var filteredEntities: [Entity] {
        var results = entities
        
        // Apply search filter
        if !searchText.isEmpty {
            results = results.filter { entity in
                // This is a simplified search - in practice, you might want to make this configurable
                let searchString = String(describing: entity).lowercased()
                return searchString.contains(searchText.lowercased())
            }
        }
        
        return results
    }
    
    // MARK: - Initialization
    init(
        entities: [Entity],
        isLoading: Bool,
        emptyStateConfig: EmptyStateConfig,
        searchText: Binding<String>,
        selectedFilters: Binding<Set<String>>,
        filterOptions: [FilterOption],
        currentViewStyle: Binding<ListViewStyle>,
        onViewStyleChange: @escaping (ListViewStyle) -> Void,
        selectionManager: SelectionManager,
        onFilterBasedSelectAll: (() -> Void)? = nil,
        onItemTap: @escaping (Entity) -> Void,
        cardContent: @escaping (Entity) -> CardContent,
        tableColumns: [TableColumn],
        rowContent: @escaping (Entity, [TableColumn]) -> RowContent,
        domainName: String,
        userRole: String?,
        showColumnCustomizer: Binding<Bool>
    ) {
        self.entities = entities
        self.isLoading = isLoading
        self.emptyStateConfig = emptyStateConfig
        self._searchText = searchText
        self._selectedFilters = selectedFilters
        self.filterOptions = filterOptions
        self._currentViewStyle = currentViewStyle
        self.onViewStyleChange = onViewStyleChange
        self.selectionManager = selectionManager
        self.onFilterBasedSelectAll = onFilterBasedSelectAll
        self.onItemTap = onItemTap
        self.cardContent = cardContent
        self.tableColumns = tableColumns
        self.rowContent = rowContent
        self.domainName = domainName
        self.userRole = userRole
        self._showColumnCustomizer = showColumnCustomizer
    }
    
    // MARK: - Body
    var body: some View {
        VStack(spacing: 0) {
            // Filter Bar
            FilterBarComponent(
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                config: filterBarConfig,
                filterOptions: filterOptions
            )
            
            // Content
            if isLoading && entities.isEmpty {
                loadingView
            } else if filteredEntities.isEmpty {
                emptyStateView
            } else {
                AdaptiveListView(
                    items: filteredEntities,
                    viewStyle: currentViewStyle,
                    onViewStyleChange: onViewStyleChange,
                    onItemTap: onItemTap,
                    cardContent: cardContent,
                    tableColumns: tableColumns,
                    rowContent: rowContent,
                    domainName: domainName,
                    userRole: userRole,
                    isInSelectionMode: $isInSelectionMode,
                    selectedItems: $selectedItems,
                    onFilterBasedSelectAll: onFilterBasedSelectAll,
                    showColumnCustomizer: $showColumnCustomizer
                )
            }
        }
        .onChange(of: selectionManager.isInSelectionMode) { oldValue, newValue in
            isInSelectionMode = newValue
        }
        .onChange(of: selectionManager.selectedItems) { oldValue, newValue in
            selectedItems = newValue
        }
        .onChange(of: isInSelectionMode) { oldValue, newValue in
            selectionManager.isInSelectionMode = newValue
        }
        .onChange(of: selectedItems) { oldValue, newValue in
            selectionManager.selectedItems = newValue
        }
        .onChange(of: selectedFilters) { oldValue, newValue in
            // BACKEND FILTER CHANGE: Only re-trigger selection if "Select All" is active
            print("🔍 [FILTER_CHANGE] oldValue: \(oldValue) → newValue: \(newValue)")
            print("🔍 [FILTER_CHANGE] isSelectAllActive: \(selectionManager.isSelectAllActive)")
            
            if selectionManager.isSelectAllActive {
                print("🔍 [FILTER_CHANGE] ✅ Triggering backend filter selection")
                // Re-select all items matching the new backend filter criteria
                onFilterBasedSelectAll?()
            } else {
                print("🔍 [FILTER_CHANGE] ❌ Select All not active, skipping backend call")
            }
        }
        .onChange(of: searchText) { oldValue, newValue in
            // BACKEND FILTER CHANGE: Search text is also a backend filter
            if selectionManager.isSelectAllActive {
                // Re-select all items matching the new search criteria
                onFilterBasedSelectAll?()
            }
        }
    }
    
    // MARK: - Filter Bar Configuration
    private var filterBarConfig: FilterBarConfig {
        switch domainName {
        case "strategic_goals":
            return .strategicGoals
        case "projects":
            return .projects
        case "users":
            return .users
        default:
            return .default
        }
    }
    
    // MARK: - Loading View
    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)
            
            Text("Loading \(domainName.replacingOccurrences(of: "_", with: " ").capitalized)...")
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemGroupedBackground))
    }
    
    // MARK: - Empty State View
    private var emptyStateView: some View {
        VStack(spacing: 20) {
            Image(systemName: emptyStateConfig.icon)
                .font(.system(size: 60))
                .foregroundColor(.secondary)
            
            VStack(spacing: 8) {
                Text(emptyStateConfig.title)
                    .font(.title2)
                    .fontWeight(.semibold)
                    .foregroundColor(.primary)
                
                if let subtitle = emptyStateConfig.subtitle {
                    Text(subtitle)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
            }
            
            if hasActiveFilters {
                VStack(spacing: 12) {
                    Text("No results match your current filters")
                        .font(.caption)
                        .foregroundColor(.secondary)
                    
                    Button("Clear Filters") {
                        withAnimation {
                            searchText = ""
                            selectedFilters = ["all"]
                        }
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            } else if let actionTitle = emptyStateConfig.actionTitle,
                      let action = emptyStateConfig.action {
                Button(actionTitle, action: action)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.regular)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemGroupedBackground))
    }
}

// MARK: - Entity List View Extensions

extension EntityListView {
    /// Create a simplified entity list with default configuration
    static func simple<SimpleCardContent: View>(
        entities: [Entity],
        isLoading: Bool = false,
        searchText: Binding<String>,
        selectedFilters: Binding<Set<String>>,
        filterOptions: [FilterOption] = [],
        viewStyle: Binding<ListViewStyle> = .constant(.cards),
        domainName: String,
        onItemTap: @escaping (Entity) -> Void,
        @ViewBuilder cardContent: @escaping (Entity) -> SimpleCardContent
    ) -> some View where RowContent == EmptyView {
        EntityListView<Entity, SimpleCardContent, EmptyView>(
            entities: entities,
            isLoading: isLoading,
            emptyStateConfig: EmptyStateConfig(
                icon: "tray",
                title: "No Items",
                subtitle: "No \(domainName.replacingOccurrences(of: "_", with: " ")) found"
            ),
            searchText: searchText,
            selectedFilters: selectedFilters,
            filterOptions: filterOptions,
            currentViewStyle: viewStyle,
            onViewStyleChange: { _ in },
            selectionManager: SelectionManager(),
            onFilterBasedSelectAll: nil,
            onItemTap: onItemTap,
            cardContent: cardContent,
            tableColumns: [],
            rowContent: { _, _ in EmptyView() },
            domainName: domainName,
            userRole: nil,
            showColumnCustomizer: .constant(false)
        )
    }
}

// MARK: - Entity List View Modifiers

extension View {
    /// Apply loading state overlay to any view
    func withLoadingState(_ isLoading: Bool, message: String = "Loading...") -> some View {
        self.overlay {
            if isLoading {
                Color.black.opacity(0.3)
                    .ignoresSafeArea()
                
                VStack(spacing: 12) {
                    ProgressView()
                        .scaleEffect(1.2)
                    
                    Text(message)
                        .font(.subheadline)
                        .foregroundColor(.white)
                }
                .padding()
                .background(Color.black.opacity(0.7))
                .cornerRadius(12)
            }
        }
    }
    
    /// Apply empty state when a collection is empty
    func withEmptyState<EmptyContent: View>(
        isEmpty: Bool,
        @ViewBuilder emptyContent: () -> EmptyContent
    ) -> some View {
        Group {
            if isEmpty {
                emptyContent()
            } else {
                self
            }
        }
    }
}

// MARK: - Entity Stats Component

struct EntityStatsCard: View {
    let stats: [EntityStat]
    
    struct EntityStat {
        let title: String
        let value: String
        let icon: String
        let color: Color
        
        init(title: String, value: Int, icon: String, color: Color) {
            self.title = title
            self.value = "\(value)"
            self.icon = icon
            self.color = color
        }
        
        init(title: String, value: String, icon: String, color: Color) {
            self.title = title
            self.value = value
            self.icon = icon
            self.color = color
        }
    }
    
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 12) {
                ForEach(stats.indices, id: \.self) { index in
                    let stat = stats[index]
                    
                    VStack(spacing: 8) {
                        HStack {
                            Image(systemName: stat.icon)
                                .font(.title3)
                                .foregroundColor(stat.color)
                            
                            Spacer()
                        }
                        
                        VStack(alignment: .leading, spacing: 2) {
                            Text(stat.value)
                                .font(.title2)
                                .fontWeight(.bold)
                                .foregroundColor(.primary)
                            
                            Text(stat.title)
                                .font(.caption)
                                .foregroundColor(.secondary)
                                .lineLimit(2)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding()
                    .frame(width: 100, height: 80)
                    .background(Color(.systemGray6))
                    .cornerRadius(12)
                }
            }
            .padding(.horizontal)
        }
    }
}

// MARK: - Quick Filter Chips

struct QuickFilterChips: View {
    @Binding var selectedFilters: Set<String>
    let filterOptions: [FilterOption]
    let maxVisible: Int
    
    init(
        selectedFilters: Binding<Set<String>>,
        filterOptions: [FilterOption],
        maxVisible: Int = 3
    ) {
        self._selectedFilters = selectedFilters
        self.filterOptions = filterOptions
        self.maxVisible = maxVisible
    }
    
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(Array(filterOptions.prefix(maxVisible)), id: \.id) { option in
                    MultiSelectFilterChip(
                        title: option.displayName,
                        value: option.id,
                        selections: $selectedFilters,
                        color: option.color ?? .blue
                    )
                }
            }
            .padding(.horizontal)
        }
    }
}

// MARK: - Preview

#if DEBUG
struct EntityListView_Previews: PreviewProvider {
    struct SampleEntity: Identifiable, MonthGroupable, Equatable {
        let id = UUID()
        let name: String
        let createdAt: String
        let updatedAt: String
        
        init(name: String) {
            self.name = name
            self.createdAt = ISO8601DateFormatter().string(from: Date())
            self.updatedAt = ISO8601DateFormatter().string(from: Date())
        }
    }
    
    @State static var searchText = ""
    @State static var selectedFilters: Set<String> = ["all"]
    @State static var viewStyle: ListViewStyle = .cards
    
    static var previews: some View {
        EntityListView<SampleEntity, AnyView, EmptyView>.simple(
            entities: [
                SampleEntity(name: "Sample 1"),
                SampleEntity(name: "Sample 2"),
                SampleEntity(name: "Sample 3")
            ],
            searchText: $searchText,
            selectedFilters: $selectedFilters,
            filterOptions: FilterOption.strategicGoalFilters,
            viewStyle: $viewStyle,
            domainName: "samples",
            onItemTap: { _ in }
        ) { entity in
            AnyView(
                VStack(alignment: .leading) {
                    Text(entity.name)
                        .font(.headline)
                    Text("Sample entity")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                .padding()
                .background(Color(.systemGray6))
                .cornerRadius(8)
            )
        }
    }
}
#endif 