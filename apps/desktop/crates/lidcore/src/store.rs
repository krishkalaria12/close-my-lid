use std::fs;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::config;
use crate::error::{LidError, Result};
use crate::state::SleepControlState;

/// Persists the current session across restarts.
///
/// Reads are deliberately forgiving: a corrupt file is treated as "no session"
/// rather than a hard error, because the app is what releases a stranded hold
/// and must always be able to start. Writes are strict — silently failing to
/// record a hold would leave nothing to recover from — and atomic, so a
/// process that dies mid-save cannot turn a live session into a truncated
/// file that reads back as "no session".
#[derive(Debug, Clone)]
pub struct SleepSessionStore {
    path: PathBuf,
}

impl SleepSessionStore {
    /// Uses the platform config directory from [`crate::config`].
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: config::session_file()?,
        })
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> SleepControlState {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            // A missing file is the normal first-run case, not a problem.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return SleepControlState::Inactive;
            }
            Err(error) => {
                warn!(path = %self.path.display(), %error, "could not read session state");
                return SleepControlState::Inactive;
            }
        };

        serde_json::from_str(&raw).unwrap_or_else(|error| {
            warn!(
                path = %self.path.display(),
                %error,
                "discarding malformed session state"
            );
            SleepControlState::Inactive
        })
    }

    /// Strict counterpart to [`Self::load`]: reports why the file could not be
    /// parsed instead of silently resetting. Used by diagnostics.
    pub fn load_strict(&self) -> Result<SleepControlState> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SleepControlState::Inactive);
            }
            Err(error) => return Err(LidError::io("read", &self.path, error)),
        };

        serde_json::from_str(&raw).map_err(|source| LidError::Decode {
            path: self.path.clone(),
            source,
        })
    }

    pub fn save(&self, state: &SleepControlState) -> Result<()> {
        let encoded = serde_json::to_string_pretty(state).map_err(|source| LidError::Encode {
            path: self.path.clone(),
            source,
        })?;

        crate::atomic::write(&self.path, &encoded)
    }

    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(LidError::io("remove", &self.path, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn temp_store(name: &str) -> SleepSessionStore {
        SleepSessionStore::at(std::env::temp_dir().join(name))
    }

    #[test]
    fn round_trips_state() {
        let store = temp_store("close-my-lid-roundtrip.json");
        let _ = store.clear();

        let state = SleepControlState::Active {
            started_at: Utc::now(),
            ends_at: None,
        };
        store.save(&state).unwrap();
        assert_eq!(store.load(), state);

        store.clear().unwrap();
        assert_eq!(store.load(), SleepControlState::Inactive);
    }

    #[test]
    fn malformed_state_reads_as_inactive() {
        let store = temp_store("close-my-lid-corrupt.json");
        fs::create_dir_all(store.path().parent().unwrap()).unwrap();
        fs::write(store.path(), "{ not json").unwrap();

        assert_eq!(store.load(), SleepControlState::Inactive);
        store.clear().unwrap();
    }

    #[test]
    fn strict_loading_reports_why_parsing_failed() {
        let store = temp_store("close-my-lid-strict.json");
        fs::write(store.path(), "{ not json").unwrap();

        let error = store.load_strict().unwrap_err();
        assert!(
            matches!(error, LidError::Decode { .. }),
            "expected a decode error, got {error:?}"
        );
        // The path belongs in the message so the user can go and look at it.
        assert!(error.to_string().contains("close-my-lid-strict.json"));
        store.clear().unwrap();
    }

    #[test]
    fn a_missing_file_is_not_an_error_either_way() {
        let store = temp_store("close-my-lid-absent.json");
        let _ = store.clear();
        assert!(store.clear().is_ok());
        assert_eq!(store.load_strict().unwrap(), SleepControlState::Inactive);
    }

    #[test]
    fn write_failures_name_the_path() {
        // A directory where a file should be cannot be written to.
        let dir = std::env::temp_dir().join("close-my-lid-dir-clash.json");
        fs::create_dir_all(&dir).unwrap();
        let store = SleepSessionStore::at(&dir);

        let error = store.save(&SleepControlState::Inactive).unwrap_err();
        assert!(error.to_string().contains("close-my-lid-dir-clash.json"));
        fs::remove_dir_all(&dir).ok();
    }
}
