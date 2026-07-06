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

    public init(adminExecutor: AdminShellPowerCommandExecutor = AdminShellPowerCommandExecutor()) {
        self.adminExecutor = adminExecutor
    }

    public func setDisableSleep(_ enabled: Bool) throws {
        try adminExecutor.setDisableSleep(enabled)
    }

    public func disableSleepIsEnabled() throws -> Bool {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/pmset")
        process.arguments = ["-g"]

        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = outputPipe

        try process.run()
        process.waitUntilExit()

        let output = String(
            data: outputPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        )?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        guard process.terminationStatus == 0 else {
            throw PowerCommandError.commandFailed(status: process.terminationStatus, output: output)
        }

        return PowerSettingsParser.disableSleepIsEnabled(from: output)
    }
}
