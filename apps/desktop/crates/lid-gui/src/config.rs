//! Desktop app configuration.
//!
//! Shared values live in `lidcore::config` so the CLI and the app cannot drift
//! apart. Only window geometry, cadences and well-known links belong here.
//! Colours stay in `theme.rs`, which is about appearance rather than
//! configuration.

use std::time::Duration;

/// How often expiry and the battery safety release are checked. Shared with
/// the CLI so both supervise at the same cadence.
pub const SUPERVISION_INTERVAL: Duration = lidcore::config::SUPERVISION_INTERVAL;

/// How often the countdown re-renders while a hold runs.
pub const CLOCK_INTERVAL: Duration = Duration::from_secs(1);

/// How often battery and agent readouts refresh while the window has focus.
/// Matches the macOS panel's refresh while it is open.
pub const READOUT_INTERVAL: Duration = Duration::from_secs(5);

/// How often they refresh while the window is in the background. The process
/// walk is the expensive part, and nobody is reading it closely then.
pub const BACKGROUND_READOUT_INTERVAL: Duration = Duration::from_secs(30);

/// The first update check waits for the window to settle, as on macOS.
pub const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(2);

/// How often the appcast is re-read after that.
pub const UPDATE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// How long "Up to date" stays on the updates row after a manual check.
pub const UPDATE_RESULT_LINGER: Duration = Duration::from_secs(4);

/// The size the window opens at: room for the sidebar and a comfortable
/// reading width beside it.
pub const WINDOW_WIDTH: f32 = 940.0;
pub const WINDOW_HEIGHT: f32 = 660.0;

/// Below this the sidebar and the Overview cards stop fitting side by side.
pub const WINDOW_MIN_WIDTH: f32 = 780.0;
pub const WINDOW_MIN_HEIGHT: f32 = 520.0;

/// Space kept between a small display's edges and the window.
pub const DISPLAY_MARGIN: f32 = 48.0;

/// Passed by the login item, so a launch at sign-in starts minimised instead
/// of putting a window in front of whatever the user opens first.
pub const MINIMIZED_ARG: &str = "--minimized";

/// Where "Release Notes" and an available update point.
pub const RELEASES_URL: &str = "https://github.com/krishkalaria12/close-my-lid/releases";

/// Where "Report an Issue" points.
pub const ISSUES_URL: &str = "https://github.com/krishkalaria12/close-my-lid/issues/new";
