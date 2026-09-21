//! Reporting a refusal to the user.
//!
//! `LidError` already carries the attempted action and an actionable hint, so
//! an alert is only a matter of splitting those across the two lines AppKit
//! gives it — no error strings are written here.

use lidcore::LidError;
use objc2_app_kit::{NSAlert, NSAlertStyle};
use objc2_foundation::{MainThreadMarker, NSString};
use tracing::error;

/// Shows the error and returns once the user dismisses it.
pub fn show(mtm: MainThreadMarker, error: &LidError) {
    error!(%error, hint = error.hint(), "surfacing a failure to the user");

    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(&headline(error)));
    alert.setInformativeText(&NSString::from_str(&detail(error)));
    alert.setAlertStyle(NSAlertStyle::Warning);
    alert.runModal();
}

/// The same, for a failure during startup that leaves nothing to run.
pub fn fatal(mtm: MainThreadMarker, error: &LidError) {
    error!(%error, "could not start");

    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Close My Lid could not start."));
    alert.setInformativeText(&NSString::from_str(&detail(error)));
    alert.setAlertStyle(NSAlertStyle::Critical);
    alert.runModal();
}

fn headline(error: &LidError) -> String {
    match error {
        LidError::ElevationCancelled => "Administrator approval cancelled".to_string(),
        _ => "Close My Lid could not update sleep settings.".to_string(),
    }
}

/// The cause, then what to do about it. A hint alone would not say what went
/// wrong; the cause alone would not say what to try.
fn detail(error: &LidError) -> String {
    match error.hint() {
        Some(hint) if matches!(error, LidError::ElevationCancelled) => hint.to_string(),
        Some(hint) => format!("{error}\n\n{hint}"),
        None => error.to_string(),
    }
}
