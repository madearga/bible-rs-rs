// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The study sidebar and the margin marks that point into it.
//!
//! The sidebar is a read-only view of the core study store. [`rows_from_snapshot`]
//! turns one store snapshot into grouped rows, [`show`] paints those rows and
//! returns the reference the reader picked, and [`margin_markers`] answers the
//! two cheap questions the reading surface asks for every visible verse. Nothing
//! here writes to the store: a row is a place to jump to, not an edit.

use bible_core::Reference;
use bible_core::study::{Store, StudyData};
use eframe::egui;
use egui::{FontId, RichText};

use crate::theme::{self, Palette};

/// The three kinds of study mark, in the order the sidebar groups them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RowKind {
    /// A highlighted verse.
    Highlight,
    /// A verse with a note.
    Note,
    /// A bookmarked verse.
    Bookmark,
}

impl RowKind {
    /// Every kind, in the order the sidebar shows its heading.
    pub const ALL: [Self; 3] = [Self::Highlight, Self::Note, Self::Bookmark];

    /// The heading and button word for this kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Highlight => "Highlights",
            Self::Note => "Notes",
            Self::Bookmark => "Bookmarks",
        }
    }

    /// The singular word used in a row's button.
    pub fn singular(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Note => "note",
            Self::Bookmark => "bookmark",
        }
    }
}

/// One place in the sidebar, with the kind of mark that put it there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The verse to jump to when the row is activated.
    pub reference: Reference,
    /// Which study mark this row came from.
    pub kind: RowKind,
    /// A short label: the colour, the note's first line, or the bookmark label.
    pub label: String,
}

impl Row {
    /// The text painted in the row's button.
    pub fn title(&self) -> String {
        if self.label.is_empty() {
            format!("{} · {}", self.reference, self.kind.singular())
        } else {
            format!(
                "{} · {} · {}",
                self.reference,
                self.kind.singular(),
                self.label
            )
        }
    }
}

/// The two margin marks a visible verse can carry.
///
/// A verse with neither mark is represented by `Default`, so the reading surface
/// can skip painting without branching on an option.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarginMarkers {
    /// The verse has a note.
    pub note: bool,
    /// The verse has a bookmark.
    pub bookmark: bool,
}

impl MarginMarkers {
    /// Whether the verse carries any mark at all.
    pub fn is_empty(self) -> bool {
        !self.note && !self.bookmark
    }
}

/// Turns one study store snapshot into sidebar rows.
///
/// The rows are grouped by kind and keep the snapshot's canonical verse order
/// inside each group, so a re-opened sidebar shows the same list every time.
pub fn rows_from_snapshot(data: &StudyData) -> Vec<Row> {
    let mut rows =
        Vec::with_capacity(data.highlights.len() + data.notes.len() + data.bookmarks.len());
    for (reference, colour) in &data.highlights {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Highlight,
            label: colour.name().to_owned(),
        });
    }
    for (reference, note) in &data.notes {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Note,
            label: summarise(&note.body),
        });
    }
    for (reference, bookmark) in &data.bookmarks {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Bookmark,
            label: bookmark.label.clone().unwrap_or_default(),
        });
    }
    rows
}

/// The note and bookmark state of one verse.
///
/// Both store reads are mirror lookups, so the reading surface can call this for
/// every line it paints without laying the chapter out again.
pub fn margin_markers(store: &Store, reference: Reference) -> MarginMarkers {
    MarginMarkers {
        note: store.note(reference).is_some(),
        bookmark: store.bookmark(reference).is_some(),
    }
}

/// Paints the sidebar rows and returns the reference the reader activated.
///
/// The function only reads `rows`; the caller decides what a jump means and
/// records it in its own history.
pub fn show(ui: &mut egui::Ui, rows: &[Row], palette: Palette) -> Option<Reference> {
    let mut chosen = None;
    ui.vertical(|ui| {
        ui.label(
            RichText::new("Study marks")
                .font(FontId::new(14.0, theme::sans()))
                .color(palette.ink),
        );
        ui.separator();
        if rows.is_empty() {
            ui.label(
                RichText::new("No study marks yet")
                    .font(FontId::new(12.0, theme::sans()))
                    .color(palette.muted),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for kind in RowKind::ALL {
                    let group: Vec<&Row> = rows.iter().filter(|row| row.kind == kind).collect();
                    if group.is_empty() {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(kind.name())
                            .font(FontId::new(11.0, theme::sans()))
                            .color(palette.muted),
                    );
                    for row in group {
                        let title = row.title();
                        if ui
                            .add_sized(
                                [ui.available_width(), 22.0],
                                egui::Button::new(
                                    RichText::new(title)
                                        .font(FontId::new(12.0, theme::sans()))
                                        .color(palette.ink),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            chosen = Some(row.reference);
                        }
                    }
                }
            });
    });
    chosen
}

/// A one-line version of a note body, short enough for a sidebar row.
fn summarise(body: &str) -> String {
    let line = body.lines().next().unwrap_or("").trim();
    let mut out: String = line.chars().take(48).collect();
    if line.chars().count() > 48 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use bible_core::Reference;
    use bible_core::study::{Bookmark, HighlightColour, Note, Store, StudyData};

    use super::*;

    /// A canonical verse for the sidebar tests.
    fn verse(book: u8, chapter: u8, verse: u8) -> Reference {
        Reference::new(book, chapter, Some(verse)).expect("the verse is canonical")
    }

    #[test]
    fn a_snapshot_becomes_rows_grouped_by_kind() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let highlight = verse(42, 3, 16);
        let note = verse(42, 3, 17);
        let bookmark = verse(42, 3, 18);
        store
            .set_highlight(highlight, Some(HighlightColour::Green))
            .expect("highlight");
        store.set_note(note, "a synthetic note").expect("note");
        store
            .set_bookmark(bookmark, Some(Bookmark::with_label("a synthetic label")))
            .expect("bookmark");
        let rows = rows_from_snapshot(&store.snapshot());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, RowKind::Highlight);
        assert_eq!(rows[1].kind, RowKind::Note);
        assert_eq!(rows[2].kind, RowKind::Bookmark);
        assert_eq!(rows[0].label, "green");
        assert_eq!(rows[2].label, "a synthetic label");
    }

    #[test]
    fn margin_markers_report_the_mark_that_is_present() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        let noted = verse(42, 3, 16);
        let bookmarked = verse(42, 3, 17);
        let plain = verse(42, 3, 18);
        store.set_note(noted, "a synthetic note").expect("note");
        store
            .set_bookmark(bookmarked, Some(Bookmark::unlabelled()))
            .expect("bookmark");
        assert_eq!(
            margin_markers(&store, noted),
            MarginMarkers {
                note: true,
                bookmark: false
            }
        );
        assert_eq!(
            margin_markers(&store, bookmarked),
            MarginMarkers {
                note: false,
                bookmark: true
            }
        );
        assert!(margin_markers(&store, plain).is_empty());
    }

    #[test]
    fn rows_stay_in_canonical_order_inside_a_group() {
        let data = StudyData {
            highlights: Vec::new(),
            notes: vec![
                (verse(19, 23, 1), Note::new("first")),
                (verse(43, 3, 16), Note::new("second")),
            ],
            bookmarks: Vec::new(),
            settings: Default::default(),
        };
        let rows = rows_from_snapshot(&data);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].reference.book <= rows[1].reference.book);
    }
}
