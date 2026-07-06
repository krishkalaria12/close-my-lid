import Foundation

@MainActor
public final class SleepSessionController: ObservableObject {
    @Published public private(set) var state: SleepControlState

    private let executor: PowerCommandExecuting
    private let store: SleepSessionStoring

    public init(
        executor: PowerCommandExecuting,
        store: SleepSessionStoring = UserDefaultsSleepSessionStore()
    ) {
        self.executor = executor
        self.store = store
        self.state = store.load()
    }

    public func start(duration: SessionDuration, now: Date = Date()) throws {
        try executor.setDisableSleep(true)
        state = .active(startedAt: now, endsAt: duration.endDate(startingAt: now))
        store.save(state)
    }

    public func stop() throws {
        try executor.setDisableSleep(false)
        state = .inactive
        store.save(state)
    }

    public func stopIfExpired(now: Date = Date()) throws {
        guard case let .active(_, .some(endsAt)) = state, endsAt <= now else {
            return
        }

        try stop()
    }

    public func syncWithSystem(disableSleepIsEnabled: Bool, now: Date = Date()) throws {
        if case let .active(_, .some(endsAt)) = state, endsAt <= now {
            try stop()
            return
        }

        switch (state.isActive, disableSleepIsEnabled) {
        case (false, true):
            state = .active(startedAt: now, endsAt: nil)
            store.save(state)
        case (true, false):
            clearStoredSession()
        case (true, true), (false, false):
            break
        }
    }

    public func clearStoredSession() {
        state = .inactive
        store.save(state)
    }
}
