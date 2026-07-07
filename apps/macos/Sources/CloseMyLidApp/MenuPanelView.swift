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

    func refresh() {
        battery = BatteryStatusReader.read()
    }
}

struct AgentActivity: Identifiable {
    let name: String
    let detail: String
    let isWorking: Bool
    let icon: AgentIcon

    var id: String { name }
}

// Hardcoded for now; will be backed by real agent detection later.
private let agents: [AgentActivity] = [
    AgentActivity(name: "Claude Code", detail: "1 session", isWorking: true, icon: .claude),
    AgentActivity(name: "OpenAI Codex CLI", detail: "idle", isWorking: false, icon: .codex),
    AgentActivity(name: "OpenCode", detail: "idle", isWorking: false, icon: .opencode)
]

struct MenuPanelView: View {
    @ObservedObject var sleep: SleepSessionController
    @ObservedObject var model: MenuPanelModel
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

    private var agentsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Agents")
                    .font(.system(size: 15, weight: .bold))
                Spacer()
                Text(workingSummary)
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

    private var workingSummary: String {
        let count = agents.filter(\.isWorking).count
        return count == 0 ? "all idle" : "\(count) working"
    }

    private func agentRow(_ agent: AgentActivity) -> some View {
        HStack(spacing: 10) {
            ZStack {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(agent.icon.badgeColor)
                if let image = agent.icon.image {
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
        HStack {
            Text("Hold for")
                .font(.system(size: 15, weight: .bold))
            Spacer()
            HStack(spacing: 8) {
                holdButton("30 min", .timed(SessionDuration.thirtyMinutes))
                holdButton("1 hour", .timed(SessionDuration.oneHour))
                holdButton("4 hours", .timed(SessionDuration.fourHours))
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
            FooterRow(title: "Settings…", shortcut: "⌘ ,", action: actions.openSettings)
            FooterRow(title: "Quit Close My Lid", shortcut: "⌘Q", action: actions.quit)
        }
    }
}

private struct PillButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .medium))
            .padding(.horizontal, 10)
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
    }
}
