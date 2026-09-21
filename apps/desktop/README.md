# Close My Lid — Windows and Linux

Rust workspace for the non-Apple platforms. macOS keeps its own native Swift
app in [`apps/macos`](../macos) and is deliberately not reimplemented here.

## Why this is a separate implementation

The macOS core turned out to be almost platform-agnostic already — the state
machine, presets, persistence and battery policy are ordinary logic. The hard
part is the lid mechanism, and that part shares **no code at all** between the
three systems:

| | mechanism | needs root | persistent | crash-safe |
|---|---|---|---|---|
| macOS | `pmset -a disablesleep` via a scoped sudoers grant | yes | yes | needs a watchdog |
| Linux | `logind` `handle-lid-switch` inhibitor (held fd) | no | no | yes, automatically |
| Windows | power scheme `LIDACTION` + `SetThreadExecutionState` | usually no | yes | needs save/restore |

So `lidcore` shares the cheap part and each backend owns the expensive part.

### The Linux subtlety worth knowing

logind defaults to `LidSwitchIgnoreInhibited=yes`, which makes it ignore
`sleep` and `idle` inhibitors when the lid closes. A dedicated
`handle-lid-switch` inhibitor is always honoured — that is the whole reason
the type exists. Taking it means working on stock configurations without
touching `logind.conf`, which is what most guides wrongly recommend.

Because the inhibitor is a file descriptor, the kernel releases it when the
process dies, including on `SIGKILL`. Linux therefore needs no equivalent of
the macOS watchdog LaunchAgent.

## Crates

| crate | binary | platforms | role |
|---|---|---|---|
| `lidcore` | — | all | state machine, persistence, agent detection, per-OS backends |
| `lid-cli` | `close-my-lid` | Linux, Windows | the primary surface on Linux |
| `lid-gui` | `close-my-lid-gui` | Windows | tray app on adabraka-gpui |

`lidcore` must never depend on a UI framework. That boundary is what keeps the
UI choice reversible, and it is why the Linux CLI and the Windows GUI can
share everything that matters.

## Why the CLI leads on Linux

Stock GNOME ships no system tray without the AppIndicator extension, and
Wayland does not let a client anchor a window under a tray icon — the
StatusNotifierItem protocol does not even expose the icon's geometry. Rather
than ship something that breaks on the most common desktop, Linux treats the
CLI as the product and the tray as a bonus.

`close-my-lid enable` blocks while holding, like `systemd-inhibit` and macOS
`caffeinate`. It has to: the inhibitor lives exactly as long as the descriptor.
Run `close-my-lid systemd` for a user unit that backgrounds it.

## Status

| area | state |
|---|---|
| `lidcore` state machine, store, duration, battery policy | done, 25 tests passing |
| agent detection via `sysinfo` | done, verified against a live process table |
| Linux logind backend | type-checks for `x86_64-unknown-linux-gnu`, **needs hardware testing** |
| Windows power-scheme backend | type-checks for `x86_64-pc-windows-msvc`, **needs hardware testing** |
| `lid-cli` | done |
| `lid-gui` panel and tray | scaffolded, **never compiled — see below** |
| packaging and signing | not started |

## Known gaps

- **`tray_icon_bounds()` is macOS-only** in adabraka-gpui 0.5.1; Windows and
  Linux get the `None` default. `anchor.rs` falls back to the bottom-right of
  the work area. A proper Windows implementation is tractable via
  `Shell_NotifyIconGetRect` and would be worth upstreaming.
- **Tray icon assets are not shipped yet.** Windows needs separate 16x16 light
  and dark `.ico` files; unlike macOS template images they do not auto-invert.
- **Windows 11 hides new tray icons** in the taskbar overflow by default, so
  the app is effectively invisible on first run. Needs an onboarding pass.
- **Agent counting is not filtered by user**, unlike the macOS version which
  filters by uid. Equivalent on a single-user laptop.
- **`lid-gui` has never been compiled.** Building gpui on macOS needs the
  Metal shader compiler from *full* Xcode; Command Line Tools alone fails with
  `xcrun: error: unable to find utility "metal"`. So the panel cannot be
  iterated on from a Mac without a ~10 GB Xcode install, and the first real
  build will be on Windows via `ci-desktop.yml`. Expect API fixes on that
  first build: the code is written against verified 0.5.1 signatures, but
  nothing has type-checked it.

## Dependency policy

`adabraka-gpui` is pinned with `=0.5.1` and the toolchain with
`rust-toolchain.toml`. gpui is pre-1.0 and breaks between minor versions, so
nothing here may float. The fork is Apache-2.0, so it can be vendored if
upstream goes quiet.

## Development

```sh
cargo test  -p lidcore        # runs anywhere, including macOS
cargo run   -p lid-cli -- agents
cargo run   -p lid-cli -- status
```

`lidcore` builds on macOS via an `UnsupportedBackend` that errors clearly, so
the workspace stays testable from a Mac.
