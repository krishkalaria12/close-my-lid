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
/// Persisted alongside the session so a launch-time recovery pass can restore
/// them even if the process was killed outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLidAction {
    pub on_ac: u32,
    pub on_battery: u32,
}

pub struct PowerSchemeLidGuard {
    saved: Option<SavedLidAction>,
}

impl PowerSchemeLidGuard {
    pub fn new() -> Self {
        Self { saved: None }
    }

    /// Undoes a hold recorded by a previous run that did not exit cleanly.
    pub fn restore_from(saved: SavedLidAction) -> Result<()> {
        let scheme = active_scheme()?;
        write_lid_action(&scheme, saved.on_ac, saved.on_battery)?;
        debug!(?saved, "restored lid action from a previous run");
        Ok(())
    }

    /// The values to persist so a crash can be recovered from.
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
        if self.saved.is_some() {
            return Ok(());
        }

        let scheme = active_scheme()?;
        let saved = read_lid_action(&scheme)?;

        write_lid_action(&scheme, LID_ACTION_DO_NOTHING, LID_ACTION_DO_NOTHING)?;
        set_execution_state(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)?;

        self.saved = Some(saved);
        debug!(?saved, "lid action held; previous values saved");
        Ok(())
    }

    fn release(&mut self) -> Result<()> {
        let Some(saved) = self.saved.take() else {
            return Ok(());
        };

        // Drop the execution-state request first so an error restoring the
        // scheme cannot also leave the machine pinned awake.
        set_execution_state(ES_CONTINUOUS)?;

        let scheme = active_scheme()?;
        write_lid_action(&scheme, saved.on_ac, saved.on_battery)?;
        debug!(?saved, "lid action restored");
        Ok(())
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
        if self.saved.is_some()
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
        return Err(LidError::Backend("the active power scheme was null".into()));
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

    Ok(SavedLidAction { on_ac, on_battery })
}

fn write_lid_action(scheme: &GUID, on_ac: u32, on_battery: u32) -> Result<()> {
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

    // Writes only take effect once the scheme is re-activated.
    let status = unsafe { PowerSetActiveScheme(None, Some(scheme)) };
    check(status, "re-apply the active power scheme")?;

    Ok(())
}

fn set_execution_state(state: EXECUTION_STATE) -> Result<()> {
    // Returns the previous state, or 0 on failure.
    let previous = unsafe { SetThreadExecutionState(state) };
    if previous == EXECUTION_STATE(0) {
        return Err(LidError::Backend(
            "SetThreadExecutionState was refused".into(),
        ));
    }
    Ok(())
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
        return Err(LidError::Denied(format!(
            "Windows refused to {what}: {error}. The active power scheme may be \
             managed by group policy."
        )));
    }

    Err(LidError::Backend(format!("could not {what}: {error}")))
}
