import Foundation

public protocol PowerCommandExecuting: Sendable {
    func setDisableSleep(_ enabled: Bool) throws
}

public enum PowerCommandError: Error, Equatable, LocalizedError {
    case commandFailed(status: Int32, output: String)
    case elevationCancelled

    public var errorDescription: String? {
        switch self {
        case let .commandFailed(status, output):
            "pmset command failed with status \(status): \(output)"
        case .elevationCancelled:
            "Administrator authorization was cancelled."
        }
    }
}

/// Applies closed-lid sleep settings strictly through the passwordless
/// allowlisted `sudo -n` command installed by `SudoersProvisioning`.
///
/// Never elevates interactively, so it is safe for headless contexts like the
/// watchdog LaunchAgent: when the grant is missing, releasing fails and leaves
/// the heartbeat for the next tick instead of spawning an administrator dialog
/// from a background agent.
public struct PasswordlessPowerCommandExecutor: PowerCommandExecuting {
    private let commandRunner: ShellCommandRunner

    public init(commandRunner: ShellCommandRunner = ShellCommandRunner()) {
        self.commandRunner = commandRunner
    }

    public func setDisableSleep(_ enabled: Bool) throws {
        let result = try commandRunner.run(
            executablePath: "/usr/bin/sudo",
            arguments: ["-n", "/usr/bin/pmset", "-a", "disablesleep", enabled ? "1" : "0"]
        )

        guard result.status == 0 else {
            throw PowerCommandError.commandFailed(status: result.status, output: result.output)
        }
    }
}

/// Applies closed-lid sleep settings with as few administrator prompts as
/// possible.
///
/// Strategy:
/// 1. Try the passwordless allowlisted command (`sudo -n pmset …`) that the
///    sudoers drop-in enables — silent, fast, no prompt once provisioned.
/// 2. On first use (or after the grant was removed), run one elevated script
///    that installs the drop-in, validates it with `visudo`, and applies the
///    setting — all within a single macOS admin dialog.
/// 3. If installing the drop-in is impossible (managed machine, visudo
///    refusal), fall back to the classic one-shot elevated `pmset` call so
///    hold sessions keep working without the convenience grant.
public final class AdminShellPowerCommandExecutor: PowerCommandExecuting {
    private let commandRunner: ShellCommandRunner

    public init(commandRunner: ShellCommandRunner = ShellCommandRunner()) {
        self.commandRunner = commandRunner
    }

    public func setDisableSleep(_ enabled: Bool) throws {
        if tryPasswordlessApply(enabled) {
            return
        }

        do {
            try runElevated(
                SudoersProvisioning.appleScriptPayload(
                    SudoersProvisioning.installScript(applySetting: enabled)
                )
            )
        } catch PowerCommandError.elevationCancelled {
            throw PowerCommandError.elevationCancelled
        } catch {
            // Grant installation refused; degrade to the legacy one-shot
            // elevation instead of failing the user's action.
            try runElevated(legacyApplyPayload(enabled))
        }
    }

    /// Installs only the passwordless grant (no sleep change). Used by the
    /// Settings window's Administrator Access controls.
    public func installPasswordlessGrant() throws {
        try runElevated(
            SudoersProvisioning.appleScriptPayload(
                SudoersProvisioning.installScript(applySetting: nil)
            )
        )
    }

    /// Removes the passwordless grant. Elevated because `/etc/sudoers.d` is
    /// root-owned.
    public func removePasswordlessGrant() throws {
        try runElevated(
            SudoersProvisioning.appleScriptPayload(
                SudoersProvisioning.uninstallScript()
            )
        )
    }

    /// Returns true when the passwordless allowlisted command succeeded.
    private func tryPasswordlessApply(_ enabled: Bool) -> Bool {
        guard
            let result = try? commandRunner.run(
                executablePath: "/usr/bin/sudo",
                arguments: ["-n", "/usr/bin/pmset", "-a", "disablesleep", enabled ? "1" : "0"]
            )
        else {
            return false
        }

        return result.status == 0
    }

    @discardableResult
    private func runElevated(_ payload: String) throws -> ShellCommandResult {
        let result = try commandRunner.run(
            executablePath: "/usr/bin/osascript",
            arguments: ["-e", payload]
        )

        guard result.status == 0 else {
            throw Self.classify(result)
        }

        return result
    }

    static func classify(_ result: ShellCommandResult) -> PowerCommandError {
        let output = result.output.lowercased()

        if output.contains("user canceled")
            || output.contains("user cancelled")
            || output.contains("(-128)")
        {
            return .elevationCancelled
        }

        return .commandFailed(status: result.status, output: result.output)
    }

    private func legacyApplyPayload(_ enabled: Bool) -> String {
        #"do shell script "/usr/bin/pmset -a disablesleep \#(enabled ? "1" : "0")" with administrator privileges"#
    }
}
