import Foundation

@MainActor
public final class SleepSessionController: ObservableObject {
    @Published public private(set) var state: SleepControlState = .inactive

    private let executor: PowerCommandExecuting

    public init(executor: PowerCommandExecuting) {
        self.executor = executor
    }

    public func start(duration: SessionDuration, now: Date = Date()) throws {
        try executor.setDisableSleep(true)
        state = .active(startedAt: now, endsAt: duration.endDate(startingAt: now))
    }

    public func stop() throws {
        try executor.setDisableSleep(false)
        state = .inactive
    }

    public func stopIfExpired(now: Date = Date()) throws {
        guard case let .active(_, .some(endsAt)) = state, endsAt <= now else {
            return
        }

        try stop()
    }
}
