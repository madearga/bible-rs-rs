// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The native macOS menu bar.
//!
//! The menu is described as a plain data tree by [`menu_spec`] and that tree is
//! then handed to AppKit through muda. Keeping the description separate from
//! the platform objects is what lets the parity tests in `tests/menu.rs` check
//! every accelerator against [`crate::shortcuts::SHORTCUTS`] on a build machine
//! with no display.
//!
//! A menu item never owns an action path of its own. Each command item names an
//! [`Action`], muda reports a click by item identifier, and the identifier is
//! mapped back to that action before it joins the same queue the keyboard
//! fills. [`crate::app::ReaderApp`] drains that queue once per frame.
//!
//! There is deliberately no Edit menu. Its predefined cut, copy, and paste
//! items answer command-C and command-V inside AppKit before the focused text
//! field or the reader sees them, which would take the copy shortcut away from
//! the verse and the note editor.

use std::sync::{Mutex, OnceLock, mpsc};

use muda::accelerator::{Accelerator, Code, Modifiers};

use crate::shortcuts::{Action, Combo, Key, combo_for};

/// One entry of a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    /// A command that fires an action from the shortcut table.
    Action {
        /// The action the item fires.
        action: Action,
        /// The text the menu shows.
        label: String,
        /// The key equivalent, taken from the shortcut table.
        key: Option<Combo>,
    },
    /// An item AppKit draws and handles itself.
    Predefined(Predefined),
    /// A horizontal rule between groups of items.
    Separator,
}

impl Item {
    /// Builds a command item whose key equivalent comes from the shortcut
    /// table, so the menu and the keyboard cannot disagree about a combination.
    pub fn command(action: Action, label: &str) -> Self {
        Self::Action {
            action,
            label: label.to_owned(),
            key: menu_key(action),
        }
    }
}

/// The key equivalent a menu item may show for an action.
///
/// AppKit gives a menu item without a modifier the whole keyboard, so a plain
/// letter or arrow from the table would take that key away from every text
/// field in the window, including the note editor and the search box. Such an
/// action keeps its menu item and keeps its key through the keyboard handler,
/// and the item shows no accelerator rather than a combination that would do
/// something else.
pub fn menu_key(action: Action) -> Option<Combo> {
    combo_for(action).filter(|combo| combo.command || combo.option)
}

/// The items AppKit provides and handles itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Predefined {
    /// The standard about panel.
    About,
    /// The standard services submenu.
    Services,
    /// Hide the application.
    Hide,
    /// Hide every other application.
    HideOthers,
    /// Show every hidden application.
    ShowAll,
    /// Quit the application.
    Quit,
    /// Minimise the window.
    Minimize,
    /// Zoom the window.
    Zoom,
    /// Bring every window to the front.
    BringAllToFront,
}

/// One menu in the menu bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// The title the menu bar shows.
    pub title: String,
    /// The items, top to bottom.
    pub items: Vec<Item>,
}

/// The actions that carry a shortcut but are not menu items.
///
/// These act on the verse under the pointer or on a selection, so they belong to
/// the verse menu on the text rather than to the menu bar. The copy and compare
/// rows would otherwise sit in an Edit menu, and the Edit menu is what this unit
/// exists to keep out. The parity test in `tests/menu.rs` compares this list
/// with the actual difference between the shortcut table and the menu bar, so a
/// shortcut or a menu item added without the other fails the test in either
/// direction.
pub const KEYBOARD_ONLY: &[Action] = &[
    Action::Translation(7),
    Action::Translation(8),
    Action::Highlight(bible_core::HighlightColour::Yellow),
    Action::Highlight(bible_core::HighlightColour::Green),
    Action::Highlight(bible_core::HighlightColour::Blue),
    Action::Highlight(bible_core::HighlightColour::Pink),
    Action::Highlight(bible_core::HighlightColour::Orange),
    Action::Highlight(bible_core::HighlightColour::Purple),
    Action::ClearHighlight,
    Action::Note,
    Action::Bookmark,
    Action::Copy,
    Action::CopyWithReference,
    Action::Compare,
];

/// The menu tree the application shows.
///
/// The commands are listed in the order the shortcut overlay lists them, and
/// each command takes its key equivalent from [`combo_for`]. The translation
/// menu labels the bundled translations by identifier in capitals because the
/// display name and the abbreviation live in each file's own header, which is
/// not decoded until the reader asks for the text.
pub fn menu_spec() -> Vec<Menu> {
    let mut translations = Vec::new();
    for (index, translation) in crate::BUNDLED_TRANSLATIONS.iter().enumerate() {
        translations.push(Item::command(
            Action::Translation(index as u8),
            &translation.id().to_uppercase(),
        ));
    }

    vec![
        Menu {
            title: "Bible".to_owned(),
            items: vec![
                Item::Predefined(Predefined::About),
                Item::command(Action::Preferences, "Settings"),
                Item::Separator,
                Item::Predefined(Predefined::Quit),
            ],
        },
        Menu {
            title: "File".to_owned(),
            items: vec![
                Item::command(Action::ExportMarkdown, "Export Markdown"),
                Item::command(Action::ExportJson, "Export JSON"),
            ],
        },
        Menu {
            title: "Go".to_owned(),
            items: vec![
                Item::command(Action::JumpPalette, "Jump"),
                Item::command(Action::Search, "Search"),
                Item::command(Action::BookPicker, "Book"),
                Item::Separator,
                Item::command(Action::Back, "Back"),
                Item::command(Action::Forward, "Forward"),
                Item::Separator,
                Item::command(Action::PreviousChapter, "Previous Chapter"),
                Item::command(Action::NextChapter, "Next Chapter"),
            ],
        },
        Menu {
            title: "View".to_owned(),
            items: vec![
                Item::command(Action::ToggleSidebar, "Toggle Sidebar"),
                Item::command(Action::CycleTheme, "Cycle Theme"),
                Item::command(Action::ToggleRedLetter, "Words of Christ in Red"),
                Item::command(Action::TextLarger, "Larger Text"),
                Item::command(Action::TextSmaller, "Smaller Text"),
                Item::command(Action::TextDefault, "Default Text Size"),
                Item::Separator,
                Item::command(Action::ToggleVerseNumbers, "Verse Numbers"),
                Item::command(Action::ToggleHeadings, "Section Headings"),
            ],
        },
        Menu {
            title: "Translations".to_owned(),
            items: translations,
        },
        Menu {
            title: "Window".to_owned(),
            items: vec![
                Item::Predefined(Predefined::Minimize),
                Item::Predefined(Predefined::Zoom),
                Item::Separator,
                Item::Predefined(Predefined::BringAllToFront),
            ],
        },
        Menu {
            title: "Help".to_owned(),
            items: vec![Item::command(Action::ShowShortcuts, "Keyboard Shortcuts")],
        },
    ]
}

/// The identifier a menu item carries into AppKit and back.
///
/// Identifiers are derived from the action rather than written by hand, so two
/// items cannot claim the same identifier and a renamed action cannot leave a
/// stale string behind.
pub fn menu_id(action: Action) -> String {
    let slug = match action {
        Action::JumpPalette => "jump-palette".to_owned(),
        Action::Search => "search".to_owned(),
        Action::BookPicker => "book-picker".to_owned(),
        Action::ToggleSidebar => "toggle-sidebar".to_owned(),
        Action::Translation(index) => format!("translation-{index}"),
        Action::Back => "back".to_owned(),
        Action::Forward => "forward".to_owned(),
        Action::PreviousChapter => "previous-chapter".to_owned(),
        Action::NextChapter => "next-chapter".to_owned(),
        Action::Highlight(colour) => format!("highlight-{colour:?}").to_lowercase(),
        Action::ClearHighlight => "clear-highlight".to_owned(),
        Action::Note => "note".to_owned(),
        Action::Bookmark => "bookmark".to_owned(),
        Action::Copy => "copy".to_owned(),
        Action::CopyWithReference => "copy-with-reference".to_owned(),
        Action::Compare => "compare".to_owned(),
        Action::ToggleParallel => "toggle-parallel".to_owned(),
        Action::ExportMarkdown => "export-markdown".to_owned(),
        Action::ExportJson => "export-json".to_owned(),
        Action::ToggleRedLetter => "toggle-red-letter".to_owned(),
        Action::CycleTheme => "cycle-theme".to_owned(),
        Action::TextLarger => "text-larger".to_owned(),
        Action::TextSmaller => "text-smaller".to_owned(),
        Action::TextDefault => "text-default".to_owned(),
        Action::ToggleVerseNumbers => "toggle-verse-numbers".to_owned(),
        Action::ToggleHeadings => "toggle-headings".to_owned(),
        Action::ShowShortcuts => "show-shortcuts".to_owned(),
        Action::Preferences => "preferences".to_owned(),
    };
    format!("action.{slug}")
}

/// The action a menu item identifier stands for, if the tree holds one.
///
/// This is the whole of the translation from a native event to an application
/// action. An identifier that belongs to a predefined item, or to no item at
/// all, yields nothing.
pub fn action_for_id(id: &str, spec: &[Menu]) -> Option<Action> {
    spec.iter()
        .flat_map(|menu| &menu.items)
        .find_map(|item| match item {
            Item::Action { action, .. } if menu_id(*action) == id => Some(*action),
            _ => None,
        })
}

/// Hands a menu event identifier to the bridge, which forwards the action when
/// the identifier names one.
pub fn route(id: &str, spec: &[Menu], sender: &mpsc::Sender<Action>) {
    if let Some(action) = action_for_id(id, spec) {
        let _ = sender.send(action);
    }
}

/// The key equivalent muda needs for a combination from the shortcut table.
///
/// `None` means the combination has a key the native table cannot express, and
/// such a menu item is built without an accelerator rather than with a wrong one.
pub fn accelerator_for(combo: Combo) -> Option<Accelerator> {
    let mut modifiers = Modifiers::empty();
    if combo.command {
        modifiers |= Modifiers::META;
    }
    if combo.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if combo.option {
        modifiers |= Modifiers::ALT;
    }
    Some(Accelerator::new(modifiers, code_for(combo.key)?))
}

/// The native key code that corresponds to a table key.
pub fn code_for(key: Key) -> Option<Code> {
    Some(match key {
        Key::Char('a') => Code::KeyA,
        Key::Char('b') => Code::KeyB,
        Key::Char('c') => Code::KeyC,
        Key::Char('d') => Code::KeyD,
        Key::Char('e') => Code::KeyE,
        Key::Char('f') => Code::KeyF,
        Key::Char('g') => Code::KeyG,
        Key::Char('h') => Code::KeyH,
        Key::Char('i') => Code::KeyI,
        Key::Char('j') => Code::KeyJ,
        Key::Char('k') => Code::KeyK,
        Key::Char('l') => Code::KeyL,
        Key::Char('m') => Code::KeyM,
        Key::Char('n') => Code::KeyN,
        Key::Char('o') => Code::KeyO,
        Key::Char('p') => Code::KeyP,
        Key::Char('q') => Code::KeyQ,
        Key::Char('r') => Code::KeyR,
        Key::Char('s') => Code::KeyS,
        Key::Char('t') => Code::KeyT,
        Key::Char('u') => Code::KeyU,
        Key::Char('v') => Code::KeyV,
        Key::Char('w') => Code::KeyW,
        Key::Char('x') => Code::KeyX,
        Key::Char('y') => Code::KeyY,
        Key::Char('z') => Code::KeyZ,
        Key::Char('0') => Code::Digit0,
        Key::Char('1') => Code::Digit1,
        Key::Char('2') => Code::Digit2,
        Key::Char('3') => Code::Digit3,
        Key::Char('4') => Code::Digit4,
        Key::Char('5') => Code::Digit5,
        Key::Char('6') => Code::Digit6,
        Key::Char('7') => Code::Digit7,
        Key::Char('8') => Code::Digit8,
        Key::Char('9') => Code::Digit9,
        // The reader treats the plus key and the equals key as one combination,
        // and the native code for the physical key is the equals key.
        Key::Char('+') => Code::Equal,
        Key::Char('-') => Code::Minus,
        Key::Char('[') => Code::BracketLeft,
        Key::Char(']') => Code::BracketRight,
        Key::Left => Code::ArrowLeft,
        Key::Right => Code::ArrowRight,
        Key::Char(_) => return None,
    })
}

/// The receiving end of the menu bridge.
///
/// muda delivers a click while the window is idle, so the action is sent through
/// a channel, the window is asked for a frame, and the app drains the channel at
/// the top of that frame. Without the repaint request a menu item would wait for
/// the next pointer or key event before it took effect.
static ACTIONS: OnceLock<Mutex<Option<mpsc::Receiver<Action>>>> = OnceLock::new();

/// Builds the menu bar, attaches it to the application, and starts the bridge.
///
/// Call this on the main thread after the application object exists and before
/// the first frame is drawn. A plain binary launched from a terminal gets the
/// same menu as a bundled application, because AppKit owns the menu bar rather
/// than the window.
pub fn install(repaint: eframe::egui::Context) -> muda::Result<()> {
    let spec = menu_spec();
    let root = muda::Menu::new();
    for menu in &spec {
        let submenu = muda::Submenu::new(&menu.title, true);
        for item in &menu.items {
            match item {
                Item::Action { action, label, key } => {
                    let accelerator = key.and_then(accelerator_for);
                    let command =
                        muda::MenuItem::with_id(menu_id(*action), label, true, accelerator);
                    submenu.append(&command)?;
                }
                Item::Separator => submenu.append(&muda::PredefinedMenuItem::separator())?,
                Item::Predefined(kind) => submenu.append(&predefined(*kind))?,
            }
        }
        root.append(&submenu)?;
    }
    root.init_for_nsapp();

    let (sender, receiver) = mpsc::channel();
    let routed = spec.clone();
    let waking = repaint.clone();
    muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
        route(event.id().as_ref(), &routed, &sender);
        waking.request_repaint();
    }));
    let slot = ACTIONS.get_or_init(|| Mutex::new(None));
    *slot.lock().unwrap_or_else(|error| error.into_inner()) = Some(receiver);

    // AppKit keeps a pointer to the menu it was given, so the menu has to live
    // as long as the process. One menu is built per run and never replaced.
    Box::leak(Box::new(root));
    Ok(())
}

/// The actions the menu bar queued since the last call.
///
/// Before [`install`] runs, and on a build where the menu was never built, the
/// queue is simply empty.
pub fn drain_actions() -> Vec<Action> {
    let slot = ACTIONS.get_or_init(|| Mutex::new(None));
    let guard = slot.lock().unwrap_or_else(|error| error.into_inner());
    match guard.as_ref() {
        Some(receiver) => receiver.try_iter().collect(),
        None => Vec::new(),
    }
}

/// The native item for a predefined entry.
fn predefined(kind: Predefined) -> muda::PredefinedMenuItem {
    match kind {
        Predefined::About => muda::PredefinedMenuItem::about(None, None),
        Predefined::Services => muda::PredefinedMenuItem::services(None),
        Predefined::Hide => muda::PredefinedMenuItem::hide(None),
        Predefined::HideOthers => muda::PredefinedMenuItem::hide_others(None),
        Predefined::ShowAll => muda::PredefinedMenuItem::show_all(None),
        Predefined::Quit => muda::PredefinedMenuItem::quit(None),
        Predefined::Minimize => muda::PredefinedMenuItem::minimize(None),
        Predefined::Zoom => muda::PredefinedMenuItem::zoom(None),
        Predefined::BringAllToFront => muda::PredefinedMenuItem::bring_all_to_front(None),
    }
}
