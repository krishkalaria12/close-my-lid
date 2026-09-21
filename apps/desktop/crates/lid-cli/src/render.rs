//! Terminal and JSON output.
//!
//! Kept apart from command dispatch so the wording can be tuned without
//! touching behaviour, and so `--json` stays a stable scripting contract.
//!
//! Every function writes through [`line`], which turns a broken pipe into a
//! typed error instead of a panic — piping into `head` is normal usage.

use std::collections::HashMap;
use std::io::Write;

use chrono::Utc;
use lidcore::{AgentHarness, SessionDuration, SleepControlState};

use crate::error::{CliError, Result};

const SYSTEMD_UNIT: &str = r#"# Save to ~/.config/systemd/user/close-my-lid.service
# Then: systemctl --user daemon-reload && systemctl --user start close-my-lid
#
# The hold lasts exactly as long as this unit runs, because the logind
# inhibitor is tied to the process's file descriptor. Stopping the unit
# restores normal sleep; so does a crash.

[Unit]
Description=Close My Lid — hold the lid open for long-running work
Documentation=https://github.com/krishkalaria12/close-my-lid
After=systemd-logind.service
Wants=systemd-logind.service

[Service]
Type=simple
ExecStart=%h/.local/bin/close-my-lid enable --for unlimited
# No auto-restart: after a crash the state file still says Active while no
# hold exists, and the next manual `enable` reconciles that honestly instead
# of silently re-taking a hold the user may no longer want.
Restart=no

[Install]
WantedBy=default.target
"#;

/// Writes one line to stdout, mapping I/O failures to a typed error.
pub fn line(text: &str) -> Result<()> {
    writeln!(std::io::stdout(), "{text}").map_err(CliError::Output)
}

pub fn systemd_unit() -> Result<()> {
    print!("{SYSTEMD_UNIT}");
    std::io::stdout().flush().map_err(CliError::Output)
}

pub fn hold_started(duration: SessionDuration, backend: &str) -> Result<()> {
    line(&format!(
        "{} is holding the lid open ({}).",
        lidcore::APP_NAME,
        duration.label()
    ))?;
    line(&format!("Mechanism: {backend}."))?;
    line("Press Ctrl-C to release.")
}

pub fn asked_owner_to_stop(pid: u32) -> Result<()> {
    line(&format!("Asking the holding process (pid {pid}) to stop…"))
}

pub fn hold_expired() -> Result<()> {
    line("\nSession ended; normal sleep restored.")
}

pub fn hold_released() -> Result<()> {
    line("\nReleased. Normal sleep restored.")
}

/// Reports the hold.
///
/// `held` is what the system actually reports, which can disagree with the
/// recorded `state`: a killed `enable` on Linux loses its inhibitor without
/// getting the chance to update the file. Reality wins, and the disagreement
/// is called out rather than silently papered over.
pub fn status(
    state: &SleepControlState,
    held: bool,
    owner: Option<u32>,
    backend: &str,
    json: bool,
) -> Result<()> {
    let now = Utc::now();
    let stale = state.is_active() && !held;

    if json {
        let payload = serde_json::json!({
            "active": held,
            "recorded_active": state.is_active(),
            "stale": stale,
            "owner_pid": owner,
            "summary": state.summary(now),
            "ends_at": state.ends_at(),
            "remaining_seconds": state.remaining(now).map(|left| left.num_seconds()),
            "backend": backend,
        });
        return line(&serde_json::to_string_pretty(&payload).unwrap_or_default());
    }

    if stale {
        line("Off — sleeps normally")?;
        line("note: a session was recorded but nothing is holding the lid;")?;
        line("      the process that owned it is gone. Run `close-my-lid enable` to restart it.")?;
    } else {
        line(&state.summary(now))?;
    }

    if let Some(pid) = owner {
        line(&format!("held by: pid {pid}"))?;
    }
    line(&format!("mechanism: {backend}"))
}

pub fn agents(counts: &HashMap<AgentHarness, usize>, json: bool) -> Result<()> {
    if json {
        let payload: HashMap<&str, usize> = AgentHarness::ALL
            .iter()
            .map(|harness| {
                (
                    harness.display_name(),
                    counts.get(harness).copied().unwrap_or(0),
                )
            })
            .collect();
        return line(&serde_json::to_string_pretty(&payload).unwrap_or_default());
    }

    if counts.values().sum::<usize>() == 0 {
        return line("No agent sessions detected.");
    }

    // Widest name sets the column so the counts line up.
    let width = AgentHarness::ALL
        .iter()
        .map(|harness| harness.display_name().len())
        .max()
        .unwrap_or(0);

    for harness in AgentHarness::ALL {
        let count = counts.get(&harness).copied().unwrap_or(0);
        let detail = match count {
            0 => "idle".to_string(),
            1 => "1 session".to_string(),
            many => format!("{many} sessions"),
        };
        line(&format!(
            "{:<width$}  {detail}",
            harness.display_name(),
            width = width
        ))?;
    }
    Ok(())
}
