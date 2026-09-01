import AppKit
import CloseMyLidCore
import Foundation

@MainActor
enum CommandLineInterface {
    static let version = "0.4.3"

    static func run(arguments: [String]) -> Int32 {
        guard let action = CommandLineActionParser.parse(arguments) else {
            print("Unknown command: \(arguments.first ?? "")")
            print(helpText)
            return 64
        }

        let powerManager = PmsetPowerManager()

        do {
            switch action {
            case .launchMenu:
                return launchMenuApp()
            case .enable:
                try powerManager.setDisableSleep(true)
                print("Close My Lid is holding closed-lid sleep.")
                return 0
            case .disable:
                try powerManager.setDisableSleep(false)
                print("Close My Lid restored normal closed-lid sleep.")
                return 0
            case .status:
                let status = try powerManager.disableSleepIsEnabled() ? "enabled" : "disabled"
                print("closed-lid sleep hold: \(status)")
                return 0
            case .help:
                print(helpText)
                return 0
            case .version:
                print("Close My Lid \(version)")
                return 0
            case .watchdog:
                return runWatchdogPass(powerManager: powerManager)
            }
        } catch {
            print("close-my-lid: \(error.localizedDescription)")
            return 1
        }
    }

    /// One dead-man pass for the LaunchAgent. Deliberately silent: launchd
    /// runs this every minute and output would only fill logs. Uses the
    /// passwordless-only executor so a missing sudoers grant can never spawn
    /// an administrator dialog from a headless agent.
    private static func runWatchdogPass(powerManager: PmsetPowerManager) -> Int32 {
        do {
            try WatchdogRunner.runOnce(
                heartbeatStore: HoldHeartbeatStore(),
                policy: WatchdogPolicy(),
                powerSettingsReader: powerManager,
                executor: PasswordlessPowerCommandExecutor()
            )
            return 0
        } catch {
            // Leave the heartbeat for the next tick; a transient failure
            // should not strand or spam.
            return 1
        }
    }

    private static var helpText: String {
        """
        Close My Lid \(version)

        Usage:
          close-my-lid              Launch the menu bar app
          close-my-lid enable       Disable closed-lid sleep
          close-my-lid disable      Restore normal closed-lid sleep
          close-my-lid status       Print the current closed-lid sleep hold status
          close-my-lid --version    Print the version
          close-my-lid --help       Show this help
        """
    }

    private static func launchMenuApp() -> Int32 {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
        return 0
    }
}
