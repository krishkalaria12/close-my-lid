#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_DIR="$ROOT_DIR/apps/macos"
CONFIGURATION="${CONFIGURATION:-release}"
VERSION="${VERSION:-0.3.0}"
BUILD_VERSION="${BUILD_VERSION:-3}"
APP_NAME="Close My Lid"
EXECUTABLE_NAME="CloseMyLid"
BUNDLE_ID="app.closemylid.CloseMyLid"
OUTPUT_DIR="$ROOT_DIR/dist/macos"
APP_DIR="$OUTPUT_DIR/$APP_NAME.app"
SPARKLE_FRAMEWORK="$PACKAGE_DIR/.build/$CONFIGURATION/Sparkle.framework"
SPARKLE_FEED_URL="${SPARKLE_FEED_URL:-https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/appcast.xml}"
SPARKLE_PUBLIC_KEY="${SPARKLE_PUBLIC_KEY:-duVfksjvX0IE4SojdbaEBhF36yeNDTJZITYe0QCpFaY=}"
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"

swift build \
  --package-path "$PACKAGE_DIR" \
  --configuration "$CONFIGURATION" \
  --product "$EXECUTABLE_NAME" \
  --disable-sandbox

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources" "$APP_DIR/Contents/Frameworks"

install -m 755 "$PACKAGE_DIR/.build/$CONFIGURATION/$EXECUTABLE_NAME" "$APP_DIR/Contents/MacOS/$EXECUTABLE_NAME"
install -m 644 "$PACKAGE_DIR/Resources/AppIcon.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
cp -R "$SPARKLE_FRAMEWORK" "$APP_DIR/Contents/Frameworks/Sparkle.framework"

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
  <key>SUFeedURL</key>
  <string>$SPARKLE_FEED_URL</string>
  <key>SUPublicEDKey</key>
  <string>$SPARKLE_PUBLIC_KEY</string>
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
