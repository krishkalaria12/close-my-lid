//! The duration the user last picked, so the main ON/OFF toggle can re-apply it
//! instead of always starting an Unlimited hold.
//!
//! Carried over from the Swift app's `SelectedDurationStoring`. Stored as
//! seconds in a small text file next to the session file (`-1` means
//! indefinite, matching the Swift convention); a missing or corrupt file means
//! indefinite.

use std::fs;
use std::path::Path;

use crate::config;
use crate::duration::SessionDuration;
use crate::error::{LidError, Result};

const INDEFINITE_SECONDS: f64 = -1.0;

pub fn load() -> SessionDuration {
    let Ok(path) = config::selected_duration_file() else {
        return SessionDuration::Indefinite;
    };
    load_from(&path)
}

pub fn save(duration: SessionDuration) -> Result<()> {
    let path = config::selected_duration_file()?;
    save_to(&path, duration)
}

fn load_from(path: &Path) -> SessionDuration {
    let Ok(raw) = fs::read_to_string(path) else {
        return SessionDuration::Indefinite;
    };
    let Ok(seconds) = raw.trim().parse::<f64>() else {
        return SessionDuration::Indefinite;
    };
    if seconds <= 0.0 {
        return SessionDuration::Indefinite;
    }
    SessionDuration::Timed {
        minutes: (seconds / 60.0).round() as i64,
    }
}

fn save_to(path: &Path, duration: SessionDuration) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
    }
    let seconds = match duration {
        SessionDuration::Indefinite => INDEFINITE_SECONDS,
        SessionDuration::Timed { minutes } => minutes as f64 * 60.0,
    };
    fs::write(path, seconds.to_string()).map_err(|error| LidError::io("write", path, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn durations_survive_a_save_load_cycle() {
        for (duration, name) in [
            (SessionDuration::Indefinite, "cml-pref-indef"),
            (SessionDuration::THIRTY_MINUTES, "cml-pref-30"),
            (SessionDuration::ONE_HOUR, "cml-pref-60"),
            (SessionDuration::FOUR_HOURS, "cml-pref-240"),
        ] {
            let path = scratch(name);
            let _ = fs::remove_file(&path);
            save_to(&path, duration).unwrap();
            assert_eq!(load_from(&path), duration, "{name}");
            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn missing_or_corrupt_files_mean_indefinite() {
        let missing = scratch("cml-pref-absent");
        let _ = fs::remove_file(&missing);
        assert_eq!(load_from(&missing), SessionDuration::Indefinite);

        let corrupt = scratch("cml-pref-corrupt");
        fs::write(&corrupt, "soon").unwrap();
        assert_eq!(load_from(&corrupt), SessionDuration::Indefinite);

        // Zero and negatives are the Swift indefinite marker.
        for raw in ["-1", "-1.0", "0"] {
            fs::write(&corrupt, raw).unwrap();
            assert_eq!(load_from(&corrupt), SessionDuration::Indefinite, "{raw}");
        }
        let _ = fs::remove_file(&corrupt);
    }
}
