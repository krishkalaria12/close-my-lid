#!/usr/bin/env bash
set -euo pipefail

# Builds `Close My Lid.app` from the Rust workspace in apps/desktop.
#
# The bundle is a single executable plus an icon: the agent marks are compiled
# into the binary, and update discovery is a plain HTTPS read of appcast.xml
# rather than an embedded framework.
#
# By default this produces a universal binary when both Apple targets are
# installed, and falls back to the host architecture alone when they are not —
# so a local build stays fast while a release build ships for both Macs.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE_DIR="$ROOT_DIR/apps/desktop"
PROFILE="${PROFILE:-release}"
VERSION="${VERSION:-0.4.4}"
BUILD_VERSION="${BUILD_VERSION:-8}"
APP_NAME="Close My Lid"
EXECUTABLE_NAME="CloseMyLid"
BUNDLE_ID="app.closemylid.CloseMyLid"
OUTPUT_DIR="$ROOT_DIR/dist/macos"
APP_DIR="$OUTPUT_DIR/$APP_NAME.app"
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"
UNIVERSAL="${UNIVERSAL:-auto}"

installed_targets="$(rustup target list --installed 2>/dev/null || true)"
have_target() { grep -qx "$1" <<<"$installed_targets"; }

if [[ "$UNIVERSAL" == "auto" ]]; then
  if have_target aarch64-apple-darwin && have_target x86_64-apple-darwin; then
    UNIVERSAL=1
  else
    UNIVERSAL=0
    echo "note: building for the host architecture only." >&2
    echo "      run 'rustup target add aarch64-apple-darwin x86_64-apple-darwin' for a universal build." >&2
  fi
fi

cd "$WORKSPACE_DIR"

if [[ "$UNIVERSAL" == "1" ]]; then
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --profile "$PROFILE" --package lid-macos --target "$target"
  done
else
  cargo build --profile "$PROFILE" --package lid-macos
fi

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

if [[ "$UNIVERSAL" == "1" ]]; then
  lipo -create -output "$APP_DIR/Contents/MacOS/$EXECUTABLE_NAME" \
    "target/aarch64-apple-darwin/$PROFILE/$EXECUTABLE_NAME" \
    "target/x86_64-apple-darwin/$PROFILE/$EXECUTABLE_NAME"
  chmod 755 "$APP_DIR/Contents/MacOS/$EXECUTABLE_NAME"
else
  install -m 755 "target/$PROFILE/$EXECUTABLE_NAME" "$APP_DIR/Contents/MacOS/$EXECUTABLE_NAME"
fi

install -m 644 "$WORKSPACE_DIR/assets/AppIcon.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"

cat > "$APP_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundleExecutable</key>
  <string>$EXECUTABLE_NAME</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$BUILD_VERSION</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © 2026 Krish Kalaria. All rights reserved.</string>
</dict>
</plist>
PLIST

if command -v codesign >/dev/null; then
  codesign_args=(--force --sign "$CODE_SIGN_IDENTITY")
  if [[ "$CODE_SIGN_IDENTITY" != "-" ]]; then
    codesign_args+=(--options runtime --timestamp)
  fi
  codesign "${codesign_args[@]}" "$APP_DIR" >/dev/null
fi

echo "Packaged $APP_DIR"
