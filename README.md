# Close My Lid

Close My Lid keeps a Mac awake while coding agents, builds, downloads, and other long-running work continue after the laptop lid is closed.

It ships as a native macOS menu bar app, a `close-my-lid` CLI for Linux and Windows, a Raycast extension, Homebrew formula/cask packages, and the marketing site.

The whole desktop side is one Rust workspace: a shared `lidcore` crate holds the session state machine, persistence, agent detection and the per-OS lid mechanism, and each platform's interface is a thin shell over it.

## Install

Most users only need the menu bar app:

```sh
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

Install the CLI only if you want terminal/script access:

```sh
brew install krishkalaria12/close-my-lid/close-my-lid
```

If you installed Close My Lid before the dedicated Homebrew tap existed, replace the old custom tap clone once. Installed packages are preserved:

```sh
brew untap --force krishkalaria12/close-my-lid
brew tap krishkalaria12/close-my-lid
```

The cask installs `Close My Lid.app` into `/Applications`. The formula installs the `close-my-lid` command-line tool.

The GitHub release also includes a zipped `.app` bundle:

- [Close My Lid v0.4.4](https://github.com/krishkalaria12/close-my-lid/releases/tag/v0.4.4)

## Features

- Menu bar controls for 30 minute, 1 hour, 4 hour, and Unlimited sessions
- Admin-approved closed-lid sleep hold using `pmset -a disablesleep`
- One-time passwordless grant: a single admin prompt installs a tightly scoped sudoers allowlist so holds start, end, and restore without ever asking for your password again
- The ON/OFF toggle remembers the last duration you picked instead of always starting Unlimited
- Automatic cleanup when a timed session expires or the app quits, with an exact end-of-session timer
- Dead-man watchdog LaunchAgent that restores normal sleep if the app crashes while a hold is active
- Low-battery safety release that restores normal sleep at 5% when unplugged
- Notifications when a hold starts, is about to end, and has ended
- Launch at Login toggle
- Update checks in the panel, with a link to the release
- Battery Settings shortcut
- Live session counts for Claude Code, OpenAI Codex, OpenCode, Antigravity, GitHub Copilot, Cursor, and Pi in the menu panel
- Local session persistence and live `pmset` reconciliation
- Raycast commands for enable, disable, and status
- CLI commands for scripts and package managers

## macOS App

Run the app from `/Applications` after installing the cask. The menu shows the current hold status, session presets, Launch at Login, Battery Settings, and Quit.

Closed-lid sleep prevention requires administrator approval. Close My Lid restores normal sleep behavior when a session stops, expires, or the app quits.

### Notifications

Close My Lid posts a notification when a hold starts, when a timed session has about 5 minutes left, and when it ends. macOS asks for notification permission the first time the app launches; you can change it later in System Settings › Notifications. Indefinite holds only post the start notification since they have no scheduled end.

## CLI

```sh
close-my-lid --help
close-my-lid status
close-my-lid enable
close-my-lid disable
```

Running `close-my-lid` with no arguments launches the menu bar app.

On Linux and Windows the CLI is a fuller tool — `close-my-lid enable --for 2h`,
`agents`, JSON output, and a `systemd` user unit. See
[`apps/desktop/README.md`](apps/desktop/README.md).

## Raycast

The Raycast extension lives in `packages/raycast` and exposes:

- Start Holding Lid
- Stop Holding Lid
- Check Lid Hold Status

It uses the same `pmset` behavior as the native app and is restricted to macOS in the manifest.

## Development

The project is organized as a small monorepo so the native app, Raycast extension, Homebrew packages, and future website can share one product direction.

```text
apps/desktop/      Rust workspace: shared core, macOS menu bar app, CLI, Windows tray app
apps/web/          Astro + Tailwind marketing site
packages/raycast/  Raycast extension
Formula/           Legacy migration copy of the Homebrew CLI formula
Casks/             Legacy migration copy of the Homebrew app cask
docs/              Product and implementation notes
scripts/           Release and packaging helpers
```

The website is a pnpm workspace (`pnpm-workspace.yaml`); install it from the
repository root. The Raycast extension is deliberately outside that workspace
and uses npm, because `ray lint` validates that the extension directory holds
its own `package-lock.json`:

```sh
pnpm install                        # the website
npm --prefix packages/raycast ci    # the Raycast extension
```

Root scripts proxy to the workspace packages:

```sh
pnpm dev             # run the website locally
pnpm build           # type-check and build the website
pnpm raycast:dev     # run the Raycast extension
pnpm raycast:lint    # lint and format-check the Raycast extension
pnpm macos:build     # build the menu bar app
pnpm macos:test      # run the shared-core and app tests
pnpm macos:package   # build the .app bundle into dist/macos
```

Build and test the Rust workspace:

```sh
cd apps/desktop
cargo test -p lidcore -p lid-macos -p lid-cli
cargo run -p lid-macos -- --help
cargo run -p lid-macos            # the menu bar app, unbundled
cargo run -p lid-cli -- agents
```

An unbundled build has no bundle identifier, so notifications, Launch at Login
and the watchdog agent all stand down — everything else works. Package the app
to exercise those.

Building needs only the Command Line Tools: the app talks to AppKit directly
through `objc2` and has no Swift or Metal in its toolchain.

Package the menu bar app:

```sh
./scripts/package-macos-app.sh
open "dist/macos/Close My Lid.app"
```

Run the Raycast extension:

```sh
pnpm --filter ./packages/raycast dev
```

## Website

`apps/web` is the [Astro](https://astro.build) site for closemylid.app, built
with Tailwind CSS v4 through `@tailwindcss/vite`. The visual design is ported
from [SunkenInTime/which-ai](https://github.com/SunkenInTime/which-ai),
`with-design-skill/fable-5.1` iteration 2.

```sh
pnpm --filter @close-my-lid/web dev
pnpm --filter @close-my-lid/web build
pnpm --filter @close-my-lid/web preview
```

Product copy, download links and the version shown on the page all come from
`apps/web/src/data/site.ts`, and the agent marks in `apps/web/public/agents/`
are the same SVGs the menu panel renders (from
`apps/desktop/crates/lid-macos/src/assets/`). Update both when cutting a
release.

The hero shot is `apps/web/src/assets/hero.png`, optimized at build time by
`astro:assets`. Replace that file to refresh the screenshot; the responsive
`webp` variants are regenerated automatically.

## Packaging

The canonical packages live in [`krishkalaria12/homebrew-close-my-lid`](https://github.com/krishkalaria12/homebrew-close-my-lid). The copies in this repository remain temporarily for users migrating from the old custom tap remote.

## Safety

Keeping a Mac awake in a bag can create heat and battery risk. Prefer timed sessions when possible so the machine returns to normal sleep behavior automatically. As a backstop, Close My Lid automatically releases the hold and restores normal sleep when the battery drops to 5% while unplugged. A charging Mac carries no battery risk, so holds are left alone while plugged in.
