//
//  Extensions.swift
//  ActionAid SwiftUI
//
//  Helpful extensions for SwiftUI and data formatting
//

import SwiftUI
import Foundation

// MARK: - View Extensions
extension View {
    /// Apply conditional modifier
    @ViewBuilder
    func `if`<Transform: View>(
        _ condition: Bool,
        transform: (Self) -> Transform
    ) -> some View {
        if condition {
            transform(self)
        } else {
            self
        }
    }
    
    /// Apply modifier when value is not nil
    @ViewBuilder
    func ifLet<Value, Transform: View>(
        _ value: Value?,
        transform: (Self, Value) -> Transform
    ) -> some View {
        if let value = value {
            transform(self, value)
        } else {
            self
        }
    }
    
    /// Hide keyboard
    func hideKeyboard() {
        UIApplication.shared.sendAction(
            #selector(UIResponder.resignFirstResponder),
            to: nil,
            from: nil,
            for: nil
        )
    }
    
    /// Dismiss keyboard on tap
    func dismissKeyboardOnTap() -> some View {
        self.onTapGesture {
            hideKeyboard()
        }
    }
    
    /// Loading overlay
    func loadingOverlay(_ isLoading: Bool) -> some View {
        self.overlay {
            if isLoading {
                Color.black.opacity(0.3)
                    .ignoresSafeArea()
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                    .scaleEffect(1.2)
            }
        }
    }
    
    /// Error alert modifier
    func errorAlert(
        error: Binding<String?>,
        buttonTitle: String = "OK"
    ) -> some View {
        self.alert(
            "Error",
            isPresented: .constant(error.wrappedValue != nil),
            actions: {
                Button(buttonTitle) {
                    error.wrappedValue = nil
                }
            },
            message: {
                if let errorMessage = error.wrappedValue {
                    Text(errorMessage)
                }
            }
        )
    }
}

// MARK: - Color Extensions
extension Color {
    /// Initialize from hex string
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64
        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            (a, r, g, b) = (1, 1, 1, 0)
        }
        
        self.init(
            .sRGB,
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue:  Double(b) / 255,
            opacity: Double(a) / 255
        )
    }
    
    /// Lighten color
    func lighten(by percentage: CGFloat = 0.2) -> Color {
        return self.adjust(by: abs(percentage))
    }
    
    /// Darken color
    func darken(by percentage: CGFloat = 0.2) -> Color {
        return self.adjust(by: -abs(percentage))
    }
    
    private func adjust(by percentage: CGFloat) -> Color {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(self).getRed(&r, green: &g, blue: &b, alpha: &a)
        return Color(
            red: min(r + percentage, 1.0),
            green: min(g + percentage, 1.0),
            blue: min(b + percentage, 1.0),
            opacity: a
        )
    }
}

// MARK: - String Extensions
extension String {
    /// Check if string is valid email
    var isValidEmail: Bool {
        let emailRegEx = "[A-Z0-9a-z._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,64}"
        let emailPred = NSPredicate(format:"SELF MATCHES %@", emailRegEx)
        return emailPred.evaluate(with: self)
    }
    
    /// Truncate string with ellipsis
    func truncated(to length: Int, trailing: String = "...") -> String {
        if self.count > length {
            return String(self.prefix(length)) + trailing
        }
        return self
    }
    
    /// Convert to title case
    var titleCased: String {
        return self
            .lowercased()
            .split(separator: " ")
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
    }
    
    /// Remove whitespace and newlines
    var trimmed: String {
        return self.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

// MARK: - Date Extensions
extension Date {
    /// Format date to string
    func formatted(as format: DateFormat) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = format.rawValue
        return formatter.string(from: self)
    }
    
    /// Get relative time string (e.g., "2 hours ago")
    var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        return formatter.localizedString(for: self, relativeTo: Date())
    }
    
    /// Check if date is today
    var isToday: Bool {
        Calendar.current.isDateInToday(self)
    }
    
    /// Check if date is in the past
    var isPast: Bool {
        self < Date()
    }
    
    /// Check if date is in the future
    var isFuture: Bool {
        self > Date()
    }
    
    /// Days between dates
    func days(from date: Date) -> Int {
        Calendar.current.dateComponents([.day], from: date, to: self).day ?? 0
    }
}

enum DateFormat: String {
    case full = "EEEE, MMM d, yyyy"
    case long = "MMM d, yyyy"
    case medium = "MMM d"
    case short = "M/d/yy"
    case time = "h:mm a"
    case dateTime = "MMM d, yyyy 'at' h:mm a"
    case iso8601 = "yyyy-MM-dd'T'HH:mm:ss'Z'"
}

// MARK: - Number Extensions
extension Int {
    /// Format number with thousands separator
    var formatted: String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal
        return formatter.string(from: NSNumber(value: self)) ?? "\(self)"
    }
    
    /// Format as currency
    func asCurrency(code: String = "USD") -> String {
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = code
        return formatter.string(from: NSNumber(value: self)) ?? "$\(self)"
    }
    
    /// Format as abbreviated (1K, 1M, etc)
    var abbreviated: String {
        let num = Double(self)
        let sign = ((num < 0) ? "-" : "" )
        let absNum = abs(num)
        
        if absNum < 1000 {
            return "\(sign)\(Int(absNum))"
        } else if absNum < 1_000_000 {
            return String(format: "\(sign)%.1fK", absNum/1000)
        } else if absNum < 1_000_000_000 {
            return String(format: "\(sign)%.1fM", absNum/1_000_000)
        } else {
            return String(format: "\(sign)%.1fB", absNum/1_000_000_000)
        }
    }
}

extension Double {
    /// Format as percentage
    func asPercentage(decimals: Int = 0) -> String {
        return String(format: "%.\(decimals)f%%", self * 100)
    }
    
    /// Round to specified decimal places
    func rounded(to places: Int) -> Double {
        let divisor = pow(10.0, Double(places))
        return (self * divisor).rounded() / divisor
    }
}

// MARK: - Array Extensions
extension Array {
    /// Safe array access
    subscript(safe index: Int) -> Element? {
        return indices.contains(index) ? self[index] : nil
    }
    
    /// Chunk array into smaller arrays
    func chunked(into size: Int) -> [[Element]] {
        return stride(from: 0, to: count, by: size).map {
            Array(self[$0 ..< Swift.min($0 + size, count)])
        }
    }
}

// MARK: - Bundle Extensions
extension Bundle {
    /// App version
    var appVersion: String {
        return infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0"
    }
    
    /// App build number
    var appBuild: String {
        return infoDictionary?["CFBundleVersion"] as? String ?? "1"
    }
    
    /// Full version string
    var fullVersion: String {
        return "\(appVersion) (\(appBuild))"
    }
}

// MARK: - FileManager Extensions
extension FileManager {
    /// Documents directory URL
    static var documentsDirectory: URL {
        return FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
    }
    
    /// App support directory URL
    static var appSupportDirectory: URL {
        return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
    }
    
    /// Check if file exists at path
    func fileExists(at url: URL) -> Bool {
        return fileExists(atPath: url.path)
    }
    
    /// File size in bytes
    func fileSize(at url: URL) -> Int64? {
        do {
            let attributes = try attributesOfItem(atPath: url.path)
            return attributes[.size] as? Int64
        } catch {
            return nil
        }
    }
}

// MARK: - UserDefaults Extensions
extension UserDefaults {
    /// Type-safe UserDefaults keys
    enum Keys: String {
        case hasCompletedOnboarding
        case lastSyncDate
        case preferredTheme
        case notificationsEnabled
    }
    
    /// Get value for key
    func value<T>(for key: Keys) -> T? {
        return object(forKey: key.rawValue) as? T
    }
    
    /// Set value for key
    func set<T>(_ value: T?, for key: Keys) {
        set(value, forKey: key.rawValue)
    }
}

// MARK: - Binding Extensions
extension Binding {
    /// Create a binding with a default value
    init(_ source: Binding<Value?>, defaultValue: Value) {
        self.init(
            get: { source.wrappedValue ?? defaultValue },
            set: { source.wrappedValue = $0 }
        )
    }
    
    /// Map binding to another type
    func map<T>(
        get: @escaping (Value) -> T,
        set: @escaping (T) -> Value
    ) -> Binding<T> {
        Binding<T>(
            get: { get(self.wrappedValue) },
            set: { self.wrappedValue = set($0) }
        )
    }
}

// MARK: - Task Extensions
extension Task where Success == Never, Failure == Never {
    /// Delay task execution
    static func sleep(seconds: Double) async {
        let nanoseconds = UInt64(seconds * 1_000_000_000)
        try? await Task.sleep(nanoseconds: nanoseconds)
    }
}

// MARK: - Preview Helpers
#if DEBUG
struct PreviewDevice {
    static let iPhone15Pro = "iPhone 15 Pro"
    static let iPhone15ProMax = "iPhone 15 Pro Max"
    static let iPadPro12 = "iPad Pro (12.9-inch) (6th generation)"
    static let iPadPro11 = "iPad Pro (11-inch) (4th generation)"
}

extension View {
    /// Preview on multiple devices
    func previewDevices(_ devices: [String]) -> some View {
        ForEach(devices, id: \.self) { device in
            self
                .previewDevice(.init(stringLiteral: device))
                .previewDisplayName(device)
        }
    }
}
#endif