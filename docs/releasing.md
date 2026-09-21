# Releasing Close My Lid

The committed `appcast.xml` is the stable update feed. The app reads the newest item's `sparkle:shortVersionString`, compares it against its own version, and offers to open the release page — it never downloads or installs anything itself. Never publish an update item before the matching GitHub Release archive is available, or the app will send people to a page that has nothing on it.

The feed keeps Sparkle's element names because the site and older installs already read them; the framework itself is gone.

## Homebrew updater setup

The `update Homebrew tap` workflow uses the `TAP_REPO_TOKEN` Actions secret. Create a fine-grained personal access token restricted to `krishkalaria12/homebrew-close-my-lid` with:

- Contents: Read and write
- Pull requests: Read and write
- Administration: Read-only

The Administration permission lets the updater verify that `main` still enforces the exact GitHub Actions `validate` check before enabling auto-merge. Do not use a broad account token for this secret.

## Signing setup

Releases are signed with a Developer ID Application identity and notarized by Apple. That is the only signature that matters now: Gatekeeper checks it when the user opens the downloaded app.

The feed carries no signature of its own for new items. Nothing verifies one, because nothing is fetched and executed from the feed — `scripts/validate-appcast.rb` explains the reasoning in full.

## Publish an update

1. Increase `VERSION` and the monotonically increasing integer `BUILD_VERSION`.
2. Build with a Developer ID identity. Ad-hoc signing is only for local validation. The packaging script produces a universal binary when both Apple targets are installed:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

```bash
VERSION=<version> BUILD_VERSION=<integer> \
  CODE_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  ./scripts/package-macos-app.sh
codesign --verify --deep --strict --verbose=2 "dist/macos/Close My Lid.app"
```

3. Submit a temporary ZIP for notarization, staple the accepted ticket to the app, then create the release ZIP from the stapled app:

```bash
ditto -c -k --keepParent "dist/macos/Close My Lid.app" /tmp/Close-My-Lid-notarization.zip
xcrun notarytool submit /tmp/Close-My-Lid-notarization.zip \
  --keychain-profile close-my-lid-notary --wait
xcrun stapler staple "dist/macos/Close My Lid.app"
ditto -c -k --keepParent "dist/macos/Close My Lid.app" \
  "Close-My-Lid-v<version>-macOS.zip"
```

4. Upload the immutable `Close-My-Lid-v<version>-macOS.zip` archive to its GitHub Release.
5. Add an item to the top of `appcast.xml`, copying the shape of the one below it. `length` is the archive's size in bytes:

```bash
stat -f%z "Close-My-Lid-v<version>-macOS.zip"
```

   Items are newest-first — the app reads the first one and stops. Drop `sparkle:hardwareRequirements` from new items: the bundle is universal, so it is no longer arm64-only.

6. Run `ruby scripts/validate-appcast.rb` before committing the feed.
7. Confirm the Homebrew update workflow opens or updates a pull request in `krishkalaria12/homebrew-close-my-lid` with the matching formula and cask checksums.
8. Include the conventional fresh-install command in its own release-note code block:

```bash
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

Keep this migration command in release notes through at least the first release after the dedicated tap launches:

```bash
brew untap --force krishkalaria12/close-my-lid
brew tap krishkalaria12/close-my-lid
```

Put upgrade instructions in a separate "Existing installations" section so they cannot be mistaken for fresh-install instructions.

Test the full path from an older installed, Developer ID-signed build: a build of the current version never shows the update row, so it cannot exercise the notice at all.
