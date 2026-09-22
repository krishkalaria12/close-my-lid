#!/bin/sh
# Installs Close My Lid from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/scripts/install.sh | sh
#
# macOS: installs `Close My Lid.app` into /Applications (or ~/Applications when
# /Applications is not writable) and links the `close-my-lid` command.
# Linux: installs the `close-my-lid` CLI.
#
# Environment:
#   CLOSE_MY_LID_VERSION      release to install, e.g. v0.5.0 (default: latest)
#   CLOSE_MY_LID_BIN_DIR      where the command goes (default: ~/.local/bin)
#   CLOSE_MY_LID_APP_DIR      macOS only: where the app goes (default: /Applications)
#
# Windows has its own installer: scripts/install.ps1.

set -eu

REPO="krishkalaria12/close-my-lid"
BIN_DIR="${CLOSE_MY_LID_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but was not found"; }

need curl
need uname

# `releases/latest` redirects to the tag's page, which names the version
# without spending an unauthenticated API request.
resolve_version() {
  if [ -n "${CLOSE_MY_LID_VERSION:-}" ]; then
    case "$CLOSE_MY_LID_VERSION" in
      v*) printf '%s' "$CLOSE_MY_LID_VERSION" ;;
      *) printf 'v%s' "$CLOSE_MY_LID_VERSION" ;;
    esac
    return
  fi
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest")" \
    || die "could not reach GitHub to find the latest release"
  tag="${url##*/}"
  case "$tag" in
    v[0-9]*) printf '%s' "$tag" ;;
    *) die "could not work out the latest release from $url" ;;
  esac
}

download() {
  say "Downloading $1"
  curl -fL --progress-bar -o "$2" "https://github.com/$REPO/releases/download/$TAG/$1" \
    || die "download failed: $1 is not attached to $TAG"
}

path_hint() {
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "Note: $BIN_DIR is not on your PATH. Add it with:"
       say "  export PATH=\"$BIN_DIR:\$PATH\"" ;;
  esac
}

install_macos() {
  need ditto
  app_name="Close My Lid.app"
  app_dir="${CLOSE_MY_LID_APP_DIR:-/Applications}"
  if [ -z "${CLOSE_MY_LID_APP_DIR:-}" ] && [ ! -w "$app_dir" ]; then
    app_dir="$HOME/Applications"
  fi
  mkdir -p "$app_dir"

  archive="Close-My-Lid-$TAG-macOS.zip"
  download "$archive" "$TMP/$archive"
  ditto -x -k "$TMP/$archive" "$TMP/unpacked"
  [ -d "$TMP/unpacked/$app_name" ] || die "$archive does not contain $app_name"

  # Quitting the app releases any active hold, so the swap cannot strand one.
  was_running=0
  if pgrep -x CloseMyLid >/dev/null 2>&1; then
    was_running=1
    say "Quitting the running copy"
    osascript -e 'quit app "Close My Lid"' >/dev/null 2>&1 || true
    sleep 2
  fi

  rm -rf "$app_dir/$app_name"
  ditto "$TMP/unpacked/$app_name" "$app_dir/$app_name"
  xattr -dr com.apple.quarantine "$app_dir/$app_name" 2>/dev/null || true

  mkdir -p "$BIN_DIR"
  ln -sf "$app_dir/$app_name/Contents/MacOS/CloseMyLid" "$BIN_DIR/close-my-lid"

  say ""
  say "Installed $app_name $TAG to $app_dir"
  say "Linked the close-my-lid command into $BIN_DIR"
  path_hint

  if [ "$was_running" = 1 ]; then
    open "$app_dir/$app_name"
  else
    say "Open it from $app_dir, or run: open \"$app_dir/$app_name\""
  fi
}

install_linux() {
  need tar
  case "$(uname -m)" in
    x86_64 | amd64) arch="x86_64" ;;
    *) die "no Linux build for $(uname -m) yet; build from source with: cargo install --git https://github.com/$REPO lid-cli" ;;
  esac

  name="close-my-lid-$TAG-linux-$arch"
  download "$name.tar.gz" "$TMP/$name.tar.gz"
  tar -xzf "$TMP/$name.tar.gz" -C "$TMP"
  [ -f "$TMP/$name/close-my-lid" ] || die "$name.tar.gz does not contain the close-my-lid binary"

  mkdir -p "$BIN_DIR"
  # Replaced through a rename so a running `close-my-lid enable` keeps its inode.
  cp "$TMP/$name/close-my-lid" "$BIN_DIR/.close-my-lid.new"
  chmod 755 "$BIN_DIR/.close-my-lid.new"
  mv -f "$BIN_DIR/.close-my-lid.new" "$BIN_DIR/close-my-lid"

  say ""
  say "Installed close-my-lid $TAG to $BIN_DIR/close-my-lid"
  path_hint
  say "Try: close-my-lid enable --for 2h"
  say "Run it in the background with: close-my-lid systemd"
}

TAG="$(resolve_version)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM

case "$(uname -s)" in
  Darwin) install_macos ;;
  Linux) install_linux ;;
  MINGW* | MSYS* | CYGWIN*)
    die "on Windows, run in PowerShell: irm https://raw.githubusercontent.com/$REPO/main/scripts/install.ps1 | iex" ;;
  *) die "unsupported operating system: $(uname -s)" ;;
esac
