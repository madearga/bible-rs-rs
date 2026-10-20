// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The search panel that opens on command-F.
//!
//! The panel owns the input and the result list. It refreshes the list from the
//! core [`Corpus`] whenever the input changes, keeps a selection the arrow keys
//! move, and returns the selected verse so the shell can jump to it. Every result
//! carries the byte ranges of the matching words in its verse text, so the panel
//! paints the hits highlighted and the reader can flash the verse.

use bible_core::{Corpus, Hit, Query, Reference, Translation, books};
use eframe::egui;

use crate::theme::Palette as Colours;

/// How many results the panel keeps, which bounds the work a paint does.
pub const RESULT_LIMIT: usize = 200;

/// A byte range inside a verse's display text that matched the query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordRange {
    /// The first byte of the match.
    pub start: usize,
    /// One past the last byte of the match.
    pub end: usize,
}

/// One verse that matched, with the ranges the panel highlights.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// The flat canonical verse the match landed on.
    pub hit: Hit,
    /// The same place written as a reference.
    pub reference: Reference,
    /// The byte ranges of the matching words in the verse text.
    pub ranges: Vec<WordRange>,
}

/// The search panel and its result list.
#[derive(Clone, Debug, Default)]
pub struct SearchPanel {
    open: bool,
    input: String,
    results: Vec<SearchResult>,
    selected: usize,
    computed: Option<String>,
}

impl SearchPanel {
    /// A closed panel with no results.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the panel and keeps the previous input so a reopen is cheap.
    pub fn open(&mut self) {
        self.open = true;
        self.selected = 0;
    }

    /// Closes the panel.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the panel is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The text in the panel's input.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Replaces the input and resets the selection to the first result.
    pub fn set_input(&mut self, input: impl Into<String>) {
        self.input = input.into();
        self.selected = 0;
        self.computed = None;
    }

    /// The results for the current input.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }

    /// The index of the selected result.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The verse the selected result points at.
    pub fn selected_reference(&self) -> Option<Reference> {
        self.results
            .get(self.selected)
            .map(|result| result.reference)
    }

    /// Moves the selection by a number of rows, stopping at the ends.
    pub fn move_selection(&mut self, delta: i32) {
        if self.results.is_empty() {
            self.selected = 0;
            return;
        }
        let last = self.results.len() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }

    /// Recomputes the results when the input changed since the last search.
    ///
    /// The panel keeps the input it searched for, so a repaint that did not touch
    /// the input costs nothing and typing costs one scan.
    pub fn refresh(&mut self, corpus: &Corpus, translation: &Translation) {
        if self.computed.as_deref() == Some(self.input.as_str()) {
            return;
        }
        self.computed = Some(self.input.clone());
        self.results.clear();
        self.selected = 0;
        let query = Query::parse(&self.input);
        if query.is_empty() {
            return;
        }
        for hit in corpus.search(&query).into_iter().take(RESULT_LIMIT) {
            let Some(reference) = reference_of_verse(hit.verse) else {
                continue;
            };
            let Some(verse) = translation.verse(&reference) else {
                continue;
            };
            let text = translation.verse_text(verse);
            let ranges = ranges_for(text, &query);
            self.results.push(SearchResult {
                hit,
                reference,
                ranges,
            });
        }
    }

    /// Paints the panel and returns the verse the reader accepted.
    ///
    /// The arrow keys move the selection, Return accepts, and a click both
    /// selects and accepts. Escape closes the panel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        colours: Colours,
        corpus: &Corpus,
        translation: &Translation,
    ) -> Option<Reference> {
        if !self.open {
            return None;
        }
        self.refresh(corpus, translation);
        let mut chosen = None;
        egui::Area::new(egui::Id::new("search-panel"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 70.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ctx.global_style().as_ref())
                    .fill(colours.paper)
                    .stroke(egui::Stroke::new(1.0, colours.rule))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(560.0);
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .hint_text("Search the translation")
                                .desired_width(f32::INFINITY),
                        );
                        if !response.has_focus() {
                            response.request_focus();
                        }
                        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                            self.open = false;
                            return;
                        }
                        if ctx.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
                            self.move_selection(1);
                        }
                        if ctx.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
                            self.move_selection(-1);
                        }
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                self.results.len(),
                                if self.results.len() == 1 {
                                    "verse"
                                } else {
                                    "verses"
                                }
                            ))
                            .font(egui::FontId::new(11.0, crate::theme::sans()))
                            .color(colours.muted),
                        );
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .show(ui, |ui| {
                                for index in 0..self.results.len() {
                                    let result = &self.results[index];
                                    let Some(verse) = translation.verse(&result.reference) else {
                                        continue;
                                    };
                                    let text = translation.verse_text(verse);
                                    let job = layout_job(
                                        text,
                                        &result.ranges,
                                        index == self.selected,
                                        colours,
                                    );
                                    let label =
                                        ui.add(egui::Label::new(job).sense(egui::Sense::click()));
                                    if label.clicked() {
                                        self.selected = index;
                                        chosen = Some(result.reference);
                                    }
                                }
                            });
                        if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
                            chosen = self.selected_reference();
                        }
                    });
            });
        if chosen.is_some() {
            self.open = false;
        }
        chosen
    }
}

/// Builds a labelled run with the matching words in the accent colour.
fn layout_job(
    text: &str,
    ranges: &[WordRange],
    selected: bool,
    colours: Colours,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let base = egui::TextFormat {
        font_id: egui::FontId::new(13.0, crate::theme::body_family()),
        color: if selected { colours.ink } else { colours.muted },
        ..Default::default()
    };
    let hit = egui::TextFormat {
        font_id: egui::FontId::new(13.0, crate::theme::body_family()),
        color: colours.accent,
        background: colours.selection,
        ..Default::default()
    };
    let mut cursor = 0;
    for range in ranges {
        if range.start >= text.len() || range.end > text.len() || range.start < cursor {
            continue;
        }
        if range.start > cursor {
            job.append(&text[cursor..range.start], 0.0, base.clone());
        }
        job.append(&text[range.start..range.end], 0.0, hit.clone());
        cursor = range.end;
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, base);
    }
    job
}

/// Finds the byte ranges of the query's terms inside a verse text.
///
/// Matching is case-insensitive and starts on a word boundary, which mirrors the
/// prefix matching the corpus scan uses. Overlapping ranges are merged so the
/// panel paints one span per matched word.
pub fn ranges_for(text: &str, query: &Query) -> Vec<WordRange> {
    let lowered = text.to_lowercase();
    let mut ranges = Vec::new();
    for term in query.terms() {
        if term.is_excluded() {
            continue;
        }
        let needle = term.text();
        if needle.is_empty() {
            continue;
        }
        let mut from = 0;
        while let Some(relative) = lowered[from..].find(needle) {
            let start = from + relative;
            let end = start + needle.len();
            let boundary = start == 0
                || !lowered[..start]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric);
            if boundary && end <= lowered.len() {
                ranges.push(WordRange { start, end });
            }
            from = end.max(start + 1);
            if from >= lowered.len() {
                break;
            }
        }
    }
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<WordRange> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// Turns a flat canonical verse index into a reference.
fn reference_of_verse(verse: u32) -> Option<Reference> {
    let verse = usize::try_from(verse).ok()?;
    let book = books::books()
        .iter()
        .find(|book| {
            let start = book.verse_start as usize;
            let end = start + chapter_verse_total(book);
            verse >= start && verse < end
        })
        .copied()?;
    let mut remaining = verse - book.verse_start as usize;
    for chapter in 1..=book.chapter_count {
        let count = books::verses_in_chapter(book.index, chapter)? as usize;
        if remaining < count {
            let number = u8::try_from(remaining + 1).ok()?;
            return Reference::new(book.index, chapter, Some(number)).ok();
        }
        remaining -= count;
    }
    None
}

/// How many verse slots a whole book fills.
fn chapter_verse_total(book: &books::Book) -> usize {
    (1..=book.chapter_count)
        .filter_map(|chapter| books::verses_in_chapter(book.index, chapter))
        .map(usize::from)
        .sum()
}

#[cfg(test)]
mod tests {
    use bible_core::{Translation, TranslationBuilder, TranslationMeta, books};

    use super::*;

    /// Builds a whole synthetic translation whose text names its own place.
    fn synthetic_bible() -> Translation {
        let mut builder = TranslationBuilder::new(TranslationMeta::new("synthetic", "Synthetic"));
        for book in books::books() {
            for chapter in 1..=book.chapter_count {
                let count = books::verses_in_chapter(book.index, chapter).unwrap_or(0);
                for number in 1..=count as u8 {
                    let text = format!(
                        "alpha{book_index} beta{chapter} gamma{number} delta",
                        book_index = book.index
                    );
                    builder
                        .push_verse(number, &text)
                        .expect("the grid accepts it");
                }
            }
        }
        builder.finish().expect("the synthetic grid is complete")
    }

    #[test]
    fn a_query_returns_results_with_highlight_ranges() {
        let translation = synthetic_bible();
        let corpus = Corpus::from_translation(&translation);
        let mut panel = SearchPanel::new();
        panel.set_input("gamma16");
        panel.refresh(&corpus, &translation);
        assert!(!panel.results().is_empty());
        let result = &panel.results()[0];
        assert!(!result.ranges.is_empty());
        let verse = translation
            .verse(&result.reference)
            .expect("the verse exists");
        let text = translation.verse_text(verse);
        let range = result.ranges[0];
        assert_eq!(&text[range.start..range.end], "gamma16");
    }

    #[test]
    fn the_selection_moves_with_the_arrow_keys() {
        let translation = synthetic_bible();
        let corpus = Corpus::from_translation(&translation);
        let mut panel = SearchPanel::new();
        panel.set_input("alpha42");
        panel.refresh(&corpus, &translation);
        assert!(panel.results().len() > 1);
        assert_eq!(panel.selected(), 0);
        panel.move_selection(1);
        assert_eq!(panel.selected(), 1);
        panel.move_selection(-1);
        assert_eq!(panel.selected(), 0);
        panel.move_selection(-1);
        assert_eq!(panel.selected(), 0);
        panel.move_selection(10_000);
        assert_eq!(panel.selected(), panel.results().len() - 1);
    }

    #[test]
    fn an_empty_input_clears_the_results() {
        let translation = synthetic_bible();
        let corpus = Corpus::from_translation(&translation);
        let mut panel = SearchPanel::new();
        panel.set_input("alpha0");
        panel.refresh(&corpus, &translation);
        assert!(!panel.results().is_empty());
        panel.set_input("");
        panel.refresh(&corpus, &translation);
        assert!(panel.results().is_empty());
    }

    #[test]
    fn a_flat_verse_index_maps_to_its_reference() {
        let reference = Reference::new(42, 3, Some(16)).expect("canonical");
        let offset = books::verse_offset(42, 3, 16).expect("the grid knows it") as u32;
        assert_eq!(reference_of_verse(offset), Some(reference));
    }
}
