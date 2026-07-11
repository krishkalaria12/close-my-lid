import AppKit
import CloseMyLidCore

@MainActor
final class StatusMenuController: NSObject {
    private let statusItem: NSStatusItem
    private let sleepController: SleepSessionController
    private let powerSettingsReader: PowerSettingsReading
    private let updateController: UpdateController
    private let launchAtLoginController: LaunchAtLoginController
    private let notifier: SessionNotifying
    private var panelController: MenuBarPanelController?
    private var settingsWindowController: SettingsWindowController?
    private var refreshTimer: Timer?
    private var wasActive: Bool

    init(
        sleepController: SleepSessionController,
        powerSettingsReader: PowerSettingsReading,
        updateController: UpdateController,
        launchAtLoginController: LaunchAtLoginController = LaunchAtLoginController(),
        notifier: SessionNotifying = SessionNotificationScheduler()
    ) {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.sleepController = sleepController
        self.powerSettingsReader = powerSettingsReader
        self.updateController = updateController
        self.launchAtLoginController = launchAtLoginController
        self.notifier = notifier
        self.wasActive = sleepController.state.isActive
        super.init()

        notifier.requestAuthorization()

        panelController = MenuBarPanelController(
            sleep: sleepController,
            actions: MenuPanelActions(
                setHolding: { [weak self] holding in self?.setHolding(holding) },
                hold: { [weak self] duration in self?.startSession(duration) },
                openSettings: { [weak self] in self?.openSettings() },
                quit: { [weak self] in self?.quit() }
            ),
            updates: self.updateController
        )

        configureStatusItem()
        syncSessionState()
        scheduleRefreshTimer()
    }

    func stopSession() throws {
        try sleepController.stop()
        notifier.cancelPending()
        wasActive = false
    }

    private func configureStatusItem() {
        guard let button = statusItem.button else {
            return
        }

        button.image = NSImage(systemSymbolName: "laptopcomputer", accessibilityDescription: "Close My Lid")
        button.image?.isTemplate = true
        button.toolTip = "Close My Lid"
        button.target = self
        button.action = #selector(togglePanel)
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
    }

    private func scheduleRefreshTimer() {
        refreshTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.syncSessionState()
            }
        }
    }

    private func syncSessionState() {
        do {
            try sleepController.syncWithSystem(
                disableSleepIsEnabled: powerSettingsReader.disableSleepIsEnabled()
            )
        } catch {
            try? sleepController.stopIfExpired()
        }

        enforceBatterySafety()

        // A timed session's "ended" message is delivered by the system at its
        // fire date; when the reconciliation timer observes the hold turning
        // off, clear any remaining pending requests.
        let isActive = sleepController.state.isActive
        if wasActive && !isActive {
            notifier.cancelPending()
        }
        wasActive = isActive
    }

    private func enforceBatterySafety() {
        guard let battery = BatteryStatusReader.read() else {
            return
        }

        _ = try? sleepController.stopIfBatteryLow(
            percentage: battery.percentage,
            isCharging: battery.isCharging
        )
    }

    @objc private func togglePanel() {
        guard let button = statusItem.button else {
            return
        }

        panelController?.toggle(relativeTo: button)
    }

    private func setHolding(_ holding: Bool) {
        do {
            if holding {
                try beginSession(.indefinitely)
            } else {
                try sleepController.stop()
                notifier.cancelPending()
                wasActive = false
            }
        } catch {
            showError(error)
        }
    }

    private func startSession(_ duration: SessionDuration) {
        do {
            try beginSession(duration)
        } catch {
            showError(error)
        }
    }

    private func beginSession(_ duration: SessionDuration) throws {
        let now = Date()
        try sleepController.start(duration: duration, now: now)
        notifier.apply(SessionNotificationPlanner.plan(duration: duration, startedAt: now))
        wasActive = true
    }

    private func openSettings() {
        if settingsWindowController == nil {
            settingsWindowController = SettingsWindowController(launchAtLogin: launchAtLoginController)
        }

        settingsWindowController?.show()
    }

    private func quit() {
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
