GOAL
Turn the study sidebar into the column the reader navigates from: move it to the
left of the window and give it a Books section that reaches any chapter in two
clicks, with the study list kept below it.

SCOPE
This unit may edit:
  crates/app/src/sidebar.rs       the Books section, its state, and the chapter activity set
  crates/app/src/app.rs           the panel side, the reveal on open, the caches
  crates/app/tests/sidebar.rs     the integration tests

CONTEXT
The sidebar was a right-hand list of study marks, 240 points wide, reachable only
from the toolbar button, with no navigation of its own. The Books section follows
the canonical grid: two collapsible testament sections, one book row per book, and
the one expanded book showing its chapters as a grid of round chips, seven per row.
The current chapter is filled with the accent, and a chapter that carries any study
mark gets a dot beneath its chip, computed from one store snapshot into a
`(book, chapter)` set. Opening the sidebar reveals the place the reader is in:
it clears the filter, expands the current book, and opens only its testament.
While the sidebar stays open, a navigation into another book carries the expanded
grid along. A filter field at the top narrows the books by name or abbreviation,
and letters typed into it must not reach the shortcut table, which also stops the
plain verse keys from firing while a note is being written. The disclosure markers
are ASCII because the bundled interface face has no triangle glyphs.

ACCEPTANCE
- Opening the sidebar shows the current book's chapter grid and no books from the
  other testament.
- Clicking a chapter chip navigates to that chapter and records the history.
- Chapters carrying marks are dotted, computed from the store snapshot.
- The book filter narrows the list and swallows the letters typed into it.
- cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings,
  and cargo test --workspace all pass, and the offscreen sidebar shot shows the
  new section.
