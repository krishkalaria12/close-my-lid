import Foundation

public enum SessionDuration: Equatable, Sendable {
    case indefinitely
    case timed(TimeInterval)

    public static let menuPresets: [SessionDuration] = [
        .timed(30 * 60),
        .timed(60 * 60),
        .timed(4 * 60 * 60),
        .indefinitely
    ]

    public var title: String {
        switch self {
        case .indefinitely:
            return "Indefinitely"
        case .timed(30 * 60):
            return "30 Minutes"
        case .timed(60 * 60):
            return "1 Hour"
        case .timed(4 * 60 * 60):
            return "4 Hours"
        case let .timed(seconds):
            return "\(Int(seconds / 60)) Minutes"
        }
    }

    public func endDate(startingAt startDate: Date) -> Date? {
        switch self {
        case .indefinitely:
            nil
        case let .timed(seconds):
            startDate.addingTimeInterval(seconds)
        }
    }
}
