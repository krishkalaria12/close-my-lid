//! The passwordless `sudo` allowlist that lets Close My Lid toggle closed-lid
//! sleep without prompting for an administrator password on every hold start,
//! stop, expiry, wake restore, or quit.
//!
//! Carried over from the Swift app's `SudoersProvisioning`. The drop-in grants
//! exactly two commands — `pmset -a disablesleep 1` and `pmset -a disablesleep
//! 0` — to the admin group. No wildcards, no other arguments, so the grant
//! cannot be reused for anything else.
//!
//! The same pattern ships in Amphetamine's "Power Protect" helper.

use std::fs;

/// Absolute path of the drop-in file consumed by `sudo`.
pub const FILE_PATH: &str = "/etc/sudoers.d/close-my-lid";

/// The exact rule installed on disk. `NOPASSWD` applies to both listed
/// commands; argument strings must match what the app executes exactly.
pub const RULE_LINE: &str = "%admin ALL=(root) NOPASSWD: /usr/bin/pmset -a disablesleep 1, /usr/bin/pmset -a disablesleep 0";

pub const COMMENT_LINE: &str = "# Installed by Close My Lid";

/// The exact file contents installed on disk.
pub fn desired_contents() -> String {
    format!("{COMMENT_LINE}\n{RULE_LINE}\n")
}

/// True when a file's contents contain the expected rule. Pure so it can be
/// tested without touching `/etc`.
pub fn contents_match_rule(contents: Option<&str>) -> bool {
    let Some(contents) = contents else {
        return false;
    };

    contents
        .lines()
        .map(str::trim)
        .any(|line| line == RULE_LINE)
}

/// Reads the drop-in from disk. `/etc/sudoers.d` files are world-readable, so
/// this works without elevated privileges when mode 0444 is used.
pub fn installed_contents() -> Option<String> {
    fs::read_to_string(FILE_PATH).ok()
}

pub fn is_installed() -> bool {
    contents_match_rule(installed_contents().as_deref())
}

/// Builds the shell script behind a single administrator prompt.
///
/// The script writes the drop-in to a temp file, validates it with `visudo`,
/// and moves it into place. When `apply_setting` is set, the sleep change runs
/// inside that same elevated script on both the validated and refused paths, so
/// a managed machine that rejects the drop-in still gets the requested hold
/// without a second password entry.
///
/// Passing `apply_setting: None` installs the grant only (Settings UI) and
/// exits nonzero when validation refuses the drop-in.
pub fn install_script(apply_setting: Option<bool>) -> String {
    let pid = std::process::id();
    let temp_path = format!("{FILE_PATH}.tmp.{pid}");
    let mut script = vec![
        "umask 022".to_string(),
        format!("{{ echo '{COMMENT_LINE}'; echo '{RULE_LINE}'; }} > {temp_path}"),
        format!("/usr/sbin/chown root:wheel {temp_path}"),
        format!("/bin/chmod 444 {temp_path}"),
    ];

    let apply_clause = apply_setting
        .map(|setting| format!(" && /usr/bin/pmset -a disablesleep {}", i32::from(setting)))
        .unwrap_or_default();

    let exit_clause = if apply_setting.is_none() {
        "; exit 1"
    } else {
        ""
    };

    script.push(format!(
        "if /usr/sbin/visudo -cf {temp_path} > /dev/null; then \
         /bin/mv {temp_path} {FILE_PATH}{apply_clause}; \
         else /bin/rm -f {temp_path}{apply_clause}{exit_clause}; fi"
    ));

    script.join("; ")
}

pub fn uninstall_script() -> String {
    format!("/bin/rm -f {FILE_PATH}")
}

/// Escapes a shell script for embedding inside an AppleScript string literal
/// passed to `osascript -e`.
pub fn applescript_payload(shell_script: &str) -> String {
    let escaped = shell_script.replace('\\', "\\\\").replace('"', "\\\"");
    format!("do shell script \"{escaped}\" with administrator privileges")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_matches_what_the_backend_executes() {
        // The backend runs `sudo -n /usr/bin/pmset -a disablesleep <0|1>`.
        // sudoers matches argument strings exactly, so the rule must contain
        // both literal commands.
        assert!(RULE_LINE.contains("/usr/bin/pmset -a disablesleep 1"));
        assert!(RULE_LINE.contains("/usr/bin/pmset -a disablesleep 0"));
        assert!(RULE_LINE.contains("NOPASSWD"));
        assert!(!RULE_LINE.contains('*'), "no wildcards allowed");
    }

    #[test]
    fn contents_match_ignores_surrounding_lines_and_whitespace() {
        assert!(contents_match_rule(Some(&desired_contents())));
        assert!(contents_match_rule(Some(&format!(
            "# some other tool's line\n  {RULE_LINE}  \n"
        ))));
        assert!(!contents_match_rule(None));
        assert!(!contents_match_rule(Some("")));
        assert!(!contents_match_rule(Some("# Installed by Close My Lid\n")));
    }

    #[test]
    fn install_script_validates_before_moving_into_place() {
        let script = install_script(Some(true));
        assert!(script.contains("visudo -cf"), "{script}");
        assert!(script.contains("/bin/mv"), "{script}");
        assert!(script.contains("pmset -a disablesleep 1"), "{script}");
        // Refused drop-ins are removed, never installed half-written.
        assert!(script.contains("/bin/rm -f"), "{script}");
    }

    #[test]
    fn grant_only_install_fails_loudly_when_refused() {
        let script = install_script(None);
        // The rule text is echoed into the temp file, but no pmset apply
        // command may run outside that echo.
        assert!(!script.contains("&& /usr/bin/pmset"), "{script}");
        assert!(script.contains("exit 1"), "{script}");
    }

    #[test]
    fn uninstall_removes_only_our_drop_in() {
        assert_eq!(uninstall_script(), format!("/bin/rm -f {FILE_PATH}"));
    }

    #[test]
    fn applescript_payload_escapes_quotes_and_backslashes() {
        let payload = applescript_payload(r#"echo "a\b""#);
        assert!(payload.starts_with("do shell script \""), "{payload}");
        assert!(
            payload.ends_with("\" with administrator privileges"),
            "{payload}"
        );
        assert!(payload.contains("\\\"a\\\\b\\\""), "{payload}");
    }
}
