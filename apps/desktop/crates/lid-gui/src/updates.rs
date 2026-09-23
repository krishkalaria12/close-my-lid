//! Fetching the update feed.
//!
//! Parsing, version comparison and the check that a download URL is one of
//! ours all live in `lidcore::updates`, shared with the macOS app. This is
//! only the GET, which desktop gpui has no client for.

use lidcore::updates::{APPCAST_URL, UpdateInfo, newer_release};

use crate::config::RELEASES_URL;
use crate::error::{GuiError, Result};

/// A newer release, or `None` when this build is current. Blocking: run it on
/// the background executor.
pub fn check() -> Result<Option<UpdateInfo>> {
    let fail = |error: &dyn std::fmt::Display| GuiError::UpdateFeed {
        detail: error.to_string(),
    };
    let feed = ureq::get(APPCAST_URL)
        .call()
        .map_err(|error| fail(&error))?
        .body_mut()
        .read_to_string()
        .map_err(|error| fail(&error))?;
    Ok(newer_release(&feed))
}

/// The release's page rather than the feed's download. The appcast enclosure
/// is the macOS archive, and a Windows or Linux user wants to pick their own
/// asset from the release.
pub fn release_page(update: &UpdateInfo) -> String {
    format!(
        "{RELEASES_URL}/tag/v{}",
        update.version.trim_start_matches('v')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_opens_its_release_page_not_the_macos_archive() {
        let update = UpdateInfo {
            version: "0.6.0".to_string(),
            url: format!("{RELEASES_URL}/download/v0.6.0/Close-My-Lid-v0.6.0-macOS.zip"),
        };
        assert_eq!(release_page(&update), format!("{RELEASES_URL}/tag/v0.6.0"));
    }
}
