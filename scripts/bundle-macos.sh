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
POSTSIGN_RECORD="$REPO_ROOT/dist/build-info.postsign.txt"

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
require_tool python3

if [[ "$(uname -s)" != "Darwin" ]]; then
  fail "this bundle is for macOS and this machine is $(uname -s)"
fi

# Build the release binary and select the exact executable Cargo reports. This
# handles CARGO_BUILD_TARGET and target settings in .cargo/config.toml instead
# of guessing target/release/bible.
log "Building the release binary."
BUILD_LOG="$(mktemp "${TMPDIR:-/tmp}/bible-build.XXXXXX")" || fail "cannot create the Cargo output log"
cleanup_build_log() { rm -f "$BUILD_LOG"; }
trap cleanup_build_log EXIT
if ! cargo build --release -p bible-app --message-format=json-render-diagnostics >"$BUILD_LOG" 2>&1; then
  cat "$BUILD_LOG" >&2
  fail "cargo build failed"
fi
BINARY_PATH="$(python3 - "$BUILD_LOG" <<'PY'
import json, pathlib, sys
for line in pathlib.Path(sys.argv[1]).read_text().splitlines():
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        continue
    if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == "bible":
        executable = message.get("executable")
        if executable:
            print(executable)
            break
PY
)"
[[ -n "$BINARY_PATH" ]] || fail "cargo did not report the bible executable"
[[ -f "$BINARY_PATH" ]] || fail "cargo reported a missing executable at $BINARY_PATH"
[[ -x "$BINARY_PATH" ]] || fail "$BINARY_PATH is not executable"

# Refuse to bundle a binary that does not look like the one this build
# produced. A stale binary in the target directory is the failure mode a
# reviewer would hit, so the check is explicit rather than assumed.
FILE_INFO="$(file -b "$BINARY_PATH")"
case "$FILE_INFO" in
  *Mach-O*) ;;
  *) fail "$BINARY_PATH is not a Mach-O binary: $FILE_INFO" ;;
esac

# Validate the physical output directory before any mutation. A symlinked
# dist/ or Bible.app could make a path-shaped guard delete outside the repo.
DIST_DIR="$REPO_ROOT/dist"
[[ -L "$DIST_DIR" ]] && fail "refusing symlinked output directory $DIST_DIR"
mkdir -p "$DIST_DIR"
DIST_REAL="$(cd "$DIST_DIR" && pwd -P)" || fail "cannot resolve $DIST_DIR"
REPO_REAL="$(cd "$REPO_ROOT" && pwd -P)" || fail "cannot resolve $REPO_ROOT"
[[ "$DIST_REAL" == "$REPO_REAL/dist" ]] || fail "output directory $DIST_DIR resolves to $DIST_REAL, outside the repository"
[[ -L "$BUNDLE_DIR" ]] && fail "refusing symlinked bundle $BUNDLE_DIR"
# The bundle itself is the only child this script deletes, and it must not be a
# symlink. Its parent is the physical dist directory validated above.

# Replace the bundle rather than merging into it. The physical containment
# checks above run before this deletion.
if [[ -e "$BUNDLE_DIR" ]]; then
  find "$BUNDLE_DIR" -depth -delete
fi
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BINARY_PATH" "$MACOS_DIR/$BINARY_NAME"
chmod 755 "$MACOS_DIR/$BINARY_NAME"

# The bundled copy has to be the binary cargo just produced. Comparing the two
# byte for byte catches a stale binary in the target directory.
if ! cmp -s "$BINARY_PATH" "$MACOS_DIR/$BINARY_NAME"; then
  fail "the bundled binary is not the binary cargo just built at $BINARY_PATH"
fi

# The manifest records the digest of the executable as it will be in the
# bundle, that is after signing. It is written after the signing step so the
# signature covers it and the file inside the bundle is the signed one.

# The property list. The identifier is a local development identifier, and the
# bundle carries no icon and no resources beyond the notices, so the window
# falls back to the system's generic application icon.
#
# LSMinimumSystemVersion is derived from the executable's own minimum macOS
# version rather than written by hand, so the plist cannot claim a system
# version the binary does not support.
BINARY_MIN_OS="$(python3 - "$MACOS_DIR/$BINARY_NAME" <<'PY'
import subprocess, sys
out = subprocess.run(
    ["otool", "-l", sys.argv[1]], capture_output=True, text=True, check=True
).stdout
versions = []
lines = out.splitlines()
for index, line in enumerate(lines):
    if "LC_BUILD_VERSION" in line or "LC_VERSION_MIN_MACOSX" in line:
        for follow in lines[index + 1:index + 5]:
            parts = follow.split()
            if parts and parts[0] == "minos":
                raw = parts[1]
                if "." in raw:
                    major, minor = raw.split(".")[:2]
                    versions.append((int(major), int(minor)))
                break
if not versions:
    print("")
else:
    print("%d.%d" % max(versions))
PY
)"
[[ -n "$BINARY_MIN_OS" ]] || fail "cannot read the minimum macOS version from $MACOS_DIR/$BINARY_NAME"

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
  <string>$BINARY_MIN_OS</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSHumanReadableCopyright</key>
  <string>MIT licensed. Bundled translations and fonts carry their own notices in Contents/Resources.</string>
</dict>
</plist>
PLIST

plutil -lint "$BUNDLE_DIR/Contents/Info.plist" >/dev/null || fail "the generated Info.plist does not lint"

# The notices that have to travel with the binary: the licence of this program,
# the third-party notices, the collected licence texts, and the font licences
# for the faces compiled into it.
cp "$REPO_ROOT/LICENSE" "$RESOURCES_DIR/LICENSE"
cp "$REPO_ROOT/THIRD_PARTY.md" "$RESOURCES_DIR/THIRD_PARTY.md"
cp "$REPO_ROOT/assets/fonts/OFL-Inter.txt" "$RESOURCES_DIR/OFL-Inter.txt"
cp "$REPO_ROOT/assets/fonts/OFL-Literata.txt" "$RESOURCES_DIR/OFL-Literata.txt"

# The licence texts of the shipped dependencies. The collector fails when a
# required licence file is missing, so the bundle never claims completeness it
# does not have.
# Collection is mandatory: the check fails when a crate in the runtime closure
# has no notice text, so the bundle cannot be produced with a silent gap.
python3 "$REPO_ROOT/scripts/collect-licenses.py" --check --output "$REPO_ROOT/LICENSES" >/dev/null \
  || fail "the dependency notices in $REPO_ROOT/LICENSES are missing or stale; run scripts/collect-licenses.py"
mkdir -p "$RESOURCES_DIR/LICENSES"
find "$REPO_ROOT/LICENSES" -type f -print0 | while IFS= read -r -d '' notice; do
  cp "$notice" "$RESOURCES_DIR/LICENSES/$(basename "$notice")"
done
NOTICE_COUNT="$(find "$RESOURCES_DIR/LICENSES" -type f | wc -l | tr -d ' ')"
[[ "$NOTICE_COUNT" -gt 0 ]] || fail "the LICENSES folder is empty, so the bundle ships no dependency notices"
cmp -s "$REPO_ROOT/LICENSES/INDEX.md" "$RESOURCES_DIR/LICENSES/INDEX.md" \
  || fail "the bundled licence index is not the collected index"
log "Copied $NOTICE_COUNT dependency notices into Contents/Resources/LICENSES."

# Sign the bundle ad-hoc. This is not a developer identity, it is not a
# distribution signature, and it does not make the bundle notarizable. It
# exists so the system treats the bundle as a signed application and gives a
# stable identity to the binary rather than refusing it outright.
sign_bundle() {
  if ! SIGN_OUTPUT="$(codesign --force --sign - --timestamp=none "$BUNDLE_DIR" 2>&1)"; then
    printf '%s\n' "$SIGN_OUTPUT" >&2
    fail "the ad-hoc signature could not be applied"
  fi
}

if command -v codesign >/dev/null 2>&1; then
  sign_bundle
  log "Signed the bundle ad-hoc. The signature is a local placeholder, not a developer identity."
else
  log "codesign is not on the PATH, so the bundle is left unsigned."
fi

# The in-bundle manifest records what was built: the source binary, its digest,
# the derived minimum macOS version, and the architectures. It is written
# before the signature below, so codesign seals it with the rest of the bundle.
SOURCE_DIGEST="$(shasum -a 256 "$BINARY_PATH" | cut -d' ' -f1)"
SOURCE_SIZE="$(stat -f %z "$BINARY_PATH")"
{
  printf 'binary %s\n' "$BINARY_NAME"
  printf 'bundle-id %s\n' "$BUNDLE_ID"
  printf 'source-binary %s\n' "$BINARY_PATH"
  printf 'source-sha256 %s\n' "$SOURCE_DIGEST"
  printf 'source-size %s\n' "$SOURCE_SIZE"
  printf 'minimum-macos %s\n' "$BINARY_MIN_OS"
  printf 'architectures %s\n' "$(lipo -archs "$MACOS_DIR/$BINARY_NAME" 2>/dev/null | tr ' ' ',')"
} > "$RESOURCES_DIR/build-info.txt"

# A signature that seals a manifest cannot have that manifest contain the
# signature's own hash: changing the manifest changes the executable's
# signature and therefore its digest. The post-sign record is written beside
# the bundle instead. It names the executable digest and codesign's own
# CDHash for the final signed bundle, and the verifier checks both.
if [[ "$(command -v codesign >/dev/null 2>&1 && echo yes)" == "yes" ]]; then
  sign_bundle
  codesign --verify --deep --strict "$BUNDLE_DIR" >/dev/null 2>&1 \
    || fail "the signed bundle does not verify after the manifest was sealed into it"
  FINAL_DIGEST="$(shasum -a 256 "$MACOS_DIR/$BINARY_NAME" | cut -d' ' -f1)"
  FINAL_SIZE="$(stat -f %z "$MACOS_DIR/$BINARY_NAME")"
  FINAL_CDHASH="$(codesign --display --verbose=4 "$BUNDLE_DIR" 2>&1 | sed -n 's/^CDHash=//p' | head -1)"
  [[ -n "$FINAL_CDHASH" ]] || fail "codesign did not report a CDHash for the signed bundle"
  {
    printf 'bundle %s\n' "$BUNDLE_DIR"
    printf 'bundled-sha256 %s\n' "$FINAL_DIGEST"
    printf 'bundled-size %s\n' "$FINAL_SIZE"
    printf 'cdhash %s\n' "$FINAL_CDHASH"
    printf 'source-binary %s\n' "$BINARY_PATH"
    printf 'source-sha256 %s\n' "$SOURCE_DIGEST"
    printf 'source-size %s\n' "$SOURCE_SIZE"
    printf 'minimum-macos %s\n' "$BINARY_MIN_OS"
    printf 'architectures %s\n' "$(lipo -archs "$MACOS_DIR/$BINARY_NAME" 2>/dev/null | tr ' ' ',')"
  } > "$POSTSIGN_RECORD"
  log "Recorded the final signed executable digest in $POSTSIGN_RECORD."
fi

log "Bundle written to $BUNDLE_DIR"
log "It is an unsigned developer build with an ad-hoc signature."
log "A bundle you built here has no quarantine attribute, so open it with:"
log "  open \"$BUNDLE_DIR\""
log "A bundle that arrived from somewhere else carries a quarantine attribute."
log "macOS refuses it the first time, so right-click it in Finder and choose Open."
