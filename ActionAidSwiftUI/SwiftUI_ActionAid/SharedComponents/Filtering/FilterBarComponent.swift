//
//  FilterBarComponent.swift
//  ActionAid SwiftUI
//
//  Reusable filter bar component with search and filter chips
//

import SwiftUI

// MARK: - Filter Bar Configuration
struct FilterBarConfig {
    let placeholder: String
    let showSearchBar: Bool
    let allowMultipleSelection: Bool
    let maxVisibleFilters: Int
    let compactMode: Bool
    let showActiveFilterSummary: Bool
    let adaptiveWidth: Bool  // New property for adaptive width
    
    init(
        placeholder: String = "Search...",
        showSearchBar: Bool = true,
        allowMultipleSelection: Bool = true,
        maxVisibleFilters: Int = 4,
        compactMode: Bool = false,
        showActiveFilterSummary: Bool = false,  // Default to false as it's redundant
        adaptiveWidth: Bool = true  // Default to true for smart behavior
    ) {
        self.placeholder = placeholder
        self.showSearchBar = showSearchBar
        self.allowMultipleSelection = allowMultipleSelection
        self.maxVisibleFilters = maxVisibleFilters
        self.compactMode = compactMode
        self.showActiveFilterSummary = showActiveFilterSummary
        self.adaptiveWidth = adaptiveWidth
    }
    
    static let `default` = FilterBarConfig()
    
    static let strategicGoals = FilterBarConfig(
        placeholder: "Search goals...",
        showSearchBar: true,
        allowMultipleSelection: true,
        maxVisibleFilters: 4,
        compactMode: false,
        showActiveFilterSummary: false,
        adaptiveWidth: true
    )
    
    static let projects = FilterBarConfig(
        placeholder: "Search projects...",
        showSearchBar: true,
        allowMultipleSelection: true,
        maxVisibleFilters: 3,
        compactMode: false,
        showActiveFilterSummary: false,
        adaptiveWidth: true
    )
    
    static let users = FilterBarConfig(
        placeholder: "Search users...",
        showSearchBar: true,
        allowMultipleSelection: false,
        maxVisibleFilters: 3,
        compactMode: true,
        showActiveFilterSummary: false,
        adaptiveWidth: true
    )
    
    static let compact = FilterBarConfig(
        placeholder: "Search...",
        showSearchBar: true,
        allowMultipleSelection: true,
        maxVisibleFilters: 2,
        compactMode: true,
        showActiveFilterSummary: false,
        adaptiveWidth: true
    )
}

// MARK: - Filter Option Model
struct FilterOption: Identifiable {
    let id: String
    let displayName: String
    let icon: String?
    let color: Color?
    let isDefault: Bool
    
    init(
        id: String,
        displayName: String,
        icon: String? = nil,
        color: Color? = nil,
        isDefault: Bool = false
    ) {
        self.id = id
        self.displayName = displayName
        self.icon = icon
        self.color = color
        self.isDefault = isDefault
    }
    
    // MARK: - Strategic Goals Filters
    static let strategicGoalFilters: [FilterOption] = [
        FilterOption(id: "all", displayName: "All", icon: "square.stack", color: .gray, isDefault: true),
        FilterOption(id: "on_track", displayName: "On Track", icon: "checkmark.circle", color: .green),
        FilterOption(id: "at_risk", displayName: "At Risk", icon: "exclamationmark.triangle", color: .orange),
        FilterOption(id: "behind", displayName: "Behind", icon: "xmark.circle", color: .red),
        FilterOption(id: "completed", displayName: "Completed", icon: "checkmark.circle.fill", color: .blue)
    ]
    
    // MARK: - Projects Filters
    static let projectFilters: [FilterOption] = [
        FilterOption(id: "all", displayName: "All", icon: "folder", color: .gray, isDefault: true),
        FilterOption(id: "on_track", displayName: "On Track", icon: "checkmark.circle", color: .green),
        FilterOption(id: "at_risk", displayName: "At Risk", icon: "exclamationmark.triangle", color: .orange),
        FilterOption(id: "delayed", displayName: "Delayed", icon: "xmark.circle", color: .red),
        FilterOption(id: "completed", displayName: "Completed", icon: "checkmark.circle.fill", color: .blue)
    ]
    
    // MARK: - Users Filters
    static let userFilters: [FilterOption] = [
        FilterOption(id: "all", displayName: "All", icon: "person.2", color: .gray, isDefault: true),
        FilterOption(id: "admin", displayName: "Admin", icon: "crown", color: .yellow),
        FilterOption(id: "manager", displayName: "Manager", icon: "person.badge.key", color: .blue),
        FilterOption(id: "staff", displayName: "Staff", icon: "person", color: .green)
    ]
}

// MARK: - Filter Bar Component
struct FilterBarComponent: View {
    @Binding var searchText: String
    @Binding var selectedFilters: Set<String>
    let config: FilterBarConfig
    let filterOptions: [FilterOption]
    
    // Internal state
    @State private var showAllFilters = false
    @State private var isSearchFocused = false
    @State private var availableWidth: CGFloat = 0
    @FocusState private var searchFieldFocused: Bool
    
    // Computed properties
    private var hasActiveFilters: Bool {
        !searchText.isEmpty || !selectedFilters.contains("all")
    }
    
    private var activeFilterCount: Int {
        searchText.isEmpty ? selectedFilters.filter { $0 != "all" }.count : selectedFilters.filter { $0 != "all" }.count + 1
    }
    
    private var adaptiveMaxVisibleFilters: Int {
        guard config.adaptiveWidth && availableWidth > 0 else {
            return config.maxVisibleFilters
        }
        
        let moreButtonWidth: CGFloat = 80 // Estimated width for "More" button
        let chipSpacing: CGFloat = 8
        let horizontalPadding: CGFloat = 32 // Total horizontal padding from container
        
        var usedWidth: CGFloat = horizontalPadding
        var visibleCount = 0
        
        for (index, option) in filterOptions.enumerated() {
            let estimatedChipWidth = estimateChipWidth(for: option.displayName)
            
            // Check if we can fit this chip
            if usedWidth + estimatedChipWidth <= availableWidth {
                usedWidth += estimatedChipWidth + chipSpacing
                visibleCount += 1
            } else {
                // If there are more filters, reserve space for "More" button
                if index < filterOptions.count - 1 {
                    if usedWidth + moreButtonWidth <= availableWidth {
                        break
                    } else if visibleCount > 0 {
                        visibleCount -= 1 // Remove last filter to make room for "More" button
                    }
                }
                break
            }
        }
        
        // Ensure we show at least 1 filter (the "All" filter)
        return max(1, visibleCount)
    }
    
    private var visibleFilterOptions: [FilterOption] {
        if showAllFilters || config.compactMode {
            return filterOptions
        } else {
            return Array(filterOptions.prefix(adaptiveMaxVisibleFilters))
        }
    }
    
    private var hasMoreFilters: Bool {
        let maxVisible = config.adaptiveWidth ? adaptiveMaxVisibleFilters : config.maxVisibleFilters
        return filterOptions.count > maxVisible && !config.compactMode
    }
    
    // Helper method to estimate chip width based on text content
    private func estimateChipWidth(for text: String) -> CGFloat {
        let basePadding: CGFloat = 32 // 16px horizontal padding on each side
        let characterWidth: CGFloat = 9 // Slightly more accurate character width for system font
        let extraBuffer: CGFloat = 10 // Buffer for checkmark icon and safe spacing
        return basePadding + (CGFloat(text.count) * characterWidth) + extraBuffer
    }
    
    var body: some View {
        VStack(spacing: config.compactMode ? 8 : 12) {
            // Search bar
            if config.showSearchBar {
                searchBar
            }
            
            // Filter chips with adaptive width detection
            GeometryReader { geometry in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(visibleFilterOptions, id: \.id) { option in
                            MultiSelectFilterChip(
                                title: option.displayName,
                                value: option.id,
                                selections: $selectedFilters,
                                color: option.color ?? .blue
                            )
                        }
                        
                        // Show more/less button
                        if hasMoreFilters {
                            Button(action: { 
                                withAnimation(.easeInOut(duration: 0.3)) {
                                    showAllFilters.toggle()
                                }
                            }) {
                                HStack(spacing: 4) {
                                    Image(systemName: showAllFilters ? "chevron.up" : "chevron.down")
                                        .font(.caption2)
                                    Text(showAllFilters ? "Less" : "More")
                                        .font(.caption)
                                        .fontWeight(.medium)
                                }
                                .padding(.horizontal, 8)
                                .padding(.vertical, 6)
                                .background(Color(.systemGray5))
                                .foregroundColor(.secondary)
                                .cornerRadius(6)
                            }
                            .transition(.scale.combined(with: .opacity))
                        }
                    }
                    .padding(.horizontal)
                }
                .onAppear {
                    availableWidth = geometry.size.width
                }
                .onChange(of: geometry.size.width) { oldValue, newValue in
                    availableWidth = newValue
                }
            }
            .frame(height: 50) // Fixed height for the filter chips area
            
            // Active filter summary
            if hasActiveFilters && config.showActiveFilterSummary {
                activeFilterSummary
            }
        }
    }
    
    // MARK: - Search Bar
    private var searchBar: some View {
        HStack(spacing: 8) {
            // Search field
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(.secondary)
                    .font(.subheadline)
                
                TextField(config.placeholder, text: $searchText)
                    .focused($searchFieldFocused)
                    .textFieldStyle(PlainTextFieldStyle())
                    .onTapGesture {
                        isSearchFocused = true
                    }
                
                // Clear button
                if !searchText.isEmpty {
                    Button(action: { 
                        withAnimation(.easeInOut(duration: 0.2)) {
                            searchText = ""
                        }
                    }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(.secondary)
                            .font(.subheadline)
                    }
                    .transition(.scale.combined(with: .opacity))
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, config.compactMode ? 8 : 10)
            .background(Color(.systemGray6))
            .cornerRadius(config.compactMode ? 8 : 10)
            .overlay(
                RoundedRectangle(cornerRadius: config.compactMode ? 8 : 10)
                    .stroke(isSearchFocused ? Color.blue : Color.clear, lineWidth: 1)
            )
            .onTapGesture {
                searchFieldFocused = true
                isSearchFocused = true
            }
            .onChange(of: searchFieldFocused) { oldValue, newValue in
                isSearchFocused = newValue
            }
        }
        .padding(.horizontal)
    }
    
    // MARK: - Active Filter Summary
    private var activeFilterSummary: some View {
        HStack {
            Image(systemName: "line.3.horizontal.decrease.circle")
                .font(.caption)
                .foregroundColor(.blue)
            
            Text("\(activeFilterCount) filter\(activeFilterCount == 1 ? "" : "s") active")
                .font(.caption)
                .foregroundColor(.blue)
                .fontWeight(.medium)
            
            Spacer()
            
            // Quick clear button
            Button(action: clearAllFilters) {
                Text("Clear all")
                    .font(.caption)
                    .foregroundColor(.red)
                    .fontWeight(.medium)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Color.blue.opacity(0.1))
        .cornerRadius(8)
        .padding(.horizontal)
        .transition(.move(edge: .top).combined(with: .opacity))
    }
    
    // MARK: - Helper Methods
    
    private func clearAllFilters() {
        withAnimation(.easeInOut(duration: 0.2)) {
            selectedFilters = ["all"]
            searchText = ""
        }
    }
}

// MARK: - Filter Bar View Modifier
extension View {
    /// Apply a filter bar to any view
    func withFilterBar(
        searchText: Binding<String>,
        selectedFilters: Binding<Set<String>>,
        config: FilterBarConfig = .default,
        filterOptions: [FilterOption] = []
    ) -> some View {
        VStack(spacing: 0) {
            FilterBarComponent(
                searchText: searchText,
                selectedFilters: selectedFilters,
                config: config,
                filterOptions: filterOptions
            )
            
            self
        }
    }
}

// MARK: - Preview
#if DEBUG
struct FilterBarComponent_Previews: PreviewProvider {
    @State static var searchText = ""
    @State static var selectedFilters: Set<String> = ["all"]
    
    static var previews: some View {
        VStack {
            FilterBarComponent(
                searchText: $searchText,
                selectedFilters: $selectedFilters,
                config: .strategicGoals,
                filterOptions: FilterOption.strategicGoalFilters
            )
            
            Spacer()
            
            Text("Search: '\(searchText)'")
            Text("Filters: \(Array(selectedFilters).joined(separator: ", "))")
        }
    }
}
#endif 