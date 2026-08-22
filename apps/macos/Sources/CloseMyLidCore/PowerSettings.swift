import Foundation

public protocol PowerSettingsReading: Sendable {
    func disableSleepIsEnabled() throws -> Bool

    /// Same as `disableSleepIsEnabled()` but never blocks the calling thread;
    /// the `pmset` process runs detached so UI callers can await it.
    func disableSleepIsEnabledAsync() async throws -> Bool
}

public enum PowerSettingsParser {
    /// Keys that report the closed-lid sleep disable flag in `pmset` output.
    ///
    /// `pmset -g` prints the setting as `SleepDisabled` (tab-separated, e.g.
    /// `"\t SleepDisabled \t\t 1"`), while some tooling and older references
    /// show `disablesleep`. Both are accepted, case-insensitively, so a macOS
    /// wording change cannot silently strand a hold again.
    static func reportsSleepDisabled(_ field: some StringProtocol) -> Bool {
        let key = field.lowercased()
        return key == "sleepdisabled" || key == "disablesleep"
    }

    public static func disableSleepIsEnabled(from output: String) -> Bool {
        output
            .split(separator: "\n")
            .contains { line in
                let fields = line.split(whereSeparator: \.isWhitespace)
                return fields.count >= 2
                    && reportsSleepDisabled(fields[0])
                    && fields[1].lowercased() == "1"
            }
    }
}

public final class PmsetPowerManager: PowerCommandExecuting, PowerSettingsReading {
    private let adminExecutor: AdminShellPowerCommandExecutor
    private let commandRunner: ShellCommandRunner

    public init(
        adminExecutor: AdminShellPowerCommandExecutor = AdminShellPowerCommandExecutor(),
        commandRunner: ShellCommandRunner = ShellCommandRunner()
    ) {
        self.adminExecutor = adminExecutor
        self.commandRunner = commandRunner
    }

    public func setDisableSleep(_ enabled: Bool) throws {
        try adminExecutor.setDisableSleep(enabled)
    }

    public func disableSleepIsEnabled() throws -> Bool {
        let result = try commandRunner.run(
            executablePath: "/usr/bin/pmset",
            arguments: ["-g"]
        )

        guard result.status == 0 else {
            throw PowerCommandError.commandFailed(status: result.status, output: result.output)
        }

        return PowerSettingsParser.disableSleepIsEnabled(from: result.output)
    }

    public func disableSleepIsEnabledAsync() async throws -> Bool {
        let runner = commandRunner

        let result = try await Task.detached(priority: .utility) {
            try runner.run(executablePath: "/usr/bin/pmset", arguments: ["-g"])
        }.value

        guard result.status == 0 else {
            throw PowerCommandError.commandFailed(status: result.status, output: result.output)
        }

        return PowerSettingsParser.disableSleepIsEnabled(from: result.output)
    }
}
