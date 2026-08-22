import CloseMyLidCore
import Foundation

/// Installs and removes the dead-man watchdog LaunchAgent that restores
/// normal sleep if this app dies while a hold is stranded.
///
/// The agent lives in `~/Library/LaunchAgents` and is managed directly with
/// `launchctl` rather than through SMAppService, so it works from hand-built
/// bundles and CLI-only installs without an approval flow. It invokes the
/// same executable with `--watchdog`.
final class WatchdogAgentController {
    static let label = "app.closemylid.watchdog"
    static let startIntervalSeconds = 60

    private let commandRunner: ShellCommandRunner
    private let fileManager: FileManager

    init(
        commandRunner: ShellCommandRunner = ShellCommandRunner(),
        fileManager: FileManager = .default
    ) {
        self.commandRunner = commandRunner
        self.fileManager = fileManager
    }

    var plistURL: URL {
        fileManager.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents", isDirectory: true)
            .appendingPathComponent("\(Self.label).plist", isDirectory: false)
    }

    var isInstalled: Bool {
        fileManager.fileExists(atPath: plistURL.path)
    }

    /// Writes the agent plist and registers it with launchd. Skipped silently
    /// for unbundled development runs (`swift run`), where there is no stable
    /// executable path to point at.
    func install() throws {
        guard let executablePath = resolvedExecutablePath else {
            return
        }

        try fileManager.createDirectory(
            at: plistURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )

        let xml = Self.plistXML(executablePath: executablePath, label: Self.label)
        try Data(xml.utf8).write(to: plistURL, options: .atomic)

        // Refresh any stale registration that points at an older binary.
        bootout()
        _ = try? commandRunner.run(
            executablePath: "/bin/launchctl",
            arguments: ["bootstrap", "gui/\(getuid())", plistURL.path]
        )
    }

    func uninstall() {
        bootout()
        try? fileManager.removeItem(at: plistURL)
    }

    private var resolvedExecutablePath: String? {
        Bundle.main.executableURL?.path
    }

    private func bootout() {
        _ = try? commandRunner.run(
            executablePath: "/bin/launchctl",
            arguments: ["bootout", "gui/\(getuid())/\(Self.label)"]
        )
    }

    static func plistXML(executablePath: String, label: String) -> String {
        func escaped(_ value: String) -> String {
            value
                .replacingOccurrences(of: "&", with: "&amp;")
                .replacingOccurrences(of: "<", with: "&lt;")
                .replacingOccurrences(of: ">", with: "&gt;")
        }

        return """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
          <key>Label</key>
          <string>\(escaped(label))</string>
          <key>ProgramArguments</key>
          <array>
            <string>\(escaped(executablePath))</string>
            <string>--watchdog</string>
          </array>
          <key>StartInterval</key>
          <integer>\(startIntervalSeconds)</integer>
          <key>RunAtLoad</key>
          <false/>
          <key>ProcessType</key>
          <string>Background</string>
        </dict>
        </plist>
        """
    }
}
