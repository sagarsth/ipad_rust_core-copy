//
//  SelectionActionBar.swift
//  ActionAid SwiftUI
//
//  Generic selection action bar for bulk operations
//

import SwiftUI

/// Generic selection action bar for bulk operations across all domains
struct SelectionActionBar: View {
    let selectedCount: Int
    let userRole: String?
    let isPerformingBulkOperation: Bool
    let onClearSelection: () -> Void
    let onExport: () -> Void
    let onDelete: (() -> Void)?  // Optional - only shown if provided and user is admin
    
    var body: some View {
        VStack {
            Spacer()
            HStack(spacing: 20) {
                // Clear selection
                Button(action: {
                    withAnimation {
                        onClearSelection()
                    }
                }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.title2)
                        .foregroundColor(.white)
                        .padding()
                        .background(Color.gray.opacity(0.8))
                        .clipShape(Circle())
                }
                
                Spacer()
                
                // Export button
                Button(action: onExport) {
                    Image(systemName: "square.and.arrow.up.fill")
                        .font(.title2)
                        .foregroundColor(.white)
                        .padding()
                        .background(Color.blue.opacity(0.8))
                        .clipShape(Circle())
                }
                
                // Selection count indicator
                Text("\(selectedCount)")
                    .font(.headline)
                    .fontWeight(.bold)
                    .foregroundColor(.white)
                    .frame(minWidth: 30)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .background(Color.blue.opacity(0.8))
                    .clipShape(Capsule())
                
                // Delete button (only for admins and if onDelete is provided)
                if let onDelete = onDelete, userRole?.lowercased() == "admin" {
                    Button(action: onDelete) {
                        Image(systemName: "trash.fill")
                            .font(.title2)
                            .foregroundColor(.white)
                            .padding()
                            .background(Color.red.opacity(0.8))
                            .clipShape(Circle())
                    }
                    .disabled(isPerformingBulkOperation)
                }
                
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
            .liquidGlassEffect(cornerRadius: 25)
            .padding(.horizontal, 20)
            .padding(.bottom, 30)
        }
    }
}

// SelectionManager is defined in a separate file: SelectionManager.swift

#Preview {
    SelectionActionBar(
        selectedCount: 5,
        userRole: "admin",
        isPerformingBulkOperation: false,
        onClearSelection: {},
        onExport: {},
        onDelete: {}
    )
} 