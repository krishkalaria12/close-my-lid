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
                .timed(30 * 60),
                .timed(60 * 60),
                .timed(4 * 60 * 60),
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
}

private final class RecordingPowerCommandExecutor: PowerCommandExecuting, @unchecked Sendable {
    private(set) var commands: [Bool] = []

    func setDisableSleep(_ enabled: Bool) {
        commands.append(enabled)
    }
}

var runner = TestRunner()
runner.run()
