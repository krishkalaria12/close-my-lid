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
        testBatterySafetyPolicyDefaults()
        testBatterySafetyPolicyIgnoresChargingMac()
        testLowBatteryReleasesActiveHold()
        testLowBatteryLeavesChargingHoldAlone()
        testHealthyBatteryLeavesHoldAlone()
        testLowBatteryIgnoresInactiveController()
        testNotificationLeadTimeIsFiveMinutes()
        testNotificationPlanForIndefiniteSession()
        testNotificationPlanForTimedSession()
        testNotificationPlanOmitsEndingSoonForShortSession()
        testNotificationPlanOmitsEndingSoonAtLeadTimeBoundary()
        testAgentSessionsCountNativeBinaries()
        testAgentSessionsCountScriptRuntimeInstalls()
        testAgentSessionsDetectPackagePathWithoutBinaryBasename()
        testAgentSessionsIgnoreUnrelatedProcesses()
        testAgentSessionsSkipSameHarnessChildren()
        testAgentSessionsCountNestedDifferentHarnesses()
        testAgentSessionsCountNewNativeBinaries()
        testAgentSessionsCountNewScriptRuntimeInstalls()
        testAgentSessionsDetectPiAlternatePackageScope()

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

    private mutating func testBatterySafetyPolicyDefaults() {
        let policy = BatterySafetyPolicy()
        expect(policy.threshold == 15, "battery safety threshold defaults to 15 percent")
        expect(
            policy.shouldReleaseHold(percentage: 15, isCharging: false),
            "battery at the threshold releases the hold"
        )
        expect(
            policy.shouldReleaseHold(percentage: 5, isCharging: false),
            "battery below the threshold releases the hold"
        )
        expect(
            !policy.shouldReleaseHold(percentage: 16, isCharging: false),
            "battery above the threshold keeps the hold"
        )
    }

    private mutating func testBatterySafetyPolicyIgnoresChargingMac() {
        let policy = BatterySafetyPolicy(threshold: 20)
        expect(
            !policy.shouldReleaseHold(percentage: 3, isCharging: true),
            "a charging Mac keeps the hold even on low battery"
        )
    }

    private mutating func testLowBatteryReleasesActiveHold() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .indefinitely, now: now)
            let released = try controller.stopIfBatteryLow(percentage: 10, isCharging: false)
            expect(released, "a low battery reports that it released the hold")
        } catch {
            failures.append("FAILED: low battery release threw \(error)")
            return
        }

        expect(executor.commands == [true, false], "a low battery restores normal sleep")
        expect(controller.state == .inactive, "a low battery makes the controller inactive")
    }

    private mutating func testLowBatteryLeavesChargingHoldAlone() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .indefinitely, now: now)
            let released = try controller.stopIfBatteryLow(percentage: 5, isCharging: true)
            expect(!released, "a charging Mac does not release the hold")
        } catch {
            failures.append("FAILED: charging low battery flow threw \(error)")
            return
        }

        expect(executor.commands == [true], "a charging Mac leaves sleep disabled")
        expect(controller.state.isActive, "a charging Mac keeps the hold active")
    }

    private mutating func testHealthyBatteryLeavesHoldAlone() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(executor: executor)
        let now = Date(timeIntervalSince1970: 100)

        do {
            try controller.start(duration: .indefinitely, now: now)
            let released = try controller.stopIfBatteryLow(percentage: 80, isCharging: false)
            expect(!released, "a healthy battery does not release the hold")
        } catch {
            failures.append("FAILED: healthy battery flow threw \(error)")
            return
        }

        expect(executor.commands == [true], "a healthy battery leaves sleep disabled")
        expect(controller.state.isActive, "a healthy battery keeps the hold active")
    }

    private mutating func testLowBatteryIgnoresInactiveController() {
        let executor = RecordingPowerCommandExecutor()
        let controller = SleepSessionController(
            executor: executor,
            store: InMemorySleepSessionStore()
        )

        do {
            let released = try controller.stopIfBatteryLow(percentage: 2, isCharging: false)
            expect(!released, "an inactive controller has no hold to release")
        } catch {
            failures.append("FAILED: inactive low battery flow threw \(error)")
            return
        }

        expect(executor.commands.isEmpty, "an inactive controller issues no power commands")
        expect(controller.state == .inactive, "an inactive controller stays inactive")
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

    private mutating func testNotificationLeadTimeIsFiveMinutes() {
        expect(
            SessionNotificationPlanner.endingSoonLeadTime == 5 * 60,
            "ending-soon warning leads the end by five minutes"
        )
    }

    private mutating func testNotificationPlanForIndefiniteSession() {
        let startedAt = Date(timeIntervalSince1970: 100)
        let plan = SessionNotificationPlanner.plan(duration: .indefinitely, startedAt: startedAt)

        expect(plan.startTitle == "Close My Lid", "start notification uses the product title")
        expect(
            plan.startBody == "Your Mac will stay awake with the lid closed until you stop it.",
            "indefinite start copy explains it runs until stopped"
        )
        expect(plan.endingSoon == nil, "indefinite sessions have no ending-soon warning")
        expect(plan.ended == nil, "indefinite sessions have no scheduled end notification")
    }

    private mutating func testNotificationPlanForTimedSession() {
        let startedAt = Date(timeIntervalSince1970: 100)
        let plan = SessionNotificationPlanner.plan(
            duration: .timed(SessionDuration.thirtyMinutes),
            startedAt: startedAt
        )

        expect(
            plan.startBody == "Your Mac will stay awake with the lid closed for the next 30 minutes.",
            "timed start copy names the session length"
        )
        expect(
            plan.ended?.fireDate == Date(timeIntervalSince1970: 1_900),
            "the end notification fires at the session end"
        )
        expect(
            plan.ended?.body == "Your Mac now sleeps normally when the lid is closed.",
            "the end notification explains normal sleep is restored"
        )
        expect(
            plan.endingSoon?.fireDate == Date(timeIntervalSince1970: 1_600),
            "the ending-soon warning fires five minutes before the end"
        )
        expect(
            plan.endingSoon?.title == "Close My Lid",
            "scheduled notifications carry the product title"
        )
        expect(
            plan.endingSoon?.body == "About 5 minutes left before your Mac sleeps normally with the lid closed.",
            "the ending-soon warning explains the upcoming sleep behavior"
        )
    }

    private mutating func testNotificationPlanOmitsEndingSoonForShortSession() {
        let startedAt = Date(timeIntervalSince1970: 100)
        let plan = SessionNotificationPlanner.plan(duration: .timed(240), startedAt: startedAt)

        expect(
            plan.endingSoon == nil,
            "sessions shorter than the lead time skip the ending-soon warning"
        )
        expect(
            plan.ended?.fireDate == Date(timeIntervalSince1970: 340),
            "short sessions still schedule an end notification"
        )
    }

    private mutating func testNotificationPlanOmitsEndingSoonAtLeadTimeBoundary() {
        let startedAt = Date(timeIntervalSince1970: 100)
        let plan = SessionNotificationPlanner.plan(
            duration: .timed(SessionNotificationPlanner.endingSoonLeadTime),
            startedAt: startedAt
        )

        expect(
            plan.endingSoon == nil,
            "sessions exactly as long as the lead time skip the ending-soon warning"
        )
        expect(
            plan.ended?.fireDate == Date(timeIntervalSince1970: 400),
            "lead-time-length sessions still schedule an end notification"
        )
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

    private mutating func testAgentSessionsDetectPackagePathWithoutBinaryBasename() {
        // The script path's basename ("codex.js") is not a harness name, so this
        // exercises the scriptPathMarkers branch on its own, with no native child
        // to supply the count in its place.
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(
                id: 600,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@openai/codex/bin/codex.js"]
            )
        ])

        expect(
            counts == [.codex: 1],
            "an npm install matched only by its package-path marker is counted"
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

    private mutating func testAgentSessionsCountNewNativeBinaries() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(id: 700, parentID: 1, executableName: "gemini"),
            RunningProcess(id: 701, parentID: 1, executableName: "copilot"),
            RunningProcess(id: 702, parentID: 1, executableName: "cursor-agent"),
            RunningProcess(id: 703, parentID: 1, executableName: "pi")
        ])

        expect(
            counts == [.gemini: 1, .copilot: 1, .cursor: 1, .pi: 1],
            "newly added native harness binaries are counted one session per process"
        )
    }

    private mutating func testAgentSessionsCountNewScriptRuntimeInstalls() {
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(
                id: 710,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@google/gemini-cli/dist/index.js"]
            ),
            RunningProcess(
                id: 711,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@github/copilot/index.js"]
            ),
            RunningProcess(
                id: 712,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/opt/homebrew/lib/node_modules/@earendil-works/pi-coding-agent/dist/cli.js"]
            )
        ])

        expect(
            counts == [.gemini: 1, .copilot: 1, .pi: 1],
            "npm installs of the new harnesses are detected from their package paths"
        )
    }

    private mutating func testAgentSessionsDetectPiAlternatePackageScope() {
        // Pi migrated npm scopes; the earlier @mariozechner package still
        // resolves and must keep matching alongside @earendil-works.
        let counts = AgentSessionClassifier.sessionCounts(in: [
            RunningProcess(
                id: 720,
                parentID: 1,
                executableName: "node",
                arguments: ["node", "/usr/local/lib/node_modules/@mariozechner/pi-coding-agent/dist/cli.js"]
            )
        ])

        expect(
            counts == [.pi: 1],
            "an npm install under Pi's earlier package scope is still counted"
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
