GOAL
Make the sidebar reachable the way every other panel is: a keyboard shortcut, a
menu item, a palette command, and a remembered open state.

SCOPE
This unit may edit:
  crates/core/src/study/record.rs      the sidebar_open setting
  crates/core/src/study/export.rs      the settings export shape
  crates/app/src/shortcuts.rs          the action and the command-B row
  crates/app/src/macos_menu.rs         the View menu item and the menu id
  crates/app/src/app.rs                the handler, the restore, the save
  crates/app/tests/menu.rs             the menu parity expectations
  crates/app/tests/navigation.rs       the command-B test
  crates/app/tests/sidebar.rs          the restore test
  crates/app/tests/cli.rs              the export literal

CONTEXT
Command-B belonged to the floating book and chapter picker. It now toggles the
sidebar, which is what a reader of the books grid expects the key to mean; the
picker keeps its menu item and its toolbar button and loses the key, so the menu
parity test gains it as an exempt menu-only command. ToggleSidebar joins the
shortcut table, which is what also lists it in the jump palette, and the View
menu gains Toggle Sidebar as its first item. The open state is stored as
sidebar_open on the saved settings row, written through the same save path as the
appearance and read back only when the window restores the reader's appearance,
so harness shots stay deterministic. A settings row or export written before the
field existed reads as closed, and an older build reading a newer export ignores
the field, which is why the export version does not move.

ACCEPTANCE
- command-B opens and closes the sidebar through the same action path the menu
  uses.
- The View menu and the palette list the toggle, and the menu parity tests pass.
- Toggling, dropping the app, and reopening on the same data folder restores the
  open sidebar with the current chapter revealed.
- An export written by this build carries sidebar_open and an old fixture fails
  the CLI export test until it gains the field, proving the format moved.
