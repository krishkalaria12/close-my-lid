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

# Read from the workspace rather than repeated here. The binary takes its own
# version from CARGO_PKG_VERSION and compares it against the release feed, so a
# hardcoded copy that missed a bump would ship a bundle whose Info.plist,
# `--version` and update notice all disagreed.
workspace_version() {
  awk '/^\[workspace\.package\]/ { in_section = 1; next }
       /^\[/                       { in_section = 0 }
       in_section && /^version[[:space:]]*=/ {
         gsub(/[^0-9A-Za-z.+-]/, "", $3); print $3; exit
       }' "$WORKSPACE_DIR/Cargo.toml"
}

VERSION="${VERSION:-$(workspace_version)}"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read the version from $WORKSPACE_DIR/Cargo.toml" >&2
  exit 1
fi

# CFBundleVersion has to increase with every release, and this was pinned at a
# literal 8 — so 0.4.4, 0.4.5 and everything after all claimed the same build.
# LaunchServices compares this field when it finds two copies of a bundle and
# decides which one to keep, so a frozen number lets a stale copy win.
#
# Derived from the version instead: `0.4.4` becomes `4004`, which rises
# monotonically for any component under a thousand and needs no second thing to
# remember to bump. An explicit BUILD_VERSION still overrides it.
build_version_from() {
  IFS=. read -r major minor patch <<<"$1"
  printf '%d\n' "$(( ${major:-0} * 1000000 + ${minor:-0} * 1000 + ${patch:-0} ))"
}

BUILD_VERSION="${BUILD_VERSION:-$(build_version_from "${VERSION%%-*}")}"
APP_NAME="Close My Lid"
EXECUTABLE_NAME="CloseMyLid"
BUNDLE_ID="app.closemylid.CloseMyLid"
OUTPUT_DIR="$ROOT_DIR/dist/macos"
APP_DIR="$OUTPUT_DIR/$APP_NAME.app"
CODE_SIGN_IDENTITY="${CODE_SIGN_IDENTITY:--}"
UNIVERSAL="${UNIVERSAL:-auto}"

# The workspace pins its toolchain in `apps/desktop/rust-toolchain.toml`, so
# every rustup and cargo question has to be asked from inside it. Asked from
# the repository root they answer for the *default* toolchain instead, which is
# how `rustup target add x86_64-apple-darwin` can succeed and the build still
# fail with "can't find crate for `std`": the target was added to one toolchain
# and the build ran under another.
cd "$WORKSPACE_DIR"

# Whether a target can actually be built for, rather than whether rustup lists
# it. Asking the compiler where the target's libraries live, and looking, is
# the check that matches what the build is about to attempt — and, run from
# here, it asks about the pinned toolchain that will do the attempting.
have_target() {
  local libdir
  libdir="$(rustc --print target-libdir --target "$1" 2>/dev/null)" || return 1
  [[ -n "$libdir" && -d "$libdir" ]] || return 1
  compgen -G "$libdir/libstd-*.rlib" >/dev/null
}

if [[ "$UNIVERSAL" == "auto" ]]; then
  if have_target aarch64-apple-darwin && have_target x86_64-apple-darwin; then
    UNIVERSAL=1
  else
    UNIVERSAL=0
    echo "note: building for the host architecture only." >&2
    echo "      run 'rustup target add aarch64-apple-darwin x86_64-apple-darwin'" >&2
    echo "      from $WORKSPACE_DIR for a universal build." >&2
  fi
elif [[ "$UNIVERSAL" == "1" ]]; then
  # Asked for explicitly, as a release build does: say which target is missing
  # rather than letting the build fail on it a minute later.
  for target in aarch64-apple-darwin x86_64-apple-darwin; do
    if ! have_target "$target"; then
      echo "error: UNIVERSAL=1 needs the $target standard library, which the" >&2
      echo "       toolchain pinned in $WORKSPACE_DIR does not have." >&2
      echo "       run 'rustup target add $target' from that directory." >&2
      exit 1
    fi
  done
fi

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

# The bundle claims a version; the binary inside it has its own. A release
# whose Info.plist and `--version` disagree sends people the wrong update
# notice, so catch it here rather than after it has shipped.
built_version="$("$APP_DIR/Contents/MacOS/$EXECUTABLE_NAME" --version | awk '{ print $NF }')"
if [[ "$built_version" != "$VERSION" ]]; then
  echo "error: the bundle says $VERSION but the binary reports $built_version." >&2
  echo "       VERSION must match [workspace.package] version in apps/desktop/Cargo.toml." >&2
  exit 1
fi

if command -v codesign >/dev/null; then
  codesign_args=(--force --sign "$CODE_SIGN_IDENTITY")
  if [[ "$CODE_SIGN_IDENTITY" != "-" ]]; then
    codesign_args+=(--options runtime --timestamp)
  fi
  codesign "${codesign_args[@]}" "$APP_DIR" >/dev/null
fi

echo "Packaged $APP_DIR"
