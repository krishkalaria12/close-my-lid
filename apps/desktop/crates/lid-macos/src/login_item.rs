//! Launch at login, through `SMAppService`.
//!
//! Carried over from the Swift app's `LaunchAtLoginController`.
//! `SMAppService.mainApp` registers the bundle itself rather than a helper,
//! which is what lets this work without shipping a second executable — but it
//! also means it does nothing useful for an unbundled build, where there is no
//! app to register.

use lidcore::{LidError, Result};
use objc2_service_management::{SMAppService, SMAppServiceStatus};
use tracing::debug;

pub fn is_enabled() -> bool {
    unsafe { SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled }
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let service = unsafe { SMAppService::mainAppService() };
    let status = unsafe { service.status() };

    // Registering something already registered raises; so does unregistering
    // something that is not. Both are the state the caller asked for.
    let result = match (enabled, status) {
        (true, SMAppServiceStatus::Enabled) | (false, SMAppServiceStatus::NotRegistered) => {
            return Ok(());
        }
        (true, _) => unsafe { service.registerAndReturnError() },
        (false, _) => unsafe { service.unregisterAndReturnError() },
    };

    result.map_err(|error| {
        debug!(%error, enabled, "SMAppService refused the login item change");
        let action = if enabled {
            "add Close My Lid to your login items"
        } else {
            "remove Close My Lid from your login items"
        };
        LidError::denied(action, error.localizedDescription().to_string()).with_hint(
            "Check Login Items in System Settings › General, and that the app is \
             in /Applications rather than being run from a build directory.",
        )
    })
}
