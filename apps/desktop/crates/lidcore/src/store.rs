use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use tracing::warn;

use crate::error::Result;
use crate::state::SleepControlState;

/// Persists the current session across restarts.
///
/// Corrupt or unreadable state is treated as "no session" rather than a hard
/// error: a bad file must never stop the app from starting, because the app is
/// what releases a stranded hold.
#[derive(Debug, Clone)]
pub struct SleepSessionStore {
    path: PathBuf,
}

impl SleepSessionStore {
    /// Stores under the platform config dir — `~/.config/close-my-lid` on
    /// Linux, `%APPDATA%\close-my-lid` on Windows.
    pub fn new() -> Self {
        let path = ProjectDirs::from("com", "krishkalaria", "close-my-lid")
            .map(|dirs| dirs.config_dir().join("session.json"))
            .unwrap_or_else(|| PathBuf::from("close-my-lid-session.json"));
        Self { path }
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn load(&self) -> SleepControlState {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(_) => return SleepControlState::Inactive,
        };

        serde_json::from_str(&raw).unwrap_or_else(|error| {
            warn!(%error, "discarding malformed session state");
            SleepControlState::Inactive
        })
    }

    pub fn save(&self, state: &SleepControlState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(state)?)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Default for SleepSessionStore {
    fn default() -> Self {
        Self::new()
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
    fn clearing_a_missing_file_is_not_an_error() {
        let store = temp_store("close-my-lid-absent.json");
        let _ = store.clear();
        assert!(store.clear().is_ok());
    }
}
