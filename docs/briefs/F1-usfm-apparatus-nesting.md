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
