import AppKit
import CloseMyLidCore
import SwiftUI

@MainActor
final class SettingsWindowController: NSWindowController {
    convenience init(
        launchAtLogin: LaunchAtLoginController,
        privilegedExecutor: AdminShellPowerCommandExecutor,
        watchdogAgent: WatchdogAgentController
    ) {
        let window = NSWindow(
            contentRect: .zero,
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: true
        )
        window.title = "Close My Lid Settings"
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(
            rootView: SettingsView(
                launchAtLogin: launchAtLogin,
                privilegedExecutor: privilegedExecutor,
                watchdogAgent: watchdogAgent
            )
        )
        window.setContentSize(window.contentView?.fittingSize ?? .zero)
        window.center()

        self.init(window: window)
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }
}

private struct SettingsView: View {
    let launchAtLogin: LaunchAtLoginController
    let privilegedExecutor: AdminShellPowerCommandExecutor
    let watchdogAgent: WatchdogAgentController

    @State private var launchAtLoginEnabled = false
    @State private var passwordlessInstalled = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Toggle("Launch at Login", isOn: $launchAtLoginEnabled)
                .onChange(of: launchAtLoginEnabled) { _, enabled in
                    do {
                        try launchAtLogin.setEnabled(enabled)
                        errorMessage = nil
                    } catch {
                        errorMessage = error.localizedDescription
                        launchAtLoginEnabled = launchAtLogin.isEnabled
                    }
                }

            Divider()

            administratorAccessSection

            Divider()

            Button("Open Battery Settings…") {
                if let url = URL(string: "x-apple.systempreferences:com.apple.Battery-Settings.extension") {
                    NSWorkspace.shared.open(url)
                }
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Text("Close My Lid \(CommandLineInterface.version)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(width: 300, alignment: .leading)
        .onAppear {
            launchAtLoginEnabled = launchAtLogin.isEnabled
            passwordlessInstalled = SudoersProvisioning.isInstalled()
        }
    }

    // MARK: - Administrator access

    private var administratorAccessSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Administrator Access")
                .font(.system(size: 13, weight: .semibold))

            Text(
                passwordlessInstalled
                    ? "Granted. Holds start, end, and restore without asking for your password."
                    : "Not granted yet. Every hold change asks for your password."
            )
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)

            HStack(spacing: 10) {
                if passwordlessInstalled {
                    Button("Remove…", action: removeGrant)
                } else {
                    Button("Grant Once…", action: installGrant)
                }
            }
        }
    }

    private func installGrant() {
        do {
            try privilegedExecutor.installPasswordlessGrant()
            try? watchdogAgent.install()
            errorMessage = nil
        } catch {
            errorMessage = Self.message(for: error)
        }

        passwordlessInstalled = SudoersProvisioning.isInstalled()
    }

    private func removeGrant() {
        do {
            try privilegedExecutor.removePasswordlessGrant()
            watchdogAgent.uninstall()
            errorMessage = nil
        } catch {
            errorMessage = Self.message(for: error)
        }

        passwordlessInstalled = SudoersProvisioning.isInstalled()
    }

    private static func message(for error: Error) -> String {
        if case PowerCommandError.elevationCancelled = error {
            return "Administrator approval cancelled."
        }

        return error.localizedDescription
    }
}
