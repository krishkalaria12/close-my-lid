//! Crash-safe file writes for the two files that record a live hold.
//!
//! `session.json` and `heartbeat.json` share a directory with a watchdog that
//! wakes once a minute, and both are read back by whatever runs next after a
//! crash. A plain `fs::write` truncates first, so a process that dies between
//! the truncate and the write leaves an empty or half-written file — which
//! [`crate::store::SleepSessionStore::load`] can only read as "no session",
//! silently discarding the record of a hold that is still in force.
//!
//! Writing to a temporary file in the same directory and renaming over the
//! target makes the replacement atomic: a reader sees either the old contents
//! or the new ones, never a prefix of the new ones.

use std::io::Write as _;
use std::path::Path;

use crate::error::{LidError, Result};

/// Private to the user, which is all a session record needs to be.
pub const PRIVATE: u32 = 0o600;

/// World-readable, the convention for a LaunchAgent property list.
#[cfg(target_os = "macos")]
pub const READABLE: u32 = 0o644;

/// Replaces `path` with `contents`, atomically, leaving it at [`PRIVATE`].
pub fn write(path: &Path, contents: &str) -> Result<()> {
    write_with_mode(path, contents, PRIVATE)
}

/// Replaces `path` with `contents`, atomically, at the given permissions.
///
/// The temporary file is created in the destination's own directory because
/// `rename` is only atomic within one filesystem. Its mode is set before the
/// rename, so the file is never briefly visible at the wrong permissions.
pub fn write_with_mode(path: &Path, contents: &str, mode: u32) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;

    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| LidError::io("write", path, error))?;
    file.write_all(contents.as_bytes())
        .map_err(|error| LidError::io("write", path, error))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // `NamedTempFile` opens at 0600; anything else has to be asked for.
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|error| LidError::io("write", path, error))?;
    }
    #[cfg(not(unix))]
    let _ = mode;

    // Flush the bytes before the rename, so a power loss cannot leave the
    // directory entry pointing at a file whose contents never landed.
    file.as_file()
        .sync_all()
        .map_err(|error| LidError::io("write", path, error))?;

    file.persist(path)
        .map_err(|error| LidError::io("write", path, error.error))?;

    // The rename itself is a directory change, and `sync_all` above covered
    // only the file's own blocks. Without this a power loss between the two
    // can leave the directory entry still pointing at the *old* inode — which
    // for `session.json` is a hold recorded as running that no longer is.
    // Best-effort: a filesystem that refuses to open a directory (or to sync
    // one) must not turn a successful write into a failure.
    #[cfg(unix)]
    if let Ok(directory) = std::fs::File::open(parent) {
        let _ = directory.sync_all();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        write(&path, "first").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");

        write(&path, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deeper").join("state.json");

        write(&path, "{}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    }

    #[test]
    fn leaves_no_temporary_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        write(&path, "value").unwrap();

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("state.json")]);
    }

    #[cfg(unix)]
    #[test]
    fn files_land_at_the_requested_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();

        let private = dir.path().join("session.json");
        write(&private, "{}").unwrap();
        assert_eq!(
            std::fs::metadata(&private).unwrap().permissions().mode() & 0o777,
            PRIVATE
        );

        // A LaunchAgent plist keeps the 0644 every other agent in
        // `~/Library/LaunchAgents` has; the temporary file's own 0600 would
        // be an unexplained deviation.
        let readable = dir.path().join("agent.plist");
        write_with_mode(&readable, "<plist/>", 0o644).unwrap();
        assert_eq!(
            std::fs::metadata(&readable).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn write_failures_name_the_path() {
        // A directory where a file should be cannot be renamed over.
        let dir = tempfile::tempdir().unwrap();
        let clash = dir.path().join("occupied.json");
        std::fs::create_dir(&clash).unwrap();

        let error = write(&clash, "value").unwrap_err();
        assert!(error.to_string().contains("occupied.json"), "{error}");
    }
}
