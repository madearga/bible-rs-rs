GOAL
Build the navigation and the study interface: the jump palette, the book and chapter picker,
the back and forward history, the search panel, and the verse menu that offers highlight,
note, bookmark, copy, and compare.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/palette.rs        the jump palette
  crates/app/src/palette_test.rs   its tests
  crates/app/src/book_picker.rs    the book and chapter picker
  crates/app/src/history.rs        the back and forward history
  crates/app/src/search_panel.rs   the search panel
  crates/app/src/verse_menu.rs     the verse menu
  crates/app/src/shortcuts.rs      the keyboard shortcut table
  crates/app/Cargo.toml            add the dependencies you need
Do not touch crates/core. Another unit owns it, and you use the core crate as a library.
Do not touch reader.rs, typography.rs, selection.rs, theme.rs, app.rs, cli.rs, or
examples/shot.rs. Another unit owns the shell and the reader.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "Navigation", "Search in the app",
"Study in the app", and "Keyboard" before you start.

You build on the core crate. Read the merged core code for the exact names, in
particular the reference parser and the search engine.

The jump palette opens on the keyboard shortcut for command-K. It accepts a reference with
a fuzzy book name and tolerant of typos, so `jn 3:16`, `Rom 8`, `1co13`, and even
`revalation 21` all resolve. It also offers recent places, the installed translations, and
the app's commands. The fuzzy matching is a scored search over the book names, the
abbreviations, and the alternate spellings, where a typo costs less than a missing
character.

The book and chapter picker is reachable from the toolbar and from a sidebar. Back and
forward history work, and so do previous and next chapter.

The search panel opens on the keyboard shortcut for command-F. Results update as the user
types. The arrow keys move through the results and return jumps to the verse, which
flashes so the reader can see where the match is. The matching words are highlighted in
the results.

The verse menu offers highlight in six colours, a note, a bookmark, copy, copy with the
reference, and compare across translations. It is reachable from the context menu.

The keyboard shortcuts are listed in the spec. Put them in one table in one file so the
menu bar, the shortcut overlay, and the key handling all read the same source. That is
important: a shortcut that exists in the menu but not in the handler is a bug.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The palette test asserts that `jn 3:16` resolves to John 3 verse 16, that `1co13`
  resolves to 1 Corinthians 13, that `revalation 21` resolves to Revelation 21, that
  `ps 23` resolves to Psalm 23, and that `Rom 8` resolves to Romans 8.
- The palette test asserts that a fuzzy match ranks an exact name above a fuzzy one.
- The palette test asserts that an unknown input resolves to nothing rather than to an
  arbitrary book.
- The history test asserts that going back returns to the previous place, that going
  forward returns again, and that going forward with nothing ahead does nothing.
- The history test asserts that the history is capped at a fixed depth and drops the
  oldest entry.
- The search panel test asserts that a query returns results, that the results carry the
  matching word ranges so they can be highlighted, and that the selection moves with the
  arrow keys.
- The shortcut table test asserts that every shortcut in the table has a unique key
  combination, and that the number of entries is at least the number listed in the spec.
- The verse menu test asserts that choosing a highlight colour calls the study store, that
  a note can be set, and that a bookmark can be toggled.
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

Then prove the palette renders, and produce a PNG:

    cargo run --release --example shot -- /tmp/shot-palette.png 1280x800

The harness must show the open palette with its input focused. Report the file path and
its size.

TIMEBOX
Six hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not.
Do not add a dependency that is not needed.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, and the PNG path and size.
- The data shape you chose for the palette, the history, and the shortcut table, in a few
  sentences. Say how the fuzzy match is scored.
- The public API, as a short list of the type and function names.
- Deviations from this brief, and why.
- Suggested follow-ups.
