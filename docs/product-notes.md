# Product Notes

Close My Lid is aimed at developers who want coding agents, builds, downloads, or long-running tasks to keep working after a MacBook lid is closed.

## Initial Scope

- Native macOS menu bar app.
- One-click sleep hold sessions: 30 minutes, 1 hour, 4 hours, and indefinite.
- Automatic cleanup when a timed session expires or the app quits.
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
