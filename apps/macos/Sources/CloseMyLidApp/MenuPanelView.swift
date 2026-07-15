import AppKit
import CloseMyLidCore
import SwiftUI

struct MenuPanelActions {
    var setHolding: (Bool) -> Void
    var hold: (SessionDuration) -> Void
    var openSettings: () -> Void
    var quit: () -> Void
}

@MainActor
final class MenuPanelModel: ObservableObject {
    @Published var battery: BatteryStatus?
    @Published var agentSessions: [AgentHarness: Int] = [:]

    func refresh() {
        let currentBattery = BatteryStatusReader.read()
        if battery != currentBattery {
            battery = currentBattery
        }

        // The process-table scan runs off the main thread; only the resulting
        // Sendable counts hop back to the main actor to update the UI.
        Task.detached(priority: .utility) { [weak self] in
            let counts = AgentSessionDetector.sessionCounts()
            await self?.applyAgentSessions(counts)
        }
    }

    private func applyAgentSessions(_ counts: [AgentHarness: Int]) {
        if agentSessions != counts {
            agentSessions = counts
        }
    }
}

struct AgentActivity: Identifiable {
    let harness: AgentHarness
    let sessionCount: Int

    var id: AgentHarness { harness }
    var name: String { harness.displayName }
    var isWorking: Bool { sessionCount > 0 }

    var detail: String {
        switch sessionCount {
        case 0: "idle"
        case 1: "1 session"
        default: "\(sessionCount) sessions"
        }
    }
}

struct MenuPanelView: View {
    @ObservedObject var sleep: SleepSessionController
    @ObservedObject var model: MenuPanelModel
    @ObservedObject var updates: UpdateController
    let actions: MenuPanelActions
    var batterySafetyPolicy = BatterySafetyPolicy()

    @State private var now = Date()

    private let ticker = Timer.publish(every: 30, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
                .padding(.horizontal, 16)
                .padding(.top, 16)
                .padding(.bottom, 14)

            sectionDivider

            if let battery = model.battery {
                batterySection(battery)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 14)
                sectionDivider
            }

            agentsSection
                .padding(.horizontal, 16)
                .padding(.vertical, 14)

            sectionDivider

            holdSection
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

            sectionDivider

            footer
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
        }
        .frame(width: 300)
        .onReceive(ticker) { now = $0 }
        .onAppear { now = Date() }
    }

    private var sectionDivider: some View {
        Divider().padding(.horizontal, 16)
    }

    // MARK: - Header

    private var header: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Close My Lid")
                    .font(.system(size: 17, weight: .bold))
                Text(statusLine)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            Toggle("", isOn: holdingBinding)
                .toggleStyle(.switch)
                .labelsHidden()
        }
    }

    private var holdingBinding: Binding<Bool> {
        Binding(
            get: { sleep.state.isActive },
            set: { actions.setHolding($0) }
        )
    }

    private var statusLine: String {
        switch sleep.state {
        case .inactive:
            return "Off — sleeps normally"
        case .active(let startedAt, nil):
            return "Awake — lid-proof · \(Self.clockText(seconds: now.timeIntervalSince(startedAt)))"
        case .active(_, .some(let endsAt)):
            let remaining = max(0, endsAt.timeIntervalSince(now))
            return "Awake — lid-proof · \(Self.clockText(seconds: remaining)) left"
        }
    }

    private static func clockText(seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds))
        let hours = total / 3600
        let minutes = (total % 3600) / 60
        if hours == 0 {
            return "\(max(1, minutes))m"
        }
        return "\(hours)h \(minutes)m"
    }

    // MARK: - Battery

    private func batterySection(_ battery: BatteryStatus) -> some View {
        let isLow = shouldReleaseHold(for: battery)
        let barColor = isLow ? Color(nsColor: .systemRed) : Color(nsColor: .systemGreen)

        return VStack(alignment: .leading, spacing: 10) {
            Text("Battery")
                .font(.system(size: 15, weight: .bold))

            GeometryReader { proxy in
                ZStack(alignment: .leading) {
                    Capsule().fill(.quaternary)
                    Capsule()
                        .fill(barColor)
                        .frame(width: max(8, proxy.size.width * CGFloat(battery.percentage) / 100))
                }
            }
            .frame(height: 8)

            HStack {
                Text("\(battery.percentage)% left")
                    .font(.system(size: 14))
                Spacer()
                Text(batteryCaption(for: battery, isLow: isLow))
                    .font(.system(size: 13))
                    .foregroundStyle(batteryCaptionStyle(isLow: isLow))
            }
        }
    }

    private func shouldReleaseHold(for battery: BatteryStatus) -> Bool {
        batterySafetyPolicy.shouldReleaseHold(
            percentage: battery.percentage,
            isCharging: battery.isCharging
        )
    }

    private func batteryCaption(for battery: BatteryStatus, isLow: Bool) -> String {
        if battery.isCharging {
            return "charging"
        }

        if isLow {
            return "stopping to protect battery"
        }

        return "stops at \(batterySafetyPolicy.threshold)%"
    }

    private func batteryCaptionStyle(isLow: Bool) -> AnyShapeStyle {
        if isLow {
            return AnyShapeStyle(Color(nsColor: .systemRed))
        }

        return AnyShapeStyle(.secondary)
    }

    // MARK: - Agents

    private var agents: [AgentActivity] {
        AgentHarness.allCases.map {
            AgentActivity(harness: $0, sessionCount: model.agentSessions[$0] ?? 0)
        }
    }

    private var agentsSection: some View {
        let agents = self.agents
        let workingCount = agents.filter(\.isWorking).count

        return VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Agents")
                    .font(.system(size: 15, weight: .bold))
                Spacer()
                Text(workingCount == 0 ? "all idle" : "\(workingCount) working")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }

            VStack(spacing: 10) {
                ForEach(agents) { agent in
                    agentRow(agent)
                }
            }
        }
    }

    private func agentRow(_ agent: AgentActivity) -> some View {
        HStack(spacing: 10) {
            ZStack {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(agent.harness.badgeColor)
                if let image = agent.harness.icon {
                    Image(nsImage: image)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 16, height: 16)
                }
            }
            .frame(width: 28, height: 28)

            Text(agent.name)
                .font(.system(size: 14))

            Spacer()

            Text(agent.detail)
                .font(.system(size: 13))
                .foregroundStyle(agent.isWorking ? AnyShapeStyle(.secondary) : AnyShapeStyle(.tertiary))

            if agent.isWorking {
                Circle()
                    .fill(Color(nsColor: .systemGreen))
                    .frame(width: 7, height: 7)
            }
        }
        .opacity(agent.isWorking ? 1 : 0.55)
    }

    // MARK: - Hold presets

    private var holdSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Hold for")
                .font(.system(size: 15, weight: .bold))
            HStack(spacing: 6) {
                holdButton("30 min", .timed(SessionDuration.thirtyMinutes))
                holdButton("1 hour", .timed(SessionDuration.oneHour))
                holdButton("4 hours", .timed(SessionDuration.fourHours))
                holdButton("Unlimited", .indefinitely)
            }
        }
    }

    private func holdButton(_ title: String, _ duration: SessionDuration) -> some View {
        Button(title) {
            actions.hold(duration)
        }
        .buttonStyle(PillButtonStyle())
    }

    // MARK: - Footer

    private var footer: some View {
        VStack(spacing: 2) {
            if let version = updates.availableVersion {
                UpdateAvailableRow(version: version, action: updates.checkForUpdates)
            } else {
                FooterRow(
                    title: "Check for Updates…",
                    shortcut: "",
                    action: updates.checkForUpdates,
                    isEnabled: updates.canCheckForUpdates
                )
            }
            FooterRow(title: "Settings…", shortcut: "⌘ ,", action: actions.openSettings)
            FooterRow(title: "Quit Close My Lid", shortcut: "⌘Q", action: actions.quit)
        }
    }
}

private struct PillButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .medium))
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(configuration.isPressed ? AnyShapeStyle(.tertiary) : AnyShapeStyle(.quaternary))
            )
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

private struct FooterRow: View {
    let title: String
    let shortcut: String
    let action: () -> Void
    var isEnabled = true

    @State private var isHovered = false

    var body: some View {
        Button(action: action) {
            HStack {
                Text(title)
                    .font(.system(size: 14))
                Spacer()
                Text(shortcut)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 9)
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.primary.opacity(isHovered ? 0.08 : 0))
        )
        .onHover { isHovered = $0 }
        .disabled(!isEnabled)
    }
}

private struct UpdateAvailableRow: View {
    let version: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: "arrow.down.circle.fill")
                    .font(.system(size: 20))
                    .foregroundStyle(Color.accentColor)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Update Available")
                        .font(.system(size: 14, weight: .semibold))
                    Text("Version \(version) — install and restart")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 9)
            .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}
