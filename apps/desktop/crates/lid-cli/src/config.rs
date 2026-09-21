//! CLI-specific configuration.
//!
//! Shared values (app name, battery threshold, supervision interval) live in
//! `lidcore::config` so the GUI and CLI cannot drift apart. Only things that
//! are purely about terminal behaviour belong here.

use std::time::Duration;

/// How often a foreground hold checks for expiry and low battery.
///
/// Re-exported from the core so the CLI and the tray app supervise at the same
/// cadence.
pub const SUPERVISION_INTERVAL: Duration = lidcore::config::SUPERVISION_INTERVAL;

/// Default when `--for` is omitted. Unlimited matches the macOS menu's
/// behaviour of holding until told otherwise.
pub const DEFAULT_DURATION: &str = "unlimited";

/// How long `disable` waits for a signalled holder to exit, as a poll count
/// times the interval below.
pub const RELEASE_WAIT_POLLS: u32 = 50;

/// Gap between those polls.
pub const RELEASE_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Granularity of the shutdown check inside each supervision interval, so
/// Ctrl-C during `enable` exits within ~250ms instead of after 15s.
pub const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Tracing filter used when `--verbose` is absent.
pub const DEFAULT_LOG_LEVEL: &str = "warn";

/// Tracing filter used when `--verbose` is passed.
pub const VERBOSE_LOG_LEVEL: &str = "debug";
