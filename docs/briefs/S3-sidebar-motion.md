GOAL
Animate the sidebar's open and close as a slide instead of a one-frame flip,
driven by a critically damped spring, honoring the system's reduce-motion
request.

SCOPE
This unit may edit:
  crates/app/src/motion.rs        the spring, new in this unit
  crates/app/src/lib.rs           the module declaration
  crates/app/src/app.rs           the per-frame advance and the animated panel
  crates/app/src/sidebar.rs       the panel width constant
  crates/app/Cargo.toml           objc2-app-kit, macOS only
  crates/app/tests/sidebar.rs     the slide test

CONTEXT
The panel is animated as one fraction between closed and open. The fraction
follows a critically damped spring evaluated in closed form, so the motion is
smooth at any frame rate, can be retargeted mid-slide from wherever the panel is,
and settles within about a quarter of a second at the chosen stiffness. Each
advance is bounded, so a stalled frame cannot throw the panel across the screen,
and the spring snaps on arrival. The panel's width is the fraction times its full
width, while the content keeps that full width and is translated with the panel,
so the column never re-wraps during the slide; only the reading surface reflows
against the moving edge, as it already did on a width change. Reduce motion is
read from NSWorkspace through objc2-app-kit, which was already in the shipped
binary's closure, so the licence index does not move. The harness photographs
panels with no frames to wait for, so a demo view snaps the spring to its target,
and the offscreen tests raise the harness step budget because one run carries the
whole slide. The frame step is the larger of the toolkit's stable and predicted
deltas, because the test harness never advances the wall clock.

ACCEPTANCE
- The spring settles on its target without overshooting and is provably
  continuous through a retarget.
- One stalled call advances by the bounded step, never the whole slide.
- A harness test reads the fraction mid-slide strictly between zero and one and
  settled above 0.999 after the slide, unless the machine asked for reduced
  motion, in which case the target arrives at once.
- cargo fmt, clippy, and the workspace tests pass, and the licence check passes
  unchanged.
