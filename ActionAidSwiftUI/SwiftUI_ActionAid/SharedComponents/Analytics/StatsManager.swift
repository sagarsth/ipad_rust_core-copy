//
//  StatsManager.swift
//  ActionAid SwiftUI
//
//  Generic statistics tracking and calculation for any entity type
//

import SwiftUI
import Foundation

// MARK: - Stat Configuration
struct StatConfig {
    let key: String
    let title: String
    let icon: String
    let color: Color
    let calculator: StatCalculator
    
    init(key: String, title: String, icon: String, color: Color, calculator: StatCalculator) {
        self.key = key
        self.title = title
        self.icon = icon
        self.color = color
        self.calculator = calculator
    }
}

// MARK: - Stat Calculator Protocol
protocol StatCalculator {
    func calculate<Entity>(from entities: [Entity]) -> StatValue
}

// MARK: - Stat Value
enum StatValue {
    case count(Int)
    case percentage(Double)
    case text(String)
    case ratio(Int, Int) // numerator, denominator
    
    var displayValue: String {
        switch self {
        case .count(let value):
            return "\(value)"
        case .percentage(let value):
            return "\(Int(value))%"
        case .text(let value):
            return value
        case .ratio(let num, let den):
            return "\(num)/\(den)"
        }
    }
    
    var numericValue: Double {
        switch self {
        case .count(let value):
            return Double(value)
        case .percentage(let value):
            return value
        case .text(_):
            return 0.0
        case .ratio(let num, let den):
            return den > 0 ? Double(num) / Double(den) * 100 : 0.0
        }
    }
}

// MARK: - Stat Result
struct StatResult {
    let config: StatConfig
    let value: StatValue
    let trend: StatTrend?
    let lastUpdated: Date
    
    struct StatTrend {
        let direction: TrendDirection
        let percentage: Double
        
        enum TrendDirection {
            case up, down, stable
            
            var icon: String {
                switch self {
                case .up: return "arrow.up.right"
                case .down: return "arrow.down.right"
                case .stable: return "arrow.right"
                }
            }
            
            var color: Color {
                switch self {
                case .up: return .green
                case .down: return .red
                case .stable: return .gray
                }
            }
        }
    }
}

// MARK: - Common Stat Calculators
struct CountCalculator: StatCalculator {
    func calculate<Entity>(from entities: [Entity]) -> StatValue {
        return .count(entities.count)
    }
}

struct ConditionalCountCalculator<T>: StatCalculator {
    let condition: (T) -> Bool
    
    init(_ condition: @escaping (T) -> Bool) {
        self.condition = condition
    }
    
    func calculate<Entity>(from entities: [Entity]) -> StatValue {
        // Type-safe filtering: only apply condition if Entity matches T
        if T.self == Entity.self {
            let typedEntities = entities as! [T]
            let count = typedEntities.filter(condition).count
            return .count(count)
        } else {
            // Fallback: return zero if types don't match
            return .count(0)
        }
    }
}

struct PercentageCalculator<T>: StatCalculator {
    let condition: (T) -> Bool
    
    init(_ condition: @escaping (T) -> Bool) {
        self.condition = condition
    }
    
    func calculate<Entity>(from entities: [Entity]) -> StatValue {
        // Type-safe filtering: only apply condition if Entity matches T
        if T.self == Entity.self {
            let typedEntities = entities as! [T]
            guard !typedEntities.isEmpty else { return .percentage(0.0) }
            
            let matchingCount = typedEntities.filter(condition).count
            let percentage = Double(matchingCount) / Double(typedEntities.count) * 100.0
            return .percentage(percentage)
        } else {
            return .percentage(0.0)
        }
    }
}

struct RatioCalculator<T>: StatCalculator {
    let numeratorCondition: (T) -> Bool
    let denominatorCondition: (T) -> Bool
    
    init(
        numerator: @escaping (T) -> Bool,
        denominator: @escaping (T) -> Bool
    ) {
        self.numeratorCondition = numerator
        self.denominatorCondition = denominator
    }
    
    func calculate<Entity>(from entities: [Entity]) -> StatValue {
        // Type-safe filtering: only apply conditions if Entity matches T
        if T.self == Entity.self {
            let typedEntities = entities as! [T]
            let numerator = typedEntities.filter(numeratorCondition).count
            let denominator = typedEntities.filter(denominatorCondition).count
            return .ratio(numerator, denominator)
        } else {
            return .ratio(0, 0)
        }
    }
}

// MARK: - Stats Manager
@MainActor
class StatsManager<Entity>: ObservableObject {
    // MARK: - Published State
    @Published var stats: [StatResult] = []
    @Published var isCalculating = false
    @Published var lastCalculated = Date.distantPast
    
    // MARK: - Configuration
    private let statConfigs: [StatConfig]
    private let refreshInterval: TimeInterval
    private var previousStats: [String: StatValue] = [:]
    
    // MARK: - Initialization
    init(statConfigs: [StatConfig], refreshInterval: TimeInterval = 1.0) {
        self.statConfigs = statConfigs
        self.refreshInterval = refreshInterval
    }
    
    // MARK: - Public Methods
    
    /// Calculate stats from entities
    func calculateStats(from entities: [Entity]) {
        guard !isCalculating else { return }
        
        isCalculating = true
        defer { isCalculating = false }
        
        var newStats: [StatResult] = []
        let now = Date()
        
        for config in statConfigs {
            let value = config.calculator.calculate(from: entities)
            
            // Calculate trend if we have previous data
            let trend = calculateTrend(for: config.key, currentValue: value)
            
            let statResult = StatResult(
                config: config,
                value: value,
                trend: trend,
                lastUpdated: now
            )
            
            newStats.append(statResult)
            
            // Store for trend calculation
            previousStats[config.key] = value
        }
        
        stats = newStats
        lastCalculated = now
    }
    
    /// Get a specific stat by key
    func getStat(by key: String) -> StatResult? {
        return stats.first { $0.config.key == key }
    }
    
    /// Get stats for display in a horizontal scroll view
    func getDisplayStats() -> [EntityStatsCard.EntityStat] {
        return stats.map { statResult in
            EntityStatsCard.EntityStat(
                title: statResult.config.title,
                value: statResult.value.displayValue,
                icon: statResult.config.icon,
                color: statResult.config.color
            )
        }
    }
    
    /// Reset all cached trend data
    func resetTrends() {
        previousStats.removeAll()
    }
    
    // MARK: - Private Methods
    
    private func calculateTrend(for key: String, currentValue: StatValue) -> StatResult.StatTrend? {
        guard let previousValue = previousStats[key] else { return nil }
        
        let currentNumeric = currentValue.numericValue
        let previousNumeric = previousValue.numericValue
        
        guard previousNumeric > 0 else { return nil }
        
        let change = currentNumeric - previousNumeric
        let percentageChange = abs(change) / previousNumeric * 100.0
        
        // Only show trend if change is significant (>1%)
        guard percentageChange > 1.0 else {
            return StatResult.StatTrend(
                direction: .stable,
                percentage: 0.0
            )
        }
        
        let direction: StatResult.StatTrend.TrendDirection = change > 0 ? .up : .down
        
        return StatResult.StatTrend(
            direction: direction,
            percentage: percentageChange
        )
    }
}

// MARK: - Strategic Goals Stats Configuration
extension StatsManager where Entity == StrategicGoalResponse {
    static func strategicGoalsManager() -> StatsManager<StrategicGoalResponse> {
        let configs = [
            StatConfig(
                key: "total",
                title: "Total Goals",
                icon: "target",
                color: .blue,
                calculator: CountCalculator()
            ),
            StatConfig(
                key: "on_track",
                title: "On Track",
                icon: "checkmark.circle",
                color: .green,
                calculator: ConditionalCountCalculator<StrategicGoalResponse> { $0.statusId == 1 }
            ),
            StatConfig(
                key: "at_risk",
                title: "At Risk",
                icon: "exclamationmark.triangle",
                color: .orange,
                calculator: ConditionalCountCalculator<StrategicGoalResponse> { $0.statusId == 2 }
            ),
            StatConfig(
                key: "behind",
                title: "Behind",
                icon: "xmark.circle",
                color: .red,
                calculator: ConditionalCountCalculator<StrategicGoalResponse> { $0.statusId == 3 }
            ),
            StatConfig(
                key: "completed",
                title: "Completed",
                icon: "checkmark.circle.fill",
                color: .blue,
                calculator: ConditionalCountCalculator<StrategicGoalResponse> { $0.statusId == 4 }
            ),
            StatConfig(
                key: "completion_rate",
                title: "Completion Rate",
                icon: "percent",
                color: .purple,
                calculator: PercentageCalculator<StrategicGoalResponse> { $0.statusId == 4 }
            )
        ]
        
        return StatsManager(statConfigs: configs)
    }
}

// MARK: - Projects Stats Configuration
extension StatsManager {
    static func projectsManager<ProjectEntity>() -> StatsManager<ProjectEntity> {
        let configs = [
            StatConfig(
                key: "total",
                title: "Total Projects",
                icon: "folder",
                color: .blue,
                calculator: CountCalculator()
            ),
            // Add project-specific stat configs here
            // StatConfig(
            //     key: "active",
            //     title: "Active",
            //     icon: "play.circle",
            //     color: .green,
            //     calculator: ConditionalCountCalculator<ProjectEntity> { $0.status == "active" }
            // )
        ]
        
        return StatsManager<ProjectEntity>(statConfigs: configs)
    }
}

// MARK: - Users Stats Configuration
extension StatsManager {
    static func usersManager<UserEntity>() -> StatsManager<UserEntity> {
        let configs = [
            StatConfig(
                key: "total",
                title: "Total Users",
                icon: "person.2",
                color: .blue,
                calculator: CountCalculator()
            ),
            // Add user-specific stat configs here
        ]
        
        return StatsManager<UserEntity>(statConfigs: configs)
    }
}

// MARK: - Stats Dashboard Component
struct StatsDashboard<Entity>: View {
    @ObservedObject var statsManager: StatsManager<Entity>
    let entities: [Entity]
    let showTrends: Bool
    
    init(
        statsManager: StatsManager<Entity>,
        entities: [Entity],
        showTrends: Bool = true
    ) {
        self.statsManager = statsManager
        self.entities = entities
        self.showTrends = showTrends
    }
    
    var body: some View {
        VStack(spacing: 12) {
            // Stats Cards
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(statsManager.stats.indices, id: \.self) { index in
                        StatCard(
                            statResult: statsManager.stats[index],
                            showTrend: showTrends
                        )
                    }
                }
                .padding(.horizontal)
            }
            
            // Last Updated
            if !statsManager.stats.isEmpty {
                HStack {
                    Spacer()
                    Text("Updated \(formatRelativeTime(statsManager.lastCalculated))")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                .padding(.horizontal)
            }
        }
        .onAppear {
            statsManager.calculateStats(from: entities)
        }
        .onChange(of: entities.count) { oldCount, newCount in
            if newCount != oldCount {
                statsManager.calculateStats(from: entities)
            }
        }
    }
    
    private func formatRelativeTime(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

// MARK: - Stat Card Component
struct StatCard: View {
    let statResult: StatResult
    let showTrend: Bool
    
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header with icon and trend
            HStack {
                Image(systemName: statResult.config.icon)
                    .font(.title3)
                    .foregroundColor(statResult.config.color)
                
                Spacer()
                
                if showTrend, let trend = statResult.trend {
                    HStack(spacing: 2) {
                        Image(systemName: trend.direction.icon)
                            .font(.caption2)
                        Text("\(Int(trend.percentage))%")
                            .font(.caption2)
                            .fontWeight(.medium)
                    }
                    .foregroundColor(trend.direction.color)
                }
            }
            
            // Value and title
            VStack(alignment: .leading, spacing: 2) {
                Text(statResult.value.displayValue)
                    .font(.title2)
                    .fontWeight(.bold)
                    .foregroundColor(.primary)
                
                Text(statResult.config.title)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }
        }
        .padding()
        .frame(width: 120, height: 80)
        .background(Color(.systemGray6))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(statResult.config.color.opacity(0.2), lineWidth: 1)
        )
    }
}

// MARK: - Stats View Modifier
extension View {
    /// Apply stats tracking to any view
    func withStats<Entity>(
        manager: StatsManager<Entity>,
        entities: [Entity],
        showTrends: Bool = true
    ) -> some View {
        VStack(spacing: 0) {
            StatsDashboard(
                statsManager: manager,
                entities: entities,
                showTrends: showTrends
            )
            
            self
        }
    }
    
    /// Register stats with shared context for bottom navigation display
    func withSharedStats<Entity>(
        manager: StatsManager<Entity>,
        entities: [Entity],
        entityName: String
    ) -> some View {
        self.modifier(SharedStatsModifier(
            manager: manager,
            entities: entities,
            entityName: entityName
        ))
    }
}

// MARK: - Shared Stats View Modifier
struct SharedStatsModifier<Entity>: ViewModifier {
    let manager: StatsManager<Entity>
    let entities: [Entity]
    let entityName: String
    
    @EnvironmentObject var sharedStatsContext: SharedStatsContext
    
    func body(content: Content) -> some View {
        content
            .onChange(of: entities.count) { oldCount, newCount in
                if newCount != oldCount {
                    Task { @MainActor in
                        manager.calculateStats(from: entities)
                        sharedStatsContext.registerStats(manager: manager, entityName: entityName)
                    }
                }
            }
            .onAppear {
                if !entities.isEmpty {
                    Task { @MainActor in
                        manager.calculateStats(from: entities)
                        sharedStatsContext.registerStats(manager: manager, entityName: entityName)
                    }
                }
            }
    }
}

// MARK: - Preview
#if DEBUG
struct StatsManager_Previews: PreviewProvider {
    struct SampleEntity {
        let id = UUID()
        let status: String
    }
    
    static let sampleEntities = [
        SampleEntity(status: "active"),
        SampleEntity(status: "active"),
        SampleEntity(status: "completed"),
        SampleEntity(status: "pending")
    ]
    
    static let statsManager = StatsManager<SampleEntity>(
        statConfigs: [
            StatConfig(
                key: "total",
                title: "Total",
                icon: "square.stack",
                color: .blue,
                calculator: CountCalculator()
            ),
            StatConfig(
                key: "active",
                title: "Active",
                icon: "play.circle",
                color: .green,
                calculator: ConditionalCountCalculator<SampleEntity> { $0.status == "active" }
            )
        ]
    )
    
    static var previews: some View {
        StatsDashboard(
            statsManager: statsManager,
            entities: sampleEntities
        )
        .padding()
    }
}
#endif

// MARK: - Shared Stats Context for Bottom Navigation
@MainActor
class SharedStatsContext: ObservableObject {
    @Published var currentEntityStats: AnyStatsManager?
    @Published var entityName: String?
    
    @MainActor
    func registerStats<Entity>(manager: StatsManager<Entity>, entityName: String) {
        self.currentEntityStats = AnyStatsManager(manager)
        self.entityName = entityName
    }
    
    func clearStats() {
        self.currentEntityStats = nil
        self.entityName = nil
    }
}

// MARK: - Type-erased StatsManager
struct AnyStatsManager {
    let stats: [StatResult]
    let isCalculating: Bool
    let lastCalculated: Date
    
    @MainActor
    init<Entity>(_ manager: StatsManager<Entity>) {
        self.stats = manager.stats
        self.isCalculating = manager.isCalculating
        self.lastCalculated = manager.lastCalculated
    }
}

// MARK: - Stats Tab View
struct StatsTabView: View {
    @ObservedObject var sharedContext: SharedStatsContext
    
    var body: some View {
        NavigationStack {
            if let statsManager = sharedContext.currentEntityStats,
               let entityName = sharedContext.entityName {
                
                VStack(spacing: 0) {
                    // Stats Dashboard
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 12) {
                            ForEach(statsManager.stats.indices, id: \.self) { index in
                                StatCard(
                                    statResult: statsManager.stats[index],
                                    showTrend: true
                                )
                            }
                        }
                        .padding(.horizontal)
                    }
                    .padding(.vertical)
                    
                    // Last Updated
                    if !statsManager.stats.isEmpty {
                        HStack {
                            Spacer()
                            Text("Updated \(formatRelativeTime(statsManager.lastCalculated))")
                                .font(.caption2)
                                .foregroundColor(.secondary)
                        }
                        .padding(.horizontal)
                        .padding(.bottom)
                    }
                    
                    Spacer()
                }
                .navigationTitle("\(entityName) Stats")
                .navigationBarTitleDisplayMode(.large)
            } else {
                // Empty state when no entity is selected
                VStack(spacing: 20) {
                    Image(systemName: "chart.bar.fill")
                        .font(.system(size: 60))
                        .foregroundColor(.gray)
                    
                    Text("No Stats Available")
                        .font(.title2)
                        .fontWeight(.semibold)
                    
                    Text("Navigate to an entity view to see statistics")
                        .font(.body)
                        .foregroundColor(.secondary)
                        .multilineTextAlignment(.center)
                }
                .navigationTitle("Stats")
                .navigationBarTitleDisplayMode(.large)
            }
        }
    }
    
    private func formatRelativeTime(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
} 