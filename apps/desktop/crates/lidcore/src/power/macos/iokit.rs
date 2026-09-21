//! Reading the system-wide `SleepDisabled` setting through IOKit.
//!
//! `pmset -g` prints this value, but the menu bar app asks for it on every
//! reconciliation pass for as long as it runs — a forked process every thirty
//! seconds, indefinitely, to learn one boolean. `IOPMCopySystemPowerSettings`
//! returns the same setting from the power management daemon directly:
//!
//! ```text
//! { SleepDisabled = 0; "Update DarkWakeBG Setting" = 1; }
//! ```
//!
//! Only the read moves here. Writing still goes through `pmset`, because the
//! sudoers grant is scoped to those two exact command lines and
//! `IOPMSetSystemPowerSetting` would need root in-process instead.

use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};

// SAFETY: `IOPMCopySystemPowerSettings` takes no arguments and returns either
// NULL or a +1 CFDictionary, which `CFRetained::from_raw` then owns.
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMCopySystemPowerSettings() -> *mut CFDictionary;
}

/// The key `pmset -a disablesleep` writes, as `pmset -g` prints it.
const SLEEP_DISABLED: &str = "SleepDisabled";

/// Whether closed-lid sleep is currently disabled system-wide.
///
/// `None` means IOKit had no answer — the dictionary was absent, or it did not
/// carry the key — and the caller should fall back to `pmset -g` rather than
/// guess. A machine that genuinely has the setting off reports `Some(false)`.
pub fn sleep_disabled() -> Option<bool> {
    let settings = system_power_settings()?;
    let key = CFString::from_str(SLEEP_DISABLED);
    let value = settings.get(&key)?;
    as_bool(&value)
}

fn system_power_settings() -> Option<CFRetained<CFDictionary<CFString, CFType>>> {
    // SAFETY: the call takes no arguments. It follows the Copy rule, so the
    // returned dictionary is owned here and released when the `CFRetained`
    // drops.
    let raw = unsafe { IOPMCopySystemPowerSettings() };
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` is a non-null, +1 CFDictionary of CFString keys, which is
    // what this API documents and what the runtime returns.
    Some(unsafe { CFRetained::from_raw(std::ptr::NonNull::new(raw)?.cast()) })
}

/// The value arrives as a `CFBoolean` in practice, but `pmset` has written the
/// setting as a number in the past and a property list round trip can turn one
/// into the other. Accept both rather than silently read a live hold as off.
fn as_bool(value: &CFType) -> Option<bool> {
    if let Some(boolean) = value.downcast_ref::<CFBoolean>() {
        return Some(boolean.as_bool());
    }
    if let Some(number) = value.downcast_ref::<CFNumber>() {
        return number.as_i64().map(|value| value != 0);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_is_readable_on_this_machine() {
        // Every Mac reports this setting; a `None` here means the IOKit path
        // is broken and every reconciliation would be forking `pmset` instead.
        let held = sleep_disabled();
        assert!(
            held.is_some(),
            "IOPMCopySystemPowerSettings should carry {SLEEP_DISABLED}"
        );
    }

    #[test]
    fn it_agrees_with_pmset() {
        let Some(held) = sleep_disabled() else {
            return;
        };
        let output = std::process::Command::new("/usr/bin/pmset")
            .arg("-g")
            .output()
            .expect("pmset runs on macOS");
        let printed =
            super::super::disable_sleep_is_enabled(&String::from_utf8_lossy(&output.stdout));
        assert_eq!(held, printed, "IOKit and pmset must report the same hold");
    }

    #[test]
    fn both_plist_spellings_of_the_value_are_read() {
        assert_eq!(as_bool(CFBoolean::new(true).as_ref()), Some(true));
        assert_eq!(as_bool(CFBoolean::new(false).as_ref()), Some(false));

        assert_eq!(as_bool(CFNumber::new_i64(1).as_ref()), Some(true));
        assert_eq!(as_bool(CFNumber::new_i64(0).as_ref()), Some(false));

        // Anything else means "IOKit had no usable answer", not "off".
        assert_eq!(as_bool(CFString::from_str("maybe").as_ref()), None);
    }
}
