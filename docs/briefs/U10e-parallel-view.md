GOAL
Add the parallel translations reading view: two or more translations side by side, verse-aligned
on the canonical grid, scrolling together.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/parallel.rs     the parallel view model and layout
  crates/app/src/app.rs          wiring: entering and leaving parallel mode, rendering, scrolling
  crates/app/src/lib.rs          module declaration
  crates/app/tests/parallel.rs   the tests
Do not touch crates/core, xtask, data, assets, docs, scripts, shortcuts.rs, sidebar.rs,
macos_menu.rs. Do not add dependencies. Do not build the native menu or any other feature.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. Requirements are in
docs/SPEC.md under "Translations" (the parallel and compare parts) and "The reader". Read them,
then read on your branch: crates/app/src/reader.rs (the single-translation Reader, its anchor,
layout cache, and Frame), crates/app/src/app.rs (how the reader is driven and painted, the
Startup struct, ShotView), crates/app/src/sidebar.rs (the pattern a panel follows), and
crates/core/src/model.rs (chapter(), verse_text(), the canonical grid).

The design rule: parallel mode reuses the existing per-chapter layout machinery. Do not write a
second layout engine. The view is N readers sharing one scroll anchor: each column lays out the
same chapter in its own translation, and the column with the tallest content sets the scroll
extent. A scroll in any column moves the shared anchor, and every column repaints from it.

Entering and leaving parallel mode:
- The verse menu's existing compare action stays as it is.
- Parallel mode is entered through a toolbar button labeled "Parallel" next to "Study", and left
  through the same button or the Escape key while it is open. Add Action::ToggleParallel to the
  shortcuts table with no key equivalent (record that as a deliberate choice; command-P is
  print-adjacent and confusing).
- The columns shown are the current translation plus the next two installed or bundled ones, in
  bundled order. A combobox per column header lets the reader change that column's translation.

Rendering:
- Each column paints the chapter heading, then verses in order. Verse numbers superscript, red
  letters, small-caps divine name: reuse the existing typography paths. Poetry and paragraph
  marks carry over.
- The selected verse is highlighted in every column, since the grid is shared.
- Clicking a verse in any column selects that verse for the whole view.
- The reader's existing single-translation mode stays the default; parallel mode is a state, not
  a replacement.

ACCEPTANCE
- cargo build succeeds with no warnings on macOS.
- cargo test passes, and the tests cover the cases below.
- The alignment test asserts two translations of the same chapter produce rows that carry the
  same verse numbers in the same order, row for row, on the canonical grid.
- The scroll test asserts moving the shared anchor repaints both columns at the new offset.
- The selection test asserts a click in one column selects the verse for all columns.
- The toggle test asserts entering and leaving parallel mode returns the reader to the exact
  prior single-translation state (same reference, same scroll anchor).
- A test asserts the column count is at least two and at most three, and the translation
  combobox swaps a column's translation without disturbing the others.
- The shot harness gains ShotView::Parallel and SHOT_DEMO=parallel, seeding its own demo marks
  into a temp data dir only when that mode is asked for.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    SHOT_DEMO=parallel cargo run --release --example shot -- /tmp/shot-parallel.png 1280x800
Then render the plain reader at the same reference and confirm the two PNGs differ:
    cargo run --release --example shot -- /tmp/shot-plain.png 1280x800
    python3 -c "import hashlib; a=hashlib.md5(open('/tmp/shot-parallel.png','rb').read()).hexdigest(); b=hashlib.md5(open('/tmp/shot-plain.png','rb').read()).hexdigest(); print(a, b, 'DIFFER' if a!=b else 'SAME')"

Do not run the release benchmark suite; the coordinator measures budgets on final main.

TIMEBOX
Six hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the simplest option that matches SPEC.md and record it under
BLOCKED-ITEMS.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes, no new
dependencies. No force-push, rebase, or merging. No edits outside SCOPE. The user's real study
data is never touched; tests use temp dirs.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, both PNG paths and the md5s, the
data shape you chose for the shared anchor and the columns, limitations, BLOCKED-ITEMS.
