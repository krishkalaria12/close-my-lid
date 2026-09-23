//! The app's own preferences — only what the CLI has no use for.
//!
//! The last-picked hold duration is not here: that lives in `lidcore::prefs`
//! next to the session file, shared with the macOS app's ON/OFF toggle.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::{GuiError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiPrefs {
    /// Post notifications when a hold starts, is about to end, and ends.
    pub notifications: bool,
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            notifications: true,
        }
    }
}

fn path() -> Result<PathBuf> {
    Ok(lidcore::config::config_dir()
        .map_err(|error| GuiError::Settings {
            detail: error.to_string(),
        })?
        .join("gui.json"))
}

/// A missing or unreadable file means the defaults: preferences are a
/// convenience, and a bad one must not keep the app from starting.
pub fn load() -> GuiPrefs {
    let Ok(path) = path() else {
        return GuiPrefs::default();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return GuiPrefs::default();
    };
    serde_json::from_str(&raw)
        .inspect_err(|error| warn!(%error, "ignoring unreadable preferences"))
        .unwrap_or_default()
}

pub fn save(prefs: GuiPrefs) -> Result<()> {
    let path = path()?;
    let fail = |error: &dyn std::fmt::Display| GuiError::Settings {
        detail: error.to_string(),
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| fail(&error))?;
    }
    let json = serde_json::to_string_pretty(&prefs).map_err(|error| fail(&error))?;
    fs::write(&path, json).map_err(|error| fail(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_from_an_older_build_keeps_the_defaults_it_does_not_mention() {
        let prefs: GuiPrefs = serde_json::from_str("{}").unwrap();
        assert_eq!(prefs, GuiPrefs::default());
    }

    #[test]
    fn notifications_are_on_until_turned_off() {
        assert!(GuiPrefs::default().notifications);
    }
}
