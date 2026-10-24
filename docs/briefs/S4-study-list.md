GOAL
Make the study list worth reading as a list: give every row its verse text, give
highlight rows their colour, let the reader narrow the highlights to one colour,
and let a row's context menu remove the mark it came from.

SCOPE
This unit may edit:
  crates/app/src/sidebar.rs       the rows, the swatches, the pills, the pick
  crates/app/src/app.rs           the pick handling and the removal path
  crates/app/src/verse_menu.rs    the ClearNote action
  crates/app/tests/sidebar.rs     the integration tests

CONTEXT
A row was a plain "reference, kind, label" button. It now carries a one-line
snippet of the verse text from the translation the reader is in, which is why
rows_from_snapshot takes the translation with the snapshot. Highlight rows carry
their colour, painted as a fixed swatch and a three-point bar at the row's left
edge; the six swatches are fixed RGB because the colour is the mark's identity,
not a theme colour. A pill row above the highlights group holds All and one
swatch per colour, and the chosen colour narrows the group. Every row opens a
context menu naming its removal: Remove highlight, Delete note, or Remove
bookmark. The menu reports it through a Pick the app acts on: a jump navigates as
before, and a removal aims the verse menu at the row's verse first, so the
removal lands on the row rather than on the selection, then takes the same apply
path that refreshes the rows and reports the outcome. Removing a note goes
through a new ClearNote menu action, because the note action reports "saved" even
when the blank draft deletes. The empty state names the keys that create each
kind of mark. The rows stay in canonical order because the store records no
timestamps to order them by; reordering waits on a schema change.

ACCEPTANCE
- Rows carry the verse text, and highlight rows carry their colour.
- The pill row narrows the highlights to one colour and All restores them.
- Right-clicking a bookmark row and choosing Remove bookmark empties the mark
  from the store and the list.
- The note removal reports removed, not saved.
- cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings,
  and cargo test --workspace all pass, and the offscreen shot shows the bars,
  the pills, and the snippets.
