import AppKit
import CloseMyLidCore

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var menuController: StatusMenuController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let powerManager = PmsetPowerManager()
        NSApp.setActivationPolicy(.accessory)
        menuController = StatusMenuController(
            sleepController: SleepSessionController(executor: powerManager),
            powerSettingsReader: powerManager
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        try? menuController?.stopSession()
    }
}

@MainActor
enum CommandLineInterface {
    static let version = "0.1.0"

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
            }
        } catch {
            print("close-my-lid: \(error.localizedDescription)")
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
}

@MainActor
private func launchMenuApp() -> Int32 {
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    app.run()
    return 0
}

exit(CommandLineInterface.run(arguments: Array(CommandLine.arguments.dropFirst())))

@MainActor
final class StatusMenuController: NSObject {
    private let statusItem: NSStatusItem
    private let sleepController: SleepSessionController
    private let powerSettingsReader: PowerSettingsReading
    private let launchAtLoginController: LaunchAtLoginController
    private var refreshTimer: Timer?

    init(
        sleepController: SleepSessionController,
        powerSettingsReader: PowerSettingsReading,
        launchAtLoginController: LaunchAtLoginController = LaunchAtLoginController()
    ) {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.sleepController = sleepController
        self.powerSettingsReader = powerSettingsReader
        self.launchAtLoginController = launchAtLoginController
        super.init()

        configureStatusItem()
        syncSessionState()
        rebuildMenu()
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.syncSessionState()
                self?.rebuildMenu()
            }
        }
    }

    func stopSession() throws {
        try sleepController.stop()
        rebuildMenu()
    }

    private func configureStatusItem() {
        guard let button = statusItem.button else {
            return
        }

        button.image = NSImage(systemSymbolName: "laptopcomputer", accessibilityDescription: "Close My Lid")
        button.image?.isTemplate = true
        button.toolTip = "Close My Lid"
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        let heading = NSMenuItem(title: sleepController.state.statusText(), action: nil, keyEquivalent: "")
        heading.isEnabled = false
        menu.addItem(heading)
        menu.addItem(.separator())

        for duration in SessionDuration.menuPresets {
            let item = NSMenuItem(title: "Start \(duration.title)", action: #selector(startSession(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = duration
            menu.addItem(item)
        }

        let stopItem = NSMenuItem(title: "Stop Holding", action: #selector(stopSessionFromMenu), keyEquivalent: "")
        stopItem.target = self
        stopItem.isEnabled = sleepController.state.isActive
        menu.addItem(stopItem)

        menu.addItem(.separator())

        let powerSettings = NSMenuItem(title: "Open Battery Settings", action: #selector(openBatterySettings), keyEquivalent: "")
        powerSettings.target = self
        menu.addItem(powerSettings)

        let launchAtLogin = NSMenuItem(title: "Launch at Login", action: #selector(toggleLaunchAtLogin), keyEquivalent: "")
        launchAtLogin.target = self
        launchAtLogin.state = launchAtLoginController.isEnabled ? .on : .off
        menu.addItem(launchAtLogin)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "Quit Close My Lid", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem.menu = menu
    }

    private func syncSessionState() {
        do {
            try sleepController.syncWithSystem(
                disableSleepIsEnabled: powerSettingsReader.disableSleepIsEnabled()
            )
        } catch {
            try? sleepController.stopIfExpired()
        }
    }

    @objc private func startSession(_ sender: NSMenuItem) {
        guard let duration = sender.representedObject as? SessionDuration else {
            return
        }

        do {
            try sleepController.start(duration: duration)
        } catch {
            showError(error)
        }

        rebuildMenu()
    }

    @objc private func stopSessionFromMenu() {
        do {
            try stopSession()
        } catch {
            showError(error)
        }
    }

    @objc private func openBatterySettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.Battery-Settings.extension") else {
            return
        }

        NSWorkspace.shared.open(url)
    }

    @objc private func toggleLaunchAtLogin() {
        do {
            try launchAtLoginController.setEnabled(!launchAtLoginController.isEnabled)
        } catch {
            showError(error)
        }

        rebuildMenu()
    }

    @objc private func quit() {
        try? stopSession()
        NSApp.terminate(nil)
    }

    private func showError(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = "Close My Lid could not update sleep settings."
        alert.informativeText = error.localizedDescription
        alert.alertStyle = .warning
        alert.runModal()
    }
}
