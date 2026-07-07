import Foundation
import CloseMyLidCore

@MainActor
struct TestRunner {
    private var failures: [String] = []

    mutating func run() {
        testStartingTimedSessionDisablesSleepAndRecordsEndDate()
        testIndefiniteSessionsHaveNoEndDate()
        testExpiredTimedSessionsRestoreNormalSleep()
        testActiveTimedSessionsAreLeftAloneBeforeExpiry()
        testMenuPresetsMatchProductDefaults()
        testStatusCopyRoundsUpRemainingTime()
        testCommandLineActionParser()
        testPowerSettingsParser()
        testSessionStateLoadsFromStore()
        testSessionStatePersistsAfterStartAndStop()
        testSessionSyncReflectsExternalEnable()
        testSessionSyncClearsStaleActiveState()
        testAgentSessionsCountNativeBinaries()
        testAgentSessionsCountScriptRuntimeInstalls()
        testAgentSessionsIgnoreUnrelatedProcesses()
        testAgentSessionsSkipSameHarnessChildren()
        testAgentSessionsCountNestedDifferentHarnesses()

        if failures.isEmpty {
            print("All CloseMyLidCore tests passed")
        } else {
            print(failures.joined(separator: "\n"))
            exit(1)
        }
    }

    private mutating func expect(_ condition: @autoclosure () -> Bool, _ message: String) {
        if !condition() {
            failures.append("FAILED: \(message)")
        }
    }

    private mutating func testStartingTimedSessionDisablesSleepAndRecordsEndDate() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .timed(1_800), now: now)
        } catch {
            failures.append("FAILED: start timed session threw \(error)")
            return
        }

        expect(executor.commands == [true], "starting a timed session disables sleep")
        expect(
            controller.state == .active(startedAt: now, endsAt: Date(timeIntervalSince1970: 1_900)),
            "starting a timed session records the end date"
        )
    }

    private mutating func testIndefiniteSessionsHaveNoEndDate() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .indefinitely, now: now)
        } catch {
            failures.append("FAILED: start indefinite session threw \(error)")
            return
        }

        expect(
            controller.state == .active(startedAt: now, endsAt: nil),
            "indefinite sessions have no end date"
        )
    }

    private mutating func testExpiredTimedSessionsRestoreNormalSleep() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .timed(60), now: now)
            try controller.stopIfExpired(now: Date(timeIntervalSince1970: 161))
        } catch {
            failures.append("FAILED: expired session flow threw \(error)")
            return
        }

        expect(executor.commands == [true, false], "expired timed sessions restore normal sleep")
        expect(controller.state == .inactive, "expired timed sessions become inactive")
    }

    private mutating func testActiveTimedSessionsAreLeftAloneBeforeExpiry() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .timed(60), now: now)
            try controller.stopIfExpired(now: Date(timeIntervalSince1970: 159))
        } catch {
            failures.append("FAILED: active session flow threw \(error)")
            return
        }

        expect(executor.commands == [true], "active timed sessions are left alone before expiry")
        expect(controller.state.isActive, "active timed sessions stay active before expiry")
    }

    private mutating func testMenuPresetsMatchProductDefaults() {
        expect(
            SessionDuration.menuPresets == [
                .timed(SessionDuration.thirtyMinutes),
                .timed(SessionDuration.oneHour),
                .timed(SessionDuration.fourHours),
                .indefinitely
            ],
            "menu presets match product defaults"
        )
    }

    private mutating func testStatusCopyRoundsUpRemainingTime() {
        let state = SleepControlState.active(
            startedAt: Date(timeIntervalSince1970: 0),
            endsAt: Date(timeIntervalSince1970: 61)
        )

        expect(
            state.statusText(now: Date(timeIntervalSince1970: 1)) == "Holding for 1m",
            "status copy rounds exact minute"
        )
        expect(
            state.statusText(now: Date(timeIntervalSince1970: 2)) == "Holding for 1m",
            "status copy rounds sub-minute"
        )
    }

    private mutating func testCommandLineActionParser() {
        expect(CommandLineActionParser.parse([]) == .launchMenu, "empty CLI arguments launch the menu app")
        expect(CommandLineActionParser.parse(["enable"]) == .enable, "enable CLI command parses")
        expect(CommandLineActionParser.parse(["stop"]) == .disable, "stop CLI alias parses")
        expect(CommandLineActionParser.parse(["--status"]) == .status, "status CLI flag parses")
        expect(CommandLineActionParser.parse(["wat"]) == nil, "unknown CLI command fails parsing")
    }

    private mutating func testPowerSettingsParser() {
        let enabledOutput = """
        System-wide power settings:
        Currently in use:
         sleep                1
         disablesleep         1
        """

        let disabledOutput = """
        System-wide power settings:
        Currently in use:
         sleep                1
         disablesleep         0
        """

        expect(
            PowerSettingsParser.disableSleepIsEnabled(from: enabledOutput),
            "pmset parser treats disablesleep 1 as enabled"
        )
        expect(
            !PowerSettingsParser.disableSleepIsEnabled(from: disabledOutput),
            "pmset parser treats disablesleep 0 as disabled"
        )
        expect(
            !PowerSettingsParser.disableSleepIsEnabled(from: "sleep 1"),
            "pmset parser treats missing disablesleep as disabled"
        )
    }

    private mutating func testSessionStateLoadsFromStore() {
        let storedState = SleepControlState.active(
            startedAt: Date(timeIntervalSince1970: 10),
            endsAt: Date(timeIntervalSince1970: 70)
        )
        let store = InMemorySleepSessionStore(initialState: storedState)
        let controller = SleepSessionController(
            executor: RecordingPowerCommandExecutor(),
            store: store
        )

        expect(controller.state == storedState, "session controller loads persisted state")
    }

    private mutating func testSessionStatePersistsAfterStartAndStop() {
        let executor = RecordingPowerCommandExecutor()
        let store = InMemorySleepSessionStore()
        let controller = SleepSessionController(executor: executor, store: store)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .timed(60), now: now)
            try controller.stop()
        } catch {
            failures.append("FAILED: persisting session state threw \(error)")
            return
        }

        expect(
            store.savedStates == [
                .inactive,
                .active(startedAt: now, endsAt: Date(timeIntervalSince1970: 160)),
                .inactive
            ],
            "session controller persists start and stop state"
        )
    }

    private mutating func testSessionSyncReflectsExternalEnable() {
        let store = InMemorySleepSessionStore()
        let controller = SleepSessionController(
            executor: RecordingPowerCommandExecutor(),
            store: store
        )
        let now = Date(timeIntervalSince1970: 200)

        do {
            try controller.syncWithSystem(disableSleepIsEnabled: true, now: now)
        } catch {
            failures.append("FAILED: sync external enable threw \(error)")
            return
        }

        expect(
            controller.state == .active(startedAt: now, endsAt: nil),
            "session sync reflects externally enabled sleep hold"
        )
    }

    private mutating func testSessionSyncClearsStaleActiveState() {
        let storedState = SleepControlState.active(
            startedAt: Date(timeIntervalSince1970: 10),
            endsAt: nil
        )
        let store = InMemorySleepSessionStore(initialState: storedState)
        let controller = SleepSessionController(
            executor: RecordingPowerCommandExecutor(),
            store: store
        )

        do {
            try controller.syncWithSystem(disableSleepIsEnabled: false)
        } catch {
            failures.append("FAILED: sync stale active state threw \(error)")
            return
        }

        expect(controller.state == .inactive, "session sync clears stale active state")
        expect(store.savedStates.last == .inactive, "session sync persists stale state cleanup")
    }

    private mutating func testAgentSessionsCountNativeBinaries() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(id: 100, parentID: 1, executableName: "claude"),
            RunningProcess(id: 101, parentID: 2, executableName: "claude"),
            RunningProcess(id: 102, parentID: 1, executableName: "codex"),
            RunningProcess(id: 103, parentID: 1, executableName: "opencode"),
            RunningProcess(id: 104, parentID: 1, executableName: "zsh")
        ])

        expect(
            counts == [.claudeCode: 2, .codex: 1, .openCode: 1],
            "native harness binaries are counted one session per process"
        )
    }

    private mutating func testAgentSessionsCountScriptRuntimeInstalls() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(
                id: 200,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@anthropic-ai/claude-code/cli.js"]
            ),
            RunningProcess(
                id: 201,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "--no-warnings", "/repo/node_modules/.bin/claude"]
            ),
            RunningProcess(
                id: 202,
                parentID: 1,
                executableName: "bun",
                arguments: ["bun", "/opt/homebrew/lib/node_modules/opencode-ai/bin/opencode"]
            )
        ])

        expect(
            counts == [.claudeCode: 2, .openCode: 1],
            "npm installs running under a JavaScript runtime are detected from arguments"
        )
    }

    private mutating func testAgentSessionsIgnoreUnrelatedProcesses() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(id: 300, parentID: 1, executableName: "zsh"),
            RunningProcess(id: 301, parentID: 1, executableName: "node", arguments: ["node", "/srv/claude-dashboard/server.js"]),
            RunningProcess(id: 302, parentID: 1, executableName: "node")
        ])

        expect(counts.isEmpty, "unrelated processes do not produce agent sessions")
    }

    private mutating func testAgentSessionsSkipSameHarnessChildren() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(
                id: 400,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@openai/codex/bin/codex.js"]
            ),
            RunningProcess(id: 401, parentID: 400, executableName: "codex"),
            RunningProcess(id: 402, parentID: 1, executableName: "opencode"),
            RunningProcess(id: 403, parentID: 402, executableName: "sh"),
            RunningProcess(id: 404, parentID: 403, executableName: "opencode")
        ])

        expect(
            counts == [.codex: 1, .openCode: 1],
            "helper children of the same harness are not counted as extra sessions"
        )
    }

    private mutating func testAgentSessionsCountNestedDifferentHarnesses() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(id: 500, parentID: 1, executableName: "claude"),
            RunningProcess(id: 501, parentID: 500, executableName: "codex")
        ])

        expect(
            counts == [.claudeCode: 1, .codex: 1],
            "a harness launched inside another harness still counts as a session"
        )
    }
}

private final class RecordingPowerCommandExecutor: PowerCommandExecuting, @unchecked Sendable {
    private(set) var commands: [Bool] = []

    func setDisableSleep(_ enabled: Bool) {
        commands.append(enabled)
    }
}

var runner = TestRunner()
runner.run()
