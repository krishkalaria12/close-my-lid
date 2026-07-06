import AppKit
import CloseMyLidCore

@main
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var menuController: StatusMenuController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        menuController = StatusMenuController(
            sleepController: SleepSessionController(executor: AdminShellPowerCommandExecutor())
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        try? menuController?.stopSession()
    }
}

@MainActor
final class StatusMenuController: NSObject {
    private let statusItem: NSStatusItem
    private let sleepController: SleepSessionController
    private var refreshTimer: Timer?

    init(sleepController: SleepSessionController) {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.sleepController = sleepController
        super.init()

        configureStatusItem()
        rebuildMenu()
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in
                try? self?.sleepController.stopIfExpired()
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

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "Quit Close My Lid", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem.menu = menu
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
