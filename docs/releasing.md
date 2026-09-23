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

The only signature that matters is the one on the archive the user downloads: Gatekeeper checks it when they open the app. A Developer ID Application identity, notarized by Apple, is what clears it silently — see [Signing the macOS bundle](#signing-the-macos-bundle) for what the automated build does instead, and how to substitute a notarized archive.

The feed carries no signature of its own for new items. Nothing verifies one, because nothing is fetched and executed from the feed — `scripts/validate-appcast.rb` explains the reasoning in full.

What does still matter is *where* an item points. The app refuses to open an enclosure URL that is not under `https://github.com/krishkalaria12/close-my-lid/releases/`, and `scripts/validate-appcast.rb` refuses to pass a feed containing one, so a tampered feed cannot send anyone somewhere else.

## Publish an update

The three artifacts are built by `.github/workflows/release.yml`, each on its
own operating system's runner. Nothing is cross-compiled: the Windows desktop
app links against the Windows SDK through gpui, the Linux desktop app links
against the host's glibc, fontconfig and Wayland/X11 libraries, and the macOS bundle needs `lipo`, `codesign` and the AppKit SDK.
A release cut from one developer machine could only ever ship one third of the
product.

1. Bump the version in one commit. `version` under `[workspace.package]` in
   `apps/desktop/Cargo.toml` is the source of truth — the packaging script, the
   binary's own `--version`, `CFBundleVersion` (`0.5.0` → `5000`) and the feed
   comparison all read it. Run `cargo update -w` from `apps/desktop` so
   `Cargo.lock` follows, and bump the copies in `package.json`,
   `apps/web/package.json`, `apps/web/src/data/site.ts`, the download links
   and release examples in `README.md`, and `version` (plus the tag in the formula's `url`) in
   `Formula/close-my-lid.rb` and `Casks/close-my-lid.rb`. The two checksums in
   those files cannot be known yet; they are step 5.

2. Tag the bump commit and push the tag. `release` refuses a tag that disagrees
   with the manifest:

```bash
git tag -a v<version> -m "Close My Lid v<version>"
git push origin main v<version>
```

3. The workflow creates a **draft** release and uploads three assets to it:

   | asset | built on | contents |
   |---|---|---|
   | `Close-My-Lid-v<version>-macOS.zip` | `macos-15` | universal `Close My Lid.app` |
   | `close-my-lid-v<version>-linux-x86_64.tar.gz` | `ubuntu-latest` | `close-my-lid`, `close-my-lid-gui`, desktop entry and icon |
   | `Close-My-Lid-v<version>-windows-x86_64.zip` | `windows-latest` | `close-my-lid.exe`, `close-my-lid-gui.exe` |

   The macOS name is fixed: `scripts/update-homebrew-tap.rb` refuses any other
   spelling, because the cask's URL is built from it.

   Draft, not published, and deliberately so — publishing is what fans out.
   `update-homebrew-tap.yml` opens and auto-merges the tap update, and
   `release-notes.yml` validates the body. Both need the assets to exist first.

   Re-run a failed platform against the same tag with
   `gh workflow run release.yml -f tag=v<version>`; uploads use `--clobber`.

4. Write the notes and publish. The draft starts from
   `docs/release-notes-template.md`, which already carries the blocks
   `scripts/validate-release-instructions.rb` requires — a "What changed"
   section goes above them:

```bash
gh release edit v<version> --notes-file <notes>
gh release view v<version> --json assets --jq '.assets[].name'
gh release edit v<version> --draft=false
```

   Keep upgrade instructions in their own "Existing installations" section so
   they cannot be mistaken for fresh-install instructions. The conventional
   fresh-install command belongs in its own code block:

```bash
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

   Keep this migration command in release notes through at least the first
   release after the dedicated tap launches:

```bash
brew untap --force krishkalaria12/close-my-lid
brew tap krishkalaria12/close-my-lid
```

5. Confirm the Homebrew update workflow opened and merged a pull request in
   `krishkalaria12/homebrew-close-my-lid`, then copy the two checksums it
   computed into this repository's migration copies of the formula and cask:

```bash
gh release download v<version> --pattern 'Close-My-Lid-*-macOS.zip' --dir /tmp
shasum -a 256 /tmp/Close-My-Lid-v<version>-macOS.zip
curl -sL https://github.com/krishkalaria12/close-my-lid/archive/refs/tags/v<version>.tar.gz | shasum -a 256
```

6. Add an item to the top of `appcast.xml` last, once the archive is downloadable.
   Copy the shape of the one below it; `length` is the archive's size in bytes:

```bash
stat -f%z /tmp/Close-My-Lid-v<version>-macOS.zip
```

   Items are newest-first. Drop `sparkle:hardwareRequirements` from new items:
   the bundle is universal, so it is no longer arm64-only. Run
   `ruby scripts/validate-appcast.rb` before committing the feed.

## Signing the macOS bundle

There is no Developer ID identity in CI, so the bundle the workflow archives is
ad-hoc signed, as every release through v0.4.4 was. Say so in the notes, and
keep the first-launch instruction with it: right-click the app and choose Open,
or clear the quarantine attribute.

To ship a notarized build instead, do steps 2 and 3 from a machine that holds
the identity and a stored notary profile, then upload the archive to the draft
release by hand:

```bash
CODE_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  UNIVERSAL=1 ./scripts/package-macos-app.sh
codesign --verify --deep --strict --verbose=2 "dist/macos/Close My Lid.app"
ditto -c -k --keepParent "dist/macos/Close My Lid.app" /tmp/Close-My-Lid-notarization.zip
xcrun notarytool submit /tmp/Close-My-Lid-notarization.zip \
  --keychain-profile close-my-lid-notary --wait
xcrun stapler staple "dist/macos/Close My Lid.app"
ditto -c -k --keepParent "dist/macos/Close My Lid.app" \
  "Close-My-Lid-v<version>-macOS.zip"
gh release upload v<version> "Close-My-Lid-v<version>-macOS.zip" --clobber
```

The universal build needs both Apple targets in the pinned toolchain:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Test the update notice from an older installed build: a build of the current
version never shows the update row, so it cannot exercise the notice at all.
