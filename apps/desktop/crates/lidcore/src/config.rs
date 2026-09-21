//! Every tunable value and well-known path in one place.
//!
//! Constants were previously scattered across `lib.rs`, `store.rs` and
//! `battery.rs`, which made it hard to see what was actually configurable.
//! Anything a user or packager might reasonably want to change belongs here.

use std::path::PathBuf;
use std::time::Duration;

use directories::ProjectDirs;

use crate::error::{LidError, Result};

/// Reverse-DNS identifier: the single-instance lock, the config directory and
/// the systemd user unit all derive from this.
///
/// Deliberately *not* the macOS bundle identifier, which is
/// `app.closemylid.CloseMyLid` — see `scripts/package-macos-app.sh`. The two
/// have always differed, and changing either is a migration rather than a
/// tidy-up: the bundle identifier is what the notification authorization,
/// the `SMAppService` login item registration and Gatekeeper's record of the
/// app are all keyed on, and this one is what names the directory holding a
/// live hold's state. Renaming the bundle would re-prompt every existing
/// install for notification permission and drop its login item; renaming this
/// would strand the session file that lets a hold be released after a crash.
///
/// The watchdog LaunchAgent is a third name again (`app.closemylid.watchdog`,
/// in `launchd`), matching the bundle's namespace, and its path is what makes
/// replacing `/Applications/Close My Lid.app` a drop-in swap.
pub const APP_ID: &str = "com.krishkalaria.close-my-lid";

/// Human-facing app name, used in notifications and CLI output.
pub const APP_NAME: &str = "Close My Lid";

/// Kept in sync with the root `package.json` and the packaged bundle's
/// `CFBundleShortVersionString`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Battery percentage at which an unplugged hold is released. Matches the
/// macOS app, so all platforms protect the battery at the same point.
pub const BATTERY_RELEASE_THRESHOLD: u8 = 5;

// Enforced at compile time: a threshold of 0 would never fire and one at or
// above 100 would release a hold the moment it started.
const _: () = assert!(BATTERY_RELEASE_THRESHOLD > 0 && BATTERY_RELEASE_THRESHOLD < 100);

/// How often expiry and the battery safety release are checked.
///
/// Short enough that a session ends close to on time, long enough that the
/// battery read is not itself a drain.
pub const SUPERVISION_INTERVAL: Duration = Duration::from_secs(15);

/// Qualifier, organisation and application for the platform config directory.
const DIRS: (&str, &str, &str) = ("com", "krishkalaria", "close-my-lid");

/// The platform config directory — `~/.config/close-my-lid` on Linux,
/// `%APPDATA%\close-my-lid` on Windows.
///
/// Fails loudly rather than falling back to the working directory: a session
/// file written somewhere unexpected is worse than a clear error, because the
/// next launch would not find it and could not release a stranded hold.
pub fn config_dir() -> Result<PathBuf> {
    ProjectDirs::from(DIRS.0, DIRS.1, DIRS.2)
        .map(|dirs| dirs.config_dir().to_path_buf())
        .ok_or(LidError::NoConfigDir)
}

/// Where the current session is persisted.
pub fn session_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("session.json"))
}

/// Where the watchdog heartbeat is persisted (macOS). Written by the app while
/// a hold is active; read by the `--watchdog` pass.
pub fn heartbeat_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("heartbeat.json"))
}

/// Where the last-picked hold duration is persisted, so the main toggle can
/// re-apply it.
pub fn selected_duration_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("selected-duration"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_file_sits_under_the_config_directory() {
        let dir = config_dir().expect("a config directory on a test machine");
        let file = session_file().unwrap();
        assert!(file.starts_with(&dir));
        assert_eq!(file.file_name().unwrap(), "session.json");
    }
}
