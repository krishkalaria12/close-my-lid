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
| `lid-gui` | `close-my-lid-gui` | Windows, Linux | desktop app on gpui-kit |

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
- **It builds with Command Line Tools alone**, with no UI toolkit between it
  and the system, so the app is one small executable.

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

## The Windows and Linux app

`lid-gui` is a desktop app rather than a tray panel. Stock GNOME ships no
system tray without the AppIndicator extension, Wayland does not let a client
anchor a window under a tray icon, and Windows 11 hides new tray icons in the
taskbar overflow — so a menu bar panel carried over would have been invisible
or misplaced on most of the machines it targets. The same features are laid
out as an ordinary window instead: a sidebar with Overview, Agents and
Settings.

It is built on [gpui-kit](https://gpui-kit.com/): Zed's GPUI (published as
`gpui-pre`) plus the gpui-component library. Buttons, cards, the segmented
duration picker and the palette are drawn from gpui primitives so they match
exactly in light and dark; the switch, title bar, icons and theme come from
gpui-component. The title bar draws the window controls itself, which on
Linux is also what gives the window decorations under GNOME's Wayland session.

| file | role |
|---|---|
| `shell.rs` | window, sidebar, page routing, keyboard shortcuts |
| `overview.rs`, `agent_list.rs`, `settings.rs` | the three pages |
| `widgets.rs`, `theme.rs` | shared pieces and the light/dark palette |
| `state.rs` | the hold, readouts, pending notifications — plain methods, no gpui |
| `tasks.rs` | supervision, the countdown clock, readouts, update checks, quit |
| `system.rs` | notifications, launch at login, power settings, single instance |

What runs when:

| loop | interval | does |
|---|---|---|
| clock | 1s, only while a hold runs | re-renders the countdown |
| supervision | 15s, or sooner when the hold ends or a notification is due | expiry, battery release, scheduled notifications |
| readouts | 5s focused, 30s in the background | battery and agent sessions, on the background executor |
| updates | 2s after launch, then 6h | reads the appcast, on the background executor |

Closing the window quits, and every way out releases the hold first. There is
deliberately no background mode: a hold with no window to show it would be a
hold nobody could see or stop.

### It builds on a Mac too

gpui-kit compiles GPUI's Metal shaders at runtime, so unlike the old
adabraka-gpui tray app, `lid-gui` builds on macOS with Command Line Tools
alone. That build exists only for working on the interface: it swaps in an
in-memory preview backend (`preview.rs`) and never touches `pmset`, the hold
lock, or the files the installed menu bar app reads.

```sh
cargo run -p lid-gui                                       # the app, on a Mac
CLOSE_MY_LID_PREVIEW=holding-1h cargo run -p lid-gui       # with a hold running
CLOSE_MY_LID_PREVIEW=agents cargo run -p lid-gui           # or: settings, holding
CLOSE_MY_LID_PREVIEW=holding-1h,ctrl cargo run -p lid-gui  # Ctrl+ shortcut labels, for screenshots
```

## Why `enable` blocks

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
| `lid-gui` desktop app | runs on macOS (preview backend); type-checks and lints for Windows and Linux, **needs hardware testing** |
| macOS packaging | done (`scripts/package-macos-app.sh`) |
| Windows and Linux packaging | release archives and install scripts; no signed installer yet |

## Known gaps

- **No Windows installer or code signing.** The zip and `install.ps1` work,
  but SmartScreen will warn on first launch until the binary is signed.
- **The Windows executable has no embedded icon.** The window and taskbar use
  the default until a `.ico` is compiled in as a resource.
- **Linux notifications need a notification daemon.** Every mainstream
  desktop runs one; a bare window manager may not, and then they are skipped.

## Dependency policy

`gpui-kit` is pinned with `=0.6.6` — it in turn pins its `gpui-pre` crates
exactly — and the toolchain with `rust-toolchain.toml`. gpui is pre-1.0 and breaks between minor versions, so
nothing there may float. The `objc2` crates the macOS app uses are stable and
are taken at a caret range like everything else.

## Development

```sh
cargo test  -p lidcore -p lid-macos -p lid-cli -p lid-gui   # all run on macOS
cargo run   -p lid-macos                         # the menu bar app, unbundled
cargo run   -p lid-gui                           # the Windows/Linux app, preview backend
cargo run   -p lid-cli -- agents
cargo run   -p lid-cli -- status
```

An unbundled macOS build has no bundle identifier, so notifications, Launch at
Login and the watchdog agent all stand down — everything else works. Run
`../../scripts/package-macos-app.sh` to exercise those.

### Checking the Windows and Linux builds from a Mac

A Mac build of `lid-gui` compiles none of the Windows or Linux code paths, so
type-check those targets directly. For Windows the blocker is the CRT and SDK
headers, which [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) fetches
and wires up:

```sh
brew install llvm                       # clang-cl, to compile the C in `ring`
cargo install cargo-xwin --locked
rustup target add x86_64-pc-windows-msvc

# Homebrew's llvm is keg-only, so its bin directory has to be asked for.
export PATH="$(brew --prefix llvm)/bin:$PATH"
cargo xwin clippy -p lid-gui --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

The first run also fetches the Windows CRT and SDK headers into
`~/Library/Caches/cargo-xwin` (`~/.cache/cargo-xwin` on Linux); later runs reuse them. `lld` is a separate formula these
days and is *not* needed here — clippy type-checks without linking.

For Linux, a few `-sys` crates compile C. `zig` stands in for a Linux C
compiler — through a small wrapper, because cc-rs passes a `--target` spelling
zig does not accept — and fontconfig is loaded at runtime so no Linux sysroot
is needed:

```sh
brew install zig
rustup target add x86_64-unknown-linux-gnu

cat > /tmp/zcc <<'SH'
#!/bin/sh
args=""
for a in "$@"; do case "$a" in --target=*) ;; *) args="$args '$(printf %s "$a" | sed "s/'/'\\\\''/g")'";; esac; done
eval exec zig cc -target x86_64-linux-gnu $args
SH
sed 's/zig cc/zig c++/' /tmp/zcc > /tmp/zcxx
printf '#!/bin/sh\nexec zig ar "$@"\n' > /tmp/zar
chmod +x /tmp/zcc /tmp/zcxx /tmp/zar

CC_x86_64_unknown_linux_gnu=/tmp/zcc CXX_x86_64_unknown_linux_gnu=/tmp/zcxx \
AR_x86_64_unknown_linux_gnu=/tmp/zar RUST_FONTCONFIG_DLOPEN=1 \
CARGO_TARGET_DIR=target/linux-check \
cargo clippy -p lid-gui --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
```

Worth doing before pushing a change to that crate: the `gui` jobs in
`ci-desktop.yml` are otherwise the only things that build it for its real
targets, and a round trip through CI to learn about a typo is a slow way to
find one.

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
