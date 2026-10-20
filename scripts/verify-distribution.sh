#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 madearga
#
# Checks the built macOS bundle. The script is the artifact a reviewer runs to
# confirm the bundle on disk is the one the build produced.
#
# The script reads the bundle and never writes to it. It exits zero when every
# check passes and non-zero on the first failure, and it prints the command and
# the output for every check so the evidence is in the transcript.
#
# The optional launch check runs the bundled binary with a scratch data
# directory so the reviewer's own study data is never touched. It is skipped
# unless --launch is passed.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT" || exit 1

APP_NAME="Bible"
BUNDLE_DIR="$REPO_ROOT/dist/$APP_NAME.app"
MACOS_DIR="$BUNDLE_DIR/Contents/MacOS"
RESOURCES_DIR="$BUNDLE_DIR/Contents/Resources"
BINARY="$MACOS_DIR/$APP_NAME"

FAILURES=0
CHECKS=0

pass() {
  CHECKS=$((CHECKS + 1))
  printf 'ok    %s\n' "$*"
}

fail() {
  CHECKS=$((CHECKS + 1))
  FAILURES=$((FAILURES + 1))
  printf 'FAIL  %s\n' "$*"
}

# Runs a command, prints it, and prints its exit code. The output of the
# command is shown as well, so the reviewer sees what was checked.
run() {
  shift
  printf '\n$ %s\n' "$@"
  "$@"
  local status=$?
  printf '[exit %d]\n' "$status"
  return $status
}

section() {
  printf '\n== %s ==\n' "$*"
}

section "Bundle layout"

if [[ -d "$BUNDLE_DIR" ]]; then
  pass "the bundle directory exists at $BUNDLE_DIR"
else
  fail "the bundle directory is missing at $BUNDLE_DIR"
  printf '\nNo bundle to check. Run scripts/bundle-macos.sh first.\n'
  exit 1
fi

for required in \
  "$BUNDLE_DIR/Contents/Info.plist" \
  "$BUNDLE_DIR/Contents/MacOS/$APP_NAME" \
  "$RESOURCES_DIR/LICENSE" \
  "$RESOURCES_DIR/THIRD_PARTY.md" \
  "$RESOURCES_DIR/OFL-Inter.txt" \
  "$RESOURCES_DIR/OFL-Literata.txt" \
  "$RESOURCES_DIR/build-info.txt"; do
  if [[ -e "$required" ]]; then
    pass "$required exists"
  else
    fail "$required is missing"
  fi
done

if [[ -x "$BINARY" ]]; then
  pass "the bundled binary is executable"
else
  fail "the bundled binary is missing or not executable at $BINARY"
fi

section "Info.plist"

if run "plutil lint" plutil -lint "$BUNDLE_DIR/Contents/Info.plist"; then
  pass "the Info.plist lints"
else
  fail "the Info.plist does not lint"
fi

if run "plutil print" plutil -p "$BUNDLE_DIR/Contents/Info.plist"; then
  pass "the Info.plist parses"
else
  fail "the Info.plist does not parse"
fi

EXECUTABLE_NAME="$(plutil -extract CFBundleExecutable raw -o - "$BUNDLE_DIR/Contents/Info.plist" 2>/dev/null)"
if [[ "$EXECUTABLE_NAME" == "$APP_NAME" ]]; then
  pass "CFBundleExecutable names $APP_NAME"
else
  fail "CFBundleExecutable is ${EXECUTABLE_NAME:-<missing>}, expected $APP_NAME"
fi

BUNDLE_IDENTIFIER="$(plutil -extract CFBundleIdentifier raw -o - "$BUNDLE_DIR/Contents/Info.plist" 2>/dev/null)"
if [[ -n "$BUNDLE_IDENTIFIER" ]]; then
  pass "CFBundleIdentifier is $BUNDLE_IDENTIFIER"
else
  fail "CFBundleIdentifier is missing"
fi

section "Binary"

if run "file" file "$BINARY"; then
  pass "the file command reads the binary"
else
  fail "the file command could not read the binary"
fi

FILE_TYPE="$(file -b "$BINARY")"
if [[ "$FILE_TYPE" == *Mach-O* ]]; then
  pass "the binary is a Mach-O executable"
else
  fail "the binary is not a Mach-O executable: $FILE_TYPE"
fi

if run "lipo" lipo -info "$BINARY"; then
  pass "lipo reads the architectures"
else
  fail "lipo could not read the binary"
fi

if run "otool" otool -L "$BINARY"; then
  pass "otool reads the linked libraries"
else
  fail "otool could not read the binary"
fi

# The bundled binary is signed in place, so it is not byte for byte the file
# cargo produced. The bundle records the digest of the binary as cargo built
# it, so a reviewer can check the bundle against the build without rebuilding.
BUILD_INFO="$RESOURCES_DIR/build-info.txt"
if [[ -f "$BUILD_INFO" ]]; then
  pass "the bundle records what was built in build-info.txt"
  printf '\n'
  cat "$BUILD_INFO"
  printf '\n'
  RECORDED_DIGEST="$(sed -n 's/^source-sha256 //p' "$BUILD_INFO")"
  RECORDED_SIZE="$(sed -n 's/^source-size //p' "$BUILD_INFO")"

  TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
  case "$TARGET_DIR" in
    /*) ;;
    *) TARGET_DIR="$REPO_ROOT/$TARGET_DIR" ;;
  esac
  BUILT_BINARY="$TARGET_DIR/release/bible"
  if [[ -f "$BUILT_BINARY" ]]; then
    CURRENT_DIGEST="$(shasum -a 256 "$BUILT_BINARY" | cut -d' ' -f1)"
    CURRENT_SIZE="$(stat -f %z "$BUILT_BINARY")"
    if [[ "$CURRENT_DIGEST" == "$RECORDED_DIGEST" ]]; then
      pass "the binary in the target directory is the one the bundle was built from"
    else
      fail "the binary in the target directory is $CURRENT_DIGEST and the bundle was built from $RECORDED_DIGEST"
    fi
    if [[ "$CURRENT_SIZE" == "$RECORDED_SIZE" ]]; then
      pass "the binary in the target directory has the size the bundle recorded"
    else
      fail "the binary in the target directory is $CURRENT_SIZE bytes and the bundle recorded $RECORDED_SIZE"
    fi
  else
    printf 'note  %s is not there, so the bundle cannot be compared against a local build\n' "$BUILT_BINARY"
  fi
else
  fail "the bundle has no build-info.txt, so the build it came from is unrecorded"
fi

section "Signature"

if command -v codesign >/dev/null 2>&1; then
  if run "codesign verify" codesign --verify --deep --strict --verbose=2 "$BUNDLE_DIR"; then
    pass "the bundle's signature verifies"
  else
    fail "the bundle's signature does not verify"
  fi

  if run "codesign details" codesign --display --verbose=4 "$BUNDLE_DIR"; then
    pass "the bundle's signature details are readable"
  else
    fail "the bundle's signature details could not be read"
  fi

  # The output is captured first rather than piped into grep. Piping into
  # grep -q closes the pipe as soon as grep has its match, and under pipefail
  # that turns the codesign exit code into a broken-pipe failure.
  SIGNATURE_DETAIL="$(codesign --display --verbose=4 "$BUNDLE_DIR" 2>&1)"
  printf 'note  the bundle carries an ad-hoc signature, not a developer identity or a notarization\n'
  if printf '%s\n' "$SIGNATURE_DETAIL" | grep -q "Signature=adhoc"; then
    pass "the signature is ad-hoc, so the bundle is an unsigned developer build"
  else
    printf 'note  the signature is not ad-hoc, so it is a real identity rather than a local placeholder\n'
  fi
else
  printf 'note  codesign is not on the PATH, so the signature was not checked\n'
fi

section "Notices"

if diff -q "$REPO_ROOT/LICENSE" "$RESOURCES_DIR/LICENSE" >/dev/null 2>&1; then
  pass "the bundled LICENSE is the repository LICENSE"
else
  fail "the bundled LICENSE differs from the repository LICENSE"
fi

if diff -q "$REPO_ROOT/THIRD_PARTY.md" "$RESOURCES_DIR/THIRD_PARTY.md" >/dev/null 2>&1; then
  pass "the bundled THIRD_PARTY.md is the repository THIRD_PARTY.md"
else
  fail "the bundled THIRD_PARTY.md differs from the repository THIRD_PARTY.md"
fi

if diff -q "$REPO_ROOT/assets/fonts/OFL-Inter.txt" "$RESOURCES_DIR/OFL-Inter.txt" >/dev/null 2>&1; then
  pass "the bundled Inter licence is the repository OFL-Inter.txt"
else
  fail "the bundled Inter licence differs from the repository OFL-Inter.txt"
fi

if diff -q "$REPO_ROOT/assets/fonts/OFL-Literata.txt" "$RESOURCES_DIR/OFL-Literata.txt" >/dev/null 2>&1; then
  pass "the bundled Literata licence is the repository OFL-Literata.txt"
else
  fail "the bundled Literata licence differs from the repository OFL-Literata.txt"
fi

# The notices have to name the licences they cover, or a reader cannot tell
# what applies to what.
if grep -q "SIL Open Font License" "$RESOURCES_DIR/OFL-Inter.txt"; then
  pass "the Inter licence names the SIL Open Font License"
else
  fail "the Inter licence does not name the SIL Open Font License"
fi

if grep -q "SIL Open Font License" "$RESOURCES_DIR/OFL-Literata.txt"; then
  pass "the Literata licence names the SIL Open Font License"
else
  fail "the Literata licence does not name the SIL Open Font License"
fi

for named in Inter Literata zstd; do
  if grep -q "$named" "$RESOURCES_DIR/THIRD_PARTY.md"; then
    pass "the third-party notice names $named"
  else
    fail "the third-party notice does not name $named"
  fi
done

section "Launch"

if [[ "${1:-}" == "--launch" ]]; then
  SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/u11-verify.XXXXXX")"
  printf 'note  the launch check writes only into %s\n' "$SCRATCH"

  if run "bundled CLI" env BIBLE_DATA_DIR="$SCRATCH" "$BINARY" translations; then
    pass "the bundled binary runs the translations verb"
  else
    fail "the bundled binary did not run the translations verb"
  fi

  if run "bundled read" env BIBLE_DATA_DIR="$SCRATCH" "$BINARY" read "John 3:16"; then
    pass "the bundled binary reads a verse from the bundled translation"
  else
    fail "the bundled binary did not read a verse"
  fi

  if run "bundled help" env BIBLE_DATA_DIR="$SCRATCH" "$BINARY" help; then
    pass "the bundled binary prints its usage"
  else
    fail "the bundled binary did not print its usage"
  fi
else
  printf 'note  the launch check was skipped, pass --launch to run it\n'
fi

section "Result"

printf 'checks: %d, failures: %d\n' "$CHECKS" "$FAILURES"
if [[ "$FAILURES" -gt 0 ]]; then
  printf 'the bundle did not pass every check\n'
  exit 1
fi
printf 'the bundle passed every check\n'
