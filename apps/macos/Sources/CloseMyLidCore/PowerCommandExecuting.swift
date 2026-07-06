import Foundation

public protocol PowerCommandExecuting: Sendable {
    func setDisableSleep(_ enabled: Bool) throws
}

public enum PowerCommandError: Error, Equatable, LocalizedError {
    case commandFailed(status: Int32, output: String)

    public var errorDescription: String? {
        switch self {
        case let .commandFailed(status, output):
            "pmset command failed with status \(status): \(output)"
        }
    }
}

public final class AdminShellPowerCommandExecutor: PowerCommandExecuting {
    public init() {}

    public func setDisableSleep(_ enabled: Bool) throws {
        let value = enabled ? "1" : "0"
        let script = #"do shell script "/usr/bin/pmset -a disablesleep \#(value)" with administrator privileges"#

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]

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
    }
}
