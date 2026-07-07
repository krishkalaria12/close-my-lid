import Foundation

/// A single notification the app should deliver at a specific time.
public struct ScheduledNotification: Equatable, Sendable {
    public let fireDate: Date
    public let title: String
    public let body: String

    public init(fireDate: Date, title: String, body: String) {
        self.fireDate = fireDate
        self.title = title
        self.body = body
    }
}

/// The full set of notifications for one hold session: an immediate "started"
/// message plus, for timed sessions, an optional "ending soon" warning and an
/// "ended" message scheduled at the appropriate times.
public struct SessionNotificationPlan: Equatable, Sendable {
    public let startTitle: String
    public let startBody: String
    public let endingSoon: ScheduledNotification?
    public let ended: ScheduledNotification?

    public init(
        startTitle: String,
        startBody: String,
        endingSoon: ScheduledNotification?,
        ended: ScheduledNotification?
    ) {
        self.startTitle = startTitle
        self.startBody = startBody
        self.endingSoon = endingSoon
        self.ended = ended
    }
}

/// Builds a `SessionNotificationPlan` from a session's duration and start time.
/// Pure and deterministic so the timing and copy can be tested without the
/// `UserNotifications` framework.
public enum SessionNotificationPlanner {
    public static let title = "Close My Lid"

    /// How far ahead of the end time the "ending soon" warning fires.
    public static let endingSoonLeadTime: TimeInterval = 5 * 60

    public static func plan(duration: SessionDuration, startedAt: Date) -> SessionNotificationPlan {
        let startBody: String
        switch duration {
        case .indefinitely:
            startBody = "Your Mac will stay awake with the lid closed until you stop it."
        case .timed:
            startBody = "Your Mac will stay awake with the lid closed for the next \(duration.title.lowercased())."
        }

        guard let endsAt = duration.endDate(startingAt: startedAt) else {
            return SessionNotificationPlan(
                startTitle: title,
                startBody: startBody,
                endingSoon: nil,
                ended: nil
            )
        }

        let ended = ScheduledNotification(
            fireDate: endsAt,
            title: title,
            body: "Your Mac now sleeps normally when the lid is closed."
        )

        // Only warn when the session is long enough that the warning lands
        // after it starts; otherwise the "started" and warning would collide.
        let endingSoonFire = endsAt.addingTimeInterval(-endingSoonLeadTime)
        let endingSoon: ScheduledNotification? = endingSoonFire > startedAt
            ? ScheduledNotification(
                fireDate: endingSoonFire,
                title: title,
                body: "About 5 minutes left before your Mac sleeps normally with the lid closed."
            )
            : nil

        return SessionNotificationPlan(
            startTitle: title,
            startBody: startBody,
            endingSoon: endingSoon,
            ended: ended
        )
    }
}
