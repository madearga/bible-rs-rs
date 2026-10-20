// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The verse menu.
//!
//! The menu acts on one selected verse. It offers the six highlight colours, a
//! note, a bookmark, copy, copy with the reference, and a comparison across
//! translations. The model holds only the target and the note draft; every write
//! goes through the core study store, which owns persistence.

use anyhow::{Result, bail};
use bible_core::study::Store;
use bible_core::{HighlightColour, Reference, Translation};

/// One row of the verse menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    /// Highlight in a colour, or clear the highlight when the colour is absent.
    Highlight(Option<HighlightColour>),
    /// Write the draft note.
    Note,
    /// Toggle a bookmark.
    Bookmark,
    /// Copy the verse text.
    Copy,
    /// Copy the verse text with its reference.
    CopyWithReference,
    /// Compare the verse across translations.
    Compare,
}

/// What a menu action produced for the shell to show or put on the clipboard.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuOutcome {
    /// Text the shell should put on the clipboard, when the action copied.
    pub clipboard: Option<String>,
    /// A short line the shell can show in the status bar.
    pub message: String,
}

/// One translation's rendering of the target verse in a comparison.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompareRow {
    /// The translation's abbreviation.
    pub abbreviation: String,
    /// The verse text in that translation.
    pub text: String,
}

/// The verse menu and the verse it is acting on.
#[derive(Clone, Debug, Default)]
pub struct VerseMenu {
    open: bool,
    target: Option<Reference>,
    note_draft: String,
    comparison: Vec<CompareRow>,
}

impl VerseMenu {
    /// A closed menu with no target.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the menu on a verse.
    pub fn open_at(&mut self, reference: Reference) {
        self.open = true;
        self.target = Some(reference);
        self.note_draft.clear();
    }

    /// Closes the menu.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the menu is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The verse the menu acts on.
    pub fn target(&self) -> Option<Reference> {
        self.target
    }

    /// The note text the reader is editing.
    pub fn note_draft(&self) -> &str {
        &self.note_draft
    }

    /// The rows of the last comparison, which the menu paints under its buttons.
    pub fn comparison(&self) -> &[CompareRow] {
        &self.comparison
    }

    /// Replaces the rows of the comparison.
    pub fn set_comparison(&mut self, rows: Vec<CompareRow>) {
        self.comparison = rows;
    }

    /// Replaces the note draft.
    pub fn set_note_draft(&mut self, draft: impl Into<String>) {
        self.note_draft = draft.into();
    }

    /// Sets the target's highlight, or clears it when the colour is absent.
    pub fn highlight(&self, store: &mut Store, colour: Option<HighlightColour>) -> Result<()> {
        let target = self.require_target()?;
        store.set_highlight(target, colour)
    }

    /// Writes the note draft onto the target verse.
    ///
    /// A blank draft removes the note, which is the same rule the store applies.
    pub fn commit_note(&self, store: &mut Store) -> Result<()> {
        let target = self.require_target()?;
        store.set_note(target, &self.note_draft)
    }

    /// Toggles the target's bookmark and returns whether it is marked now.
    pub fn toggle_bookmark(&self, store: &mut Store) -> Result<bool> {
        let target = self.require_target()?;
        store.toggle_bookmark(target)
    }

    /// The target verse's text in a translation.
    pub fn verse_text<'a>(&self, translation: &'a Translation) -> Option<&'a str> {
        let target = self.target?;
        translation.reference_text(&target)
    }

    /// The target verse's text followed by its reference.
    pub fn text_with_reference(&self, translation: &Translation) -> Option<String> {
        let target = self.target?;
        let text = translation.reference_text(&target)?;
        Some(format!("{text} ({target})"))
    }

    /// The target verse in each translation given, in the order given.
    ///
    /// A translation that has no text for the verse is skipped rather than shown
    /// as an empty row.
    pub fn compare(&self, translations: &[&Translation]) -> Vec<CompareRow> {
        let Some(target) = self.target else {
            return Vec::new();
        };
        translations
            .iter()
            .filter_map(|translation| {
                let text = translation.reference_text(&target)?;
                Some(CompareRow {
                    abbreviation: translation.meta().abbreviation.clone(),
                    text: text.to_owned(),
                })
            })
            .collect()
    }

    /// Runs one menu action against the store and the current translation.
    pub fn apply(
        &self,
        action: MenuAction,
        store: &mut Store,
        translation: &Translation,
    ) -> Result<MenuOutcome> {
        let target = self.require_target()?;
        let outcome = match action {
            MenuAction::Highlight(colour) => {
                self.highlight(store, colour)?;
                let message = match colour {
                    Some(colour) => format!("{target} highlighted {}", colour.name()),
                    None => format!("{target} highlight cleared"),
                };
                MenuOutcome {
                    clipboard: None,
                    message,
                }
            }
            MenuAction::Note => {
                self.commit_note(store)?;
                MenuOutcome {
                    clipboard: None,
                    message: format!("note saved on {target}"),
                }
            }
            MenuAction::Bookmark => {
                let marked = self.toggle_bookmark(store)?;
                MenuOutcome {
                    clipboard: None,
                    message: if marked {
                        format!("bookmarked {target}")
                    } else {
                        format!("bookmark removed from {target}")
                    },
                }
            }
            MenuAction::Copy => MenuOutcome {
                clipboard: self.verse_text(translation).map(str::to_owned),
                message: format!("copied {target}"),
            },
            MenuAction::CopyWithReference => MenuOutcome {
                clipboard: self.text_with_reference(translation),
                message: format!("copied {target} with its reference"),
            },
            MenuAction::Compare => {
                let rows = self.compare(&[translation]);
                MenuOutcome {
                    clipboard: None,
                    message: format!("{} translation for {target}", rows.len()),
                }
            }
        };
        Ok(outcome)
    }

    /// Returns the target or an error when the menu was opened without one.
    fn require_target(&self) -> Result<Reference> {
        match self.target {
            Some(reference) => Ok(reference),
            None => bail!("the verse menu has no verse to act on"),
        }
    }
}

#[cfg(test)]
mod tests {
    use bible_core::study::Store;
    use bible_core::{HighlightColour, Reference};

    use super::*;

    /// A canonical verse for the study tests.
    fn target() -> Reference {
        Reference::new(42, 3, Some(16)).expect("the verse is canonical")
    }

    #[test]
    fn choosing_a_colour_calls_the_study_store() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let mut menu = VerseMenu::new();
        menu.open_at(target());
        menu.highlight(&mut store, Some(HighlightColour::Green))
            .expect("the store accepts a highlight");
        assert_eq!(store.highlight(target()), Some(HighlightColour::Green));
        menu.highlight(&mut store, None)
            .expect("the store clears a highlight");
        assert_eq!(store.highlight(target()), None);
    }

    #[test]
    fn a_note_can_be_set() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let mut menu = VerseMenu::new();
        menu.open_at(target());
        menu.set_note_draft("a synthetic note");
        menu.commit_note(&mut store)
            .expect("the store accepts a note");
        assert_eq!(store.note(target()), Some("a synthetic note"));
    }

    #[test]
    fn a_bookmark_can_be_toggled() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let mut menu = VerseMenu::new();
        menu.open_at(target());
        assert!(menu.toggle_bookmark(&mut store).expect("bookmark on"));
        assert!(store.bookmark(target()).is_some());
        assert!(!menu.toggle_bookmark(&mut store).expect("bookmark off"));
        assert!(store.bookmark(target()).is_none());
    }

    #[test]
    fn the_menu_without_a_target_refuses() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let menu = VerseMenu::new();
        assert!(
            menu.highlight(&mut store, Some(HighlightColour::Blue))
                .is_err()
        );
    }
}
