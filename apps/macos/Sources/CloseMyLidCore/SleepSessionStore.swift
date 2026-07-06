import Foundation

public protocol SleepSessionStoring: Sendable {
    func load() -> SleepControlState
    func save(_ state: SleepControlState)
}

public final class UserDefaultsSleepSessionStore: SleepSessionStoring, @unchecked Sendable {
    private let defaults: UserDefaults
    private let key: String

    public init(
        defaults: UserDefaults = .standard,
        key: String = "app.closemylid.session-state"
    ) {
        self.defaults = defaults
        self.key = key
    }

    public func load() -> SleepControlState {
        guard let data = defaults.data(forKey: key),
              let snapshot = try? JSONDecoder().decode(SleepSessionSnapshot.self, from: data)
        else {
            return .inactive
        }

        return snapshot.state
    }

    public func save(_ state: SleepControlState) {
        guard state.isActive else {
            defaults.removeObject(forKey: key)
            return
        }

        let snapshot = SleepSessionSnapshot(state: state)
        guard let data = try? JSONEncoder().encode(snapshot) else {
            return
        }

        defaults.set(data, forKey: key)
    }
}

public final class InMemorySleepSessionStore: SleepSessionStoring, @unchecked Sendable {
    public private(set) var savedStates: [SleepControlState]

    public init(initialState: SleepControlState = .inactive) {
        self.savedStates = [initialState]
    }

    public func load() -> SleepControlState {
        savedStates.last ?? .inactive
    }

    public func save(_ state: SleepControlState) {
        savedStates.append(state)
    }
}

private struct SleepSessionSnapshot: Codable {
    let startedAt: Date
    let endsAt: Date?

    init(state: SleepControlState) {
        switch state {
        case .inactive:
            startedAt = Date(timeIntervalSince1970: 0)
            endsAt = Date(timeIntervalSince1970: 0)
        case let .active(startedAt, endsAt):
            self.startedAt = startedAt
            self.endsAt = endsAt
        }
    }

    var state: SleepControlState {
        guard startedAt != Date(timeIntervalSince1970: 0) else {
            return .inactive
        }

        return .active(startedAt: startedAt, endsAt: endsAt)
    }
}
