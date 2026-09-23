//! Every tunable value the app layer owns.
//!
//! Anything shared with the CLI and the desktop app lives in `lidcore::config`
//! and is re-exported here, so the surfaces cannot drift apart on things like
//! the battery threshold. What is left is genuinely about this UI.

use std::time::Duration;

/// How often the app reconciles its session against what `pmset` reports.
///
/// Slower than the CLI's supervision loop on purpose: a timed hold is released
/// by a one-shot timer armed at its exact end, so this pass only has to catch
/// changes made behind the app's back.
pub const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// How often the panel re-reads the battery and rescans for agent sessions,
/// while it is open. Nothing scans when it is closed.
pub const PANEL_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// Timers are given this much slack so the scheduler can coalesce them with
/// other work rather than waking the CPU on its own for each one.
pub const TIMER_TOLERANCE: Duration = Duration::from_secs(1);

/// Added to a session's end before the release timer fires, so the state
/// machine sees an end that has definitively passed rather than one it is
/// racing.
pub const EXPIRY_OVERSHOOT: Duration = Duration::from_millis(500);

/// How long after launch the first update check runs. Late enough that it
/// never competes with the menu bar icon appearing.
pub const FIRST_UPDATE_CHECK_DELAY: Duration = Duration::from_secs(2);

/// How often the app re-checks the release feed afterwards.
pub const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// The System Settings pane the Settings window links to.
pub const BATTERY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.Battery-Settings.extension";

/// The SF Symbol used for the menu bar icon, as a template image so macOS
/// tints it for the light or dark menu bar itself.
pub const STATUS_ITEM_SYMBOL: &str = "laptopcomputer";

pub const DEFAULT_LOG_LEVEL: &str = "warn";

/// Unified-log subsystem used when the process has no bundle identifier —
/// every `cargo run`. A bundled app uses its own identifier instead, so the
/// two can never drift apart.
pub const FALLBACK_LOG_SUBSYSTEM: &str = "app.closemylid.CloseMyLid";

/// Unified-log category. One is enough: this is a menu bar app, not a service.
pub const LOG_CATEGORY: &str = "app";
