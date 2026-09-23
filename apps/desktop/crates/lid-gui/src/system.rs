//! Everything the app asks of the operating system besides the lid itself:
//! notifications, launch at login, the power settings shortcut, and the one
//! instance allowed to run.
//!
//! On macOS each of these has a native home in `lid-macos`, so a Mac build of
//! this crate — which exists only for working on the interface — stubs them.

use std::fs::{File, OpenOptions};

use tracing::warn;

use crate::error::{GuiError, Result};

// MARK: notifications

/// Posts a notification now. Failures are logged and swallowed: a missing
/// notification daemon must never stop a hold from starting or ending.
pub fn notify(body: &str) {
    #[cfg(not(target_os = "macos"))]
    {
        let result = notify_rust::Notification::new()
            .appname(lidcore::APP_NAME)
            .summary(lidcore::APP_NAME)
            .body(body)
            .show();
        if let Err(error) = result {
            warn!(%error, "could not post a notification");
        }
    }

    #[cfg(target_os = "macos")]
    tracing::info!(body, "notification (not posted from a development build)");
}

// MARK: launch at login

#[cfg(not(target_os = "macos"))]
fn login_item() -> Result<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().map_err(|error| GuiError::LoginItem {
        detail: error.to_string(),
    })?;
    auto_launch::AutoLaunchBuilder::new()
        .set_app_name(lidcore::APP_NAME)
        .set_app_path(&exe.to_string_lossy())
        .set_args(&[crate::config::MINIMIZED_ARG])
        .set_linux_launch_mode(auto_launch::LinuxLaunchMode::XdgAutostart)
        .build()
        .map_err(|error| GuiError::LoginItem {
            detail: error.to_string(),
        })
}

pub fn launches_at_login() -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        login_item()
            .and_then(|item| {
                item.is_enabled().map_err(|error| GuiError::LoginItem {
                    detail: error.to_string(),
                })
            })
            .inspect_err(|error| warn!(%error, "could not read the login item"))
            .unwrap_or(false)
    }

    #[cfg(target_os = "macos")]
    false
}

pub fn set_launch_at_login(enabled: bool) -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        let item = login_item()?;
        let result = if enabled {
            item.enable()
        } else {
            item.disable()
        };
        result.map_err(|error| GuiError::LoginItem {
            detail: error.to_string(),
        })
    }

    #[cfg(target_os = "macos")]
    {
        let _ = enabled;
        Err(GuiError::LoginItem {
            detail: "Use the menu bar app's settings on macOS.".to_string(),
        })
    }
}

// MARK: power settings

/// Opens the system's own power settings, where the lid action lives. Returns
/// false when no known settings app could be started, so the caller can say
/// so instead of the click doing nothing.
pub fn open_power_settings() -> bool {
    #[cfg(target_os = "windows")]
    {
        // `start` resolves the ms-settings: URI through the shell. The empty
        // title argument keeps `start` from reading the URI as a window title.
        spawn("cmd", &["/C", "start", "", "ms-settings:powersleep"])
    }

    #[cfg(target_os = "linux")]
    {
        // Each desktop ships its own panel and none is universal, so try the
        // common ones in order of how many people run them.
        const CANDIDATES: &[(&str, &[&str])] = &[
            ("gnome-control-center", &["power"]),
            ("systemsettings", &["kcm_powerdevilprofilesconfig"]),
            ("kcmshell6", &["kcm_powerdevilprofilesconfig"]),
            ("cinnamon-settings", &["power"]),
            ("mate-power-preferences", &[]),
            ("xfce4-power-manager-settings", &[]),
        ];
        CANDIDATES
            .iter()
            .any(|(program, args)| spawn(program, args))
    }

    #[cfg(target_os = "macos")]
    {
        spawn(
            "open",
            &["x-apple.systempreferences:com.apple.Battery-Settings.extension"],
        )
    }
}

fn spawn(program: &str, args: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

// MARK: single instance

/// Held for the life of the process. A second copy would fight the first over
/// the same global power setting, and would open a second window showing a
/// hold it does not own.
pub struct InstanceGuard {
    _file: Option<File>,
}

/// `None` when another instance already holds the lock.
///
/// An OS file lock rather than a pid file: the kernel drops it when the
/// process dies, so a crash cannot leave the app refusing to start.
pub fn claim_single_instance() -> Option<InstanceGuard> {
    let path = match lidcore::config::config_dir() {
        Ok(dir) => {
            if let Err(error) = std::fs::create_dir_all(&dir) {
                warn!(%error, "no config directory; running without the instance lock");
                return Some(InstanceGuard::unlocked());
            }
            dir.join("gui.lock")
        }
        Err(error) => {
            warn!(%error, "no config directory; running without the instance lock");
            return Some(InstanceGuard::unlocked());
        }
    };

    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            warn!(%error, "could not open the instance lock; running without it");
            return Some(InstanceGuard::unlocked());
        }
    };

    match file.try_lock() {
        Ok(()) => Some(InstanceGuard { _file: Some(file) }),
        Err(std::fs::TryLockError::WouldBlock) => None,
        Err(std::fs::TryLockError::Error(error)) => {
            // A filesystem without locking support should not stop the app.
            warn!(%error, "could not take the instance lock; running without it");
            Some(InstanceGuard { _file: Some(file) })
        }
    }
}

impl InstanceGuard {
    /// A guard that holds nothing, for when there is nowhere to put the lock.
    /// Refusing to start over that would be worse than a rare second window.
    fn unlocked() -> Self {
        Self { _file: None }
    }
}
