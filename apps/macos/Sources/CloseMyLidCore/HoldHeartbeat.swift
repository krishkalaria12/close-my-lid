import Foundation

/// A liveness record written by the app while a closed-lid hold is active.
///
/// The watchdog agent compares this file against wall-clock time to detect a
/// stranded `pmset disablesleep 1` after a crash or force-quit. `updatedAt`
/// is refreshed by the running app; `endsAt` is the session's scheduled end.
public struct HoldHeartbeat: Equatable, Codable, Sendable {
    public var endsAt: Date?
    public var updatedAt: Date

    public init(endsAt: Date?, updatedAt: Date) {
        self.endsAt = endsAt
        self.updatedAt = updatedAt
    }
}

/// Reads and writes the heartbeat file shared between the app and the
/// watchdog agent. Both run as the current user, so no elevation is needed.
public struct HoldHeartbeatStore: Sendable {
    public let directory: URL

    public init(directory: URL? = nil) {
        if let directory {
            self.directory = directory
        } else {
            let support = FileManager.default.urls(
                for: .applicationSupportDirectory,
                in: .userDomainMask
            ).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
            self.directory = support.appendingPathComponent("CloseMyLid", isDirectory: true)
        }
    }

    public var fileURL: URL {
        directory.appendingPathComponent("heartbeat.json", isDirectory: false)
    }

    public func write(_ heartbeat: HoldHeartbeat) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(heartbeat)
        try data.write(to: fileURL, options: .atomic)
    }

    public func load() -> HoldHeartbeat? {
        guard let data = try? Data(contentsOf: fileURL) else {
            return nil
        }

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        return try? decoder.decode(HoldHeartbeat.self, from: data)
    }

    public func remove() {
        try? FileManager.default.removeItem(at: fileURL)
    }
}

/// Decides when the watchdog should force normal sleep behavior back on.
///
/// Two failure modes are covered:
/// - **Dead app:** the heartbeat stopped refreshing (crash, force quit, power
///   loss), so the app cannot clean up after itself.
/// - **Missed expiry:** a timed hold ran past its end plus grace without the
///   app releasing it.
public struct WatchdogPolicy: Equatable, Sendable {
    /// A heartbeat older than this means the owning app is gone.
    public let livenessInterval: TimeInterval

    /// Extra time past a timed session's end before forced release.
    public let expiryGrace: TimeInterval

    public init(
        livenessInterval: TimeInterval = WatchdogPolicy.defaultLivenessInterval,
        expiryGrace: TimeInterval = WatchdogPolicy.defaultExpiryGrace
    ) {
        self.livenessInterval = livenessInterval
        self.expiryGrace = expiryGrace
    }

    /// The app refreshes its heartbeat every reconciliation tick (~30s).
    public static let defaultLivenessInterval: TimeInterval = 3 * 60
    public static let defaultExpiryGrace: TimeInterval = 2 * 60

    /// True when the recorded hold looks stranded at `now`.
    public func shouldReleaseHold(heartbeat: HoldHeartbeat?, now: Date) -> Bool {
        guard let heartbeat else {
            return false
        }

        let appIsGone = now.timeIntervalSince(heartbeat.updatedAt) > livenessInterval
        let holdExpired: Bool
        if let endsAt = heartbeat.endsAt {
            holdExpired = now.timeIntervalSince(endsAt) > expiryGrace
        } else {
            holdExpired = false
        }

        return appIsGone || holdExpired
    }
}
