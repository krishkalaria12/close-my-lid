import Foundation

public enum SessionDuration: Equatable, Sendable {
    case indefinitely
    case timed(TimeInterval)

    public static let thirtyMinutes: TimeInterval = 30.0 * 60.0
    public static let oneHour: TimeInterval = 60.0 * 60.0
    public static let fourHours: TimeInterval = 4.0 * 60.0 * 60.0

    public static let menuPresets: [SessionDuration] = [
        .timed(thirtyMinutes),
        .timed(oneHour),
        .timed(fourHours),
        .indefinitely
    ]

    public var title: String {
        switch self {
        case .indefinitely:
            return "Unlimited"
        case .timed(Self.thirtyMinutes):
            return "30 Minutes"
        case .timed(Self.oneHour):
            return "1 Hour"
        case .timed(Self.fourHours):
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
