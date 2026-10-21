GOAL
Complete the app's remaining spec surfaces: the native macOS menu bar, the study sidebar with
margin indicators, the preference persistence, and the parallel translations view.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/macos_menu.rs   the native menu bar (macOS only)
  crates/app/src/sidebar.rs      the study sidebar and the margin indicators
  crates/app/src/parallel.rs     the parallel translations view
  crates/app/src/app.rs          the wiring of all three
  crates/app/src/lib.rs          module declarations
  crates/app/src/main.rs         renderer choice and menu bootstrap
  crates/app/Cargo.toml          the macOS dependencies you need
  crates/app/tests/*.rs          the tests
Do not touch crates/core, xtask, or data.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements
are in docs/SPEC.md. Read "macOS integration", "Study in the app", "Translations" (the parallel
and compare parts), and "The reader".

Everything else is already merged. Read the code on your branch before writing: app.rs
(ReaderApp, the frame, the view routing), shortcuts.rs (the one shortcut table every surface
reads), study store (crates/core/src/study.rs: Store, snapshot, Settings), reader.rs and
selection.rs, palette.rs and verse_menu.rs, bundled.rs and the Library.

The native menu bar. On macOS the app builds a native menu bar through muda, cfg-gated to
target_os = "macos". Every menu item with a key equivalent routes into the same Action enum the
keyboard handler uses, through the one shortcut table; a menu item that exists in the table but
not in the menu, or the reverse, is a bug. There is deliberately no Edit menu, because the
system's predefined copy and paste items would swallow command-C and command-V before the text
fields see them. On other platforms no menu bar is built and the in-app shortcuts overlay stays
the keyboard surface.

The study sidebar. A panel that lists every highlight, note, and bookmark from the study store,
grouped by kind, each row jumping to its reference when chosen. In the reader, a verse carrying
a note or a bookmark shows a small indicator in the margin, so the reader can see study data
while reading.

The preference persistence. The theme, the text size, the paragraph mode, and the other
appearance choices persist through the study store's settings and restore on launch, the same
way the reading position already does.

The parallel view. A reading mode showing two or more translations in side-by-side columns
aligned verse by verse on the canonical grid, scrolled together. The compare rows in the verse
menu already read the same data for one verse; the parallel view is the reading-surface
version. Switching into and out of parallel mode is a command in the shortcut table.

ACCEPTANCE
- cargo build succeeds with no warnings on macOS.
- cargo test passes, and the tests cover the cases below.
- A test asserts every Action in the shortcut table that the spec marks as a menu action has a
  menu item, and every menu item maps to an Action the keyboard handler understands, so the two
  surfaces cannot drift.
- A test asserts no Edit menu exists in the built menu structure.
- A test asserts the sidebar lists a highlight, a note, and a bookmark from a store snapshot,
  and that choosing a row produces its reference.
- A test asserts a verse with a note reports a margin indicator and a verse without one reports
  none.
- A test asserting the parallel view lays two translations out with the same verse numbers
  aligned row for row on the canonical grid, and that scrolling moves both columns together.
- A test asserting the theme and text size round-trip through the store settings and restore.
- The shot harness renders the sidebar open, the parallel view, and the menu where the platform
  allows; the PNGs differ from the plain reader render.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.

VERIFY
Run exactly these and paste the output into your report:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    SHOT_DEMO=sidebar cargo run --release --example shot -- /tmp/shot-sidebar.png 1280x800
    SHOT_DEMO=parallel cargo run --release --example shot -- /tmp/shot-parallel.png 1280x800

TIMEBOX
Six hours.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not commit any new Bible text; the bundled files are already committed.
Do not add a dependency that is not needed. muda and the objc2 crates are the macOS ones this
unit needs; gate them to target_os = "macos".
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: the verify outputs and the PNG paths.
- How the menu and the shortcut table stay in step, in a few sentences.
- Deviations from this brief, and why.
- Suggested follow-ups.
