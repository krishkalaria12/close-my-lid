import Foundation

public protocol PowerSettingsReading: Sendable {
    func disableSleepIsEnabled() throws -> Bool
}

public enum PowerSettingsParser {
    public static func disableSleepIsEnabled(from output: String) -> Bool {
        output
            .split(separator: "\n")
            .contains { line in
                let fields = line.split(whereSeparator: \.isWhitespace)
                return fields.count >= 2 && fields[0] == "disablesleep" && fields[1] == "1"
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
}
