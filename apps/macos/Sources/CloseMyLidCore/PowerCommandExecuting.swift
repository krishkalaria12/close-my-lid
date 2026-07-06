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
    private let commandRunner: ShellCommandRunner

    public init(commandRunner: ShellCommandRunner = ShellCommandRunner()) {
        self.commandRunner = commandRunner
    }

    public func setDisableSleep(_ enabled: Bool) throws {
        let value = enabled ? "1" : "0"
        let script = #"do shell script "/usr/bin/pmset -a disablesleep \#(value)" with administrator privileges"#

        let result = try commandRunner.run(
            executablePath: "/usr/bin/osascript",
            arguments: ["-e", script]
        )

        guard result.status == 0 else {
            throw PowerCommandError.commandFailed(status: result.status, output: result.output)
        }
    }
}
