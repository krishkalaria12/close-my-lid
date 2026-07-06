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

## Development

Build and test the Swift package:

```sh
cd apps/macos
swift run CloseMyLidCoreTests
swift build
swift run CloseMyLid
```

Run the Raycast extension:

```sh
cd packages/raycast
npm install
npm run dev
```

## Packaging

The Homebrew formula in `Formula/close-my-lid.rb` is ready to wire up after the first tagged release. Replace the placeholder SHA256 after publishing `v0.1.0`.

## Safety

Keeping a Mac awake in a bag can create heat and battery risk. The default app surface favors timed sessions so the machine returns to normal sleep behavior automatically.
