# Close My Lid

Close My Lid is a macOS menu bar app for keeping a Mac awake while coding agents, builds, downloads, and other long-running work continue after the lid is closed.

The project is organized as a small monorepo so the native app, Raycast extension, Homebrew formula, and future website can share the same product direction.

## Workspace

```text
apps/macos/        Native macOS menu bar app and Swift tests
packages/raycast/  Raycast extension scaffold
Formula/           Homebrew formula scaffold
docs/              Product and implementation notes
```

## macOS App

The app lives in the menu bar and offers these session presets:

- 30 minutes
- 1 hour
- 4 hours
- Indefinitely

Closed-lid sleep prevention uses `pmset -a disablesleep`, which requires administrator approval. The app restores normal sleep behavior when a session stops, expires, or the app quits.

The menu also includes:

- current hold status
- Launch at Login toggle
- Battery Settings shortcut

Session state is persisted locally so the menu can reconcile itself with the current `pmset` state after relaunches or Raycast-triggered changes.

## Development

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

The Raycast package exposes commands to enable, disable, and check the closed-lid hold. It uses the same `pmset` behavior as the native app and is restricted to macOS in the manifest.

## Packaging

Install from Homebrew:

```sh
brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid
brew install krishkalaria12/close-my-lid/close-my-lid
```

The installed `close-my-lid` binary can launch the menu bar app or run package-friendly commands:

```sh
close-my-lid --help
close-my-lid status
close-my-lid enable
close-my-lid disable
```

The formula builds from the `v0.1.0` source tag.

## Safety

Keeping a Mac awake in a bag can create heat and battery risk. The default app surface favors timed sessions so the machine returns to normal sleep behavior automatically.
