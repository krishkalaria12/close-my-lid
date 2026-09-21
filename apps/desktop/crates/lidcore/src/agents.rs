//! Counts running coding-agent sessions, so the UI can show what the hold is
//! protecting.
//!
//! Ported from the Swift `AgentHarness` / `AgentSessionClassifier` pair. The
//! macOS version walks the process table with `sysctl`; here `sysinfo` covers
//! `/proc` on Linux and Toolhelp32 on Windows behind one API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A coding agent CLI whose sessions can be detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHarness {
    ClaudeCode,
    Codex,
    OpenCode,
    Antigravity,
    Copilot,
    Cursor,
    Pi,
}

impl AgentHarness {
    pub const ALL: [Self; 7] = [
        Self::ClaudeCode,
        Self::Codex,
        Self::OpenCode,
        Self::Antigravity,
        Self::Copilot,
        Self::Cursor,
        Self::Pi,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "OpenAI Codex CLI",
            Self::OpenCode => "OpenCode",
            Self::Antigravity => "Antigravity",
            Self::Copilot => "GitHub Copilot CLI",
            Self::Cursor => "Cursor CLI",
            Self::Pi => "Pi",
        }
    }

    /// Badge colour used by the tray panel, matching the macOS `AgentIcons`.
    pub fn badge_rgb(&self) -> u32 {
        match self {
            Self::ClaudeCode => 0x1c1c1f,
            Self::Codex => 0x111111,
            Self::OpenCode => 0x17171c,
            // Antigravity's mark is a black silhouette and needs a light badge.
            Self::Antigravity => 0xf5f5fa,
            Self::Copilot => 0x0d1217,
            Self::Cursor => 0x0d0d0f,
            Self::Pi => 0x0a0a0d,
        }
    }

    /// Native executable names, without any platform extension.
    fn executable_names(&self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &["claude"],
            Self::Codex => &["codex"],
            Self::OpenCode => &["opencode"],
            // Separate CLI and desktop surfaces, reported as one row.
            Self::Antigravity => &["agy", "antigravity"],
            Self::Copilot => &["copilot"],
            Self::Cursor => &["cursor-agent"],
            Self::Pi => &["pi"],
        }
    }

    /// Install-directory fragments identifying a harness that runs as a script
    /// under a JavaScript runtime. Anchored to the install layout so a
    /// similarly named project directory cannot match.
    fn script_path_markers(&self) -> &'static [&'static str] {
        match self {
            Self::ClaudeCode => &["node_modules/@anthropic-ai/claude-code"],
            Self::Codex => &["node_modules/@openai/codex"],
            Self::OpenCode => &["node_modules/opencode-ai"],
            Self::Antigravity => &[],
            Self::Copilot => &["node_modules/@github/copilot"],
            Self::Cursor => &["cursor-agent/versions/"],
            // The agent moved npm scopes; both remain installable.
            Self::Pi => &[
                "node_modules/@earendil-works/pi-coding-agent",
                "node_modules/@mariozechner/pi-coding-agent",
            ],
        }
    }

    fn matching_executable(name: &str) -> Option<Self> {
        // Windows reports `claude.exe`; compare against the bare stem.
        let stem = name.strip_suffix(".exe").unwrap_or(name).to_lowercase();
        Self::ALL
            .into_iter()
            .find(|harness| harness.executable_names().contains(&stem.as_str()))
    }

    fn matching_script_path(path: &str) -> Option<Self> {
        // Normalise Windows separators so one marker list serves both.
        let normalised = path.replace('\\', "/");
        Self::ALL.into_iter().find(|harness| {
            harness
                .script_path_markers()
                .iter()
                .any(|marker| normalised.contains(marker))
        })
    }
}

/// JavaScript runtimes that npm-installed harnesses run under.
const SCRIPT_RUNTIMES: [&str; 2] = ["node", "bun"];

/// One running process, reduced to the fields needed for classification.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub executable_name: String,
    pub arguments: Vec<String>,
}

impl RunningProcess {
    fn harness(&self) -> Option<AgentHarness> {
        if let Some(harness) = AgentHarness::matching_executable(&self.executable_name) {
            return Some(harness);
        }

        let stem = self
            .executable_name
            .strip_suffix(".exe")
            .unwrap_or(&self.executable_name)
            .to_lowercase();
        if !SCRIPT_RUNTIMES.contains(&stem.as_str()) {
            return None;
        }

        // Runtime flags and subcommands never contain a path separator, so only
        // path-like arguments are script candidates. A candidate matches when
        // its basename is a harness executable (bin shims keep the name) or
        // when it sits inside the harness's install directory.
        for argument in self.arguments.iter().skip(1) {
            if !argument.contains('/') && !argument.contains('\\') {
                continue;
            }
            let basename = argument.rsplit(['/', '\\']).next().unwrap_or(argument);
            if let Some(harness) = AgentHarness::matching_executable(basename) {
                return Some(harness);
            }
            if let Some(harness) = AgentHarness::matching_script_path(argument) {
                return Some(harness);
            }
        }
        None
    }
}

/// Counts sessions per harness.
///
/// A session is a matched process with no ancestor matched to the *same*
/// harness, so a launcher that spawns its own native binary (Codex's npm
/// wrapper, OpenCode's server/TUI split) counts once rather than twice.
pub fn session_counts(processes: &[RunningProcess]) -> HashMap<AgentHarness, usize> {
    let mut parents: HashMap<u32, u32> = HashMap::with_capacity(processes.len());
    let mut matches: HashMap<u32, AgentHarness> = HashMap::new();

    for process in processes {
        if let Some(parent) = process.parent_pid {
            parents.insert(process.pid, parent);
        }
        if let Some(harness) = process.harness() {
            matches.insert(process.pid, harness);
        }
    }

    let mut counts = HashMap::new();
    for (pid, harness) in &matches {
        if !has_ancestor(*pid, *harness, &matches, &parents) {
            *counts.entry(*harness).or_insert(0) += 1;
        }
    }
    counts
}

fn has_ancestor(
    pid: u32,
    harness: AgentHarness,
    matches: &HashMap<u32, AgentHarness>,
    parents: &HashMap<u32, u32>,
) -> bool {
    let mut current = parents.get(&pid).copied();
    // Chains are short; the hop bound terminates any ppid cycle that pid reuse
    // could introduce mid-snapshot.
    let mut hops = 0;
    while let Some(parent) = current {
        if hops >= 64 || parent == 0 {
            break;
        }
        if matches.get(&parent) == Some(&harness) {
            return true;
        }
        current = parents.get(&parent).copied();
        hops += 1;
    }
    false
}

/// Snapshots the process table and counts sessions.
///
/// Note this counts every visible process, not just the current user's — the
/// macOS version filters by uid, which has no clean cross-platform equivalent.
/// On a single-user laptop, which is the target, the result is the same.
pub fn sessions_now() -> HashMap<AgentHarness, usize> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );

    let processes: Vec<RunningProcess> = system
        .processes()
        .iter()
        .map(|(pid, process)| RunningProcess {
            pid: pid.as_u32(),
            parent_pid: process.parent().map(|parent| parent.as_u32()),
            executable_name: process.name().to_string_lossy().to_string(),
            arguments: process
                .cmd()
                .iter()
                .map(|argument| argument.to_string_lossy().to_string())
                .collect(),
        })
        .collect();

    session_counts(&processes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent: u32, name: &str, args: &[&str]) -> RunningProcess {
        RunningProcess {
            pid,
            parent_pid: Some(parent),
            executable_name: name.to_string(),
            arguments: args.iter().map(|arg| arg.to_string()).collect(),
        }
    }

    #[test]
    fn counts_a_native_binary() {
        let counts = session_counts(&[process(10, 1, "claude", &["claude"])]);
        assert_eq!(counts.get(&AgentHarness::ClaudeCode), Some(&1));
    }

    #[test]
    fn matches_windows_executables() {
        let counts = session_counts(&[process(10, 1, "cursor-agent.exe", &[])]);
        assert_eq!(counts.get(&AgentHarness::Cursor), Some(&1));
    }

    #[test]
    fn recognises_a_script_under_node() {
        let counts = session_counts(&[process(
            11,
            1,
            "node",
            &["node", "/usr/lib/node_modules/@openai/codex/bin/codex.js"],
        )]);
        assert_eq!(counts.get(&AgentHarness::Codex), Some(&1));
    }

    #[test]
    fn does_not_count_a_wrapper_and_its_child_twice() {
        let counts = session_counts(&[
            process(
                20,
                1,
                "node",
                &["node", "/opt/node_modules/@openai/codex/bin/codex.js"],
            ),
            process(21, 20, "codex", &["codex"]),
        ]);
        assert_eq!(counts.get(&AgentHarness::Codex), Some(&1));
    }

    #[test]
    fn separate_trees_count_separately() {
        let counts =
            session_counts(&[process(30, 1, "claude", &[]), process(31, 1, "claude", &[])]);
        assert_eq!(counts.get(&AgentHarness::ClaudeCode), Some(&2));
    }

    #[test]
    fn a_project_directory_named_like_a_harness_does_not_match() {
        let counts = session_counts(&[process(
            40,
            1,
            "node",
            &["node", "/home/k/projects/codex/server.js"],
        )]);
        assert!(counts.is_empty());
    }

    #[test]
    fn ignores_unrelated_processes() {
        let counts = session_counts(&[process(50, 1, "firefox", &["firefox"])]);
        assert!(counts.is_empty());
    }

    #[test]
    fn survives_a_parent_cycle() {
        // pid reuse can make a snapshot self-referential; this must terminate.
        let counts = session_counts(&[
            process(60, 61, "claude", &[]),
            process(61, 60, "claude", &[]),
        ]);
        assert!(counts.values().sum::<usize>() <= 2);
    }
}
