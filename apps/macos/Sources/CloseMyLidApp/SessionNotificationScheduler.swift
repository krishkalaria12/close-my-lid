import CloseMyLidCore
import Foundation
import UserNotifications

/// Delivers hold-session notifications. Abstracted so `StatusMenuController`
/// can be exercised without the real notification center.
@MainActor
protocol SessionNotifying: AnyObject {
    func requestAuthorization()
    func apply(_ plan: SessionNotificationPlan)
    func cancelPending()
}

/// `UserNotifications`-backed implementation. Delivers the "started" message
/// immediately and schedules the "ending soon" and "ended" messages with the
/// system so their timing does not depend on the app's reconciliation timer.
@MainActor
final class SessionNotificationScheduler: SessionNotifying {
    private enum Identifier {
        static let started = "app.closemylid.notification.started"
        static let endingSoon = "app.closemylid.notification.ending-soon"
        static let ended = "app.closemylid.notification.ended"

        static let all = [started, endingSoon, ended]
    }

    /// `UNUserNotificationCenter.current()` traps when the process has no
    /// bundle identifier (for example `swift run CloseMyLid`), so notifications
    /// are only wired up for the packaged, bundled app.
    private var center: UNUserNotificationCenter? {
        guard Bundle.main.bundleIdentifier != nil else {
            return nil
        }

        return UNUserNotificationCenter.current()
    }

    func requestAuthorization() {
        center?.requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func apply(_ plan: SessionNotificationPlan) {
        guard let center else {
            return
        }

        center.removePendingNotificationRequests(withIdentifiers: Identifier.all)

        deliverNow(center, identifier: Identifier.started, title: plan.startTitle, body: plan.startBody)

        if let endingSoon = plan.endingSoon {
            schedule(center, identifier: Identifier.endingSoon, notification: endingSoon)
        }

        if let ended = plan.ended {
            schedule(center, identifier: Identifier.ended, notification: ended)
        }
    }

    func cancelPending() {
        // Only pending requests are removed; already-delivered notifications
        // (e.g. an "ended" message the user just saw) are left in place.
        center?.removePendingNotificationRequests(withIdentifiers: Identifier.all)
    }

    private func deliverNow(
        _ center: UNUserNotificationCenter,
        identifier: String,
        title: String,
        body: String
    ) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body

        center.add(UNNotificationRequest(identifier: identifier, content: content, trigger: nil))
    }

    private func schedule(
        _ center: UNUserNotificationCenter,
        identifier: String,
        notification: ScheduledNotification
    ) {
        let interval = notification.fireDate.timeIntervalSinceNow
        guard interval > 0 else {
            return
        }

        let content = UNMutableNotificationContent()
        content.title = notification.title
        content.body = notification.body

        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: interval, repeats: false)
        center.add(UNNotificationRequest(identifier: identifier, content: content, trigger: trigger))
    }
}
