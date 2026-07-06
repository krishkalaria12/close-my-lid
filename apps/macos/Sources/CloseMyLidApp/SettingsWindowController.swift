import AppKit
import SwiftUI

@MainActor
final class SettingsWindowController: NSWindowController {
    convenience init(launchAtLogin: LaunchAtLoginController) {
        let window = NSWindow(
            contentRect: .zero,
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: true
        )
        window.title = "Close My Lid Settings"
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: SettingsView(launchAtLogin: launchAtLogin))
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

    @State private var launchAtLoginEnabled = false
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

            if let errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            Button("Open Battery Settings…") {
                if let url = URL(string: "x-apple.systempreferences:com.apple.Battery-Settings.extension") {
                    NSWorkspace.shared.open(url)
                }
            }

            Divider()

            Text("Close My Lid \(CommandLineInterface.version)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(width: 280, alignment: .leading)
        .onAppear {
            launchAtLoginEnabled = launchAtLogin.isEnabled
        }
    }
}
