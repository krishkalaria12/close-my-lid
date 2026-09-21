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

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System};
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
    pub fn acquire() -> Result<Self> {
        let path = lock_path()?;

        if let Some(pid) = live_owner(&path) {
            return Err(LidError::denied(
                "start a hold",
                format!("another Close My Lid process (pid {pid}) is already holding"),
            )
            .with_hint(
                "Stop the existing hold first with `close-my-lid disable`, or \
                 press Ctrl-C in the terminal running it.",
            ));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
        }
        fs::write(&path, std::process::id().to_string())
            .map_err(|error| LidError::io("write", &path, error))?;

        debug!(pid = std::process::id(), "took the hold lock");
        Ok(Self { path })
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

        let mut system = System::new();
        let target = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[target]),
            true,
            ProcessRefreshKind::nothing(),
        );

        let process = system.process(target)?;
        // Term, not Kill: the owner must run its release path on the way out.
        match process.kill_with(Signal::Term) {
            Some(true) => Some(pid),
            _ => {
                warn!(pid, "could not signal the process holding the lid");
                None
            }
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

    let mut system = System::new();
    let target = Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[target]),
        true,
        ProcessRefreshKind::nothing(),
    );

    if system.process(target).is_some() {
        Some(pid)
    } else {
        debug!(pid, "clearing a stale hold lock");
        let _ = fs::remove_file(path);
        None
    }
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
