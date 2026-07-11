import Foundation
import Sparkle

@MainActor
final class UpdateController: NSObject, ObservableObject, SPUUpdaterDelegate {
    private static let noUpdateErrorCode = 1001 // SUNoUpdateError from Sparkle's SUErrors.h

    @Published private(set) var availableVersion: String?
    @Published private(set) var canCheckForUpdates = false

    private var informationTimer: Timer?
    private lazy var updaterController = SPUStandardUpdaterController(
        startingUpdater: true,
        updaterDelegate: self,
        userDriverDelegate: nil
    )

    override init() {
        super.init()

        // Materialize and start the controller only after self is initialized,
        // because Sparkle keeps its delegate weakly.
        let updater = updaterController.updater
        canCheckForUpdates = updater.canCheckForUpdates

        updater.publisher(for: \SPUUpdater.canCheckForUpdates)
            .receive(on: RunLoop.main)
            .assign(to: &$canCheckForUpdates)

        Timer.scheduledTimer(withTimeInterval: 2, repeats: false) { [weak self] _ in
            Task { @MainActor in
                self?.checkForUpdateInformation()
            }
        }
        informationTimer = Timer.scheduledTimer(withTimeInterval: 6 * 60 * 60, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.checkForUpdateInformation()
            }
        }
    }

    func checkForUpdates() {
        updaterController.checkForUpdates(nil)
    }

    private func checkForUpdateInformation() {
        let updater = updaterController.updater
        guard updater.canCheckForUpdates else {
            return
        }

        // This probe updates the menu badge without presenting Sparkle's UI.
        // The user remains in control of download, installation, and relaunch.
        updater.checkForUpdateInformation()
    }

    func updater(_ updater: SPUUpdater, didFindValidUpdate item: SUAppcastItem) {
        availableVersion = item.displayVersionString
    }

    func updaterDidNotFindUpdate(_ updater: SPUUpdater, error: any Error) {
        let error = error as NSError
        if error.domain == SUSparkleErrorDomain && error.code == Self.noUpdateErrorCode {
            availableVersion = nil
        }
    }
}
