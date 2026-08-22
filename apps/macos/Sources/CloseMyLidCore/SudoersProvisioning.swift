import Foundation

/// Installs and removes the passwordless `sudo` allowlist that lets Close My
/// Lid toggle closed-lid sleep without prompting for an administrator
/// password on every hold start, stop, expiry, wake restore, or quit.
///
/// The drop-in grants exactly two commands — `pmset -a disablesleep 1` and
/// `pmset -a disablesleep 0` — to the admin group. No wildcards, no other
/// arguments, so the grant cannot be reused for anything else.
///
/// The same pattern ships in Amphetamine's "Power Protect" helper.
public enum SudoersProvisioning {
    /// Absolute path of the drop-in file consumed by `sudo`.
    public static let filePath = "/etc/sudoers.d/close-my-lid"

    /// The exact rule installed on disk. `NOPASSWD` applies to both listed
    /// commands; argument strings must match what the app executes exactly.
    public static let ruleLine =
        "%admin ALL=(root) NOPASSWD: /usr/bin/pmset -a disablesleep 1, /usr/bin/pmset -a disablesleep 0"

    public static let commentLine = "# Installed by Close My Lid"

    public static var desiredFileContents: String {
        "\(commentLine)\n\(ruleLine)\n"
    }

    /// True when a file's contents contain the expected rule. Pure so it can
    /// be tested without touching `/etc`.
    public static func contentsMatchRule(_ contents: String?) -> Bool {
        guard let contents else {
            return false
        }

        return contents
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .contains { $0 == ruleLine }
    }

    /// Reads the drop-in from disk. `/etc/sudoers.d` files are world-readable,
    /// so this works without elevated privileges when mode 0444 is used.
    public static func installedContents(fileManager: FileManager = .default) -> String? {
        guard fileManager.fileExists(atPath: filePath),
              let contents = fileManager.contents(atPath: filePath),
              let text = String(data: contents, encoding: .utf8)
        else {
            return nil
        }

        return text
    }

    public static func isInstalled(fileManager: FileManager = .default) -> Bool {
        contentsMatchRule(installedContents(fileManager: fileManager))
    }

    /// Builds the AppleScript payload that installs the drop-in with one
    /// administrator prompt, validates it with `visudo`, applies the requested
    /// sleep setting as root, and cleans up the temp file if validation fails.
    ///
    /// Passing `applySetting: nil` installs the grant only (Settings UI).
    public static func installScript(applySetting: Bool?) -> String {
        let tempPath = "\(filePath).tmp.\(getpid())"
        var script = [
            "umask 022",
            "{ echo '\(commentLine)'; echo '\(ruleLine)'; } > \(tempPath)",
            "/usr/sbin/chown root:wheel \(tempPath)",
            "/bin/chmod 444 \(tempPath)",
        ]

        script.append(
            "if /usr/sbin/visudo -cf \(tempPath) > /dev/null; then "
                + "/bin/mv \(tempPath) \(filePath)"
                + (applySetting == nil ? "" : " && /usr/bin/pmset -a disablesleep \(applySetting! ? 1 : 0)")
                + "; else /bin/rm -f \(tempPath); exit 1; fi"
        )

        return script.joined(separator: "; ")
    }

    static func uninstallScript() -> String {
        "/bin/rm -f \(filePath)"
    }

    /// Escapes a shell script for embedding inside an AppleScript string
    /// literal passed to `osascript -e`.
    static func appleScriptPayload(_ shellScript: String) -> String {
        let escaped = shellScript
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")

        return "do shell script \"\(escaped)\" with administrator privileges"
    }
}
