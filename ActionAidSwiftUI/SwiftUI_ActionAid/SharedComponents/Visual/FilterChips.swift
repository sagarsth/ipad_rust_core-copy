import SwiftUI

// MARK: - Filter Chip (Legacy - kept for backward compatibility)
struct FilterChip: View {
    let title: String
    let value: String
    @Binding var selection: String
    var color: Color = .blue
    
    var isSelected: Bool {
        selection == value
    }
    
    var body: some View {
        Button(action: { selection = value }) {
            Text(title)
                .font(.subheadline)
                .fontWeight(isSelected ? .medium : .regular)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(isSelected ? color : Color(.systemGray6))
                .foregroundColor(isSelected ? .white : .primary)
                .cornerRadius(20)
        }
    }
}

// MARK: - Multi-Select Filter Chip (OR Gate Logic)
struct MultiSelectFilterChip: View {
    let title: String
    let value: String
    @Binding var selections: Set<String>
    var color: Color = .blue
    
    var isSelected: Bool {
        selections.contains(value)
    }
    
    var body: some View {
        Button(action: {
            if value == "all" {
                // Special handling for "All" - when clicked, clear all other selections
                if selections.contains("all") {
                    // If "All" is already selected, do nothing (keep it selected)
                    return
                } else {
                    // Select "All" and clear other selections
                    selections = ["all"]
                }
            } else {
                // Handle individual status selections
                if selections.contains("all") {
                    // If "All" was selected, clear it and select this specific status
                    selections = [value]
                } else {
                    // Toggle this specific status
                    if selections.contains(value) {
                        selections.remove(value)
                        // If no statuses are selected, select "All"
                        if selections.isEmpty {
                            selections = ["all"]
                        }
                    } else {
                        selections.insert(value)
                    }
                }
            }
        }) {
            HStack(spacing: 4) {
                Text(title)
                    .font(.subheadline)
                    .fontWeight(isSelected ? .medium : .regular)
                
                // Show selection indicator for multi-select (except for "All")
                if isSelected && value != "all" && !selections.contains("all") {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption2)
                        .foregroundColor(isSelected ? .white : color)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(isSelected ? color : Color(.systemGray6))
            .foregroundColor(isSelected ? .white : .primary)
            .cornerRadius(20)
            .overlay(
                // Add border for multi-selected items
                RoundedRectangle(cornerRadius: 20)
                    .stroke(
                        isSelected && !selections.contains("all") ? color.opacity(0.8) : Color.clear,
                        lineWidth: 1
                    )
            )
        }
    }
} 

// MARK: - Grouped Filter Chip with Popup Selection
struct GroupedFilterChip: View {
    let group: GroupedFilterOption
    @Binding var selectedSubOptions: Set<String>
    @State private var showSelectionPopup = false
    
    // Computed properties
    private var isActive: Bool {
        if group.id == "all" {
            return selectedSubOptions.isEmpty || selectedSubOptions.contains("all")
        }
        
        // Special handling for disability_type group: show as active if "disability_with" is selected
        // and no specific types are chosen (meaning all types are implicitly included)
        if group.id == "disability_type" {
            let hasSpecificTypes = hasActiveSubOptions
            let hasGeneralDisability = selectedSubOptions.contains("disability_with")
            return hasSpecificTypes || hasGeneralDisability
        }
        
        return hasActiveSubOptions
    }
    
    private var hasActiveSubOptions: Bool {
        group.subOptions.contains { selectedSubOptions.contains($0.id) }
    }
    
    private var activeCount: Int {
        group.subOptions.filter { selectedSubOptions.contains($0.id) }.count
    }
    
    private var displayText: String {
        if group.id == "all" {
            return group.displayName
        }
        
        // Special handling for disability_type group
        if group.id == "disability_type" {
            let hasSpecificTypes = hasActiveSubOptions
            let hasGeneralDisability = selectedSubOptions.contains("disability_with")
            
            if hasSpecificTypes {
                if activeCount == 1 {
                    // Show the single selected type
                    if let selectedOption = group.subOptions.first(where: { selectedSubOptions.contains($0.id) }) {
                        return selectedOption.displayName
                    }
                }
                return "\(group.displayName) (\(activeCount))"
            } else if hasGeneralDisability {
                // General disability selected but no specific types
                return "\(group.displayName) (All)"
            }
        }
        
        if hasActiveSubOptions {
            if activeCount == 1 {
                // Show the single selected option name
                if let selectedOption = group.subOptions.first(where: { selectedSubOptions.contains($0.id) }) {
                    return selectedOption.displayName
                }
            }
            return "\(group.displayName) (\(activeCount))"
        }
        
        return group.displayName
    }
    
    var body: some View {
        Button(action: {
            if group.id == "all" {
                // Clear all selections when "All" is tapped
                selectedSubOptions = ["all"]
            } else {
                // Show popup for grouped options
                showSelectionPopup = true
            }
        }) {
            HStack(spacing: 4) {
                if let icon = group.icon {
                    Image(systemName: icon)
                        .font(.caption2)
                }
                
                Text(displayText)
                    .font(.subheadline)
                    .fontWeight(isActive ? .medium : .regular)
                
                // Show dropdown indicator for grouped options
                if !group.subOptions.isEmpty {
                    Image(systemName: "chevron.down")
                        .font(.caption2)
                        .foregroundColor(isActive ? .white : (group.color ?? .blue))
                }
                
                // Show selection indicator
                if isActive && group.id != "all" && hasActiveSubOptions {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption2)
                        .foregroundColor(isActive ? .white : (group.color ?? .blue))
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background(isActive ? (group.color ?? .blue) : Color(.systemGray6))
            .foregroundColor(isActive ? .white : .primary)
            .cornerRadius(20)
            .overlay(
                RoundedRectangle(cornerRadius: 20)
                    .stroke(
                        isActive ? (group.color ?? .blue).opacity(0.8) : Color.clear,
                        lineWidth: 1
                    )
            )
        }
        .sheet(isPresented: $showSelectionPopup) {
            GroupedFilterSelectionPopup(
                group: group,
                selectedSubOptions: $selectedSubOptions
            )
        }
    }
}

// MARK: - Grouped Filter Selection Popup
struct GroupedFilterSelectionPopup: View {
    let group: GroupedFilterOption
    @Binding var selectedSubOptions: Set<String>
    @Environment(\.dismiss) var dismiss
    
    // Local state for the popup
    @State private var localSelections: Set<String> = []
    
    var body: some View {
        NavigationView {
            VStack(spacing: 0) {
                // Header info
                VStack(spacing: 8) {
                    HStack {
                        if let icon = group.icon {
                            Image(systemName: icon)
                                .font(.title2)
                                .foregroundColor(group.color ?? .blue)
                        }
                        Text(group.displayName)
                            .font(.title2)
                            .fontWeight(.semibold)
                        Spacer()
                    }
                    
                    let descriptionText = group.id == "disability_type" 
                        ? "Select specific disability types to filter. All types are initially selected when 'With Disability' is chosen from the Disability filter."
                        : "Select one or more options. Use OR logic within this category."
                    
                    Text(descriptionText)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding()
                .background(Color(.systemGray6))
                
                // Selection options
                List {
                    ForEach(group.subOptions) { subOption in
                        GroupedFilterOptionRow(
                            subOption: subOption,
                            isSelected: localSelections.contains(subOption.id),
                            onTap: { 
                                toggleSubOption(subOption.id)
                            }
                        )
                    }
                }
                .listStyle(PlainListStyle())
                
                // Bottom action buttons
                HStack(spacing: 12) {
                    Button("Clear All") {
                        localSelections.removeAll()
                    }
                    .foregroundColor(.red)
                    .disabled(localSelections.isEmpty)
                    
                    Spacer()
                    
                    Button("Cancel") {
                        dismiss()
                    }
                    .foregroundColor(.secondary)
                    
                    Button("Apply") {
                        applySelections()
                        dismiss()
                    }
                    .foregroundColor(.white)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(group.color ?? .blue)
                    .cornerRadius(8)
                }
                .padding()
                .background(Color(.systemGray6))
            }
            .navigationTitle("Filter by \(group.displayName)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Apply") {
                        applySelections()
                        dismiss()
                    }
                    .fontWeight(.semibold)
                }
            }
        }
        .onAppear {
            // Initialize local selections with current state
            initializeLocalSelections()
        }
    }
    
    private func initializeLocalSelections() {
        // Load current selections for this group
        localSelections = Set(group.subOptions.compactMap { subOption in
            selectedSubOptions.contains(subOption.id) ? subOption.id : nil
        })
        
        // Special handling for disability group: if "disability_with" is selected but no specific types,
        // auto-select all types to give user a starting point
        if group.id == "disability_type" && selectedSubOptions.contains("disability_with") && localSelections.isEmpty {
            // Auto-select all disability types when "with disability" is chosen
            localSelections = Set(group.subOptions.map(\.id))
        }
    }
    
    private func toggleSubOption(_ id: String) {
        if localSelections.contains(id) {
            localSelections.remove(id)
        } else {
            localSelections.insert(id)
        }
    }
    
    private func applySelections() {
        // Remove previous selections for this group
        let currentGroupOptionIds = Set(group.subOptions.map(\.id))
        selectedSubOptions = selectedSubOptions.subtracting(currentGroupOptionIds)
        
        // Add new selections
        selectedSubOptions = selectedSubOptions.union(localSelections)
        
        // Remove "all" if any specific filters are selected
        if !localSelections.isEmpty {
            selectedSubOptions.remove("all")
        }
        
        // If no specific filters are selected, add "all"
        if selectedSubOptions.isEmpty {
            selectedSubOptions.insert("all")
        }
    }
}

// MARK: - Grouped Filter Option Row
struct GroupedFilterOptionRow: View {
    let subOption: FilterSubOption
    let isSelected: Bool
    let onTap: () -> Void
    
    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                // Icon
                if let icon = subOption.icon {
                    Image(systemName: icon)
                        .font(.title3)
                        .foregroundColor(subOption.color ?? .blue)
                        .frame(width: 24, height: 24)
                }
                
                // Title
                Text(subOption.displayName)
                    .font(.subheadline)
                    .foregroundColor(.primary)
                
                Spacer()
                
                // Selection indicator
                Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                    .font(.title3)
                    .foregroundColor(isSelected ? (subOption.color ?? .blue) : .secondary)
            }
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(PlainButtonStyle())
    }
} 