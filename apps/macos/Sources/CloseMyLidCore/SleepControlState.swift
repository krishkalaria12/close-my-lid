import Foundation

public enum SleepControlState: Equatable, Sendable {
    case inactive
    case active(startedAt: Date, endsAt: Date?)

    public var isActive: Bool {
        switch self {
        case .active:
            return true
        case .inactive:
            return false
        }
    }

    public func statusText(now: Date = Date()) -> String {
        switch self {
        case .inactive:
            return "Ready"
        case .active(_, nil):
            return "Holding until stopped"
        case let .active(_, .some(endsAt)):
            if endsAt <= now {
                return "Ending now"
            }

            let remaining = Int(endsAt.timeIntervalSince(now))
            let minutes = max(1, Int(ceil(Double(remaining) / 60.0)))
            return "Holding for \(minutes)m"
        }
    }
}
