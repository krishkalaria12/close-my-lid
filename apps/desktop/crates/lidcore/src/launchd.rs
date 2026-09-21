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
    fs::write(&plist, xml).map_err(|error| LidError::io("write", &plist, error))?;

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

pub fn plist_xml(executable_path: &str, label: &str) -> String {
    fn escaped(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
           <key>Label</key>\n\
           <string>{}</string>\n\
           <key>ProgramArguments</key>\n\
           <array>\n\
             <string>{}</string>\n\
             <string>{}</string>\n\
           </array>\n\
           <key>StartInterval</key>\n\
           <integer>{}</integer>\n\
           <key>RunAtLoad</key>\n\
           <false/>\n\
           <key>ProcessType</key>\n\
           <string>Background</string>\n\
         </dict>\n\
         </plist>\n",
        escaped(label),
        escaped(executable_path),
        WATCHDOG_ARG,
        START_INTERVAL_SECONDS,
    )
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
    fn the_plist_lives_in_the_users_launch_agents() {
        let path = agent_plist_path(Path::new("/Users/someone"));
        assert_eq!(
            path,
            PathBuf::from("/Users/someone/Library/LaunchAgents/app.closemylid.watchdog.plist")
        );
    }
}
