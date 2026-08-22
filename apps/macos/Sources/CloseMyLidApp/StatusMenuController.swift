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
    private let selectedDurationStore: SelectedDurationStoring
    private let heartbeatStore: HoldHeartbeatStore
    private let watchdogAgent: WatchdogAgentController
    private var panelController: MenuBarPanelController?
    private var settingsWindowController: SettingsWindowController?
    private var refreshTimer: Timer?
    private var expiryTimer: Timer?
    private var wasActive: Bool
    private var wakeRestorePending = false
    private var wakeRestoreReady = false

    init(
        sleepController: SleepSessionController,
        powerSettingsReader: PowerSettingsReading,
        updateController: UpdateController,
        launchAtLoginController: LaunchAtLoginController = LaunchAtLoginController(),
        notifier: SessionNotifying = SessionNotificationScheduler(),
        selectedDurationStore: SelectedDurationStoring = UserDefaultsSelectedDurationStore(),
        heartbeatStore: HoldHeartbeatStore = HoldHeartbeatStore(),
        watchdogAgent: WatchdogAgentController = WatchdogAgentController()
    ) {
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.sleepController = sleepController
        self.powerSettingsReader = powerSettingsReader
        self.updateController = updateController
        self.launchAtLoginController = launchAtLoginController
        self.notifier = notifier
        self.selectedDurationStore = selectedDurationStore
        self.heartbeatStore = heartbeatStore
        self.watchdogAgent = watchdogAgent
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
        observeSystemWake()
        scheduleRefreshTimer()
        syncSessionState()
    }

    func stopSession() throws {
        expiryTimer?.invalidate()
        expiryTimer = nil
        try sleepController.stop()
        heartbeatStore.remove()
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

    private func observeSystemWake() {
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(systemDidWake),
            name: NSWorkspace.didWakeNotification,
            object: nil
        )
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(systemWillSleep),
            name: NSWorkspace.willSleepNotification,
            object: nil
        )
    }

    // MARK: - Reconciliation

    private func syncSessionState() {
        Task { [weak self] in
            await self?.performSyncCycle()
        }
    }

    /// One reconciliation pass. The `pmset -g` read runs detached so the main
    /// thread never blocks on a process spawn; state changes stay on the main
    /// actor.
    private func performSyncCycle() async {
        if wakeRestorePending {
            enforceBatterySafety()
            if wakeRestoreReady {
                await restoreAfterWakeIfNeeded()
            }
            recordHeartbeatIfHolding()
            updateSessionTransition()
            return
        }

        do {
            let systemEnabled = try await powerSettingsReader.disableSleepIsEnabledAsync()
            try sleepController.syncWithSystem(disableSleepIsEnabled: systemEnabled)
        } catch where (error as? PowerCommandError) == .elevationCancelled {
            // The user dismissed the admin dialog during an expiry release;
            // leave the hold in place and retry on the next cycle.
        } catch {
            try? sleepController.stopIfExpired()
        }

        enforceBatterySafety()
        recordHeartbeatIfHolding()
        armExpiryTimerIfNeeded()
        ensureWatchdogInstalledIfNeeded()
        updateSessionTransition()
    }

    @objc private func systemDidWake() {
        wakeRestorePending = sleepController.state.isActive
        wakeRestoreReady = wakeRestorePending
        enforceBatterySafety()

        if wakeRestorePending {
            Task { [weak self] in
                await self?.restoreAfterWakeIfNeeded()
            }
        } else {
            updateSessionTransition()
        }
    }

    @objc private func systemWillSleep() {
        wakeRestorePending = sleepController.state.isActive
        wakeRestoreReady = false
    }

    private func restoreAfterWakeIfNeeded() async {
        do {
            let systemEnabled = try await powerSettingsReader.disableSleepIsEnabledAsync()
            try sleepController.restoreAfterWake(disableSleepIsEnabled: systemEnabled)
            wakeRestorePending = false
            wakeRestoreReady = false
        } catch {
            try? sleepController.stopIfExpired()
            if !sleepController.state.isActive {
                wakeRestorePending = false
                wakeRestoreReady = false
            }
        }

        recordHeartbeatIfHolding()
        armExpiryTimerIfNeeded()
        updateSessionTransition()
    }

    private func updateSessionTransition() {
        // A timed session's "ended" message is delivered by the system at its
        // fire date; when reconciliation observes the hold turning off, clear
        // any remaining pending requests.
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

    // MARK: - Heartbeat

    private func writeHeartbeat(for state: SleepControlState) {
        guard case let .active(_, endsAt) = state else {
            return
        }

        try? heartbeatStore.write(HoldHeartbeat(endsAt: endsAt, updatedAt: Date()))
    }

    private func recordHeartbeatIfHolding() {
        if sleepController.state.isActive {
            writeHeartbeat(for: sleepController.state)
        } else {
            heartbeatStore.remove()
        }
    }

    // MARK: - Precise session end

    /// Fires a one-shot timer exactly at the session's end so timed holds are
    /// released on time instead of up to a full poll interval late.
    private func armExpiryTimerIfNeeded() {
        armExpiryTimer(for: sleepController.state, now: Date())
    }

    private func armExpiryTimer(for state: SleepControlState, now: Date) {
        expiryTimer?.invalidate()
        expiryTimer = nil

        guard case let .active(_, .some(endsAt)) = state else {
            return
        }

        let interval = max(endsAt.timeIntervalSince(now) + 0.5, 0.5)
        let timer = Timer(timeInterval: interval, repeats: false) { [weak self] _ in
            Task { @MainActor in
                self?.syncSessionState()
            }
        }
        timer.tolerance = 1
        RunLoop.main.add(timer, forMode: .common)
        expiryTimer = timer
    }

    // MARK: - Watchdog agent

    /// Registers the dead-man LaunchAgent once the passwordless grant exists;
    /// without the grant the agent could not release anything anyway.
    private func ensureWatchdogInstalledIfNeeded() {
        guard SudoersProvisioning.isInstalled(), !watchdogAgent.isInstalled else {
            return
        }

        try? watchdogAgent.install()
    }

    // MARK: - Actions

    @objc private func togglePanel() {
        guard let button = statusItem.button else {
            return
        }

        panelController?.toggle(relativeTo: button)
    }

    private func setHolding(_ holding: Bool) {
        do {
            if holding {
                beginSession(selectedDurationStore.load())
            } else {
                expiryTimer?.invalidate()
                expiryTimer = nil
                try sleepController.stop()
                heartbeatStore.remove()
                notifier.cancelPending()
                wasActive = false
            }
        } catch {
            showError(error)
        }
    }

    private func startSession(_ duration: SessionDuration) {
        beginSession(duration)
    }

    private func beginSession(_ duration: SessionDuration) {
        do {
            let now = Date()
            try sleepController.start(duration: duration, now: now)
            selectedDurationStore.save(duration)
            writeHeartbeat(for: sleepController.state)
            notifier.apply(SessionNotificationPlanner.plan(duration: duration, startedAt: now))
            wasActive = true
            armExpiryTimer(for: sleepController.state, now: now)
            ensureWatchdogInstalledIfNeeded()
        } catch {
            showError(error)
        }
    }

    private func openSettings() {
        if settingsWindowController == nil {
            settingsWindowController = SettingsWindowController(
                launchAtLogin: launchAtLoginController,
                privilegedExecutor: AdminShellPowerCommandExecutor(),
                watchdogAgent: watchdogAgent
            )
        }

        settingsWindowController?.show()
    }

    private func quit() {
        try? stopSession()
        NSApp.terminate(nil)
    }

    private func showError(_ error: Error) {
        let alert = NSAlert()

        if case PowerCommandError.elevationCancelled = error {
            alert.messageText = "Administrator approval cancelled"
            alert.informativeText = "Close My Lid needs one-time administrator approval to keep the Mac awake with the lid closed. Try again and enter your password when asked."
        } else {
            alert.messageText = "Close My Lid could not update sleep settings."
            alert.informativeText = error.localizedDescription
        }

        alert.alertStyle = .warning
        alert.runModal()
    }
}
