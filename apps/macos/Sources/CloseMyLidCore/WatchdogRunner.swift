import Foundation

/// The dead-man switch: restores normal closed-lid sleep when a hold looks
/// stranded — the owning app crashed, was force-quit, or missed a timed
/// session's end by more than the policy grace.
///
/// Runs headless (the LaunchAgent invokes the same binary with `--watchdog`),
/// so it must never prompt. If the sudoers grant is missing, releasing fails
/// and the heartbeat is left for the next tick.
public enum WatchdogRunner {
    /// Performs one watchdog pass. Returns true when normal sleep behavior
    /// was restored.
    @discardableResult
    public static func runOnce(
        heartbeatStore: HoldHeartbeatStore,
        policy: WatchdogPolicy,
        powerSettingsReader: PowerSettingsReading,
        executor: PowerCommandExecuting,
        now: Date = Date()
    ) throws -> Bool {
        let heartbeat = heartbeatStore.load()

        guard policy.shouldReleaseHold(heartbeat: heartbeat, now: now) else {
            return false
        }

        guard try powerSettingsReader.disableSleepIsEnabled() else {
            // Nothing stranded; drop any stale marker so future passes are free.
            heartbeatStore.remove()
            return false
        }

        try executor.setDisableSleep(false)
        heartbeatStore.remove()
        return true
    }
}
