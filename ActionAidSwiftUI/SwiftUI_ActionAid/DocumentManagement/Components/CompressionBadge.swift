//
//  CompressionBadge.swift
//  ActionAid SwiftUI
//
//  Badge component showing document compression status
//

import SwiftUI

// MARK: - Compression Badge

/// Visual indicator for document compression status
struct CompressionBadge: View {
    let status: String
    @State private var isAnimating = false
    
    var statusConfig: (icon: String, color: Color, shouldAnimate: Bool) {
        switch status.uppercased() {
        case "COMPLETED": return ("checkmark.circle.fill", .green, false)
        case "SKIPPED": return ("checkmark.circle.fill", .green, false) // Green checkmark for skipped - already optimized
        case "IN_PROGRESS", "PROCESSING": return ("circle.dotted", .orange, false)
        case "FAILED", "ERROR": return ("exclamationmark.triangle.fill", .red, false)
        case "PENDING": return ("circle", .gray, false)
        default: return ("questionmark.circle", .gray, false)
        }
    }
    
    var body: some View {
        Image(systemName: statusConfig.icon)
            .font(.caption)
            .foregroundColor(statusConfig.color)
            .rotationEffect(.degrees(isAnimating ? 360 : 0))
            .onAppear {
                if statusConfig.shouldAnimate {
                    startAnimation()
                }
            }
            .onChange(of: status) { oldStatus, newStatus in
                if statusConfig.shouldAnimate {
                    startAnimation()
                } else {
                    stopAnimation()
                }
            }
    }
    
    private func startAnimation() {
        withAnimation(.linear(duration: 2.0).repeatForever(autoreverses: false)) {
            isAnimating = true
        }
    }
    
    private func stopAnimation() {
        withAnimation(.easeOut(duration: 0.3)) {
            isAnimating = false
        }
    }
} 