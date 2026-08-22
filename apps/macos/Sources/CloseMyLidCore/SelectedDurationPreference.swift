import Foundation

/// Persists the duration the user last picked, so the main ON/OFF toggle can
/// re-apply it instead of always starting an Unlimited hold.
public protocol SelectedDurationStoring: Sendable {
    func load() -> SessionDuration
    func save(_ duration: SessionDuration)
}

public final class UserDefaultsSelectedDurationStore: SelectedDurationStoring, @unchecked Sendable {
    private let defaults: UserDefaults
    private let key: String

    public init(
        defaults: UserDefaults = .standard,
        key: String = "app.closemylid.selected-duration"
    ) {
        self.defaults = defaults
        self.key = key
    }

    public func load() -> SessionDuration {
        guard let seconds = defaults.object(forKey: key) as? Double, seconds > 0 else {
            return .indefinitely
        }

        return .timed(seconds)
    }

    public func save(_ duration: SessionDuration) {
        switch duration {
        case .indefinitely:
            defaults.set(-1.0, forKey: key)
        case let .timed(seconds):
            defaults.set(seconds, forKey: key)
        }
    }
}

public final class InMemorySelectedDurationStore: SelectedDurationStoring, @unchecked Sendable {
    public private(set) var savedDurations: [SessionDuration]

    public init(initial: SessionDuration = .indefinitely) {
        self.savedDurations = [initial]
    }

    public func load() -> SessionDuration {
        savedDurations.last ?? .indefinitely
    }

    public func save(_ duration: SessionDuration) {
        savedDurations.append(duration)
    }
}
