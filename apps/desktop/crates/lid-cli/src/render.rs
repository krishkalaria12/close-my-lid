//! Terminal and JSON output.
//!
//! Kept apart from command dispatch so the wording can be tuned without
//! touching behaviour, and so `--json` stays a stable scripting contract.

use std::collections::HashMap;

use chrono::Utc;
use lidcore::{AgentHarness, SleepControlState};

pub const SYSTEMD_UNIT: &str = r#"# Save to ~/.config/systemd/user/close-my-lid.service
# Then: systemctl --user daemon-reload && systemctl --user start close-my-lid
#
# The hold lasts exactly as long as this unit runs, because the logind
# inhibitor is tied to the process's file descriptor. Stopping the unit
# restores normal sleep; so does a crash.

[Unit]
Description=Close My Lid — hold the lid open for long-running work
Documentation=https://github.com/krishkalaria12/close-my-lid

[Service]
Type=simple
ExecStart=%h/.local/bin/close-my-lid enable --for unlimited
Restart=no

[Install]
WantedBy=default.target
"#;

pub fn status(state: &SleepControlState, backend: &str, json: bool) {
    let now = Utc::now();

    if json {
        let payload = serde_json::json!({
            "active": state.is_active(),
            "summary": state.summary(now),
            "ends_at": state.ends_at(),
            "remaining_seconds": state.remaining(now).map(|left| left.num_seconds()),
            "backend": backend,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
        return;
    }

    println!("{}", state.summary(now));
    println!("mechanism: {backend}");
}

pub fn agents(counts: &HashMap<AgentHarness, usize>, json: bool) {
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
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
        return;
    }

    let working: usize = counts.values().sum();
    if working == 0 {
        println!("No agent sessions detected.");
        return;
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
        println!(
            "{:<width$}  {detail}",
            harness.display_name(),
            width = width
        );
    }
}
