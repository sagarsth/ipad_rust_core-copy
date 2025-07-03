import SwiftUI

// MARK: - Liquid Glass Effect Extension
// This mimics Apple's future .glassEffect() API using current SwiftUI materials
extension View {
    /// Applies a Liquid Glass effect using current SwiftUI capabilities
    /// Will be replaced with Apple's official .glassEffect() when available in iOS 26
    func liquidGlassEffect(cornerRadius: CGFloat = 25) -> some View {
        self
            .background(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .fill(.thinMaterial)
                    .background(
                        RoundedRectangle(cornerRadius: cornerRadius)
                            .fill(.ultraThinMaterial)
                            .blur(radius: 0.5)
                    )
                    .shadow(color: .black.opacity(0.1), radius: 20, x: 0, y: 10)
                    .shadow(color: .black.opacity(0.05), radius: 2, x: 0, y: 1)
            )
    }
}

// MARK: - Liquid Glass Container
// This mimics Apple's future GlassEffectContainer using current SwiftUI capabilities
struct LiquidGlassContainer<Content: View>: View {
    let cornerRadius: CGFloat
    let content: Content
    
    init(cornerRadius: CGFloat = 25, @ViewBuilder content: () -> Content) {
        self.cornerRadius = cornerRadius
        self.content = content()
    }
    
    var body: some View {
        content
            .liquidGlassEffect(cornerRadius: cornerRadius)
    }
} 