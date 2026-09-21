//! Windows backend: rewrite the active power scheme's lid-close action, and
//! pair it with an execution-state request.
//!
//! Both halves are required. `SetThreadExecutionState` alone stops *idle*
//! sleep but does nothing when the lid shuts; `LIDACTION` alone lets the
//! machine idle out while the lid is down.
//!
//! Unlike the Linux inhibitor, this edit is global and persists across a
//! crash, so the previous AC and DC values are saved before the change and put
//! back on release — the same obligation the macOS app carries with `pmset`.
//! [`restore_from`] exists so a launch-time recovery pass can undo a hold left
//! behind by a hard kill.
//!
//! NOTE: written against the `windows` crate 0.62 API but not yet
//! compile-checked, since the workspace was scaffolded on macOS. Build and run
//! this on a Windows machine before trusting it.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use windows::Win32::Foundation::{HLOCAL, LocalFree, WIN32_ERROR};
use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_SYSTEM_REQUIRED, EXECUTION_STATE, PowerGetActiveScheme,
    PowerReadACValueIndex, PowerReadDCValueIndex, PowerSetActiveScheme, PowerWriteACValueIndex,
    PowerWriteDCValueIndex, SetThreadExecutionState,
};
use windows::core::GUID;

use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;

/// `GUID_SYSTEM_BUTTON_SUBGROUP` — the "Power buttons and lid" subgroup.
const SUBGROUP_BUTTONS: GUID = GUID::from_u128(0x4f971e89_eebd_4455_a8de_9e59040e7347);
/// `GUID_LIDCLOSE_ACTION` — what happens when the lid closes.
const SETTING_LID_ACTION: GUID = GUID::from_u128(0x5ca83367_6e45_459f_a27b_476b1d01c936);

/// `LIDACTION` values: 0 do nothing, 1 sleep, 2 hibernate, 3 shut down.
const LID_ACTION_DO_NOTHING: u32 = 0;

/// The lid-close settings that were in force before we changed them.
///
/// Records the scheme they came from: the user can switch power plans while a
/// hold is active, and restoring the old values into whatever is active *now*
/// would both corrupt that plan and strand the one we actually modified.
///
/// Written to disk as soon as the change is made, because this is the only
/// copy of the user's original settings. A hard kill would otherwise leave
/// `LIDACTION` set to "do nothing" permanently, with nothing left to restore
/// from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLidAction {
    /// The scheme GUID these values belong to, as a `u128` so it can be
    /// serialised.
    pub scheme: u128,
    pub on_ac: u32,
    pub on_battery: u32,
}

/// Where the recovery record lives, next to the session file.
fn recovery_path() -> Result<PathBuf> {
    Ok(crate::config::config_dir()?.join("windows-lid-action.json"))
}

fn write_recovery(saved: &SavedLidAction) -> Result<()> {
    let path = recovery_path()?;
    write_recovery_at(&path, saved)
}

fn write_recovery_at(path: &std::path::Path, saved: &SavedLidAction) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
    }
    let encoded = serde_json::to_string_pretty(saved).map_err(|source| LidError::Encode {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, encoded).map_err(|error| LidError::io("write", path, error))
}

fn read_recovery() -> Option<SavedLidAction> {
    let path = recovery_path().ok()?;
    let raw = fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&raw) {
        Ok(saved) => Some(saved),
        Err(error) => {
            warn!(path = %path.display(), %error, "discarding malformed lid recovery record");
            None
        }
    }
}

fn clear_recovery() {
    if let Ok(path) = recovery_path() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => warn!(path = %path.display(), %error, "could not clear recovery record"),
        }
    }
}

pub struct PowerSchemeLidGuard {
    saved: Option<SavedLidAction>,
    /// True only after *this* instance successfully acquired. `Drop` releases
    /// only then: a guard that merely adopted a recovery record (e.g. for
    /// `status`) must never release another process's live hold on drop.
    owned: bool,
}

impl PowerSchemeLidGuard {
    /// Adopts any recovery record a previous run left behind, so a hold
    /// stranded by a hard kill can be released by `release()` or by the
    /// session controller's launch reconciliation.
    ///
    /// The adopted record does NOT make this instance an owner: dropping it
    /// releases nothing. Only `acquire()` confers ownership.
    pub fn new() -> Self {
        let saved = read_recovery();
        if saved.is_some() {
            warn!("found a lid recovery record from a previous run");
        }
        Self {
            saved,
            owned: false,
        }
    }

    /// Opens the backend without adopting any recovery record. Use for
    /// read-only paths (`status`, `agents`) so merely inspecting the system
    /// can never release another process's hold on drop.
    pub fn open_readonly() -> Self {
        Self {
            saved: None,
            owned: false,
        }
    }

    /// Whether a recovery record from a previous run was adopted.
    pub fn has_adopted_recovery(&self) -> bool {
        self.saved.is_some() && !self.owned
    }

    /// Whether this instance currently owns the hold.
    pub fn is_owner(&self) -> bool {
        self.owned
    }

    /// The values that would be restored on release, if any.
    pub fn saved(&self) -> Option<SavedLidAction> {
        self.saved
    }
}

impl Default for PowerSchemeLidGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl LidPowerBackend for PowerSchemeLidGuard {
    fn acquire(&mut self) -> Result<()> {
        if self.owned && self.saved.is_some() {
            return Ok(());
        }
        // Adopted a stranded record but never owned it: releasing someone
        // else's hold is the reconciler's job, not an implicit acquire.
        // Fall through and take a fresh hold (HoldLock serialises this).
        if self.saved.is_some() && !self.owned {
            debug!("discarding adopted recovery before fresh acquire");
        }

        let scheme = active_scheme()?;
        let saved = read_lid_action(&scheme)?;

        // Persist before changing anything: if the process dies between the
        // write and the record, the user's original settings are gone.
        write_recovery(&saved)?;

        if let Err(error) = write_lid_values(&scheme, LID_ACTION_DO_NOTHING, LID_ACTION_DO_NOTHING)
        {
            // The write may have half-completed (AC ok, DC failed). Try to
            // put the original values back; if that also fails, KEEP the
            // recovery record so a later run can still repair.
            if let Err(rollback) = write_lid_values(&scheme, saved.on_ac, saved.on_battery) {
                warn!(%rollback, %error, "half-wrote the lid action; keeping recovery record");
            } else {
                clear_recovery();
            }
            return Err(error);
        }
        // Writes only take effect once the scheme is re-activated.
        if let Err(error) = apply_scheme(&scheme) {
            if let Err(rollback) = write_lid_values(&scheme, saved.on_ac, saved.on_battery) {
                warn!(%rollback, %error, "could not activate scheme; keeping recovery record");
            } else {
                let _ = apply_scheme(&scheme);
                clear_recovery();
            }
            return Err(error);
        }

        // A failure here has already changed the lid action, so undo it rather
        // than leaving the machine permanently configured not to sleep.
        if let Err(error) = set_execution_state(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) {
            if let Err(rollback) = write_lid_values(&scheme, saved.on_ac, saved.on_battery) {
                warn!(%rollback, "could not roll back the lid action after a failed hold; keeping recovery");
            } else {
                let _ = apply_scheme(&scheme);
                clear_recovery();
            }
            return Err(error);
        }

        self.saved = Some(saved);
        self.owned = true;
        debug!(?saved, "lid action held; previous values saved");
        Ok(())
    }

    fn release(&mut self) -> Result<()> {
        // Read without taking: if any step below fails, the record is the only
        // way back to the user's original settings and must survive for the
        // next attempt.
        let Some(saved) = self.saved else {
            return Ok(());
        };

        // Always attempt the restore even if clearing the execution-state
        // request fails; otherwise a transient ES error strands LIDACTION=0.
        let es_result = set_execution_state(ES_CONTINUOUS);

        // Restore into the scheme the values came from, not whatever is active
        // now: the user may have switched power plans mid-hold. Only
        // re-activate that scheme if it is still the active one — activating
        // a stale scheme would yank the user back onto their old plan.
        let scheme = GUID::from_u128(saved.scheme);
        let restore_result = (|| -> Result<()> {
            write_lid_values(&scheme, saved.on_ac, saved.on_battery)?;
            if is_scheme_active(&scheme)? {
                apply_scheme(&scheme)?;
            } else {
                debug!("saved scheme no longer active; restored values without activating");
            }
            Ok(())
        })();

        match (es_result, restore_result) {
            (Ok(()), Ok(())) => {
                self.saved = None;
                self.owned = false;
                clear_recovery();
                debug!(?saved, "lid action restored");
                Ok(())
            }
            (Err(es), Ok(())) => {
                // Lid values are back; only the ES clear failed. Forget the
                // record (nothing left to restore) but report the ES error.
                self.saved = None;
                self.owned = false;
                clear_recovery();
                Err(es)
            }
            (_, Err(restore)) => {
                // Keep saved + recovery so the next attempt can retry.
                warn!(%restore, "could not restore the lid action; keeping recovery record");
                Err(restore)
            }
        }
    }

    fn is_held(&self) -> Result<bool> {
        let scheme = active_scheme()?;
        let current = read_lid_action(&scheme)?;
        // Read from the OS rather than trusting our own field, so a hold the
        // user undid in Control Panel is reported honestly.
        Ok(current.on_ac == LID_ACTION_DO_NOTHING && current.on_battery == LID_ACTION_DO_NOTHING)
    }

    fn describe(&self) -> &'static str {
        "power scheme LIDACTION + SetThreadExecutionState"
    }
}

impl Drop for PowerSchemeLidGuard {
    fn drop(&mut self) {
        // Only release what this instance acquired. Adopted recovery records
        // belong to a dead process's stranded hold and are handled explicitly
        // by reconcile/release — auto-releasing here made `status` steal live
        // holds from other processes.
        if self.owned
            && self.saved.is_some()
            && let Err(error) = self.release()
        {
            warn!(%error, "could not restore the lid action on shutdown");
        }
    }
}

fn active_scheme() -> Result<GUID> {
    let mut scheme: *mut GUID = std::ptr::null_mut();
    // SAFETY: the call writes a pointer we own and must free; we copy the
    // value out before dropping the allocation on the next line.
    let status = unsafe { PowerGetActiveScheme(None, &mut scheme) };
    check(status, "read the active power scheme")?;

    if scheme.is_null() {
        return Err(LidError::backend(
            "read the active power scheme",
            "Windows reported no active scheme",
        ));
    }

    let value = unsafe { *scheme };
    unsafe { LocalFree(Some(HLOCAL(scheme as _))) };
    Ok(value)
}

fn read_lid_action(scheme: &GUID) -> Result<SavedLidAction> {
    let mut on_ac = 0u32;
    let mut on_battery = 0u32;

    let status = unsafe {
        PowerReadACValueIndex(
            None,
            Some(scheme),
            Some(&SUBGROUP_BUTTONS),
            Some(&SETTING_LID_ACTION),
            &mut on_ac,
        )
    };
    check(status, "read the plugged-in lid action")?;

    let status = unsafe {
        PowerReadDCValueIndex(
            None,
            Some(scheme),
            Some(&SUBGROUP_BUTTONS),
            Some(&SETTING_LID_ACTION),
            &mut on_battery,
        )
    };
    check(WIN32_ERROR(status), "read the on-battery lid action")?;

    Ok(SavedLidAction {
        scheme: scheme.to_u128(),
        on_ac,
        on_battery,
    })
}

/// Writes the AC + DC lid values without activating anything. Split from
/// activation so release can restore a now-inactive scheme without yanking
/// the user back onto it.
fn write_lid_values(scheme: &GUID, on_ac: u32, on_battery: u32) -> Result<()> {
    let status = unsafe {
        PowerWriteACValueIndex(
            None,
            scheme,
            Some(&SUBGROUP_BUTTONS),
            Some(&SETTING_LID_ACTION),
            on_ac,
        )
    };
    check(status, "write the plugged-in lid action")?;

    let status = unsafe {
        PowerWriteDCValueIndex(
            None,
            scheme,
            Some(&SUBGROUP_BUTTONS),
            Some(&SETTING_LID_ACTION),
            on_battery,
        )
    };
    check(WIN32_ERROR(status), "write the on-battery lid action")?;

    Ok(())
}

/// Re-activates the scheme so prior writes take effect.
fn apply_scheme(scheme: &GUID) -> Result<()> {
    let status = unsafe { PowerSetActiveScheme(None, Some(scheme)) };
    check(status, "re-apply the active power scheme")?;
    Ok(())
}

fn is_scheme_active(scheme: &GUID) -> Result<bool> {
    Ok(active_scheme()? == *scheme)
}

fn set_execution_state(state: EXECUTION_STATE) -> Result<()> {
    // Returns the previous state, or 0 on failure.
    let previous = unsafe { SetThreadExecutionState(state) };
    if previous == EXECUTION_STATE(0) {
        return Err(LidError::backend(
            "request that Windows stay awake",
            "SetThreadExecutionState was refused",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_record_round_trips() {
        let saved = SavedLidAction {
            scheme: 0x4f971e89_eebd_4455_a8de_9e59040e7347u128,
            on_ac: 1,
            on_battery: 2,
        };
        let path =
            std::env::temp_dir().join(format!("cml-win-recovery-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        write_recovery_at(&path, &saved).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let back: SavedLidAction = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, saved);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn readonly_guard_never_owns() {
        let guard = PowerSchemeLidGuard::open_readonly();
        assert!(!guard.is_owner());
        assert!(guard.saved().is_none());
    }
}

/// The `*DCValueIndex` calls return a bare `u32` while their `*ACValueIndex`
/// counterparts return `WIN32_ERROR`; callers wrap the former so this takes one
/// type.
fn check(status: WIN32_ERROR, what: &str) -> Result<()> {
    if status.0 == 0 {
        return Ok(());
    }

    let error = windows::core::Error::from_hresult(status.to_hresult());

    // ERROR_ACCESS_DENIED on a managed machine is the realistic failure here.
    if status == WIN32_ERROR(5) {
        return Err(
            LidError::denied(what.to_string(), error.to_string()).with_hint(
                "The active power scheme may be managed by group policy. Check with \
             `powercfg /getactivescheme`, or ask your administrator to allow \
             changing the lid-close action.",
            ),
        );
    }

    Err(LidError::backend(what.to_string(), error.to_string()))
}
