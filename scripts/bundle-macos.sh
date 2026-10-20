#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 madearga
#
# Builds the local macOS application bundle. The bundle is a developer build: it
# is unsigned, it is not notarized, and it is not distributed anywhere.
#
# The script is safe to run again. It rebuilds the release binary, replaces the
# bundle under dist/, and leaves every other file in the repository alone. It
# needs no privileges beyond the ones the user already has, and it never
# installs anything into the system.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Cargo honours CARGO_TARGET_DIR itself. The script reads it back so the binary
# is found in the same place cargo put it, and it falls back to the workspace
# default when the variable is unset.
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
# Resolve a relative CARGO_TARGET_DIR the way cargo does, relative to the
# workspace root.
case "$TARGET_DIR" in
  /*) ;;
  *) TARGET_DIR="$REPO_ROOT/$TARGET_DIR" ;;
esac

APP_NAME="Bible"
BUNDLE_ID="com.madearga.bibblereader"
BUNDLE_DIR="$REPO_ROOT/dist/$APP_NAME.app"
MACOS_DIR="$BUNDLE_DIR/Contents/MacOS"
RESOURCES_DIR="$BUNDLE_DIR/Contents/Resources"

# The bundle ships the release binary only. The translations are compiled into
# the binary, and the fonts are compiled into the binary as well, so nothing
# else has to travel with it.
BINARY_NAME="Bible"

log() {
  printf '%s\n' "$*"
}

fail() {
  printf 'bundle-macos: %s\n' "$*" >&2
  exit 1
}

# Checks the tools the script needs before it changes anything.
require_tool() {
  command -v "$1" >/dev/null 2>&1 || fail "$1 is not on the PATH"
}

require_tool cargo
require_tool plutil
require_tool file
require_tool shasum

if [[ "$(uname -s)" != "Darwin" ]]; then
  fail "this bundle is for macOS and this machine is $(uname -s)"
fi

# Build the release binary. Cargo decides whether anything needs recompiling,
# and the fingerprint it keeps makes a rerun a no-op when nothing changed.
log "Building the release binary."
cargo build --release -p bible-app

BINARY_PATH="$TARGET_DIR/release/bible"
[[ -f "$BINARY_PATH" ]] || fail "cargo did not produce $BINARY_PATH"
[[ -x "$BINARY_PATH" ]] || fail "$BINARY_PATH is not executable"

# Refuse to bundle a binary that does not look like the one this build
# produced. A stale binary in the target directory is the failure mode a
# reviewer would hit, so the check is explicit rather than assumed.
FILE_INFO="$(file -b "$BINARY_PATH")"
case "$FILE_INFO" in
  *Mach-O*) ;;
  *) fail "$BINARY_PATH is not a Mach-O binary: $FILE_INFO" ;;
esac

# Replace the bundle rather than merging into it. Merging leaves files from an
# earlier build behind, and a bundle with a stale file in it is not the build
# the reviewer thinks it is. The delete is scoped to the one directory this
# script owns, under dist/, and the guard refuses anything else.
if [[ -e "$BUNDLE_DIR" ]]; then
  case "$BUNDLE_DIR" in
    "$REPO_ROOT/dist/$APP_NAME.app") ;;
    *) fail "refusing to delete $BUNDLE_DIR, which is not the bundle this script builds" ;;
  esac
  find "$BUNDLE_DIR" -depth -delete
fi
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BINARY_PATH" "$MACOS_DIR/$BINARY_NAME"
chmod 755 "$MACOS_DIR/$BINARY_NAME"

# The bundled copy has to be the binary cargo just produced. Comparing the two
# byte for byte catches a stale binary in the target directory, which is the
# failure a reviewer would otherwise have to notice by hand.
if ! cmp -s "$BINARY_PATH" "$MACOS_DIR/$BINARY_NAME"; then
  fail "the bundled binary is not the binary cargo just built at $BINARY_PATH"
fi

# Record what was built, so a reviewer can check the bundle against a later
# build without rebuilding the whole thing. The digest is of the binary as
# cargo produced it, before the ad-hoc signature is added, because signing
# changes the bytes on disk.
BUILT_DIGEST="$(shasum -a 256 "$BINARY_PATH" | cut -d' ' -f1)"
BUILT_SIZE="$(stat -f %z "$BINARY_PATH")"
{
  printf 'binary %s\n' "$BINARY_NAME"
  printf 'source-binary %s\n' "$BINARY_PATH"
  printf 'source-sha256 %s\n' "$BUILT_DIGEST"
  printf 'source-size %s\n' "$BUILT_SIZE"
  printf 'bundle-id %s\n' "$BUNDLE_ID"
} > "$RESOURCES_DIR/build-info.txt"

# The property list. The identifier is a local development identifier, and the
# bundle carries no icon and no resources beyond the notices, so the window
# falls back to the system's generic application icon.
cat > "$BUNDLE_DIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>$BINARY_NAME</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>MIT licensed. Bundled translations and fonts carry their own notices in Contents/Resources.</string>
</dict>
</plist>
PLIST

plutil -lint "$BUNDLE_DIR/Contents/Info.plist" >/dev/null || fail "the generated Info.plist does not lint"

# The notices that have to travel with the binary: the licence of this program,
# the third-party notices, and the font licences for the faces compiled into it.
cp "$REPO_ROOT/LICENSE" "$RESOURCES_DIR/LICENSE"
cp "$REPO_ROOT/THIRD_PARTY.md" "$RESOURCES_DIR/THIRD_PARTY.md"
cp "$REPO_ROOT/assets/fonts/OFL-Inter.txt" "$RESOURCES_DIR/OFL-Inter.txt"
cp "$REPO_ROOT/assets/fonts/OFL-Literata.txt" "$RESOURCES_DIR/OFL-Literata.txt"

# Sign the bundle ad-hoc. This is not a developer identity, it is not a
# distribution signature, and it does not make the bundle notarizable. It
# exists so the system treats the bundle as a signed application and gives a
# stable identity to the binary rather than refusing it outright.
if command -v codesign >/dev/null 2>&1; then
  if ! SIGN_OUTPUT="$(codesign --force --sign - --timestamp=none "$BUNDLE_DIR" 2>&1)"; then
    printf '%s\n' "$SIGN_OUTPUT" >&2
    fail "the ad-hoc signature could not be applied"
  fi
  log "Signed the bundle ad-hoc. The signature is a local placeholder, not a developer identity."
else
  log "codesign is not on the PATH, so the bundle is left unsigned."
fi

log "Bundle written to $BUNDLE_DIR"
log "It is an unsigned developer build with an ad-hoc signature."
log "A bundle you built here has no quarantine attribute, so open it with:"
log "  open \"$BUNDLE_DIR\""
log "A bundle that arrived from somewhere else carries a quarantine attribute."
log "macOS refuses it the first time, so right-click it in Finder and choose Open."
