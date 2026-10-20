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
POSTSIGN_RECORD="$REPO_ROOT/dist/build-info.postsign.txt"
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
  "$RESOURCES_DIR/build-info.txt" \
  "$RESOURCES_DIR/LICENSES/INDEX.md"; do
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
  RECORDED_MIN_OS="$(sed -n 's/^minimum-macos //p' "$BUILD_INFO")"
  RECORDED_ARCHS="$(sed -n 's/^architectures //p' "$BUILD_INFO")"

  # The manifest names the exact executable Cargo reported, so the check does
  # not guess target/release/bible and does not accept a stale host artifact.
  BUILT_BINARY="$(sed -n 's/^source-binary //p' "$BUILD_INFO")"
  if [[ -n "$BUILT_BINARY" && -f "$BUILT_BINARY" ]]; then
    CURRENT_DIGEST="$(shasum -a 256 "$BUILT_BINARY" | cut -d' ' -f1)"
    CURRENT_SIZE="$(stat -f %z "$BUILT_BINARY")"
    if [[ "$CURRENT_DIGEST" == "$RECORDED_DIGEST" ]]; then
      pass "the source binary the bundle names is the one the bundle was built from"
    else
      fail "the source binary at $BUILT_BINARY is $CURRENT_DIGEST and the bundle was built from $RECORDED_DIGEST"
    fi
    if [[ "$CURRENT_SIZE" == "$RECORDED_SIZE" ]]; then
      pass "the source binary has the size the bundle recorded"
    else
      fail "the source binary is $CURRENT_SIZE bytes and the bundle recorded $RECORDED_SIZE"
    fi
  else
    printf 'note  the recorded source binary %s is not there, so only the bundle itself is checked\n' "${BUILT_BINARY:-<missing>}"
  fi

  # The plist must state the minimum macOS version the executable itself
  # requires, read from its Mach-O load commands.
  if [[ -f "$BINARY" ]]; then
    MACHO_MIN_OS="$(python3 - "$BINARY" <<'PY'
import subprocess, sys
out = subprocess.run(["otool", "-l", sys.argv[1]], capture_output=True, text=True, check=True).stdout
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
print("%d.%d" % max(versions) if versions else "")
PY
)"
    PLIST_MIN_OS="$(plutil -extract LSMinimumSystemVersion raw -o - "$BUNDLE_DIR/Contents/Info.plist" 2>/dev/null)"
    if [[ -z "$MACHO_MIN_OS" ]]; then
      fail "the bundled executable has no readable minimum macOS load command"
    elif [[ "$PLIST_MIN_OS" == "$MACHO_MIN_OS" ]]; then
      pass "LSMinimumSystemVersion $PLIST_MIN_OS matches the executable's Mach-O minimum"
    else
      fail "LSMinimumSystemVersion is ${PLIST_MIN_OS:-<missing>} and the executable requires $MACHO_MIN_OS"
    fi
    if [[ -n "$RECORDED_MIN_OS" && "$RECORDED_MIN_OS" == "$MACHO_MIN_OS" ]]; then
      pass "the build record's minimum macOS also matches the Mach-O load command"
    else
      fail "the build record says minimum macOS ${RECORDED_MIN_OS:-<missing>} and the executable requires ${MACHO_MIN_OS:-<unreadable>}"
    fi

    ACTUAL_ARCHS="$(lipo -archs "$BINARY" 2>/dev/null | tr ' ' ',')"
    if [[ "$RECORDED_ARCHS" == "$ACTUAL_ARCHS" ]]; then
      pass "the recorded architectures $ACTUAL_ARCHS match the bundled executable"
    else
      fail "the record says architectures ${RECORDED_ARCHS:-<missing>} and the bundled executable is ${ACTUAL_ARCHS:-<unreadable>}"
    fi
    if [[ "$ACTUAL_ARCHS" == *","* ]]; then
      printf 'note  the bundle is universal (%s), not a single-architecture build\n' "$ACTUAL_ARCHS"
    else
      printf 'note  the bundle is a host-only %s build, not a universal binary\n' "$ACTUAL_ARCHS"
    fi
  fi
else
  fail "the bundle has no build-info.txt, so the build it came from is unrecorded"
fi

section "Final signed executable"

if [[ -f "$POSTSIGN_RECORD" ]]; then
  pass "the build recorded the final signed executable in $POSTSIGN_RECORD"
  printf '\n'
  cat "$POSTSIGN_RECORD"
  printf '\n'
  FINAL_DIGEST="$(sed -n 's/^bundled-sha256 //p' "$POSTSIGN_RECORD")"
  FINAL_SIZE="$(sed -n 's/^bundled-size //p' "$POSTSIGN_RECORD")"
  FINAL_CDHASH="$(sed -n 's/^cdhash //p' "$POSTSIGN_RECORD")"
  ACTUAL_DIGEST="$(shasum -a 256 "$BINARY" | cut -d' ' -f1)"
  ACTUAL_SIZE="$(stat -f %z "$BINARY")"
  if [[ "$ACTUAL_DIGEST" == "$FINAL_DIGEST" ]]; then
    pass "the bundled executable digest is the post-sign digest the build recorded"
  else
    fail "the bundled executable is $ACTUAL_DIGEST and the build recorded $FINAL_DIGEST"
  fi
  if [[ "$ACTUAL_SIZE" == "$FINAL_SIZE" ]]; then
    pass "the bundled executable size is the post-sign size the build recorded"
  else
    fail "the bundled executable is $ACTUAL_SIZE bytes and the build recorded $FINAL_SIZE"
  fi
  if command -v codesign >/dev/null 2>&1; then
    ACTUAL_CDHASH="$(codesign --display --verbose=4 "$BUNDLE_DIR" 2>&1 | sed -n 's/^CDHash=//p' | head -1)"
    if [[ -n "$FINAL_CDHASH" && "$ACTUAL_CDHASH" == "$FINAL_CDHASH" ]]; then
      pass "codesign's CDHash is the post-sign CDHash the build recorded"
    else
      fail "codesign reports CDHash ${ACTUAL_CDHASH:-<missing>} and the build recorded ${FINAL_CDHASH:-<missing>}"
    fi
  fi
else
  fail "the final signed executable digest is not recorded at $POSTSIGN_RECORD"
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

# Every notice the collector wrote travels with the bundle, and the index in
# the bundle is the collected index.
if [[ -f "$REPO_ROOT/LICENSES/INDEX.md" ]]; then
  if diff -q "$REPO_ROOT/LICENSES/INDEX.md" "$RESOURCES_DIR/LICENSES/INDEX.md" >/dev/null 2>&1; then
    pass "the bundled licence index is the collected index"
  else
    fail "the bundled licence index differs from the collected index"
  fi
  MISSING_NOTICES=0
  while IFS= read -r -d '' notice; do
    relative="${notice#"$REPO_ROOT/LICENSES/"}"
    if [[ ! -f "$RESOURCES_DIR/LICENSES/$relative" ]]; then
      printf 'FAIL  %s is in LICENSES but not in the bundle\n' "$relative"
      MISSING_NOTICES=$((MISSING_NOTICES + 1))
    fi
  done < <(find "$REPO_ROOT/LICENSES" -type f -print0)
  if [[ "$MISSING_NOTICES" -eq 0 ]]; then
    pass "every collected licence notice is in the bundle"
  else
    CHECKS=$((CHECKS + 1))
    FAILURES=$((FAILURES + 1))
  fi
  for required_notice in \
    zstd-sys-LICENSE.BSD-3-Clause \
    fallback-fonts-Hack-Regular.txt \
    fallback-fonts-OFL.txt \
    fallback-fonts-UFL.txt \
    fallback-fonts-emoji-icon-font-mit-license.txt; do
    if [[ -f "$RESOURCES_DIR/LICENSES/$required_notice" ]]; then
      pass "the bundle ships $required_notice"
    else
      fail "the bundle is missing $required_notice"
    fi
  done
else
  fail "the repository has no LICENSES/INDEX.md, so the dependency notices cannot be checked"
fi

section "Launch"

if [[ "${1:-}" == "--launch" ]]; then
  if ! SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/u11-verify.XXXXXX")"; then
    fail "cannot create a scratch data directory for the launch check"
    SCRATCH=""
  fi
  if [[ -n "$SCRATCH" ]]; then
    cleanup_scratch() { rm -r "$SCRATCH"; }
    trap cleanup_scratch EXIT
  fi
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
