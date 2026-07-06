# Close My Lid

Close My Lid keeps a Mac awake while coding agents, builds, downloads, and other long-running work continue after the laptop lid is closed.

It ships as a native macOS menu bar app, a `close-my-lid` CLI, a Raycast extension, and Homebrew formula/cask packages.

## Install

First, add the Homebrew tap:

```sh
brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid
```

Most users only need the menu bar app:

```sh
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

Install the CLI only if you want terminal/script access:

```sh
brew install krishkalaria12/close-my-lid/close-my-lid
```

The cask installs `Close My Lid.app` into `/Applications`. The formula installs the `close-my-lid` command-line tool.

The GitHub release also includes a zipped `.app` bundle:

- [Close My Lid v0.1.0](https://github.com/krishkalaria12/close-my-lid/releases/tag/v0.1.0)

## Features

- Menu bar controls for 30 minute, 1 hour, 4 hour, and indefinite sessions
- Admin-approved closed-lid sleep hold using `pmset -a disablesleep`
- Automatic cleanup when a timed session expires or the app quits
- Launch at Login toggle
- Battery Settings shortcut
- Local session persistence and live `pmset` reconciliation
- Raycast commands for enable, disable, and status
- CLI commands for scripts and package managers

## macOS App

Run the app from `/Applications` after installing the cask. The menu shows the current hold status, session presets, Launch at Login, Battery Settings, and Quit.

Closed-lid sleep prevention requires administrator approval. Close My Lid restores normal sleep behavior when a session stops, expires, or the app quits.

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
Formula/           Homebrew formula for the CLI
Casks/             Homebrew cask for the app bundle
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

The formula builds from the `v0.1.0` source tag.
The cask installs the released `Close-My-Lid-v0.1.0-macOS.zip` app artifact.

## Safety

Keeping a Mac awake in a bag can create heat and battery risk. Prefer timed sessions when possible so the machine returns to normal sleep behavior automatically.
