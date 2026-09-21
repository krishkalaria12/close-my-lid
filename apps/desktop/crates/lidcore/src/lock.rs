//! A pid lock recording which process owns the active hold.
//!
//! Without this, three things go wrong, all of which end with a laptop that
//! never sleeps again:
//!
//! - Two `enable` invocations both take holds. On Windows the second saves the
//!   first's temporary `0/0` lid actions, so whichever exits last writes
//!   "do nothing" back as if it were the user's setting.
//! - `disable` run from a second process creates a fresh backend that owns
//!   neither the Linux inhibitor descriptor nor the Windows saved values. Its
//!   release does nothing, yet it clears the shared state file and reports
//!   success while the real holder carries on.
//! - `status` trusts the state file, so after an `enable` process is killed on
//!   Linux — where the kernel silently drops the inhibitor — it keeps
//!   reporting a hold that no longer exists.
//!
//! The lock answers "is a live process holding right now", which is the
//! question all three need. Liveness is checked by looking the pid up in the
//! process table rather than trusting the file, so a stale lock left by a hard
//! kill is detected and taken over.

use std::fs;
use std::path::PathBuf;

use tracing::{debug, warn};

use crate::config;
use crate::error::{LidError, Result};

/// Owns the lock file for as long as the hold lasts.
#[derive(Debug)]
pub struct HoldLock {
    path: PathBuf,
}

impl HoldLock {
    /// Takes the lock for this process.
    ///
    /// Fails if another *live* process already holds it. A lock left behind by
    /// a process that is gone is taken over, since nothing is holding then.
    ///
    /// Uses atomic `create_new` so two simultaneous `enable` invocations cannot
    /// both win the check-then-write race (on Windows the loser would save the
    /// winner's temporary lid values as if they were the user's own).
    pub fn acquire() -> Result<Self> {
        let path = lock_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
        }

        // Fast path: a live owner blocks us without touching the filesystem.
        if let Some(pid) = live_owner(&path) {
            return Err(Self::already_held(pid));
        }

        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write as _;
                file.write_all(std::process::id().to_string().as_bytes())
                    .map_err(|error| LidError::io("write", &path, error))?;
                debug!(pid = std::process::id(), "took the hold lock");
                Ok(Self { path })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lost the race, or a stale lock was cleared between our check
                // and the create. Re-check once: a stale lock is cleared by
                // `live_owner`, freeing us to retry exactly once.
                if live_owner(&path).is_none() {
                    // Stale was just cleared; retry the atomic create.
                    match std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                    {
                        Ok(mut file) => {
                            use std::io::Write as _;
                            file.write_all(std::process::id().to_string().as_bytes())
                                .map_err(|e| LidError::io("write", &path, e))?;
                            debug!(pid = std::process::id(), "took the hold lock after stale");
                            return Ok(Self { path });
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            // Someone else won the retry race.
                            if let Some(pid) = live_owner(&path) {
                                return Err(Self::already_held(pid));
                            }
                        }
                        Err(error) => return Err(LidError::io("write", &path, error)),
                    }
                } else if let Some(pid) = live_owner(&path) {
                    return Err(Self::already_held(pid));
                }
                // Lock file exists but names no live owner (e.g. garbage that
                // `live_owner` left behind): overwrite it — we are the owner.
                fs::write(&path, std::process::id().to_string())
                    .map_err(|error| LidError::io("write", &path, error))?;
                debug!(
                    pid = std::process::id(),
                    "took the hold lock (overwrote dead file)"
                );
                Ok(Self { path })
            }
            Err(error) => Err(LidError::io("write", &path, error)),
        }
    }

    fn already_held(pid: u32) -> LidError {
        LidError::denied(
            "start a hold",
            format!("another Close My Lid process (pid {pid}) is already holding"),
        )
        .with_hint(
            "Stop the existing hold first with `close-my-lid disable`, or \
             press Ctrl-C in the terminal running it.",
        )
    }

    /// Fails when another live process currently owns the hold, for callers
    /// that apply a hold and exit (macOS `enable`) rather than holding the
    /// lock for the session's lifetime.
    pub fn check_available() -> Result<()> {
        match Self::owner() {
            Some(pid) if pid != std::process::id() => Err(Self::already_held(pid)),
            _ => Ok(()),
        }
    }

    /// The pid of the live process currently holding, if any.
    pub fn owner() -> Option<u32> {
        live_owner(&lock_path().ok()?)
    }

    /// Asks the owning process to stop, so it can release its own hold.
    ///
    /// On Linux this is the *only* correct way to end another process's hold:
    /// the inhibitor is a descriptor owned by that process, and nothing else
    /// can close it. Returns the pid that was signalled.
    pub fn signal_owner() -> Option<u32> {
        let pid = Self::owner()?;
        if pid == std::process::id() {
            return None;
        }

        if terminate(pid) {
            Some(pid)
        } else {
            warn!(pid, "could not signal the process holding the lid");
            None
        }
    }

    pub fn release(&self) {
        match fs::remove_file(&self.path) {
            Ok(()) => debug!("released the hold lock"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warn!(%error, "could not remove the hold lock"),
        }
    }
}

impl Drop for HoldLock {
    fn drop(&mut self) {
        self.release();
    }
}

fn lock_path() -> Result<PathBuf> {
    Ok(config::config_dir()?.join("hold.pid"))
}

/// Binary names that may legitimately own the hold, used to detect pid reuse.
/// Without this, an unrelated process that recycled a dead owner's pid would
/// block new holds until that pid exits.
const OWNER_BINARIES: [&str; 3] = ["close-my-lid", "close-my-lid-gui", "closemylid"];

/// The pid in the lock file, but only if that process is still running.
///
/// Clears the file when the owner is gone so the next caller does not have to
/// re-check a pid that will never come back.
fn live_owner(path: &PathBuf) -> Option<u32> {
    let raw = fs::read_to_string(path).ok()?;
    let pid: u32 = raw.trim().parse().ok()?;

    if pid == std::process::id() {
        return Some(pid);
    }

    let Some((name, exe)) = describe_process(pid) else {
        debug!(pid, "clearing a stale hold lock");
        let _ = fs::remove_file(path);
        return None;
    };
    if !looks_like_owner(&name, exe.as_deref()) {
        debug!(pid, %name, "lock names a recycled pid; clearing it");
        let _ = fs::remove_file(path);
        return None;
    }
    Some(pid)
}

/// Asks one process to exit. Term, not Kill: the owner must run its release
/// path on the way out.
///
/// NOTE: `sysinfo` supports only `Kill` on Windows (`taskkill /F`), so the
/// graceful signal is Unix-only. On Windows the caller falls back to restoring
/// via the recovery record after the wait.
fn terminate(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: `kill` takes a pid and a signal number and reports failure
        // through its return value; nothing is dereferenced.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) == 0 }
    }

    #[cfg(not(target_os = "macos"))]
    {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System};

        let mut system = System::new();
        let target = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::nothing(),
        );
        let Some(process) = system.process(target) else {
            return false;
        };
        #[cfg(not(target_os = "windows"))]
        let signal = Signal::Term;
        #[cfg(target_os = "windows")]
        let signal = Signal::Kill;
        process.kill_with(signal) == Some(true)
    }
}

/// The process's name and executable path, or `None` when it is gone.
///
/// macOS asks `libproc` about the one pid; elsewhere `sysinfo` refreshes a
/// single-process view, which is the narrowest request that API accepts.
fn describe_process(pid: u32) -> Option<(String, Option<std::path::PathBuf>)> {
    #[cfg(target_os = "macos")]
    {
        crate::agents::describe_process(pid)
    }

    #[cfg(not(target_os = "macos"))]
    {
        use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

        let mut system = System::new();
        let target = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        let process = system.process(target)?;
        Some((
            process.name().to_string_lossy().into_owned(),
            process.exe().map(std::path::Path::to_path_buf),
        ))
    }
}

/// True when the process plausibly is a Close My Lid holder: its name or exe
/// stem matches one of our binaries. A recycled pid running e.g. a browser
/// fails this and the stale lock is cleared instead of blocking holds.
fn looks_like_owner(name: &str, exe: Option<&std::path::Path>) -> bool {
    fn is_ours(candidate: &str) -> bool {
        // Windows reports `close-my-lid.exe`; compare the bare stem.
        let lowered = candidate.to_lowercase();
        let stem = lowered.strip_suffix(".exe").unwrap_or(&lowered);
        OWNER_BINARIES.contains(&stem)
    }

    if is_ours(name) {
        return true;
    }
    exe.and_then(std::path::Path::file_stem)
        .is_some_and(|stem| is_ours(&stem.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_naming_this_process_is_live() {
        let path = std::env::temp_dir().join("close-my-lid-live.pid");
        fs::write(&path, std::process::id().to_string()).unwrap();

        assert_eq!(live_owner(&path), Some(std::process::id()));
        fs::remove_file(&path).ok();
    }

    #[test]
    fn a_lock_naming_a_dead_process_is_cleared() {
        let path = std::env::temp_dir().join("close-my-lid-stale.pid");
        // A pid that cannot plausibly be running.
        fs::write(&path, "4294967294").unwrap();

        assert_eq!(live_owner(&path), None);
        assert!(
            !path.exists(),
            "a stale lock should be removed, not left to be re-checked"
        );
    }

    #[test]
    fn a_missing_or_malformed_lock_has_no_owner() {
        let missing = std::env::temp_dir().join("close-my-lid-none.pid");
        fs::remove_file(&missing).ok();
        assert_eq!(live_owner(&missing), None);

        let garbage = std::env::temp_dir().join("close-my-lid-garbage.pid");
        fs::write(&garbage, "not-a-pid").unwrap();
        assert_eq!(live_owner(&garbage), None);
        fs::remove_file(&garbage).ok();
    }
}
