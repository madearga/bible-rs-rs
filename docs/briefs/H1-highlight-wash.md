GOAL
Paint a highlight in the reading surface as a wash of its colour behind the
verse's own words, make the selection and flash fill the same span, stop the
margin marks from repeating down a wrapped verse, and land a jump on the verse
it names.

SCOPE
This unit may edit:
  crates/app/src/reader.rs        the verse attribution on runs and the landing
  crates/app/src/reader_tests.rs  the attribution and landing tests
  crates/app/src/sidebar.rs       the swatch and the theme-aware wash
  crates/app/src/app.rs           the paint loop and the jump sites
  crates/app/tests/render.rs      the pixel proof

CONTEXT
The store held the highlight and the sidebar listed it, but the verse itself
stayed plain, which made the reading surface the only place a mark was
invisible. The layout's run merger joined every prose run on a line into one,
so the first change teaches the layout which verse each run belongs to and
makes the verse take part in the merge, because two verses flowing in one
paragraph share a role and an edge. The paint loop then groups each line's runs
into contiguous spans of one verse and paints the wash behind exactly the
highlighted verse's span, which is also the span the selection, the flash, the
hover stroke, and the press mapping use. The margin marks paint once per verse,
on its first line, keyed by a per-frame set. The wash is the fixed swatch
blended over the paper, stronger on the dark theme, which dims a translucent
colour twice over. Landing: set_position keeps the verse of the reference it is
given and the next frame places the offset just under the verse's top, found
through the run attribution because a verse continuing a paragraph opens no
line of its own. top_of_verse searches the runs for the same reason.

ACCEPTANCE
- A paragraph-mode layout labels every prose run with its verse and never
  merges across a verse boundary.
- The offscreen render shows wash pixels behind the highlighted verse and none
  worth naming behind its unhighlighted neighbour.
- A jump to a verse puts that verse within two line heights of the viewport
  top.
- cargo fmt, clippy with -D warnings, and the workspace tests pass.
