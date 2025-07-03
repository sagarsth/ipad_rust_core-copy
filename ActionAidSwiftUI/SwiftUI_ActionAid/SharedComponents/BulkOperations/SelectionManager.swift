//
//  SelectionManager.swift
//  ActionAid SwiftUI
//
//  Generic selection state manager for bulk operations
//

import Foundation
import Combine

/// Generic selection manager for handling bulk operations across all domains
@MainActor
class SelectionManager: ObservableObject {
    @Published var isInSelectionMode = false
    @Published var selectedItems: Set<String> = []
    @Published var isLoadingFilteredIds = false
    
    var hasSelection: Bool {
        !selectedItems.isEmpty
    }
    
    var selectedCount: Int {
        selectedItems.count
    }
    
    /// Enter selection mode
    func enterSelectionMode() {
        isInSelectionMode = true
    }
    
    /// Exit selection mode and clear selection
    func exitSelectionMode() {
        isInSelectionMode = false
        selectedItems.removeAll()
    }
    
    /// Clear selection without exiting selection mode
    func clearSelection() {
        selectedItems.removeAll()
        isInSelectionMode = false
    }
    
    /// Select a single item
    func selectItem(_ id: String) {
        selectedItems.insert(id)
        if !isInSelectionMode {
            isInSelectionMode = true
        }
    }
    
    /// Deselect a single item
    func deselectItem(_ id: String) {
        selectedItems.remove(id)
        // Exit selection mode if no items are selected
        if selectedItems.isEmpty {
            isInSelectionMode = false
        }
    }
    
    /// Toggle selection of an item
    func toggleSelection(_ id: String) {
        if selectedItems.contains(id) {
            deselectItem(id)
        } else {
            selectItem(id)
        }
    }
    
    /// Select multiple items at once
    func selectItems(_ ids: Set<String>) {
        selectedItems = ids
        if !ids.isEmpty && !isInSelectionMode {
            isInSelectionMode = true
        } else if ids.isEmpty {
            isInSelectionMode = false
        }
    }
    
    /// Add multiple items to current selection
    func addToSelection(_ ids: Set<String>) {
        selectedItems = selectedItems.union(ids)
        if !selectedItems.isEmpty && !isInSelectionMode {
            isInSelectionMode = true
        }
    }
    
    /// Remove multiple items from current selection
    func removeFromSelection(_ ids: Set<String>) {
        selectedItems = selectedItems.subtracting(ids)
        if selectedItems.isEmpty {
            isInSelectionMode = false
        }
    }
    
    /// Check if an item is selected
    func isSelected(_ id: String) -> Bool {
        selectedItems.contains(id)
    }
    
    /// Select all items from a list
    func selectAll<T: Identifiable>(_ items: [T]) where T.ID == String {
        let ids = Set(items.map(\.id))
        selectItems(ids)
    }
    
    /// Deselect all items
    func deselectAll() {
        clearSelection()
    }
} 