GOAL
Build the application shell: the egui app that opens a window, the typography and the three
themes, the offscreen render harness that the GUI is verified through, and the CLI that the
core crate is verified through.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/Cargo.toml            the app crate manifest
  crates/app/src/main.rs           the binary entry point
  crates/app/src/lib.rs            the crate root
  crates/app/src/theme.rs          the palettes and the font setup
  crates/app/src/app.rs            the app state and the frame
  crates/app/src/cli.rs            the command line interface
  crates/app/examples/shot.rs      the offscreen render harness
  crates/app/tests/*.rs            the tests
  assets/fonts/                    the font files
  assets/                          the other assets
Do not touch crates/core or xtask. Another unit owns those, and you use the core crate as a
library.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The
requirements are in docs/SPEC.md in the repository. Read "The reader", "Typography" as part
of "The reader", "macOS integration", "The offline render harness", "The CLI", and "The
performance budget" before you start.

The app is egui 0.36 through eframe, with the wgpu and glow renderers and the x11 and
wayland backends. The renderer is wgpu on Metal on macOS and OpenGL elsewhere, and the
choice is made by measurement. An environment variable overrides it. On macOS the app
asks for the low-power GPU.

The typography. The body text is a serif face and the interface and the verse numbers use
a sans face. The body face must have roman and italic, and a variable weight and optical
size if it can. Literata is the intended body face and Inter is the intended interface
face, both under the SIL Open Font License. Fetch them from their official sources and
commit them under assets/fonts/. If you cannot fetch them, use a system serif and sans as
a fallback and say so plainly in the report, and make the font files a single place in the
code so swapping them later is one edit.

The three themes are light, sepia, and dark, chosen for long reading. Warm paper rather
than pure white, and not pure black.

The frame paints a reading surface, the current reference, and the theme.

The offline render harness is an example binary that drives the real app through the
toolkit's test harness and writes a PNG, so it works on a build machine with no display.
It takes the output path and the size, and the environment selects what to show. It must
support showing a given reference, clicking at a point, hovering at a point, a pause
between frames, and a filmstrip mode that writes the next several frames. The harness is
the control skill for the GUI. A GUI change is not verified without a PNG that came out of
it.

The CLI needs these verbs: read, search, import, translations, export, import-study. The
CLI is the control skill for the core crate.

The performance budget: first frame under 50 ms after the window exists, full-Bible search
under 10 ms, scroll under 1 ms of CPU per frame, load and decode one translation under
50 ms.

ACCEPTANCE
- cargo build succeeds with no warnings.
- cargo test passes, and the tests cover the cases below.
- The render harness test runs the harness, writes a PNG, and asserts the file exists, is
  larger than a thousand bytes, and starts with the PNG signature bytes.
- The render harness test asserts that the harness honours a size argument and produces a
  PNG of that size, by reading the PNG header.
- The render harness test asserts that the harness can render each of the three themes and
  that the three PNGs differ from each other.
- The CLI test asserts that `read` with a reference prints the verse and exits zero, that
  `read` with a bad reference exits non-zero, and that `translations` lists the bundled
  translations.
- The CLI test asserts that `search` prints matches for a query and exits zero.
- The CLI test asserts that an unknown verb prints usage and exits non-zero.
- A test asserts that the font files are present in assets/fonts and that the theme
  module can build a font set from them.
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

Then prove the harness works and produce three PNGs, one per theme:

    cargo run --release --example shot -- /tmp/shot-light.png 1280x800
    cargo run --release --example shot -- /tmp/shot-sepia.png 1280x800
    cargo run --release --example shot -- /tmp/shot-dark.png 1280x800

On a headless Linux host, run them under a virtual display:

    Xvfb :99 -screen 0 1280x800x24 &
    export DISPLAY=:99

Each must produce a PNG. Report the size of each file and confirm the three differ. Attach
the file paths.

Then measure the startup and report the number:

    cargo run --release --example startup_probe

The probe must print the milliseconds from the start of the process to the first frame
with the window up, and it must be under 50 ms on this host. Report the number and the
host you measured on.

TIMEBOX
Six hours. If you run out, return what you have with the tests that pass, and say
exactly what is missing. Do not leave the tree in a state that does not compile.

FORBIDDEN
Do not copy source from any reference implementation. Write your own.
Do not name the reference author anywhere.
Do not bundle or commit any Bible text, public domain or not.
Do not add a dependency that is not needed beyond the core crate. The app crate needs
eframe, anyhow, serde, serde_json, and an image encoder for the PNG.
Do not add a feature flag, a config file, or an abstraction layer that no caller needs.
Do not use #[ignore], do not delete a failing test, do not loosen a clippy lint.
Do not run git push --force, do not rebase, do not use gt, do not merge.
Do not modify any file outside SCOPE.

REPORT
Reply with these lines.
- Status: done, partial, or blocked.
- Branch name and the head SHA.
- The PR number and its URL, opened against main.
- Verdict: what the three verify commands printed, the three PNG paths and their sizes,
  and the startup number in ms with the host named.
- The fonts you used, where they came from, and their licences.
- The data shape you chose for the app state, in a few sentences.
- The public API of the harness and the CLI, as a short list.
- Deviations from this brief, and why.
- Suggested follow-ups.
