# Product Notes

Close My Lid is aimed at developers who want coding agents, builds, downloads, or long-running tasks to keep working after a MacBook lid is closed.

## Initial Scope

- Native macOS menu bar app.
- One-click sleep hold sessions: 30 minutes, 1 hour, 4 hours, and indefinite.
- Automatic cleanup when a timed session expires or the app quits.
- Session notifications: an immediate "started" message plus, for timed sessions, an "ending soon" warning 5 minutes out (`SessionNotificationPlanner.endingSoonLeadTime`) and an "ended" message. The pure `SessionNotificationPlanner` decides timing/copy; `SessionNotificationScheduler` delivers them through `UNUserNotificationCenter`. Scheduled messages are cancelled when a hold is stopped early. The unbundled `swift run` build skips notifications since `UNUserNotificationCenter.current()` requires a bundle identifier.
- Raycast commands for starting and stopping the same power behavior.
- Homebrew formula scaffolding for the first tagged release.
- `v0.1.0` source tag and Homebrew SHA256 are wired into `Formula/close-my-lid.rb`.
- Homebrew cask installs the released `.app` artifact from GitHub Releases.
- CLI commands for scripted package usage: `enable`, `disable`, `status`, `--help`, and `--version`.
- `.app` bundle packaging with `LSUIElement` so the app presents as a menu bar utility instead of a Dock app.
- Local session persistence and `pmset` reconciliation so app, CLI, and Raycast changes do not drift silently.
- Launch at Login toggle via `SMAppService`.

## Implementation Notes

macOS idle sleep assertions are not enough for the closed-lid use case. The app currently uses:

```sh
pmset -a disablesleep 1
```

and restores the setting with:

```sh
pmset -a disablesleep 0
```

Those commands require administrator approval. The first implementation runs them through `osascript` so the user gets the normal macOS admin prompt. A future production build should replace this with a privileged helper installed through SMAppService.

## Agent Session Detection

The menu panel shows how many sessions of each supported agent harness (Claude Code, OpenAI Codex CLI, OpenCode) are running. Detection snapshots the current user's processes with a `sysctl(KERN_PROC_UID)` call instead of spawning `ps` or `pgrep`. The scan runs on a background utility-priority task while the panel is open; only the resulting counts hop back to the main actor. It then matches:

- Native binaries by executable name: `claude`, `codex`, `opencode`. This covers the native installers, Homebrew, and current npm packages, which all link a platform binary.
- Legacy npm installs that run under `node`/`bun` by inspecting the process arguments (via `KERN_PROCARGS2`, fetched only for JavaScript runtime processes) for the package paths, e.g. `@anthropic-ai/claude-code/cli.js`.

A matched process only counts as a session when no ancestor process matches the same harness. That keeps helper children from inflating counts: Codex's npm wrapper spawning the native binary, or OpenCode's launcher spawning its server/TUI, still count as one session each.
