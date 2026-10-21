GOAL
Persist the reader's paragraph mode, which is the one appearance setting the store cannot hold
yet, and expose the reading preferences in a real Settings window so the macOS menu item does
something.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/core/src/study/record.rs   the new Settings field with a safe default
  crates/core/src/study/export.rs    the field in the JSON export and import
  crates/core/tests/study_store.rs  the default and round-trip test
  crates/app/src/settings.rs        the settings window
  crates/app/src/app.rs             opening the settings window, applying it, the menu action
  crates/app/src/macos_menu.rs      the menu item that opens it, if it needs one
  crates/app/src/lib.rs             module declaration
  crates/app/tests/settings.rs      the tests
Do not touch xtask, data, assets, docs, scripts, THIRD_PARTY.md, LICENSES, reader.rs, parallel.rs,
sidebar.rs, or the importers.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md, section "The reader" (the paragraph modes) and "macOS integration". Read them,
then read on your branch: crates/core/src/study/record.rs (the Settings struct and its Default),
crates/core/src/study/export.rs (how settings are written and read), crates/app/src/app.rs (the
StartUp struct, the appearance actions, save_appearance, and the ShotView enum), and
crates/app/src/macos_menu.rs (the menu spec and the Settings item that currently reports that
the preferences live in the View menu).

The store change. Settings gains one field, paragraph_mode: bool, meaning the reader's
paragraph mode is on. The default is true, which is what the reader already does, so an existing
database without the field keeps behaving exactly as before. The export document gains the same
field, and importing a document that does not carry it must fall back to the default rather than
fail. A document with an unknown future field must still be accepted, as it is today.

The settings window. A modal panel with a small form:
- The theme, as three buttons or a combo, light, sepia, dark.
- The text size, a slider or plus and minus buttons.
- The paragraph mode, a checkbox labelled with the plain wording, verses flow inside a paragraph,
  or one verse per line.
- The line spacing, a slider.
- The line length, a slider.
- The justify, verse numbers, section headings, and words of Christ in red checkboxes.
- A close button.
Every control writes through the same save path the keyboard actions use, so the value is stored
the same way and survives a restart the same way. The window shows the current values when it
opens.

The menu item. The Settings item in the Bible menu opens this window instead of reporting that
the preferences live elsewhere. The Escape key closes it, the same as it closes the other panels.

ACCEPTANCE
- cargo build succeeds with no warnings on macOS.
- cargo test passes, and the tests cover the cases below.
- A core test asserts a fresh Settings has paragraph_mode true, and that a settings row written
  before the field existed still loads with the default.
- A core test asserts the exported document carries the field and that importing a document
  without it keeps the default.
- A settings window test opens the window through the real app, changes the paragraph mode and
  the text size, closes it, and asserts the store holds both values.
- A settings window test asserts the window shows the current theme, size, and paragraph mode
  when it opens.
- The Escape key closes the settings window.
- The paragraph mode actually changes the reader: a test asserts the laid-out lines differ
  between paragraph mode and verse-per-line mode for the same chapter.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.
- The tests use temp data folders and never touch the reader's real study data.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo run --release -p bible-app -- read "john 3:16"
Then render the settings window through the shot harness, adding ShotView::Settings and
SHOT_DEMO=settings if the harness does not have them yet:
    SHOT_DEMO=settings cargo run --release --example shot -- /tmp/shot-settings.png 1280x800
and confirm the PNG differs from the plain reader render.

Do not run the release benchmark suite; the coordinator measures budgets on final main.

TIMEBOX
Five hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the simplest option that matches SPEC.md and record it under
BLOCKED-ITEMS in the report.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes, no new
dependencies. No force-push, rebase, or merging. No edits outside SCOPE.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, the settings window PNG path, how
the new Settings field stays compatible with an older database, limitations, BLOCKED-ITEMS.
