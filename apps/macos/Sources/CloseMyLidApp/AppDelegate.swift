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
