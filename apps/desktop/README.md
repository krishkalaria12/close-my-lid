# Close My Lid — the desktop workspace

One Rust workspace for all three platforms. `lidcore` holds everything that is
not a user interface; each platform's interface is a thin shell over it.

## Why one core and three backends

The state machine, presets, persistence, battery policy and agent detection are
ordinary logic and are genuinely shared. The hard part is the lid mechanism,
and that part shares **no code at all** between the three systems:

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

### The macOS subtlety worth knowing

`disablesleep` is a global, persistent setting: it survives the process, and a
crash leaves a Mac that can never sleep. The dead-man LaunchAgent in
[`lidcore::launchd`](crates/lidcore/src/launchd.rs) runs the app's own binary
with `--watchdog` every minute and releases a hold whose heartbeat has gone
stale. That pass never prompts — it uses the passwordless-only executor, so a
missing grant leaves the heartbeat for the next tick instead of raising an
administrator dialog out of a background agent.

## Crates

| crate | binary | platforms | role |
|---|---|---|---|
| `lidcore` | — | all | state machine, persistence, agent detection, per-OS backends |
| `lid-macos` | `CloseMyLid` | macOS | the menu bar app, and the product on macOS |
| `lid-cli` | `close-my-lid` | all | the primary surface on Linux |
| `lid-gui` | `close-my-lid-gui` | Windows | tray app on adabraka-gpui |

`lidcore` must never depend on a UI framework. That boundary is what keeps the
UI choice reversible, and it is why three very different front ends share
everything that matters.

## The macOS app

Built directly on AppKit through [`objc2`](https://docs.rs/objc2), not on a
cross-platform UI framework. Two reasons:

- **It is a menu bar app.** `NSStatusItem`, a non-activating panel anchored
  under it, `NSVisualEffectView`, template images that follow the menu bar's
  theme, `SMAppService`, `UNUserNotificationCenter` — almost all of it is
  AppKit surface a portable toolkit would have to re-expose anyway.
- **It builds with Command Line Tools alone.** GPUI, the toolkit `lid-gui`
  uses on Windows, needs the Metal shader compiler from *full* Xcode; without
  it the build fails with `xcrun: error: unable to find utility "metal"`. An
  app nobody can build from a stock developer machine is an app nobody fixes.

The panel is laid out with explicit frames rather than Auto Layout: it is a
fixed width with a computed height, so there is nothing to solve, and the whole
layout is one pass of arithmetic in
[`panel/layout.rs`](crates/lid-macos/src/panel/layout.rs).

Three custom `NSView` subclasses cover all the drawing — a hover-highlighting
clickable surface, the battery capsule, and an agent badge — which is what
keeps the rest of the panel code a layout description rather than a pile of
Objective-C.

### What runs when

| pass | interval | does |
|---|---|---|
| reconciliation | 30s | reads `pmset` on a worker thread, syncs the session, refreshes the heartbeat |
| expiry | once, at the session's end | releases a timed hold on the second |
| panel readouts | 5s, **only while the panel is open** | battery and agent session counts |
| update check | 2s after launch, then 6h | reads the committed appcast |
| watchdog | 60s, from launchd | releases a stranded hold after a crash |

Nothing scans the process table while the panel is closed, and every timer
carries a tolerance so the scheduler can coalesce it instead of waking the CPU
on its own.

### Agent detection

`lidcore::agents` shares the classification; the snapshot is per-platform.
macOS uses `proc_listpids(PROC_UID_ONLY)`, which filters by uid in the kernel,
and reads `KERN_PROCARGS2` arguments only for JavaScript runtimes — that call
copies up to `KERN_ARGMAX` bytes per process and is the expensive part of a
scan. Linux and Windows go through `sysinfo`, which has no equivalent of the
uid filter and fetches arguments for everything.

## Errors and configuration

Every crate follows the same two-file convention:

- **`error.rs`** holds every error that crate can produce. No `String` errors
  and no ad-hoc `anyhow` at call sites. Each variant records the *action* that
  was attempted alongside the platform's own wording, and carries an optional
  hint with the actionable next step. Backends supply their own hints, so OS
  knowledge stays with the OS code.
- **`config.rs`** holds every tunable value and well-known path. Shared values
  live in `lidcore::config` and are re-exported, so the surfaces cannot drift
  apart on things like the battery threshold.

The CLI turns this into distinct `sysexits.h` exit codes so scripts can branch
without parsing text — `64` bad usage, `77` refused, `72` bad state file — and
prints the cause chain and hint to stderr. The menu bar app puts the same two
parts on the two lines of an `NSAlert`.

## Why the CLI leads on Linux

Stock GNOME ships no system tray without the AppIndicator extension, and
Wayland does not let a client anchor a window under a tray icon — the
StatusNotifierItem protocol does not even expose the icon's geometry. Rather
than ship something that breaks on the most common desktop, Linux treats the
CLI as the product and the tray as a bonus.

`close-my-lid enable` blocks while holding on Linux and Windows, like
`systemd-inhibit` and macOS `caffeinate`. On Linux it has to: the inhibitor
lives exactly as long as the descriptor. Run `close-my-lid systemd` for a user
unit that backgrounds it.

macOS is the exception — the hold is a persistent setting rather than a
descriptor, so `enable` applies it and exits, and the menu bar app or the
watchdog supervises it from there.

## Status

| area | state |
|---|---|
| `lidcore` state machine, store, duration, battery policy | done, tested |
| agent detection | done, verified against a live process table on macOS |
| macOS `pmset` backend, sudoers grant, watchdog | done, in use |
| `lid-macos` menu bar app | done |
| Linux logind backend | type-checks for `x86_64-unknown-linux-gnu`, **needs hardware testing** |
| Windows power-scheme backend | type-checks for `x86_64-pc-windows-msvc`, **needs hardware testing** |
| `lid-cli` | done |
| `lid-gui` panel and tray | scaffolded, **never compiled — see below** |
| macOS packaging | done (`scripts/package-macos-app.sh`) |
| Windows and Linux packaging | not started |

## Known gaps

- **`tray_icon_bounds()` is macOS-only** in adabraka-gpui 0.5.1; Windows and
  Linux get the `None` default. `anchor.rs` falls back to the bottom-right of
  the work area. A proper Windows implementation is tractable via
  `Shell_NotifyIconGetRect` and would be worth upstreaming.
- **Windows tray icon assets are not shipped yet.** Windows needs separate
  16x16 light and dark `.ico` files; unlike macOS template images they do not
  auto-invert.
- **Windows 11 hides new tray icons** in the taskbar overflow by default, so
  the app is effectively invisible on first run. Needs an onboarding pass.
- **`lid-gui` has never been compiled**, for the Metal reason above. The first
  real build will be on Windows via `ci-desktop.yml`. Expect API fixes on that
  first build: the code is written against verified 0.5.1 signatures, but
  nothing has type-checked it.

## Dependency policy

`adabraka-gpui` is pinned with `=0.5.1` and the toolchain with
`rust-toolchain.toml`. gpui is pre-1.0 and breaks between minor versions, so
nothing there may float. The `objc2` crates the macOS app uses are stable and
are taken at a caret range like everything else.

## Development

```sh
cargo test  -p lidcore -p lid-macos   # both run on macOS
cargo run   -p lid-macos              # the menu bar app, unbundled
cargo run   -p lid-cli -- agents
cargo run   -p lid-cli -- status
```

An unbundled macOS build has no bundle identifier, so notifications, Launch at
Login and the watchdog agent all stand down — everything else works. Run
`../../scripts/package-macos-app.sh` to exercise those.

## Assets

`assets/AppIcon.icns` is what the packaging script installs into the bundle;
`assets/AppIcon.iconset/` is the source it is built from:

```sh
iconutil -c icns assets/AppIcon.iconset -o assets/AppIcon.icns
```

The agent marks live in `crates/lid-macos/src/assets/` and are compiled into
the binary. They are the same SVGs the website serves from
`apps/web/public/agents/` — update both together.

The menu bar icon itself is the `laptopcomputer` SF Symbol, rendered as a
template image so macOS tints it for the light or dark menu bar.
