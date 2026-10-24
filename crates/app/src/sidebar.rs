// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The sidebar: a Books section that walks the canonical grid, and the study
//! list it shares the column with.
//!
//! Both sections are read-only views of state the rest of the app owns, with
//! one deliberate exception: a row's context menu can remove the mark the row
//! came from. The Books section reads the book table and a set of chapters
//! that carry study marks; the study list reads one store snapshot turned into
//! rows by [`rows_from_snapshot`]. [`show`] paints both and returns what the
//! reader did, and [`margin_markers`] answers the two cheap questions the
//! reading surface asks for every visible verse.

use std::collections::BTreeSet;

use bible_core::study::{Store, StudyData};
use bible_core::{Book, HighlightColour, Reference, Testament, Translation, books};
use eframe::egui;
use egui::{Color32, FontId, RichText, Stroke, vec2};

use crate::theme::{self, Palette};

/// The width the sidebar paints at, whatever fraction of it is revealed.
pub const SIDEBAR_WIDTH: f32 = 240.0;

/// What the reader did to one sidebar row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Go to the place the row or chip names.
    Jump(Reference),
    /// Remove the mark the row came from.
    Remove(Reference, RowKind),
}

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

    /// The word the row's context menu shows for removing this kind of mark.
    fn removal_word(self) -> &'static str {
        match self {
            Self::Highlight => "Remove highlight",
            Self::Note => "Delete note",
            Self::Bookmark => "Remove bookmark",
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
    /// The highlight colour behind the row, for its swatch and its bar.
    pub colour: Option<HighlightColour>,
    /// The verse's own text, one line, shown under the row.
    pub snippet: String,
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

/// The fixed swatch a highlight colour is painted with, whatever the theme.
///
/// The six colours are identity, not decoration: the swatch in the sidebar and
/// the pill row answer "which colour is this mark" the same way at every theme.
pub fn highlight_swatch(colour: HighlightColour) -> Color32 {
    match colour {
        HighlightColour::Yellow => Color32::from_rgb(0xD9, 0xB2, 0x3C),
        HighlightColour::Green => Color32::from_rgb(0x6E, 0xA8, 0x5B),
        HighlightColour::Blue => Color32::from_rgb(0x5B, 0x8D, 0xC9),
        HighlightColour::Pink => Color32::from_rgb(0xD1, 0x79, 0xA6),
        HighlightColour::Orange => Color32::from_rgb(0xDE, 0x8F, 0x4E),
        HighlightColour::Purple => Color32::from_rgb(0x93, 0x70, 0xBD),
    }
}

/// The navigation and filter state the sidebar keeps between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SidebarState {
    /// The book filter, matched case-insensitively against names and
    /// abbreviations. Empty means every book shows.
    pub filter: String,
    /// Whether the Old Testament section is expanded.
    pub old_open: bool,
    /// Whether the New Testament section is expanded.
    pub new_open: bool,
    /// The one book whose chapter grid is expanded.
    pub expanded_book: Option<u8>,
    /// The highlight colour the study list is narrowed to, or `None` for all.
    pub colour_filter: Option<HighlightColour>,
}

impl SidebarState {
    /// Reveals a place: clears the filter, expands its book, and opens only
    /// the testament that holds it.
    ///
    /// This is what opening the sidebar does, so the grid is already on the
    /// chapter the reader is in instead of folded away at Genesis.
    pub fn reveal(&mut self, reference: Reference) {
        self.filter.clear();
        self.expanded_book = Some(reference.book);
        match books::book(reference.book).map(|book| book.testament) {
            Some(Testament::Old) => {
                self.old_open = true;
                self.new_open = false;
            }
            Some(Testament::New) => {
                self.new_open = true;
                self.old_open = false;
            }
            None => {}
        }
    }

    /// Whether a book's name or abbreviation contains the filter.
    fn book_visible(&self, book: &Book) -> bool {
        let needle = self.filter.trim().to_lowercase();
        needle.is_empty()
            || book.name.to_lowercase().contains(&needle)
            || book.abbreviation.to_lowercase().contains(&needle)
    }
}

/// The widget id of the book filter, which the app checks before it runs the
/// plain-key shortcuts.
///
/// Letters typed into the filter must not also highlight verses or open the
/// note editor, and the app can only know to stay out of the way by asking
/// whether this exact id holds the keyboard focus.
pub fn filter_id() -> egui::Id {
    egui::Id::new("sidebar-book-filter")
}

/// Whether the book filter holds the keyboard focus.
pub fn filter_has_focus(ctx: &egui::Context) -> bool {
    ctx.memory(|memory| memory.focused() == Some(filter_id()))
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

/// The chapters that carry at least one study mark, as canonical
/// `(book, chapter)` pairs.
///
/// A chapter with any kind of mark is in the set, whatever the mark is, so the
/// chapter grid can dot a chapter without knowing which marks it holds.
pub fn chapter_activity(data: &StudyData) -> BTreeSet<(u8, u8)> {
    let mut chapters = BTreeSet::new();
    for (reference, _) in &data.highlights {
        chapters.insert((reference.book, reference.chapter));
    }
    for (reference, _) in &data.notes {
        chapters.insert((reference.book, reference.chapter));
    }
    for (reference, _) in &data.bookmarks {
        chapters.insert((reference.book, reference.chapter));
    }
    chapters
}

/// Turns one study store snapshot and one translation into sidebar rows.
///
/// The rows are grouped by kind and keep the snapshot's canonical verse order
/// inside each group, so a re-opened sidebar shows the same list every time.
/// Each row carries a one-line snippet of the verse text in the translation
/// given, which is what makes the list readable without jumping.
pub fn rows_from_snapshot(data: &StudyData, translation: &Translation) -> Vec<Row> {
    let mut rows =
        Vec::with_capacity(data.highlights.len() + data.notes.len() + data.bookmarks.len());
    for (reference, colour) in &data.highlights {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Highlight,
            label: colour.name().to_owned(),
            colour: Some(*colour),
            snippet: verse_snippet(translation, reference),
        });
    }
    for (reference, note) in &data.notes {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Note,
            label: summarise(&note.body),
            colour: None,
            snippet: verse_snippet(translation, reference),
        });
    }
    for (reference, bookmark) in &data.bookmarks {
        rows.push(Row {
            reference: *reference,
            kind: RowKind::Bookmark,
            label: bookmark.label.clone().unwrap_or_default(),
            colour: None,
            snippet: verse_snippet(translation, reference),
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

/// Paints the sidebar and returns what the reader did to it.
///
/// Both the chapter grid and the study rows report through [`Pick`]; the
/// caller decides what a jump means and what a removal removes, and records
/// the jumps in its own history.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    rows: &[Row],
    activity: &BTreeSet<(u8, u8)>,
    current: Reference,
    palette: Palette,
) -> Option<Pick> {
    let mut picked = None;
    ui.vertical(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.filter)
                .id_salt(filter_id())
                .hint_text(RichText::new("Filter books").font(FontId::new(12.0, theme::sans())))
                .desired_width(ui.available_width()),
        );
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                paint_books(ui, state, activity, current, palette, &mut picked);
                ui.add_space(4.0);
                paint_study_marks(ui, state, rows, palette, &mut picked);
            });
    });
    picked
}

/// Paints the Books section: the two testament sections, their book rows, and
/// the one expanded chapter grid.
fn paint_books(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    activity: &BTreeSet<(u8, u8)>,
    current: Reference,
    palette: Palette,
    picked: &mut Option<Pick>,
) {
    ui.label(
        RichText::new("Books")
            .font(FontId::new(14.0, theme::sans()))
            .color(palette.ink),
    );
    let filtering = !state.filter.trim().is_empty();
    let mut shown = 0;
    for testament in [Testament::Old, Testament::New] {
        let list: Vec<&Book> = books::books()
            .iter()
            .filter(|book| book.testament == testament && state.book_visible(book))
            .collect();
        if list.is_empty() {
            continue;
        }
        shown += list.len();
        let open = match testament {
            Testament::Old => state.old_open,
            Testament::New => state.new_open,
        };
        let label = match testament {
            Testament::Old => "Old Testament",
            Testament::New => "New Testament",
        };
        // The disclosure marker is ASCII because the bundled interface face
        // has no triangle glyphs, which render as tofu.
        let marker = if open || filtering { "- " } else { "+ " };
        if ui
            .add_sized(
                [ui.available_width(), 22.0],
                egui::Button::new(
                    RichText::new(format!("{marker}{label} · {}", list.len()))
                        .font(FontId::new(12.0, theme::sans()))
                        .color(palette.muted),
                )
                .frame(false),
            )
            .clicked()
        {
            match testament {
                Testament::Old => state.old_open = !state.old_open,
                Testament::New => state.new_open = !state.new_open,
            }
        }
        if open || filtering {
            for book in list {
                paint_book(ui, state, book, activity, current, palette, picked);
            }
        }
    }
    if shown == 0 {
        ui.label(
            RichText::new("No book matches the filter")
                .font(FontId::new(12.0, theme::sans()))
                .color(palette.muted),
        );
    }
}

/// Paints one book row and, when the row is the expanded one, its chapter grid.
fn paint_book(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    book: &Book,
    activity: &BTreeSet<(u8, u8)>,
    current: Reference,
    palette: Palette,
    picked: &mut Option<Pick>,
) {
    let expanded = state.expanded_book == Some(book.index);
    let here = current.book == book.index;
    let name = if expanded {
        format!("- {}", book.name)
    } else {
        book.name.to_owned()
    };
    if ui
        .add_sized(
            [ui.available_width(), 22.0],
            egui::Button::new(
                RichText::new(name)
                    .font(FontId::new(12.0, theme::sans()))
                    .color(if here { palette.accent } else { palette.ink }),
            )
            .frame(false),
        )
        .clicked()
    {
        state.expanded_book = if expanded { None } else { Some(book.index) };
    }
    if expanded {
        chapter_grid(ui, book, current, activity, palette, picked);
    }
}

/// Paints one book's chapters as a grid of round chips, seven per row.
///
/// The chapter the reader is in is filled with the accent, and a chapter that
/// carries any study mark gets a dot beneath its chip.
fn chapter_grid(
    ui: &mut egui::Ui,
    book: &Book,
    current: Reference,
    activity: &BTreeSet<(u8, u8)>,
    palette: Palette,
    picked: &mut Option<Pick>,
) {
    const PER_ROW: u8 = 7;
    const CHIP: f32 = 24.0;
    const GAP: f32 = 4.0;
    let mut first = 1u8;
    while first <= book.chapter_count {
        let last = first.saturating_add(PER_ROW - 1).min(book.chapter_count);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for chapter in first..=last {
                let here = current.book == book.index && current.chapter == chapter;
                let marked = activity.contains(&(book.index, chapter));
                let text = RichText::new(chapter.to_string())
                    .font(FontId::new(10.0, theme::sans()))
                    .color(if here { palette.paper } else { palette.ink });
                let response = ui.add(
                    egui::Button::new(text)
                        .min_size(vec2(CHIP, CHIP))
                        .fill(if here {
                            palette.accent
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(CHIP / 2.0),
                );
                if marked {
                    ui.painter().circle_filled(
                        egui::pos2(response.rect.center().x, response.rect.bottom() + 3.0),
                        1.5,
                        palette.accent,
                    );
                }
                if response.clicked() {
                    *picked = Some(Pick::Jump(
                        Reference::new(book.index, chapter, None)
                            .expect("the grid chapter is canonical"),
                    ));
                }
            }
        });
        first = last.saturating_add(1);
    }
}

/// Paints the study list: the grouped rows of marks the store holds.
///
/// Highlights carry a colour pill row that narrows the list to one colour, and
/// every row opens a context menu that removes its mark.
fn paint_study_marks(
    ui: &mut egui::Ui,
    state: &mut SidebarState,
    rows: &[Row],
    palette: Palette,
    picked: &mut Option<Pick>,
) {
    ui.label(
        RichText::new("Study marks")
            .font(FontId::new(14.0, theme::sans()))
            .color(palette.ink),
    );
    ui.separator();
    if rows.is_empty() {
        ui.label(
            RichText::new(
                "No study marks yet. Select a verse and press N to note it, \
                 B to bookmark it, or 1 to 6 to highlight it.",
            )
            .font(FontId::new(12.0, theme::sans()))
            .color(palette.muted),
        );
        return;
    }
    for kind in RowKind::ALL {
        let mut group: Vec<&Row> = rows.iter().filter(|row| row.kind == kind).collect();
        if kind == RowKind::Highlight
            && let Some(colour) = state.colour_filter
        {
            group.retain(|row| row.colour == Some(colour));
        }
        if group.is_empty() {
            continue;
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(kind.name())
                .font(FontId::new(11.0, theme::sans()))
                .color(palette.muted),
        );
        if kind == RowKind::Highlight {
            paint_colour_pills(ui, state, palette);
        }
        for row in group {
            paint_row(ui, row, palette, picked);
        }
    }
}

/// Paints the highlight colour filter: an All pill and one swatch per colour.
fn paint_colour_pills(ui: &mut egui::Ui, state: &mut SidebarState, palette: Palette) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if ui
            .selectable_label(
                state.colour_filter.is_none(),
                RichText::new("All").font(FontId::new(11.0, theme::sans())),
            )
            .clicked()
        {
            state.colour_filter = None;
        }
        for colour in HighlightColour::ALL {
            let active = state.colour_filter == Some(colour);
            let response = ui.add(
                egui::Button::new("")
                    .min_size(vec2(14.0, 14.0))
                    .fill(highlight_swatch(colour))
                    .corner_radius(7.0)
                    .stroke(if active {
                        Stroke::new(1.5, palette.ink)
                    } else {
                        Stroke::NONE
                    }),
            );
            if response.clicked() {
                state.colour_filter = if active { None } else { Some(colour) };
            }
        }
    });
}

/// Paints one study row: its button, its colour bar, and its verse snippet.
fn paint_row(ui: &mut egui::Ui, row: &Row, palette: Palette, picked: &mut Option<Pick>) {
    let title = row.title();
    let response = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::Button::new(
            RichText::new(title)
                .font(FontId::new(12.0, theme::sans()))
                .color(palette.ink),
        )
        .frame(false),
    );
    if let Some(colour) = row.colour {
        let bar = row_colour_bar(response.rect);
        ui.painter().rect_filled(bar, 1.5, highlight_swatch(colour));
    }
    if !row.snippet.is_empty() {
        ui.label(
            RichText::new(row.snippet.clone())
                .font(FontId::new(11.0, theme::sans()))
                .color(palette.muted),
        );
    }
    if response.clicked() {
        *picked = Some(Pick::Jump(row.reference));
    }
    let mut remove = false;
    response.context_menu(|ui| {
        if ui.button(row.kind.removal_word()).clicked() {
            remove = true;
        }
    });
    if remove {
        *picked = Some(Pick::Remove(row.reference, row.kind));
    }
}

/// A three point wide bar hugging a row's left edge, inset so it reads as a
/// mark rather than a rule.
fn row_colour_bar(rect: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.left() + 2.0, rect.top() + 2.0),
        vec2(3.0, rect.height() - 4.0),
    )
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

/// A one-line version of a verse, short enough for a row's second line.
fn verse_snippet(translation: &Translation, reference: &Reference) -> String {
    translation
        .reference_text(reference)
        .map(|text| {
            let trimmed = text.trim();
            let mut out: String = trimmed.chars().take(64).collect();
            if trimmed.chars().count() > 64 {
                out.push('…');
            }
            out
        })
        .unwrap_or_default()
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

    /// The sample translation the unit tests build rows against.
    fn translation() -> Translation {
        crate::sample_translation().expect("the embedded sample translation is valid")
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
        let rows = rows_from_snapshot(&store.snapshot(), &translation());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].kind, RowKind::Highlight);
        assert_eq!(rows[1].kind, RowKind::Note);
        assert_eq!(rows[2].kind, RowKind::Bookmark);
        assert_eq!(rows[0].label, "green");
        assert_eq!(rows[2].label, "a synthetic label");
        assert_eq!(rows[0].colour, Some(HighlightColour::Green));
        assert_eq!(rows[1].colour, None);
    }

    #[test]
    fn rows_carry_the_verse_text_as_a_snippet() {
        let mut store = Store::open_in_memory().expect("an in-memory store opens");
        store
            .set_highlight(verse(42, 3, 16), Some(HighlightColour::Blue))
            .expect("highlight");
        let rows = rows_from_snapshot(&store.snapshot(), &translation());
        assert!(
            !rows[0].snippet.is_empty(),
            "the highlight row has no verse text"
        );
        assert!(rows[0].snippet.chars().count() <= 65);
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
        let rows = rows_from_snapshot(&data, &translation());
        assert_eq!(rows.len(), 2);
        assert!(rows[0].reference.book <= rows[1].reference.book);
    }

    #[test]
    fn activity_covers_every_chapter_any_mark_touches() {
        let data = StudyData {
            highlights: vec![(verse(19, 23, 1), HighlightColour::Yellow)],
            notes: vec![(verse(19, 23, 2), Note::new("same chapter"))],
            bookmarks: vec![(verse(1, 1, 1), Bookmark::unlabelled())],
            settings: Default::default(),
        };
        let activity = chapter_activity(&data);
        assert_eq!(activity.len(), 2);
        assert!(activity.contains(&(19, 23)));
        assert!(activity.contains(&(1, 1)));
    }

    #[test]
    fn reveal_opens_only_the_testament_of_the_place() {
        let mut state = SidebarState::default();
        let john = Reference::new(42, 3, None).expect("John 3 is canonical");
        state.reveal(john);
        assert!(!state.old_open);
        assert!(state.new_open);
        assert_eq!(state.expanded_book, Some(42));
        assert!(state.filter.is_empty());
    }

    #[test]
    fn the_filter_matches_names_and_abbreviations() {
        let mut state = SidebarState::default();
        let genesis = books::book(0).expect("Genesis is canonical");
        state.filter = "gene".to_owned();
        assert!(state.book_visible(genesis));
        state.filter = "gen".to_owned();
        assert!(state.book_visible(genesis));
        state.filter = "zzz".to_owned();
        assert!(!state.book_visible(genesis));
        state.filter = "  ".to_owned();
        assert!(state.book_visible(genesis));
    }
}
