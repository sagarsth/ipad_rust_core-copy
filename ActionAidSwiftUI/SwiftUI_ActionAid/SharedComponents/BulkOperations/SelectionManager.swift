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
    @Published var isSelectAllActive = false { // Track if "Select All" is currently active
        didSet {
            print("🟦 [SELECTION] isSelectAllActive changed: \(oldValue) → \(isSelectAllActive)")
        }
    }
    
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
        isSelectAllActive = false
    }
    
    /// Clear selection without exiting selection mode
    func clearSelection() {
        print("🟦 [SELECTION] clearSelection called")
        selectedItems.removeAll()
        isInSelectionMode = false
        isSelectAllActive = false
        print("🟦 [SELECTION] clearSelection completed")
    }
    
    /// Select a single item
    func selectItem(_ id: String) {
        selectedItems.insert(id)
        if !isInSelectionMode {
            isInSelectionMode = true
        }
        // Individual selection disables "Select All" mode
        isSelectAllActive = false
    }
    
    /// Deselect a single item
    func deselectItem(_ id: String) {
        selectedItems.remove(id)
        // Exit selection mode if no items are selected
        if selectedItems.isEmpty {
            isInSelectionMode = false
            isSelectAllActive = false
        } else {
            // Individual deselection disables "Select All" mode
            isSelectAllActive = false
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
    
    /// Select multiple items at once (can be used for filter-based selection)
    func selectItems(_ ids: Set<String>) {
        selectedItems = ids
        if !ids.isEmpty && !isInSelectionMode {
            isInSelectionMode = true
        } else if ids.isEmpty {
            isInSelectionMode = false
            isSelectAllActive = false
        }
    }
    
    /// Select all specified items and mark as "Select All" mode
    func selectAllItems(_ items: Set<String>) {
        print("🟦 [SELECTION] selectAllItems called with \(items.count) items")
        selectedItems = items
        isSelectAllActive = true
        if !items.isEmpty && !isInSelectionMode {
            isInSelectionMode = true
        }
        print("🟦 [SELECTION] selectAllItems completed, isSelectAllActive: \(isSelectAllActive)")
    }
    
    /// Add multiple items to current selection
    func addToSelection(_ ids: Set<String>) {
        selectedItems = selectedItems.union(ids)
        if !selectedItems.isEmpty && !isInSelectionMode {
            isInSelectionMode = true
        }
        // Adding to selection disables "Select All" mode unless it was already active
        // This preserves "Select All" behavior when filters change
    }
    
    /// Remove multiple items from current selection
    func removeFromSelection(_ ids: Set<String>) {
        selectedItems = selectedItems.subtracting(ids)
        if selectedItems.isEmpty {
            isInSelectionMode = false
            isSelectAllActive = false
        }
        // Removing from selection disables "Select All" mode
        isSelectAllActive = false
    }
    
    /// Check if an item is selected
    func isSelected(_ id: String) -> Bool {
        selectedItems.contains(id)
    }
    
    /// Select all items from a list
    func selectAll<T: Identifiable>(_ items: [T]) where T.ID == String {
        let ids = Set(items.map(\.id))
        selectAllItems(ids)
    }
    
    /// Deselect all items
    func deselectAll() {
        clearSelection()
    }
    
    /// Update selection when filters change while "Select All" is active
    func updateSelectionForFilterChange(_ newIds: Set<String>) {
        if isSelectAllActive {
            // If "Select All" is active, update selection to include all new filtered items
            selectedItems = newIds
            // Keep isSelectAllActive = true to maintain "Select All" state
        }
        // If "Select All" is not active, don't change selection
    }
} 