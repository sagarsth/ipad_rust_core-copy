//
//  UIComponents.swift
//  ActionAid SwiftUI
//
//  Reusable UI components matching TypeScript design
//

import SwiftUI

// MARK: - Common Badge Component (removed due to duplicate definition in view files)

// MARK: - Stats Card Component (removed due to duplicate definition in view files)

// MARK: - Empty State View
struct EmptyStateView: View {
    let icon: String
    let title: String
    let message: String?
    let actionTitle: String?
    let action: (() -> Void)?
    
    init(
        icon: String,
        title: String,
        message: String? = nil,
        actionTitle: String? = nil,
        action: (() -> Void)? = nil
    ) {
        self.icon = icon
        self.title = title
        self.message = message
        self.actionTitle = actionTitle
        self.action = action
    }
    
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: icon)
                .font(.system(size: 60))
                .foregroundColor(.secondary)
            
            Text(title)
                .font(.headline)
                .foregroundColor(.secondary)
            
            if let message = message {
                Text(message)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
            
            if let actionTitle = actionTitle, let action = action {
                Button(action: action) {
                    Text(actionTitle)
                        .font(.subheadline)
                        .fontWeight(.medium)
                }
                .buttonStyle(.bordered)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

// MARK: - Search Bar Component
struct SearchBar: View {
    @Binding var text: String
    var placeholder: String = "Search..."
    var onSubmit: (() -> Void)? = nil
    
    var body: some View {
        HStack {
            Image(systemName: "magnifyingglass")
                .foregroundColor(.secondary)
            
            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .onSubmit {
                    onSubmit?()
                }
            
            if !text.isEmpty {
                Button(action: { text = "" }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(10)
        .background(Color(.systemGray6))
        .cornerRadius(8)
    }
}

// MARK: - Section Header Component
struct SectionHeader: View {
    let title: String
    let subtitle: String?
    let actionTitle: String?
    let action: (() -> Void)?
    let icon: String?
    
    init(
        title: String,
        subtitle: String? = nil,
        actionTitle: String? = nil,
        action: (() -> Void)? = nil,
        icon: String? = nil
    ) {
        self.title = title
        self.subtitle = subtitle
        self.actionTitle = actionTitle
        self.action = action
        self.icon = icon
    }
    
    var body: some View {
        HStack(alignment: .center) {
            if let icon = icon {
                Image(systemName: icon)
                    .foregroundColor(.accentColor)
                    .font(.subheadline)
            }
            
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.headline)
                
                if let subtitle = subtitle {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            
            Spacer()
            
            if let actionTitle = actionTitle, let action = action {
                Button(action: action) {
                    Text(actionTitle)
                        .font(.subheadline)
                        .fontWeight(.medium)
                }
            }
        }
    }
}

// MARK: - Loading View Component
struct LoadingView: View {
    let message: String
    
    var body: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)
            Text(message)
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Error View Component
struct ErrorView: View {
    let error: String
    let retry: (() -> Void)?
    
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 50))
                .foregroundColor(.red)
            
            Text("Error")
                .font(.headline)
            
            Text(error)
                .font(.subheadline)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal)
            
            if let retry = retry {
                Button("Retry", action: retry)
                    .buttonStyle(.borderedProminent)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

// MARK: - Card Container Component
struct CardContainer<Content: View>: View {
    let content: Content
    var padding: CGFloat = 16
    var cornerRadius: CGFloat = 12
    
    init(
        padding: CGFloat = 16,
        cornerRadius: CGFloat = 12,
        @ViewBuilder content: () -> Content
    ) {
        self.padding = padding
        self.cornerRadius = cornerRadius
        self.content = content()
    }
    
    var body: some View {
        content
            .padding(padding)
            .background(Color(.systemBackground))
            .cornerRadius(cornerRadius)
            .shadow(color: Color.black.opacity(0.05), radius: 3, x: 0, y: 2)
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .stroke(Color(.systemGray5), lineWidth: 1)
            )
    }
}

// MARK: - Action Button Component
struct ActionButton: View {
    let title: String
    let icon: String?
    let action: () -> Void
    var style: ActionButtonStyle = .primary
    var size: ActionButtonSize = .regular
    
    enum ActionButtonStyle {
        case primary, secondary, destructive
        
        var backgroundColor: Color {
            switch self {
            case .primary: return .blue
            case .secondary: return Color(.systemGray5)
            case .destructive: return .red
            }
        }
        
        var foregroundColor: Color {
            switch self {
            case .primary, .destructive: return .white
            case .secondary: return .primary
            }
        }
    }
    
    enum ActionButtonSize {
        case small, regular, large
        
        var padding: EdgeInsets {
            switch self {
            case .small: return EdgeInsets(top: 6, leading: 12, bottom: 6, trailing: 12)
            case .regular: return EdgeInsets(top: 10, leading: 16, bottom: 10, trailing: 16)
            case .large: return EdgeInsets(top: 14, leading: 20, bottom: 14, trailing: 20)
            }
        }
        
        var font: Font {
            switch self {
            case .small: return .caption
            case .regular: return .subheadline
            case .large: return .body
            }
        }
    }
    
    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let icon = icon {
                    Image(systemName: icon)
                        .font(size.font)
                }
                Text(title)
                    .font(size.font)
                    .fontWeight(.medium)
            }
            .padding(size.padding)
            .background(style.backgroundColor)
            .foregroundColor(style.foregroundColor)
            .cornerRadius(8)
        }
    }
}

// MARK: - List Row Component
struct ListRow<Leading: View, Trailing: View>: View {
    let leading: Leading
    let title: String
    let subtitle: String?
    let trailing: Trailing
    
    init(
        title: String,
        subtitle: String? = nil,
        @ViewBuilder leading: () -> Leading,
        @ViewBuilder trailing: () -> Trailing
    ) {
        self.title = title
        self.subtitle = subtitle
        self.leading = leading()
        self.trailing = trailing()
    }
    
    var body: some View {
        HStack(spacing: 12) {
            leading
            
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline)
                    .fontWeight(.medium)
                
                if let subtitle = subtitle {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
            
            Spacer()
            
            trailing
        }
        .padding(.vertical, 8)
    }
}

// MARK: - Progress Bar Component
struct ProgressBar: View {
    let value: Double // 0.0 to 1.0
    let color: Color
    var height: CGFloat = 8
    var showPercentage: Bool = false
    
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if showPercentage {
                HStack {
                    Text("Progress")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    Spacer()
                    Text("\(Int(value * 100))%")
                        .font(.caption)
                        .fontWeight(.medium)
                }
            }
            
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: height / 2)
                        .fill(Color(.systemGray5))
                        .frame(height: height)
                    
                    RoundedRectangle(cornerRadius: height / 2)
                        .fill(color)
                        .frame(width: max(0, geometry.size.width * max(0, min(value.isNaN ? 0 : value, 1.0))), height: height)
                }
            }
            .frame(height: height)
        }
    }
}

// MARK: - Filter Menu Component
struct FilterMenu: View {
    let title: String
    @Binding var selection: String
    let options: [(value: String, label: String)]
    
    var selectedLabel: String {
        options.first(where: { $0.value == selection })?.label ?? title
    }
    
    var body: some View {
        Menu {
            ForEach(options, id: \.value) { option in
                Button(option.label) {
                    selection = option.value
                }
            }
        } label: {
            HStack {
                Text(selectedLabel)
                    .font(.subheadline)
                    .lineLimit(1)
                Image(systemName: "chevron.down")
                    .font(.caption)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color(.systemGray6))
            .cornerRadius(8)
        }
    }
}

// MARK: - Detail Row Component (removed due to duplicate definition in view files)

// MARK: - Floating Action Button
struct FloatingActionButton: View {
    let icon: String
    let action: () -> Void
    
    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(.white)
                .frame(width: 56, height: 56)
                .background(Color.blue)
                .clipShape(Circle())
                .shadow(color: Color.black.opacity(0.3), radius: 4, x: 0, y: 2)
        }
    }
}