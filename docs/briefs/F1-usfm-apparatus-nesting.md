GOAL
Fix one bug in the USFM importer: a starred custom marker inside footnote or cross-reference
apparatus closes the apparatus run early, leaking footnote text into the verse.

SCOPE
You may edit only:
  crates/core/src/import/usfm.rs    the fix
  crates/core/tests/import_usfm.rs  the regression test
Do not touch any other file.

CONTEXT
The program is a Rust Bible reader. The USFM importer is in crates/core/src/import/usfm.rs and is
already merged and tested. Read it before editing.

The bug. A USFM footnote opens an apparatus run the importer reads with a skip destination, so the
footnote's content never reaches the verse text. Some footnotes contain nested custom character
markers, which are themselves starred: for example the World English Bible's Genesis 1:1 footnote

    \f + \fr 1:1 \ft The Hebrew word rendered "God" is "\+wh אֱלֹהִ֑ים\+wh*" (Elohim).\f*

The closing `\+wh*` is a starred marker inside the skip destination. The current code treats any
starred marker as closing the open style run, which ends the whole footnote run early. Everything
after it, ` (Elohim).`, leaks into the verse as text, so Genesis 1:1 imports as

    In the beginning, God" (Elohim). created the heavens and the earth.

The rule that should hold: a starred marker that does not correspond to the marker that opened the
apparatus run must not close that run. Only the apparatus marker that opened the run closes it,
which for a footnote is `\f*` and for a cross-reference is `\x*`.

ACCEPTANCE
- The regression test feeds the exact WEB Genesis 1:1 marker sequence above through the importer and
  asserts the verse text is exactly the clean text with no footnote fragment, and that no style span
  from inside the footnote appears in the verse.
- The regression test also covers a nested starred marker inside a cross-reference apparatus.
- The regression test covers a footnote with no nested markers, which already worked, so the fix
  cannot regress it.
- A starred marker that opens and closes on the same line outside any apparatus still behaves as
  before.
- cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings, and
  cargo test --workspace all pass with no warnings and no ignored tests.
- The fix is the smallest change that makes the rule hold. No new abstraction.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

TIMEBOX
Two hours.

FORBIDDEN
Do not touch any file outside SCOPE. Do not copy from any reference implementation. No Bible text
in any test beyond the marker shapes quoted in this brief, which are apparatus structure, not
Scripture. No #[ignore], no silenced lints, no force-push, no rebase, no merge, no gt.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- What the fix changed, in three sentences at most.
- The verify output.

## Bug 2, added after dispatch

A marker's delimiter space leaks into the verse when the marker opens directly after
non-whitespace. Darby Revelation 22:21 raw text is

    ...Christ|strong="G2962"\w* [\w be|strong="G3956"\w*] \w with...

The single space after `\w` is the marker delimiter, not content, but the accumulator keeps it,
so the verse reads `[ be]` instead of `[be]`. The same delimiter space is harmless where real
whitespace already precedes the marker.

The rule: when a marker opens, which is not a starred close, and the next text run starts with
exactly one whitespace character, drop that one character. A segment between two markers keeps
its real space, so `\w* \w the` still reads correctly.

ACCEPTANCE for bug 2: a regression test feeds the Darby marker shape above through the importer
and asserts the verse text contains `[be]` and not `[ be]`, and a second case asserts a real
space between two markers survives. Everything else in this brief is unchanged.
