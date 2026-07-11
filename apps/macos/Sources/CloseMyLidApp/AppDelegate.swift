import AppKit
import CloseMyLidCore

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var menuController: StatusMenuController?
    private var updateController: UpdateController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let powerManager = PmsetPowerManager()
        let updateController = UpdateController()
        self.updateController = updateController
        NSApp.setActivationPolicy(.accessory)
        menuController = StatusMenuController(
            sleepController: SleepSessionController(executor: powerManager),
            powerSettingsReader: powerManager,
            updateController: updateController
        )
    }

    func applicationWillTerminate(_ notification: Notification) {
        try? menuController?.stopSession()
    }
}
