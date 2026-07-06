import AppKit
import CloseMyLidCore
import SwiftUI

private final class KeyablePanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

@MainActor
final class MenuBarPanelController: NSObject, NSWindowDelegate {
    private let panel: KeyablePanel
    private let hostingView: NSHostingView<MenuPanelView>
    private let model: MenuPanelModel
    private let actions: MenuPanelActions
    private var mouseMonitor: Any?
    private var keyMonitor: Any?
    private weak var statusButton: NSStatusBarButton?

    var isVisible: Bool { panel.isVisible }

    init(sleep: SleepSessionController, actions: MenuPanelActions) {
        self.model = MenuPanelModel()
        self.actions = actions
        self.hostingView = NSHostingView(
            rootView: MenuPanelView(sleep: sleep, model: model, actions: actions)
        )

        panel = KeyablePanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isMovable = false
        panel.hidesOnDeactivate = false

        let background = NSVisualEffectView()
        background.material = .popover
        background.blendingMode = .behindWindow
        background.state = .active
        background.wantsLayer = true
        background.layer?.cornerRadius = 14
        background.layer?.cornerCurve = .continuous
        background.layer?.masksToBounds = true

        hostingView.translatesAutoresizingMaskIntoConstraints = false
        background.addSubview(hostingView)
        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: background.leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: background.trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: background.topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: background.bottomAnchor)
        ])

        panel.contentView = background

        super.init()
        panel.delegate = self
    }

    func toggle(relativeTo button: NSStatusBarButton) {
        if panel.isVisible {
            close()
        } else {
            show(relativeTo: button)
        }
    }

    func show(relativeTo button: NSStatusBarButton) {
        statusButton = button
        model.refresh()

        let size = hostingView.fittingSize
        var origin = NSPoint(x: 0, y: 0)

        if let buttonWindow = button.window {
            let buttonFrame = buttonWindow.convertToScreen(button.convert(button.bounds, to: nil))
            origin.x = buttonFrame.midX - size.width / 2
            origin.y = buttonFrame.minY - size.height - 6

            if let screen = buttonWindow.screen ?? NSScreen.main {
                let visible = screen.visibleFrame
                origin.x = min(max(origin.x, visible.minX + 8), visible.maxX - size.width - 8)
                origin.y = max(origin.y, visible.minY + 8)
            }
        }

        panel.setFrame(NSRect(origin: origin, size: size), display: true)
        panel.orderFrontRegardless()
        panel.makeKey()
        panel.invalidateShadow()
        button.highlight(true)
        installMonitors()
    }

    func close() {
        removeMonitors()
        statusButton?.highlight(false)
        panel.orderOut(nil)
    }

    func windowDidResignKey(_ notification: Notification) {
        close()
    }

    private func installMonitors() {
        removeMonitors()

        mouseMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown]
        ) { [weak self] _ in
            Task { @MainActor in
                self?.close()
            }
        }

        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, self.panel.isVisible else {
                return event
            }

            if event.keyCode == 53 { // Escape
                self.close()
                return nil
            }

            if event.modifierFlags.contains(.command) {
                switch event.charactersIgnoringModifiers {
                case "q":
                    self.actions.quit()
                    return nil
                case ",":
                    self.close()
                    self.actions.openSettings()
                    return nil
                default:
                    break
                }
            }

            return event
        }
    }

    private func removeMonitors() {
        if let mouseMonitor {
            NSEvent.removeMonitor(mouseMonitor)
            self.mouseMonitor = nil
        }
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
            self.keyMonitor = nil
        }
    }
}
