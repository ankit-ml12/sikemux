#!/usr/bin/env bash
# Publish a release that scripts/release.sh built and verified.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd -P)"
VERSION="${1:-}"
NOTES="${2:-}"
CHANNEL=stable
[[ "${3:-}" == "--nightly" ]] && CHANNEL=nightly

fail() {
  echo "Release publish failed: $*" >&2
  exit 1
}

[[ -n "$VERSION" && -n "$NOTES" ]] || { echo "Usage: $0 <version> <notes> [--nightly]" >&2; exit 2; }
[[ "${GITHUB_ACTIONS:-}" == "true" ]] || fail "releases publish from the Release workflow"

BUNDLE="$ROOT/src-tauri/target/release/bundle"
APP_NAME="$(node scripts/app-metadata.mjs name)"
TAR="$BUNDLE/macos/${APP_NAME}.app.tar.gz"
SIG="$TAR.sig"
if [[ "$CHANNEL" == "stable" ]]; then
  MANIFEST="$ROOT/latest.json"
else
  MANIFEST="$BUNDLE/latest.json"
fi
shopt -s nullglob
DMGS=("$BUNDLE"/dmg/*.dmg)
VOICES=("$ROOT"/src-tauri/binaries/sikemux-voice-*)
SIMS=("$ROOT"/src-tauri/binaries/sikemux-sim-*)
DEBS=("$BUNDLE"/deb/*.deb)
RPMS=("$BUNDLE"/rpm/*.rpm)
APPIMAGES=("$BUNDLE"/appimage/*.AppImage)
HOSTS=("$ROOT"/src-tauri/target/host/sikemux-host-"$VERSION"-linux-*.tar.gz)
shopt -u nullglob
[[ ${#DMGS[@]} -eq 1 ]] || fail "expected exactly one DMG, found ${#DMGS[@]}"
[[ ${#VOICES[@]} -eq 1 ]] || fail "expected exactly one voice helper, found ${#VOICES[@]}"
[[ ${#SIMS[@]} -eq 1 ]] || fail "expected exactly one simulator helper, found ${#SIMS[@]}"
[[ ${#DEBS[@]} -eq 1 ]] || fail "expected exactly one deb, found ${#DEBS[@]}"
[[ ${#RPMS[@]} -eq 1 ]] || fail "expected exactly one rpm, found ${#RPMS[@]}"
[[ ${#APPIMAGES[@]} -eq 1 ]] || fail "expected exactly one AppImage, found ${#APPIMAGES[@]}"
[[ ${#HOSTS[@]} -eq 2 ]] || fail "expected the x86_64 and aarch64 host cores, found ${#HOSTS[@]}"
HOST_SIGS=("${HOSTS[@]/%/.sig}")
DMG="${DMGS[0]}"
VOICE="${VOICES[0]}"
SIM="${SIMS[0]}"
DEB="${DEBS[0]}"
RPM="${RPMS[0]}"
APPIMAGE="${APPIMAGES[0]}"
APPIMAGE_SIG="$APPIMAGE.sig"
INSTALLER="$ROOT/scripts/install.sh"
for file in "$DMG" "$TAR" "$SIG" "$VOICE" "$SIM" "$DEB" "$RPM" "$APPIMAGE" "${HOSTS[@]}" "${HOST_SIGS[@]}" "$INSTALLER" "$APPIMAGE_SIG" "$MANIFEST"; do
  [[ -s "$file" ]] || fail "$file is missing or empty"
done
[[ "$(node -p "require('$MANIFEST').version")" == "$VERSION" ]] || fail "$MANIFEST is not for v$VERSION"
for signed in "${HOSTS[@]}" "$APPIMAGE"; do
  node scripts/verify-updater-signature.mjs "$signed" "$signed.sig" || fail "$signed is not signed by the updater key"
done
# The Linux app updates itself only as an AppImage, so only the AppImage joins the
# feed the macOS build wrote.
APPIMAGE_URL="https://github.com/nodelike/sikemux/releases/download/v$VERSION/$(basename "$APPIMAGE")" \
  APPIMAGE_SIG="$APPIMAGE_SIG" MANIFEST="$MANIFEST" python3 - <<'PY'
import json, os, pathlib
path = pathlib.Path(os.environ["MANIFEST"])
manifest = json.loads(path.read_text())
manifest["platforms"]["linux-x86_64"] = {
    "signature": pathlib.Path(os.environ["APPIMAGE_SIG"]).read_text().strip(),
    "url": os.environ["APPIMAGE_URL"],
}
path.write_text(json.dumps(manifest, indent=2) + "\n")
PY

STABLE_GH_CMD=(gh release create "v$VERSION" --verify-tag --title "v$VERSION" --notes "$NOTES" "$DMG" "$TAR" "$SIG" "$VOICE" "$SIM" "$DEB" "$RPM" "$APPIMAGE" "${HOSTS[@]}" "${HOST_SIGS[@]}" "$INSTALLER" "$APPIMAGE_SIG" "$MANIFEST")
NIGHTLY_GH_CMD=(gh release create "v$VERSION" --verify-tag --title "v$VERSION" --notes "$NOTES" --prerelease "$DMG" "$TAR" "$SIG" "$VOICE" "$SIM" "$DEB" "$RPM" "$APPIMAGE" "${HOSTS[@]}" "${HOST_SIGS[@]}" "$INSTALLER" "$APPIMAGE_SIG")
POINTER_NOTES="Update feed for the nightly channel.

The installable build for this feed is [v$VERSION](https://github.com/nodelike/sikemux/releases/tag/v$VERSION).

This release carries only \`latest.json\`. Its \`nightly\` tag is a fixed URL anchor that shipped clients resolve against, not a source revision — do not attach builds here."
if [[ "$CHANNEL" == "stable" ]]; then
  echo "→ Publishing stable v$VERSION"
  "${STABLE_GH_CMD[@]}"
  echo "✓ Released stable v$VERSION"
else
  echo "→ Publishing nightly v$VERSION"
  "${NIGHTLY_GH_CMD[@]}"
  if gh release view nightly >/dev/null 2>&1; then
    gh release upload nightly "$MANIFEST" --clobber
    gh release edit nightly --title "Nightly feed (v$VERSION)" --notes "$POINTER_NOTES" --prerelease
  else
    gh release create nightly --target "$(git rev-parse HEAD)" --title "Nightly feed (v$VERSION)" --notes "$POINTER_NOTES" --prerelease "$MANIFEST"
  fi
  echo "✓ Released nightly v$VERSION; nightly feed now points at v$VERSION"
fi
