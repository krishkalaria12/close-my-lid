//! Tray app configuration.
//!
//! Shared values live in `lidcore::config` so the CLI and the tray app cannot
//! drift apart. Only window geometry and tray wiring belong here. Colours stay
//! in `theme.rs`, which is about appearance rather than configuration.

use std::time::Duration;

/// How often expiry and the battery safety release are checked. Shared with
/// the CLI so both supervise at the same cadence.
pub const SUPERVISION_INTERVAL: Duration = lidcore::config::SUPERVISION_INTERVAL;

/// Panel width. Narrower than the macOS panel's 300pt because Windows system
/// fonts run wider at the same point size.
pub const PANEL_WIDTH: f32 = 320.0;

/// Panel height. Fixed rather than fitted: the agent list has a constant row
/// count, so the panel never needs to resize.
pub const PANEL_HEIGHT: f32 = 460.0;

/// Gap between the panel and the screen edge.
pub const PANEL_MARGIN: f32 = 12.0;

/// Extra bottom inset so the panel clears a standard Windows taskbar when the
/// tray icon's real position is unavailable.
pub const TASKBAR_INSET: f32 = PANEL_MARGIN * 4.0;

/// Tray menu item identifiers. Kept here so the menu definition and its
/// handler cannot disagree about a string literal.
pub mod action {
    pub const PANEL: &str = "panel";
    pub const STOP: &str = "stop";
    pub const QUIT: &str = "quit";
    /// Prefix for the duration presets, e.g. `hold:1 hour`.
    pub const HOLD_PREFIX: &str = "hold:";
}
