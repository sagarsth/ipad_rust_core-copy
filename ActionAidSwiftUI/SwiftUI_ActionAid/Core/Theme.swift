//
//  Theme.swift
//  ActionAid SwiftUI
//
//  Design system constants matching TypeScript UI
//

import SwiftUI

// MARK: - Theme Namespace
enum Theme {
    
    // MARK: - Colors
    enum Colors {
        // Primary colors matching TypeScript
        static let primary = Color.blue
        static let secondary = Color(.systemGray)
        static let success = Color.green
        static let warning = Color.orange
        static let danger = Color.red
        static let info = Color.blue
        
        // Domain-specific colors
        static let users = Color.blue
        static let livelihoods = Color.green
        static let strategicGoals = Color.purple
        static let projects = Color.orange
        static let activities = Color.red
        static let workshops = Color.indigo
        static let participants = Color.teal
        static let donors = Color.yellow
        static let funding = Color(.systemGreen)
        
        // Background colors
        static let background = Color(.systemBackground)
        static let secondaryBackground = Color(.secondarySystemBackground)
        static let tertiaryBackground = Color(.tertiarySystemBackground)
        static let groupedBackground = Color(.systemGroupedBackground)
        
        // System grays
        static let gray1 = Color(.systemGray)
        static let gray2 = Color(.systemGray2)
        static let gray3 = Color(.systemGray3)
        static let gray4 = Color(.systemGray4)
        static let gray5 = Color(.systemGray5)
        static let gray6 = Color(.systemGray6)
        
        // Status colors
        static let statusActive = Color.green
        static let statusPending = Color.orange
        static let statusInactive = Color.gray
        static let statusCompleted = Color.blue
        static let statusCancelled = Color.red
        static let statusOnTrack = Color.green
        static let statusAtRisk = Color.orange
        static let statusBehind = Color.red
    }
    
    // MARK: - Spacing
    enum Spacing {
        static let xxSmall: CGFloat = 4
        static let xSmall: CGFloat = 8
        static let small: CGFloat = 12
        static let medium: CGFloat = 16
        static let large: CGFloat = 20
        static let xLarge: CGFloat = 24
        static let xxLarge: CGFloat = 32
        static let xxxLarge: CGFloat = 40
    }
    
    // MARK: - Corner Radius
    enum CornerRadius {
        static let small: CGFloat = 4
        static let medium: CGFloat = 8
        static let large: CGFloat = 12
        static let xLarge: CGFloat = 16
        static let pill: CGFloat = 999
    }
    
    // MARK: - Shadow
    enum Shadow {
        static let small = ShadowStyle(
            color: Color.black.opacity(0.05),
            radius: 3,
            x: 0,
            y: 2
        )
        
        static let medium = ShadowStyle(
            color: Color.black.opacity(0.1),
            radius: 5,
            x: 0,
            y: 3
        )
        
        static let large = ShadowStyle(
            color: Color.black.opacity(0.15),
            radius: 10,
            x: 0,
            y: 5
        )
        
        struct ShadowStyle {
            let color: Color
            let radius: CGFloat
            let x: CGFloat
            let y: CGFloat
        }
    }
    
    // MARK: - Typography
    enum Typography {
        static let largeTitle = Font.largeTitle
        static let title = Font.title
        static let title2 = Font.title2
        static let title3 = Font.title3
        static let headline = Font.headline
        static let subheadline = Font.subheadline
        static let body = Font.body
        static let callout = Font.callout
        static let footnote = Font.footnote
        static let caption = Font.caption
        static let caption2 = Font.caption2
        
        // Custom weights
        static let headlineBold = Font.headline.weight(.bold)
        static let bodyMedium = Font.body.weight(.medium)
        static let captionMedium = Font.caption.weight(.medium)
    }
    
    // MARK: - Icons
    enum Icons {
        // Navigation
        static let back = "arrow.left"
        static let forward = "arrow.right"
        static let up = "arrow.up"
        static let down = "arrow.down"
        
        // Actions
        static let add = "plus"
        static let addCircle = "plus.circle.fill"
        static let edit = "pencil"
        static let delete = "trash"
        static let save = "square.and.arrow.down"
        static let share = "square.and.arrow.up"
        static let search = "magnifyingglass"
        static let filter = "line.3.horizontal.decrease.circle"
        static let sort = "arrow.up.arrow.down"
        static let more = "ellipsis"
        static let moreCircle = "ellipsis.circle"
        
        // Status
        static let success = "checkmark.circle.fill"
        static let error = "exclamationmark.triangle.fill"
        static let warning = "exclamationmark.circle.fill"
        static let info = "info.circle.fill"
        
        // Domains
        static let users = "person.2"
        static let livelihoods = "heart"
        static let strategicGoals = "target"
        static let projects = "folder"
        static let activities = "chart.line.uptrend.xyaxis"
        static let workshops = "graduationcap"
        static let participants = "person.crop.circle.badge.checkmark"
        static let donors = "building.2"
        static let funding = "dollarsign.circle"
        
        // Documents
        static let document = "doc"
        static let documentFill = "doc.fill"
        static let pdf = "doc.text.fill"
        static let image = "photo.fill"
        static let spreadsheet = "tablecells.fill"
        
        // Misc
        static let calendar = "calendar"
        static let clock = "clock"
        static let location = "location"
        static let phone = "phone"
        static let email = "envelope"
        static let link = "link"
        static let attachment = "paperclip"
    }
    
    // MARK: - Animation
    enum Animation {
        static let fast = SwiftUI.Animation.easeInOut(duration: 0.2)
        static let medium = SwiftUI.Animation.easeInOut(duration: 0.3)
        static let slow = SwiftUI.Animation.easeInOut(duration: 0.5)
        static let spring = SwiftUI.Animation.spring(response: 0.4, dampingFraction: 0.8)
    }
    
    // MARK: - Layout
    enum Layout {
        static let gridColumns = [
            GridItem(.flexible()),
            GridItem(.flexible()),
            GridItem(.flexible())
        ]
        
        static let twoColumnGrid = [
            GridItem(.flexible()),
            GridItem(.flexible())
        ]
        
        static let defaultPadding = EdgeInsets(
            top: Spacing.medium,
            leading: Spacing.medium,
            bottom: Spacing.medium,
            trailing: Spacing.medium
        )
    }
}

// MARK: - View Extensions for Theme
extension View {
    func cardStyle(
        padding: CGFloat = Theme.Spacing.medium,
        cornerRadius: CGFloat = Theme.CornerRadius.large
    ) -> some View {
        self
            .padding(padding)
            .background(Theme.Colors.background)
            .cornerRadius(cornerRadius)
            .shadow(
                color: Theme.Shadow.small.color,
                radius: Theme.Shadow.small.radius,
                x: Theme.Shadow.small.x,
                y: Theme.Shadow.small.y
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .stroke(Theme.Colors.gray5, lineWidth: 1)
            )
    }
    
    func sectionBackground() -> some View {
        self
            .padding(Theme.Spacing.medium)
            .background(Theme.Colors.gray6)
            .cornerRadius(Theme.CornerRadius.large)
    }
}

// MARK: - Domain Colors Helper
extension Theme.Colors {
    static func domainColor(for domain: String) -> Color {
        switch domain.lowercased() {
        case "users": return users
        case "livelihoods": return livelihoods
        case "strategic_goals", "strategicgoals": return strategicGoals
        case "projects": return projects
        case "activities": return activities
        case "workshops": return workshops
        case "participants": return participants
        case "donors": return donors
        case "funding": return funding
        default: return primary
        }
    }
}

// MARK: - Status Colors Helper
extension Theme.Colors {
    static func statusColor(for status: String) -> Color {
        switch status.lowercased() {
        case "active": return statusActive
        case "pending": return statusPending
        case "inactive": return statusInactive
        case "completed": return statusCompleted
        case "cancelled": return statusCancelled
        case "on track", "ontrack": return statusOnTrack
        case "at risk", "atrisk": return statusAtRisk
        case "behind": return statusBehind
        default: return gray3
        }
    }
}