GOAL
Build the reader: the continuously scrolling text surface with the typography rules, the
paragraph and verse-per-line modes, the poetry and heading handling, the three themes, and
the verse selection that the study features hang off.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/reader.rs        the scrolling surface and the layout
  crates/app/src/typography.rs    the text shaping and the small caps and superscripts
  crates/app/src/selection.rs     the verse selection model
  crates/app/src/reader_tests.rs  the tests
  crates/app/Cargo.toml           add the dependencies you need
Do not touch crates/core. Another unit owns it, and you use the core crate as a library.
Do not touch theme.rs, app.rs, cli.rs, or examples/shot.rs. Another unit owns the shell.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "The reader" and "Search in the
app" before you start.

You build on the model that the core crate defines. Read the merged core code for the
exact names. A translation is one text string plus flat arrays for verses, marks, and
spans. Each verse carries its number, the byte range of its text, and the ranges into the
marks and spans arrays. A mark is a structural break: paragraph, poetry line, poetry
indent level, stanza break, heading, Psalm superscription. A span is a character style:
words of Christ, translator additions, the divine name, Selah.

Text is laid out into a continuous scroll across the whole Bible. Only the chapters on
screen are laid out, and the layout of each chapter is cached. The scroll position is an
anchor of a chapter plus an offset inside it, not a position in one enormous document, so
scrolling from Genesis to Revelation costs the same per frame as scrolling inside one
psalm. A window resize or a font change re-anchors to the verse being read.

Two paragraph modes exist. In paragraph mode the verses flow inside a paragraph and the
first line is indented, except right after a heading. In verse-per-line mode every verse
gets its own line with the number in the margin. Poetry keeps its line structure in both
modes, with a level indent and a hanging indent for wrapped lines, a stanza break adding
space, and Selah right-aligned.

The typesetting rules. The measure is capped at about 34 em, so roughly 65 to 75
characters per line. The line height is 1.62. The verse number is a superscript followed
by a non-breaking space, so a number never sits at the end of a line away from its verse.
The divine name is set in small capitals, which the toolkit has no toggle for, so it is
set in capitals at 78 percent size with the line height adjusted to match the baseline.
Words of Christ can be red. Translator additions can be italic. Psalm superscriptions are
muted italic, and section headings are semibold.

The performance budget: scroll under 1 ms of CPU per frame, and a re-layout of one
chapter must not happen on every frame.

The verse selection model. Clicking a verse selects it. Shift-clicking extends the
selection to a range, and the J and K keys move the selection. The selection is a pair of
verse references, and it reports its start and its end.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The layout test asserts that laying out one chapter produces a layout whose height is
  positive and whose lines carry the verse text of that chapter.
- The layout test asserts that the verse-per-line mode produces more lines than the
  paragraph mode for a chapter with several verses, and that the paragraph mode produces
  fewer.
- The layout test asserts that a poetry line is indented more than a prose line, and that
  a stanza break adds vertical space between the two stanzas.
- The layout test asserts that a heading is rendered with a different style from the body.
- The layout test asserts that the small-caps treatment of the divine name produces the
  same baseline as the surrounding body text.
- The layout test asserts that laying out the same chapter twice returns the cached
  layout and does not recompute, by counting the layout calls.
- The selection test asserts that a click sets the selection, that a shift-click extends
  it to a range, and that the J and K keys move it by one verse and stop at the chapter
  boundary.
- The selection test asserts that an empty selection is reported as empty.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header: // SPDX-License-Identifier: MIT
  and a copyright line for madearga.
- The tests use synthetic text only. No Bible text, public domain or not.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

All three must pass with no warnings and no ignored tests. If clippy complains, fix the
code, do not silence the lint.

Then prove the reader renders, and produce a PNG:

    cargo run --release --example shot -- /tmp/shot-reader.png 1280x800

The harness must show a chapter of text. Report the file path and its size, and confirm
the PNG is not blank by checking that it has more than two distinct colours.

Then measure the scroll and report the number:

    cargo test --release -- --nocapture scroll_benchmark

The test must lay out a synthetic full Bible, scroll through it, and print the microseconds
of CPU per frame. It must be under 1000 microseconds. Paste the output.

TIMEBOX
Six hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not.
Do not add a dependency that is not needed. The layout needs no new crate.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, the PNG path and size, and the scroll
  benchmark number in microseconds per frame.
- The data shape you chose for the layout and the selection, in a few sentences. Say how
  the cache is keyed and how a re-anchor works.
- The public API, as a short list of the type and function names.
- Deviations from this brief, and why.
- Suggested follow-ups.
