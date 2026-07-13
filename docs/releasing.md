# Releasing Close My Lid

The macOS app uses Sparkle 2 to discover, download, install, and relaunch updates. The committed `appcast.xml` is the stable update feed. Never publish its update item before the matching GitHub Release archive is available.

## Signing setup

Sparkle's Ed25519 private key is stored in the developer's login Keychain under the `close-my-lid` account. The corresponding public key is embedded by `scripts/package-macos-app.sh`.

On a new release machine, securely transfer an exported private key and import it without committing the key file:

```bash
apps/macos/.build/artifacts/sparkle/Sparkle/bin/generate_keys \
  --account close-my-lid \
  -f /secure/path/close-my-lid-sparkle-private-key
```

## Publish an update

1. Increase `VERSION` and the monotonically increasing integer `BUILD_VERSION`.
2. Build with a Developer ID identity. Ad-hoc signing is only for local validation:

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
5. Put the archive in a directory containing all update archives that should remain in the feed, then generate the appcast:

```bash
apps/macos/.build/artifacts/sparkle/Sparkle/bin/generate_appcast \
  --account close-my-lid \
  --download-url-prefix "https://github.com/krishkalaria12/close-my-lid/releases/download/v<version>/" \
  /path/to/update-archives
```

6. Review the generated feed, replace `appcast.xml`, and run `ruby scripts/validate-appcast.rb` before committing it.
7. Keep the Homebrew cask version and checksum aligned with the same GitHub Release.
8. Include complete fresh-install instructions in the release notes. This project is a custom tap hosted in the application repository, so the tap command must include the repository URL and must appear in the same code block as the install command:

```bash
brew tap krishkalaria12/close-my-lid https://github.com/krishkalaria12/close-my-lid
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

Do not publish the `brew install` command by itself: without the explicit tap command, Homebrew looks for the nonexistent conventional repository `krishkalaria12/homebrew-close-my-lid`. Put upgrade instructions in a separate "Existing installations" section so they cannot be mistaken for fresh-install instructions.

Test the full path from an older installed, Developer ID-signed build. A build of the current version cannot exercise replacement and relaunch.
