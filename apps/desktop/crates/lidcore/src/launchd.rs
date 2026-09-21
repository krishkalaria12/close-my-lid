//! The dead-man watchdog LaunchAgent that restores normal sleep if the app dies
//! while a hold is stranded.
//!
//! Carried over from the Swift app's `WatchdogAgentController`. The agent
//! lives in `~/Library/LaunchAgents` and is managed directly with `launchctl`,
//! so it works from hand-built bundles without an approval flow. It invokes
//! the same executable with `--watchdog`.
//!
//! Registration happens only once the passwordless sudoers grant exists —
//! without the grant the agent could not release anything anyway.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::{LidError, Result};

pub const LABEL: &str = "app.closemylid.watchdog";
pub const START_INTERVAL_SECONDS: u64 = 60;
pub const WATCHDOG_ARG: &str = "--watchdog";

/// Where the agent plist lives for the given home directory. Pure so the
/// layout is testable without touching the real `~/Library`.
pub fn agent_plist_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Where the agent plist lives for the current user.
pub fn plist_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|base| agent_plist_path(base.home_dir()))
}

pub fn is_installed() -> bool {
    plist_path().is_some_and(|path| path.is_file())
}

/// Writes the agent plist and registers it with launchd. Skipped silently for
/// unbundled development runs, where there is no stable executable path to
/// point at.
pub fn install() -> Result<()> {
    let Some(executable) = resolved_executable_path() else {
        return Ok(());
    };
    let Some(plist) = plist_path() else {
        return Err(LidError::NoConfigDir);
    };

    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
    }

    let xml = plist_xml(&executable, LABEL);
    // Atomic, because launchd is told to load this the moment it exists and a
    // half-written plist is one it refuses without saying so. 0644 to match
    // every other agent in the directory.
    crate::atomic::write_with_mode(&plist, &xml, crate::atomic::READABLE)?;

    // Refresh any stale registration that points at an older binary.
    bootout();
    let _ = Command::new("/bin/launchctl")
        .args([
            "bootstrap",
            &format!("gui/{}", uid()),
            &plist.to_string_lossy(),
        ])
        .output();
    Ok(())
}

pub fn uninstall() {
    bootout();
    if let Some(plist) = plist_path() {
        let _ = fs::remove_file(plist);
    }
}

/// Bundled apps only: development binaries under `target/` are rebuilt and
/// moved freely, which would leave the agent pointing at a missing executable.
/// Hence the check for a `.app` bundle path.
fn resolved_executable_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.to_string_lossy().into_owned();
    if !path.contains(".app/") {
        return None;
    }
    Some(path)
}

fn bootout() {
    let _ = Command::new("/bin/launchctl")
        .args(["bootout", &format!("gui/{}/{}", uid(), LABEL)])
        .output();
}

fn uid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: getuid takes no arguments, has no failure mode, and is
        // async-signal-safe; declaring it ourselves avoids a libc dependency
        // for a single syscall.
        unsafe { getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

#[cfg(unix)]
#[link(name = "c")]
unsafe extern "C" {
    fn getuid() -> u32;
}

/// The agent's property list, as launchd reads it.
///
/// Built through the `plist` crate rather than a format string: an executable
/// path is arbitrary user-controlled text, and a bundle whose name contains
/// `&` or `<` would otherwise produce a plist launchd silently refuses to
/// load, leaving the dead-man switch installed but inert.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AgentPlist {
    label: String,
    program_arguments: Vec<String>,
    start_interval: u64,
    /// Never at load: the watchdog only has work to do once a hold exists, and
    /// launchd runs it on the interval regardless.
    run_at_load: bool,
    /// Background: the pass is a couple of syscalls and must not compete with
    /// whatever the user is doing.
    process_type: String,
}

pub fn plist_xml(executable_path: &str, label: &str) -> String {
    let agent = AgentPlist {
        label: label.to_string(),
        program_arguments: vec![executable_path.to_string(), WATCHDOG_ARG.to_string()],
        start_interval: START_INTERVAL_SECONDS,
        run_at_load: false,
        process_type: "Background".to_string(),
    };

    let mut xml = Vec::new();
    plist::to_writer_xml(&mut xml, &agent).expect("an agent plist always serialises");
    String::from_utf8(xml).expect("the plist writer emits UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plist_runs_the_watchdog_every_minute() {
        let xml = plist_xml(
            "/Applications/Close My Lid.app/Contents/MacOS/CloseMyLid",
            LABEL,
        );
        assert!(
            xml.contains("<string>app.closemylid.watchdog</string>"),
            "{xml}"
        );
        assert!(xml.contains("<string>--watchdog</string>"), "{xml}");
        assert!(xml.contains("<integer>60</integer>"), "{xml}");
        assert!(xml.contains("<false/>"), "{xml}");
    }

    #[test]
    fn paths_are_xml_escaped() {
        let xml = plist_xml("/tmp/a&b<App>.app/x", "a<b>&c");
        assert!(xml.contains("a&lt;b&gt;&amp;c"), "{xml}");
        assert!(xml.contains("/tmp/a&amp;b&lt;App&gt;.app/x"), "{xml}");
    }

    #[test]
    fn an_awkwardly_named_bundle_still_produces_a_loadable_plist() {
        // launchd refuses a malformed plist silently, which would leave the
        // dead-man switch installed but never firing.
        let path = "/Applications/Ben & Jerry's <Lid>.app/Contents/MacOS/CloseMyLid";
        let xml = plist_xml(path, LABEL);

        let parsed: plist::Value = plist::from_bytes(xml.as_bytes()).expect("valid plist");
        let dictionary = parsed.as_dictionary().expect("a dict at the root");
        let arguments = dictionary["ProgramArguments"]
            .as_array()
            .expect("an argument array");
        assert_eq!(arguments[0].as_string(), Some(path));
        assert_eq!(arguments[1].as_string(), Some(WATCHDOG_ARG));
        assert_eq!(dictionary["StartInterval"].as_unsigned_integer(), Some(60));
        assert_eq!(dictionary["RunAtLoad"].as_boolean(), Some(false));
    }

    #[test]
    fn the_plist_lives_in_the_users_launch_agents() {
        let path = agent_plist_path(Path::new("/Users/someone"));
        assert_eq!(
            path,
            PathBuf::from("/Users/someone/Library/LaunchAgents/app.closemylid.watchdog.plist")
        );
    }
}
