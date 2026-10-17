#!/bin/sh
# Installs Sikemux: curl -fsSL https://sikemux.com/install | sh
# On a Linux server, `| sh -s -- --host` installs only the core, with no window.
# Set SIKEMUX_CHANNEL=nightly for the nightly build.
set -eu

REPO="https://github.com/nodelike/sikemux"
CHANNEL="${SIKEMUX_CHANNEL:-stable}"
HOST=false
for arg in "$@"; do
  case "$arg" in
    --host) HOST=true ;;
    *) printf 'sikemux install: unknown option %s\n' "$arg" >&2; exit 2 ;;
  esac
done

say() { printf '%s\n' "$*"; }
fail() { printf 'sikemux install: %s\n' "$*" >&2; exit 1; }

command -v curl >/dev/null 2>&1 || fail "curl is required"

latest_version() {
  case "$CHANNEL" in
    stable)
      url=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$REPO/releases/latest") || fail "could not reach GitHub"
      printf '%s' "${url##*/tag/v}"
      ;;
    nightly)
      curl -fsSL "$REPO/releases/download/nightly/latest.json" |
        sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' | head -n 1
      ;;
    *) fail "SIKEMUX_CHANNEL must be stable or nightly, not $CHANNEL" ;;
  esac
}

VERSION=$(latest_version)
[ -n "$VERSION" ] || fail "could not find the latest $CHANNEL version"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

download() {
  say "Downloading $1"
  curl -fL --progress-bar -o "$TMP/$1" "$REPO/releases/download/v$VERSION/$1" ||
    fail "v$VERSION has no $1${2:+. $2}"
}

as_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@"
  elif command -v sudo >/dev/null 2>&1; then sudo "$@"
  else fail "run this as root, or install sudo"
  fi
}

install_mac() {
  [ "$(uname -m)" = arm64 ] || fail "Sikemux needs a Mac with Apple Silicon"
  if pgrep -xq Sikemux; then fail "quit Sikemux first, then run this again"; fi
  download Sikemux.app.tar.gz
  tar -xzf "$TMP/Sikemux.app.tar.gz" -C "$TMP"
  dest=/Applications
  [ -w "$dest" ] || { dest="$HOME/Applications"; mkdir -p "$dest"; }
  rm -rf "$dest/Sikemux.app"
  mv "$TMP/Sikemux.app" "$dest/Sikemux.app"
  say "Installed Sikemux $VERSION in $dest"
  open "$dest/Sikemux.app"
}

install_linux() {
  [ "$(uname -m)" = x86_64 ] || fail "Sikemux for Linux needs an x86_64 machine"
  hint="Linux builds ship in nightlies first; try SIKEMUX_CHANNEL=nightly"
  if command -v apt-get >/dev/null 2>&1; then
    download "Sikemux_${VERSION}_amd64.deb" "$hint"
    as_root apt-get install -y "$TMP/Sikemux_${VERSION}_amd64.deb"
  elif command -v dnf >/dev/null 2>&1 || command -v zypper >/dev/null 2>&1; then
    download "Sikemux-${VERSION}-1.x86_64.rpm" "$hint"
    if command -v dnf >/dev/null 2>&1; then
      as_root dnf install -y "$TMP/Sikemux-${VERSION}-1.x86_64.rpm"
    else
      as_root zypper --non-interactive install --allow-unsigned-rpm "$TMP/Sikemux-${VERSION}-1.x86_64.rpm"
    fi
  else
    download "Sikemux_${VERSION}_amd64.AppImage" "$hint"
    mkdir -p "$HOME/Applications"
    mv "$TMP/Sikemux_${VERSION}_amd64.AppImage" "$HOME/Applications/Sikemux.AppImage"
    chmod +x "$HOME/Applications/Sikemux.AppImage"
    say "Installed the AppImage as ~/Applications/Sikemux.AppImage"
  fi
  say "Installed Sikemux $VERSION"
}

install_host() {
  case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) fail "a Sikemux host needs an x86_64 or arm64 machine" ;;
  esac
  asset="sikemux-host-${VERSION}-linux-${arch}.tar.gz"
  download "$asset" "Hosts ship in nightlies first; try SIKEMUX_CHANNEL=nightly"
  tar -xzf "$TMP/$asset" -C "$TMP"
  if [ "$(id -u)" -eq 0 ]; then bin=/usr/local/bin; else bin="$HOME/.local/bin"; fi
  mkdir -p "$bin"
  updating=false
  [ -e "$bin/sikemux-editor" ] && updating=true
  cp "$TMP/sikemux-editor" "$bin/.sikemux-editor.new"
  chmod 755 "$bin/.sikemux-editor.new"
  mv -f "$bin/.sikemux-editor.new" "$bin/sikemux-editor"
  ln -sf sikemux-editor "$bin/sikemux"
  say "Installed the Sikemux $VERSION core in $bin"
  case ":$PATH:" in
    *":$bin:"*) ;;
    *) say "Add $bin to your PATH to run sikemux from a shell." ;;
  esac

  if systemctl --user show-environment >/dev/null 2>&1; then
    if ! loginctl enable-linger "$(id -un)" 2>/dev/null; then
      as_root loginctl enable-linger "$(id -un)" || say "Could not keep the core running after you log out; run: loginctl enable-linger"
    fi
    "$bin/sikemux-editor" core install
    if $updating; then
      say "A core already running keeps its old version until it restarts:"
      say "  systemctl --user restart com.nodelike.sikemux.core"
    fi
  else
    say "No systemd user session here, so the core starts on demand and not at boot."
  fi

  if (: </dev/tty) 2>/dev/null; then
    "$bin/sikemux-editor" core sign-in </dev/tty >/dev/tty 2>&1 || true
  else
    say "Sign this host in to your account with: sikemux core sign-in"
  fi
}

if $HOST; then
  [ "$(uname -s)" = Linux ] || fail "--host installs the core on Linux; on a Mac, install the app"
  install_host
  exit 0
fi

case "$(uname -s)" in
  Darwin) install_mac ;;
  Linux) install_linux ;;
  *) fail "Sikemux runs on macOS and Linux" ;;
esac
