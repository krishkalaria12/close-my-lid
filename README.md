# Close My Lid

Close My Lid keeps a Mac awake while coding agents, builds, downloads, and other long-running work continue after the laptop lid is closed.

It ships as a native macOS menu bar app, a `close-my-lid` CLI, a Raycast extension, and Homebrew formula/cask packages.

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

- [Close My Lid v0.4.1](https://github.com/krishkalaria12/close-my-lid/releases/tag/v0.4.1)

## Features

- Menu bar controls for 30 minute, 1 hour, 4 hour, and Unlimited sessions
- Admin-approved closed-lid sleep hold using `pmset -a disablesleep`
- Automatic cleanup when a timed session expires or the app quits
- Low-battery safety release that restores normal sleep at 5% when unplugged
- Notifications when a hold starts, is about to end, and has ended
- Launch at Login toggle
- In-app update checks, installation, and restart
- Battery Settings shortcut
- Live session counts for Claude Code, OpenAI Codex CLI, OpenCode, Gemini CLI, GitHub Copilot CLI, Cursor CLI, and Pi in the menu panel
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

## Raycast

The Raycast extension lives in `packages/raycast` and exposes:

- Start Holding Lid
- Stop Holding Lid
- Check Lid Hold Status

It uses the same `pmset` behavior as the native app and is restricted to macOS in the manifest.

## Development

The project is organized as a small monorepo so the native app, Raycast extension, Homebrew packages, and future website can share one product direction.

```text
apps/macos/        Native macOS menu bar app, CLI, and Swift tests
packages/raycast/  Raycast extension
Formula/           Legacy migration copy of the Homebrew CLI formula
Casks/             Legacy migration copy of the Homebrew app cask
docs/              Product and implementation notes
scripts/           Release and packaging helpers
```

Build and test the Swift package:

```sh
cd apps/macos
swift run CloseMyLidCoreTests
swift build
swift run CloseMyLid --help
swift run CloseMyLid
```

Package the menu bar app:

```sh
./scripts/package-macos-app.sh
open "dist/macos/Close My Lid.app"
```

Run the Raycast extension:

```sh
cd packages/raycast
npm install
npm run dev
```

## Packaging

The canonical packages live in [`krishkalaria12/homebrew-close-my-lid`](https://github.com/krishkalaria12/homebrew-close-my-lid). The copies in this repository remain temporarily for users migrating from the old custom tap remote.

## Safety

Keeping a Mac awake in a bag can create heat and battery risk. Prefer timed sessions when possible so the machine returns to normal sleep behavior automatically. As a backstop, Close My Lid automatically releases the hold and restores normal sleep when the battery drops to 5% while unplugged. A charging Mac carries no battery risk, so holds are left alone while plugged in.
