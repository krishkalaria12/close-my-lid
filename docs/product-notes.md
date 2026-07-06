# Product Notes

Close My Lid is aimed at developers who want coding agents, builds, downloads, or long-running tasks to keep working after a MacBook lid is closed.

## Initial Scope

- Native macOS menu bar app.
- One-click sleep hold sessions: 30 minutes, 1 hour, 4 hours, and indefinite.
- Automatic cleanup when a timed session expires or the app quits.
- Raycast commands for starting and stopping the same power behavior.
- Homebrew formula scaffolding for the first tagged release.

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
