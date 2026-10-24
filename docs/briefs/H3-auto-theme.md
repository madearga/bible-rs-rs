GOAL
Add a fourth theme choice, auto, which paints whichever of light and dark the
system is showing and switches when the system switches.

SCOPE
This unit may edit:
  crates/core/src/study/record.rs     the Auto variant
  crates/app/src/theme.rs             the choice, its resolution, the cycle
  crates/app/src/settings.rs          the button label
  crates/app/src/app.rs               the per-frame resolution
  crates/core/tests/study_store.rs    the round-trip test
  crates/app/tests/theme.rs           the resolution test

CONTEXT
The theme was a manual choice among three. Auto resolves against the system
appearance on every frame through the toolkit's system theme, which the
windowing layer feeds and updates, so a system switch repaints without the
reader touching anything; the stored choice stays auto rather than being
resolved at save time. The cycle runs light, sepia, dark, auto, and the
settings window lists the four. A settings row or an export written before the
choice existed names a concrete theme and reads exactly as before, and an older
build reading a newer row falls back to its defaults rather than failing,
which is the store's existing behaviour for a row it cannot parse. The
concrete themes resolve to themselves, so nothing changes for a reader who
picked one.

ACCEPTANCE
- Auto resolves to light without the system's dark mode and to dark with it,
  and the concrete themes resolve to themselves.
- The automatic choice survives a store round trip as itself.
- The cycle visits all four choices and returns to its start.
- cargo fmt, clippy with -D warnings, and the workspace tests pass.
