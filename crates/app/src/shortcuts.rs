// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The single keyboard shortcut table.
//!
//! The menu bar, the shortcut overlay, and the key handler all read
//! [`SHORTCUTS`]. A shortcut that exists in the menu but not in the handler is
//! therefore a compile error in the handler rather than a bug a reader finds in
//! the window. Every key combination in the table is unique.

use bible_core::HighlightColour;

/// One physical key as the table records it.
///
/// Letters are stored lower case and matched without regard to the shift key, so
/// a table entry says `Char('n')` and not `Char('N')`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printed character, lower case for letters.
    Char(char),
    /// The left arrow key.
    Left,
    /// The right arrow key.
    Right,
}

/// A key together with the modifiers that must be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Combo {
    /// The key itself.
    pub key: Key,
    /// The command key on macOS, control elsewhere.
    pub command: bool,
    /// The shift key.
    pub shift: bool,
    /// The option key on macOS, alt elsewhere.
    pub option: bool,
}

impl Combo {
    /// Builds a combination with every modifier stated.
    pub const fn new(key: Key, command: bool, shift: bool, option: bool) -> Self {
        Self {
            key,
            command,
            shift,
            option,
        }
    }

    /// A command combination such as command-K.
    pub const fn command(key: Key) -> Self {
        Self::new(key, true, false, false)
    }

    /// A plain unmodified key such as N.
    pub const fn plain(key: Key) -> Self {
        Self::new(key, false, false, false)
    }

    /// A shift and command combination such as shift-command-C.
    pub const fn command_shift(key: Key) -> Self {
        Self::new(key, true, true, false)
    }

    /// An option combination such as option-left.
    pub const fn option(key: Key) -> Self {
        Self::new(key, false, false, true)
    }
}

/// Everything a key press can ask the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Open the jump palette.
    JumpPalette,
    /// Open the search panel.
    Search,
    /// Open the book and chapter picker.
    BookPicker,
    /// Switch to the translation at this zero-based position in the list.
    Translation(u8),
    /// Go back in the history.
    Back,
    /// Go forward in the history.
    Forward,
    /// Show the previous chapter.
    PreviousChapter,
    /// Show the next chapter.
    NextChapter,
    /// Highlight the selected verse in a colour.
    Highlight(HighlightColour),
    /// Remove the highlight from the selected verse.
    ClearHighlight,
    /// Write a note on the selected verse.
    Note,
    /// Toggle a bookmark on the selected verse.
    Bookmark,
    /// Copy the selected verse.
    Copy,
    /// Copy the selected verse together with its reference.
    CopyWithReference,
    /// Compare the selected verse across translations.
    Compare,
    /// Show or hide the parallel translations view.
    ///
    /// The action is in the enum so the menu bar can reach it, and it has no key
    /// in the shortcut table on purpose. Command-P sits next to command-F and is
    /// read as print, which is the wrong thing to guess at for this action.
    ToggleParallel,
    /// Export the study data as Markdown.
    ExportMarkdown,
    /// Export the study data as JSON.
    ExportJson,
    /// Toggle words of Christ in red.
    ToggleRedLetter,
    /// Cycle the reading theme.
    CycleTheme,
    /// Make the text larger.
    TextLarger,
    /// Make the text smaller.
    TextSmaller,
    /// Return the text size to its default.
    TextDefault,
}

impl Action {
    /// The short label the palette and the shortcut overlay show.
    pub fn label(self) -> &'static str {
        match self {
            Self::JumpPalette => "Jump to a reference",
            Self::Search => "Search",
            Self::BookPicker => "Book and chapter picker",
            Self::Translation(_) => "Switch translation",
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::PreviousChapter => "Previous chapter",
            Self::NextChapter => "Next chapter",
            Self::Highlight(colour) => match colour {
                HighlightColour::Yellow => "Highlight yellow",
                HighlightColour::Green => "Highlight green",
                HighlightColour::Blue => "Highlight blue",
                HighlightColour::Pink => "Highlight pink",
                HighlightColour::Orange => "Highlight orange",
                HighlightColour::Purple => "Highlight purple",
            },
            Self::ClearHighlight => "Clear highlight",
            Self::Note => "Write a note",
            Self::Bookmark => "Toggle bookmark",
            Self::Copy => "Copy verse",
            Self::CopyWithReference => "Copy verse with reference",
            Self::Compare => "Compare translations",
            Self::ToggleParallel => "Parallel translations",
            Self::ExportMarkdown => "Export study data as Markdown",
            Self::ExportJson => "Export study data as JSON",
            Self::ToggleRedLetter => "Toggle words of Christ in red",
            Self::CycleTheme => "Cycle theme",
            Self::TextLarger => "Larger text",
            Self::TextSmaller => "Smaller text",
            Self::TextDefault => "Default text size",
        }
    }
}

/// One row of the shortcut table.
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    /// The key combination that triggers the action.
    pub combo: Combo,
    /// What the combination does.
    pub action: Action,
    /// How the shortcut reads in the menu and in the overlay.
    pub description: &'static str,
}

/// The shortcuts the app ships with, in menu order.
///
/// The list covers every row of the keyboard table in the program spec. The
/// translation and highlight rows are expanded so each concrete key is its own
/// entry, which is what makes the uniqueness check meaningful.
pub static SHORTCUTS: &[Shortcut] = &[
    shortcut(
        Combo::command(Key::Char('k')),
        Action::JumpPalette,
        "command-K",
    ),
    shortcut(Combo::command(Key::Char('f')), Action::Search, "command-F"),
    shortcut(
        Combo::command(Key::Char('b')),
        Action::BookPicker,
        "command-B",
    ),
    shortcut(
        Combo::command(Key::Char('1')),
        Action::Translation(0),
        "command-1",
    ),
    shortcut(
        Combo::command(Key::Char('2')),
        Action::Translation(1),
        "command-2",
    ),
    shortcut(
        Combo::command(Key::Char('3')),
        Action::Translation(2),
        "command-3",
    ),
    shortcut(
        Combo::command(Key::Char('4')),
        Action::Translation(3),
        "command-4",
    ),
    shortcut(
        Combo::command(Key::Char('5')),
        Action::Translation(4),
        "command-5",
    ),
    shortcut(
        Combo::command(Key::Char('6')),
        Action::Translation(5),
        "command-6",
    ),
    shortcut(
        Combo::command(Key::Char('7')),
        Action::Translation(6),
        "command-7",
    ),
    shortcut(
        Combo::command(Key::Char('8')),
        Action::Translation(7),
        "command-8",
    ),
    shortcut(
        Combo::command(Key::Char('9')),
        Action::Translation(8),
        "command-9",
    ),
    shortcut(Combo::command(Key::Char('[')), Action::Back, "command-["),
    shortcut(Combo::command(Key::Char(']')), Action::Forward, "command-]"),
    shortcut(Combo::option(Key::Left), Action::Back, "option-left"),
    shortcut(Combo::option(Key::Right), Action::Forward, "option-right"),
    shortcut(Combo::plain(Key::Left), Action::PreviousChapter, "left"),
    shortcut(Combo::plain(Key::Right), Action::NextChapter, "right"),
    shortcut(
        Combo::plain(Key::Char('1')),
        Action::Highlight(HighlightColour::Yellow),
        "1",
    ),
    shortcut(
        Combo::plain(Key::Char('2')),
        Action::Highlight(HighlightColour::Green),
        "2",
    ),
    shortcut(
        Combo::plain(Key::Char('3')),
        Action::Highlight(HighlightColour::Blue),
        "3",
    ),
    shortcut(
        Combo::plain(Key::Char('4')),
        Action::Highlight(HighlightColour::Pink),
        "4",
    ),
    shortcut(
        Combo::plain(Key::Char('5')),
        Action::Highlight(HighlightColour::Orange),
        "5",
    ),
    shortcut(
        Combo::plain(Key::Char('6')),
        Action::Highlight(HighlightColour::Purple),
        "6",
    ),
    shortcut(Combo::plain(Key::Char('0')), Action::ClearHighlight, "0"),
    shortcut(Combo::plain(Key::Char('n')), Action::Note, "N"),
    shortcut(Combo::plain(Key::Char('b')), Action::Bookmark, "B"),
    shortcut(
        Combo::command(Key::Char('d')),
        Action::Bookmark,
        "command-D",
    ),
    shortcut(Combo::command(Key::Char('c')), Action::Copy, "command-C"),
    shortcut(
        Combo::command_shift(Key::Char('c')),
        Action::CopyWithReference,
        "shift-command-C",
    ),
    shortcut(Combo::plain(Key::Char('c')), Action::Compare, "C"),
    shortcut(Combo::plain(Key::Char('e')), Action::ExportMarkdown, "E"),
    shortcut(
        Combo::command_shift(Key::Char('e')),
        Action::ExportJson,
        "shift-command-E",
    ),
    shortcut(
        Combo::command(Key::Char('r')),
        Action::ToggleRedLetter,
        "command-R",
    ),
    shortcut(
        Combo::command_shift(Key::Char('l')),
        Action::CycleTheme,
        "shift-command-L",
    ),
    shortcut(
        Combo::command(Key::Char('+')),
        Action::TextLarger,
        "command-plus",
    ),
    shortcut(
        Combo::command(Key::Char('-')),
        Action::TextSmaller,
        "command-minus",
    ),
    shortcut(
        Combo::command(Key::Char('0')),
        Action::TextDefault,
        "command-0",
    ),
];

/// Builds one table row.
const fn shortcut(combo: Combo, action: Action, description: &'static str) -> Shortcut {
    Shortcut {
        combo,
        action,
        description,
    }
}

/// The action a key combination performs, if the table lists it.
pub fn action_for(combo: Combo) -> Option<Action> {
    SHORTCUTS
        .iter()
        .find(|shortcut| shortcut.combo == combo)
        .map(|shortcut| shortcut.action)
}

/// The first key combination the table assigns to an action.
///
/// The back and forward actions have two combinations each, so the command form
/// is listed first and is the one a menu shows.
pub fn combo_for(action: Action) -> Option<Combo> {
    SHORTCUTS
        .iter()
        .find(|shortcut| shortcut.action == action)
        .map(|shortcut| shortcut.combo)
}

/// The actions the palette offers as commands, in table order and without
/// duplicates.
///
/// The highlight rows are one action each, so the palette lists them once
/// rather than six times.
pub fn commands() -> Vec<Action> {
    let mut seen: Vec<Action> = Vec::new();
    for shortcut in SHORTCUTS {
        let action = match shortcut.action {
            Action::Highlight(_) => Action::Highlight(HighlightColour::Yellow),
            other => other,
        };
        if !seen.contains(&action) {
            seen.push(action);
        }
    }
    seen
}

/// The rows the shortcut overlay shows, one per table row.
pub fn overlay_rows() -> Vec<(&'static str, &'static str)> {
    SHORTCUTS
        .iter()
        .map(|shortcut| (shortcut.description, shortcut.action.label()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The number of rows the program spec's keyboard table lists.
    const SPEC_ROWS: usize = 23;

    #[test]
    fn every_combination_is_unique() {
        let mut seen = HashSet::new();
        for shortcut in SHORTCUTS {
            assert!(
                seen.insert(shortcut.combo),
                "{} is listed twice",
                shortcut.description
            );
        }
    }

    #[test]
    fn the_table_covers_the_spec() {
        assert!(
            SHORTCUTS.len() >= SPEC_ROWS,
            "the table has {} entries, the spec lists {SPEC_ROWS}",
            SHORTCUTS.len()
        );
    }

    #[test]
    fn a_combination_finds_its_action() {
        assert_eq!(
            action_for(Combo::command(Key::Char('k'))),
            Some(Action::JumpPalette)
        );
        assert_eq!(
            action_for(Combo::command(Key::Char('f'))),
            Some(Action::Search)
        );
        assert_eq!(action_for(Combo::plain(Key::Char('z'))), None);
    }

    #[test]
    fn the_command_list_has_no_duplicates() {
        let commands = commands();
        let unique: HashSet<Action> = commands.iter().copied().collect();
        assert_eq!(commands.len(), unique.len());
        assert!(commands.contains(&Action::JumpPalette));
    }
}
