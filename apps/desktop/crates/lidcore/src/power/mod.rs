//! Per-OS mechanisms for keeping the machine awake with the lid shut.
//!
//! The three platforms share no code here, because they share no mechanism:
//!
//! - **Linux** takes a `handle-lid-switch` inhibitor from `systemd-logind` and
//!   holds the returned file descriptor. Nothing is written to disk and the
//!   kernel releases the inhibitor if we die, so no watchdog is needed.
//! - **Windows** rewrites the active power scheme's `LIDACTION` to "do
//!   nothing" and pairs it with `SetThreadExecutionState`. That change is
//!   global and persistent, so the previous value is saved and restored.
//! - **macOS** sets `pmset -a disablesleep`, first through a passwordless
//!   sudoers grant and otherwise through a single administrator prompt. Like
//!   Windows the change is global and persistent, so a watchdog LaunchAgent
//!   restores normal sleep when a hold looks stranded.

use crate::error::Result;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub mod unsupported;
#[cfg(target_os = "windows")]
pub mod windows;

/// Acquire/release of an OS-level lid hold.
///
/// Implementations must be idempotent: `acquire` on an already-held backend
/// and `release` on an idle one both succeed without doing anything. The
/// session layer above relies on that when it reconciles after a crash.
pub trait LidPowerBackend: Send {
    /// Starts holding the lid open. Idempotent.
    fn acquire(&mut self) -> Result<()>;

    /// Stops holding and restores the machine's normal behaviour. Idempotent.
    fn release(&mut self) -> Result<()>;

    /// Whether a hold is currently in force, read from the OS where possible.
    fn is_held(&self) -> Result<bool>;

    /// Short description of the mechanism, for `status` output and bug reports.
    fn describe(&self) -> &'static str;
}

/// The backend for the current OS.
pub fn backend() -> Result<Box<dyn LidPowerBackend>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LogindInhibitor::new()))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::PowerSchemeLidGuard::new()))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::PmsetLidGuard::new()))
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(crate::error::LidError::UnsupportedPlatform {
            os: std::env::consts::OS,
        })
    }
}

/// A backend that never adopts another process's recovery state, for
/// read-only paths (`status`, `agents`). On Windows a full `backend()` would
/// adopt `windows-lid-action.json`; dropping that guard must not release a
/// live hold owned elsewhere, so inspection uses this instead.
#[cfg(target_os = "windows")]
pub fn backend_readonly() -> Result<Box<dyn LidPowerBackend>> {
    Ok(Box::new(windows::PowerSchemeLidGuard::open_readonly()))
}

/// Non-Windows platforms have no adopt-on-open, so readonly is identical.
#[cfg(not(target_os = "windows"))]
pub fn backend_readonly() -> Result<Box<dyn LidPowerBackend>> {
    backend()
}
