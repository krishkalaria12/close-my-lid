import AppKit
import CloseMyLidCore

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
        scheduleRefreshTimer()
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

    private func scheduleRefreshTimer() {
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.syncSessionState()
                self?.rebuildMenu()
            }
        }
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        let heading = NSMenuItem(title: sleepController.state.statusText(), action: nil, keyEquivalent: "")
        heading.isEnabled = false
        menu.addItem(heading)
        menu.addItem(.separator())

        for duration in SessionDuration.menuPresets {
            menu.addItem(startSessionItem(for: duration))
        }

        let stopItem = NSMenuItem(title: "Stop Holding", action: #selector(stopSessionFromMenu), keyEquivalent: "")
        stopItem.target = self
        stopItem.isEnabled = sleepController.state.isActive
        menu.addItem(stopItem)

        menu.addItem(.separator())
        menu.addItem(openBatterySettingsItem())
        menu.addItem(launchAtLoginItem())
        menu.addItem(.separator())
        menu.addItem(quitItem())

        statusItem.menu = menu
    }

    private func startSessionItem(for duration: SessionDuration) -> NSMenuItem {
        let item = NSMenuItem(title: "Start \(duration.title)", action: #selector(startSession(_:)), keyEquivalent: "")
        item.target = self
        item.representedObject = duration
        return item
    }

    private func openBatterySettingsItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Open Battery Settings", action: #selector(openBatterySettings), keyEquivalent: "")
        item.target = self
        return item
    }

    private func launchAtLoginItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Launch at Login", action: #selector(toggleLaunchAtLogin), keyEquivalent: "")
        item.target = self
        item.state = launchAtLoginController.isEnabled ? .on : .off
        return item
    }

    private func quitItem() -> NSMenuItem {
        let item = NSMenuItem(title: "Quit Close My Lid", action: #selector(quit), keyEquivalent: "q")
        item.target = self
        return item
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
