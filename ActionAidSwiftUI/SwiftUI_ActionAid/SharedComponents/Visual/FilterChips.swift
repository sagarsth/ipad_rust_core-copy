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