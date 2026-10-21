GOAL
Add the native macOS menu bar through muda, routed through the one shortcut action table, with a
parity test that keeps the menu and the keyboard from drifting.

SCOPE
You may write only inside the new repository. Create or edit:
  crates/app/src/macos_menu.rs   the native menu bar (cfg target_os = "macos")
  crates/app/src/main.rs         the menu bootstrap on launch
  crates/app/src/app.rs          minimal wiring only if a menu action needs a public entry
  crates/app/src/shortcuts.rs    adding entries only if a menu item needs one
  crates/app/Cargo.toml          muda dependency, gated to target_os = "macos"
  crates/app/tests/menu.rs       the parity tests
  crates/app/src/lib.rs          module declaration
Do not touch crates/core, xtask, data, assets, docs, scripts. Do not build the sidebar, the
parallel view, or any other feature.

CONTEXT
The program is a clean-room rewrite of a fast native Bible reader for macOS. The requirements are
in docs/SPEC.md, section "macOS integration". Read it, then read these files on your branch:
crates/app/src/shortcuts.rs (the Action enum and the SHORTCUTS table), crates/app/src/main.rs (the
launch path), crates/app/src/app.rs (how actions are performed).

The rule that makes this unit small: the menu does not get its own action handling. Every menu
item maps to an existing Action from the shortcuts table, and the menu fires that action through
the same code path the keyboard uses. There is deliberately no Edit menu, because the system's
predefined copy and paste items would swallow command-C and command-V before the text fields see
them.

Menu structure:
- Bible: About, Settings separator, Quit.
- File: Export Markdown, Export JSON.
- Go: Jump (command-K), Search (command-F), Book (command-B), separator, Back, Forward, separator,
  Previous Chapter, Next Chapter.
- View: cycle theme, toggle red letters, text larger, text smaller, text default, separator,
  toggle verse numbers, toggle headings.
- Translations: the seven bundled ids, each switching the translation, with command-1 through
  command-7 equivalents.
- Window: the standard zoom and minimize items are acceptable via muda's predefined items.
- Help: shortcut overlay.

Every item with a key equivalent must match the SHORTCUTS table for that action. An item without
a table entry gets no key equivalent rather than inventing one.

WIRING
muda delivers menu events on its own thread. Bridge them into the app with a std::sync::mpsc
channel that main.rs drains on the next frame, or through the existing action queue if app.rs
already has one you can reuse. Keep the bridge under about 40 lines. The menu must be built before
the window shows, and it must work when the app is launched as a plain binary (no bundle), because
that is how developers run it.

ACCEPTANCE
- cargo build succeeds with no warnings on macOS.
- cargo test passes, and the tests cover the cases below.
- The parity test asserts every Action that carries a key equivalent in SHORTCUTS has exactly one
  menu item, and every menu item maps to an Action the keyboard handler performs. No orphans
  either direction.
- The menu structure test asserts the menus exist with the items listed above, and that no menu
  named Edit exists.
- The key equivalent test asserts each menu accelerator equals the SHORTCUTS combo for the same
  action.
- A test asserts the mpsc bridge converts a menu event id into the right Action without a window.
- The shot harness still renders after the menu bootstrap, proving launch is not broken.
- Every public function has a doc comment that says what it is for, not what it does.
- Every file starts with the SPDX header naming madearga.
- The tests must not require a display; they test the menu structure objects and the bridge, not
  native AppKit interaction. State that limitation in the report.

VERIFY
Run exactly these and paste the outputs into your report:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo run --release -p bible-app -- read "john 3:16"
Then launch the real binary for five seconds, trigger one menu action by opening the menu with
AppleScript System Events (menu bar process Bible, click menu item), and report whether the
on-screen content changed. If AppleScript cannot drive it, say so plainly and prove launch with
    timeout 5 cargo run --release -p bible-app 2>&1 | head -5
plus the shot example rendering one frame.

TIMEBOX
Five hours of work. Work autonomously; the operator is away. Do not ask questions. If a decision
is genuinely a product choice, take the simplest option that matches SPEC.md and record it under
BLOCKED-ITEMS in the report.

FORBIDDEN
No source or naming from /Users/madearga/bible-rs-reference. No SPDX headers other than MIT with
madearga. No weakening tests, no #[ignore], no clippy suppressions, no budget changes. No
dependencies beyond muda. No force-push, rebase, or merging. No edits outside SCOPE.

REPORT
Reply with: status, branch, head SHA, PR URL, verify outputs, the menu tree as built, AppleScript
result or its failure, limitations, BLOCKED-ITEMS.
