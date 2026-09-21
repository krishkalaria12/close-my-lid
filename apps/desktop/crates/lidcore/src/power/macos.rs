//! Closed-lid sleep control via `pmset disablesleep`, the only supported
//! mechanism on macOS.
//!
//! Carried over from the Swift app's `PmsetPowerManager` /
//! `PasswordlessPowerCommandExecutor` / `AdminShellPowerCommandExecutor` trio.
//! Two escalation levels, tried in order:
//!
//! 1. The passwordless allowlisted command (`sudo -n pmset …`) that the
//!    sudoers drop-in enables — silent, fast, no prompt once provisioned.
//! 2. One elevated script that installs the drop-in, validates it with
//!    `visudo`, and applies the setting — all within a single macOS admin
//!    dialog. If validation refuses the drop-in (managed machine), the same
//!    dialog still applies the sleep setting, so holds keep working without a
//!    second prompt.
//!
//! The watchdog uses [`PmsetLidGuard::passwordless`], which never elevates, so
//! a missing grant can never spawn an administrator dialog from a background
//! agent.
//!
//! Reading, unlike writing, needs no `pmset` at all: `IOPMCopySystemPowerSettings`
//! hands back the same `SleepDisabled` value the command prints. The app polls
//! that every reconciliation pass for as long as it runs, so the difference
//! between a framework call and a forked process is thousands of process
//! spawns a day. `pmset -g` stays as a fallback for the case where IOKit
//! answers with nothing.

use std::process::Command;

use tracing::debug;

use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;
use crate::sudoers;

mod iokit;

/// Guards closed-lid sleep through `pmset -a disablesleep`.
pub struct PmsetLidGuard {
    elevate: bool,
}

impl PmsetLidGuard {
    /// Interactive use: falls back to one administrator prompt when the
    /// passwordless grant is missing.
    pub fn new() -> Self {
        Self { elevate: true }
    }

    /// Headless use (the watchdog LaunchAgent): never prompts. A missing grant
    /// surfaces as an error and the heartbeat is left for the next tick.
    pub fn passwordless() -> Self {
        Self { elevate: false }
    }

    fn apply(&mut self, enabled: bool) -> Result<()> {
        if try_passwordless_apply(enabled).is_ok() {
            return Ok(());
        }

        if !self.elevate {
            return Err(passwordless_denied(enabled));
        }

        run_elevated(&sudoers::install_script(Some(enabled)))
    }

    /// Installs only the passwordless grant (no sleep change). Used by the
    /// Settings screen's Administrator Access controls.
    pub fn install_passwordless_grant() -> Result<()> {
        run_elevated(&sudoers::install_script(None))
    }

    /// Removes the passwordless grant. Elevated because `/etc/sudoers.d` is
    /// root-owned.
    pub fn remove_passwordless_grant() -> Result<()> {
        run_elevated(&sudoers::uninstall_script())
    }
}

impl Default for PmsetLidGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl LidPowerBackend for PmsetLidGuard {
    fn acquire(&mut self) -> Result<()> {
        self.apply(true)
    }

    fn release(&mut self) -> Result<()> {
        self.apply(false)
    }

    fn is_held(&self) -> Result<bool> {
        if let Some(held) = iokit::sleep_disabled() {
            return Ok(held);
        }

        debug!("IOKit reported no SleepDisabled setting; falling back to pmset");
        let output = run("/usr/bin/pmset", &["-g"]).map_err(|detail| {
            LidError::backend("read the closed-lid sleep setting", detail).with_hint(
                "Close My Lid reads this with `/usr/bin/pmset -g`. Check that pmset \
                 runs in a terminal.",
            )
        })?;
        Ok(disable_sleep_is_enabled(&output))
    }

    fn describe(&self) -> &'static str {
        "pmset disablesleep"
    }
}

/// Whether `pmset -g` output reports the closed-lid sleep disable flag.
///
/// `pmset -g` prints the setting as `SleepDisabled` (tab-separated, e.g.
/// `"\t SleepDisabled \t\t 1"`), while some tooling and older references
/// show `disablesleep`. Both are accepted, case-insensitively, so a macOS
/// wording change cannot silently strand a hold again.
pub fn disable_sleep_is_enabled(output: &str) -> bool {
    output.lines().any(|line| {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next()) {
            (Some(key), Some(value)) => {
                reports_sleep_disabled(key) && value.eq_ignore_ascii_case("1")
            }
            _ => false,
        }
    })
}

fn reports_sleep_disabled(field: &str) -> bool {
    let key = field.to_lowercase();
    key == "sleepdisabled" || key == "disablesleep"
}

fn try_passwordless_apply(enabled: bool) -> Result<()> {
    let value = if enabled { "1" } else { "0" };
    run(
        "/usr/bin/sudo",
        &["-n", "/usr/bin/pmset", "-a", "disablesleep", value],
    )
    .map(|_| ())
    .map_err(|_| passwordless_denied(enabled))
}

fn passwordless_denied(enabled: bool) -> LidError {
    let action = if enabled {
        "hold closed-lid sleep without an administrator prompt"
    } else {
        "release the closed-lid hold without an administrator prompt"
    };
    LidError::denied(action, "the passwordless sudo grant is missing or refused").with_hint(
        "Grant one-time administrator access from Settings, or approve the \
         administrator prompt when it appears.",
    )
}

fn run_elevated(shell_script: &str) -> Result<()> {
    let payload = sudoers::applescript_payload(shell_script);
    let result = run("/usr/bin/osascript", &["-e", &payload]);

    match result {
        Ok(_) => Ok(()),
        Err(detail) => {
            let lowered = detail.to_lowercase();
            if lowered.contains("user canceled")
                || lowered.contains("user cancelled")
                || lowered.contains("(-128)")
            {
                return Err(LidError::ElevationCancelled);
            }
            Err(LidError::denied(
                "change the closed-lid sleep setting",
                detail,
            ))
        }
    }
}

/// Runs a command, returning trimmed combined output or a terse detail string.
/// Never spawns a shell: the argument list is passed through exactly.
fn run(executable: &str, args: &[&str]) -> std::result::Result<String, String> {
    let output = Command::new(executable)
        .args(args)
        .output()
        .map_err(|error| format!("could not launch {executable}: {error}"))?;

    // osascript reports AppleScript failures on stderr as `0:NN: execution
    // error: … (-128)`; pmset writes to stdout. Combine both so classification
    // sees the platform's own wording either way.
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !combined.trim().is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }

    if output.status.success() {
        Ok(combined.trim().to_string())
    } else {
        Err(format!(
            "{executable} exited with status {}: {}",
            output.status,
            combined.trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_both_pmset_spellings() {
        assert!(disable_sleep_is_enabled("\t SleepDisabled \t\t 1\n"));
        assert!(disable_sleep_is_enabled("disablesleep 1\n"));
        assert!(disable_sleep_is_enabled("SLEEPDISABLED 1"));
    }

    #[test]
    fn rejects_zero_and_unrelated_keys() {
        assert!(!disable_sleep_is_enabled("\t SleepDisabled \t\t 0\n"));
        assert!(!disable_sleep_is_enabled("disablesleep 0\n"));
        assert!(!disable_sleep_is_enabled("sleep 1\n"));
        assert!(!disable_sleep_is_enabled(""));
        // A value of 10 must not match a naive substring check for "1".
        assert!(!disable_sleep_is_enabled("SleepDisabled 10\n"));
    }

    #[test]
    fn needs_both_a_key_and_a_value() {
        assert!(!disable_sleep_is_enabled("SleepDisabled\n"));
        assert!(!disable_sleep_is_enabled("1\n"));
    }

    #[test]
    fn scans_a_realistic_pmset_dump() {
        let dump = "System-wide power settings:\n\
                    Currently in use:\n\
                    \t SleepDisabled\t\t 1\n\
                    \t disksleep\t\t 10\n";
        assert!(disable_sleep_is_enabled(dump));

        let idle = dump.replace("SleepDisabled\t\t 1", "SleepDisabled\t\t 0");
        assert!(!disable_sleep_is_enabled(&idle));
    }

    #[test]
    fn the_backend_describes_its_mechanism() {
        assert_eq!(PmsetLidGuard::new().describe(), "pmset disablesleep");
    }

    #[test]
    fn passwordless_failures_point_at_the_grant() {
        let error = passwordless_denied(true);
        assert!(error.hint().unwrap().contains("administrator"), "{error:?}");
    }
}
