// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The jump palette that opens on command-K.
//!
//! The palette accepts a reference with a fuzzy book name, so `jn 3:16`,
//! `1co13`, and `revalation 21` all resolve. It also lists recent places, the
//! installed translations, and the app's commands. Resolving a reference is
//! delegated to [`Reference::parse`], which owns the tolerant book matcher; the
//! scoring in this file ranks the suggestions the palette shows while the reader
//! is still typing.

use bible_core::Reference;
use bible_core::books;
use eframe::egui;

use crate::shortcuts::{self, Action};
use crate::theme::Palette as Colours;

/// One book the input could mean, with the score the matcher gave it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// The canonical book index.
    pub book: u8,
    /// Higher is a closer match. A score of 1000 is an exact name.
    pub score: i32,
}

/// What the palette currently offers as a choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// Jump to a place.
    Jump(Reference),
    /// Jump to a place the reader visited earlier.
    Recent(Reference),
    /// Switch to an installed translation.
    Translation {
        /// The translation's stable identifier.
        id: String,
        /// What the row shows.
        label: String,
    },
    /// Run one of the app's commands.
    Command(Action),
}

impl Entry {
    /// The text the palette paints for this row.
    pub fn label(&self) -> String {
        match self {
            Self::Jump(reference) | Self::Recent(reference) => reference.to_string(),
            Self::Translation { label, .. } => label.clone(),
            Self::Command(action) => action.label().to_string(),
        }
    }
}

/// What happens when the reader accepts the selected palette row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Move the reader to a place.
    Jump(Reference),
    /// Load a translation by identifier.
    Translation(String),
    /// Run an app command.
    Command(Action),
}

/// The jump palette and its current input.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    open: bool,
    input: String,
    selected: usize,
    entries: Vec<Entry>,
}

impl Palette {
    /// A closed palette with no input, which is how a fresh window starts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the palette and clears the previous input.
    pub fn open(&mut self) {
        self.open = true;
        self.input.clear();
        self.selected = 0;
        self.entries.clear();
    }

    /// Closes the palette.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the palette is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The text in the palette's input.
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Replaces the input, which also resets the selection to the first row.
    pub fn set_input(&mut self, input: impl Into<String>) {
        self.input = input.into();
        self.selected = 0;
    }

    /// The rows the palette is offering.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The index of the selected row.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Moves the selection by a number of rows, stopping at the ends.
    pub fn move_selection(&mut self, delta: i32) {
        if self.entries.is_empty() {
            self.selected = 0;
            return;
        }
        let last = self.entries.len() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }

    /// The choice under the cursor, if any.
    pub fn accept(&self) -> Option<Choice> {
        match self.entries.get(self.selected)? {
            Entry::Jump(reference) | Entry::Recent(reference) => Some(Choice::Jump(*reference)),
            Entry::Translation { id, .. } => Some(Choice::Translation(id.clone())),
            Entry::Command(action) => Some(Choice::Command(*action)),
        }
    }

    /// Rebuilds the rows from the input and the surrounding state.
    ///
    /// `recent` is the history newest first and `translations` is a pair of the
    /// identifier and the display label for each installed translation.
    pub fn refresh(&mut self, recent: &[Reference], translations: &[(String, String)]) {
        self.entries.clear();
        let query = self.input.trim();
        if query.is_empty() {
            for reference in recent {
                self.entries.push(Entry::Recent(*reference));
            }
            for (id, label) in translations {
                self.entries.push(Entry::Translation {
                    id: id.clone(),
                    label: label.clone(),
                });
            }
            for action in shortcuts::commands() {
                self.entries.push(Entry::Command(action));
            }
        } else {
            if let Some(reference) = resolve(query) {
                self.entries.push(Entry::Jump(reference));
            }
            for suggestion in suggest(query).iter().take(5) {
                let Some(book) = books::book(suggestion.book) else {
                    continue;
                };
                // The first chapter of the book, so a book-only suggestion still
                // lands somewhere a reader can read.
                let Ok(reference) = Reference::new(book.index, 1, None) else {
                    continue;
                };
                if !self.entries.contains(&Entry::Jump(reference)) {
                    self.entries.push(Entry::Jump(reference));
                }
            }
            let folded = query.to_lowercase();
            for (id, label) in translations {
                if label.to_lowercase().contains(&folded) {
                    self.entries.push(Entry::Translation {
                        id: id.clone(),
                        label: label.clone(),
                    });
                }
            }
            for action in shortcuts::commands() {
                if action.label().to_lowercase().contains(&folded) {
                    self.entries.push(Entry::Command(action));
                }
            }
        }
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    /// Paints the palette and returns a choice when the reader accepts a row.
    ///
    /// The input takes focus every frame the palette is open, so typing lands in
    /// the palette rather than in the reading surface. Return and a click accept
    /// the selected row, and Escape closes the palette.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        colours: Colours,
        recent: &[Reference],
        translations: &[(String, String)],
    ) -> Option<Choice> {
        if !self.open {
            return None;
        }
        self.refresh(recent, translations);

        let mut chosen = None;
        egui::Area::new(egui::Id::new("jump-palette"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 90.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ctx.global_style().as_ref())
                    .fill(colours.paper)
                    .stroke(egui::Stroke::new(1.0, colours.rule))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(460.0);
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.input)
                                .hint_text("Jump to a reference, a translation, or a command")
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

                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .max_height(320.0)
                            .show(ui, |ui| {
                                for index in 0..self.entries.len() {
                                    let label = self.entries[index].label();
                                    let selected = index == self.selected;
                                    let response = ui.selectable_label(
                                        selected,
                                        egui::RichText::new(label)
                                            .font(egui::FontId::new(13.0, crate::theme::sans()))
                                            .color(if selected {
                                                colours.ink
                                            } else {
                                                colours.muted
                                            }),
                                    );
                                    if response.clicked() {
                                        self.selected = index;
                                        chosen = self.accept();
                                    }
                                }
                            });

                        if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
                            chosen = self.accept();
                        }
                    });
            });
        if chosen.is_some() {
            self.open = false;
        }
        chosen
    }
}

/// Resolves a reference the reader typed, or `None` when nothing matches.
///
/// A full reference goes through the core parser, which owns typo tolerance. An
/// input that is only a book name resolves to the first chapter of the closest
/// book, but only when the match is strong enough that it cannot be a guess.
pub fn resolve(input: &str) -> Option<Reference> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(reference) = Reference::parse(trimmed) {
        return Some(reference);
    }
    let best = suggest(trimmed).first().copied()?;
    if best.score < 880 {
        return None;
    }
    Reference::new(best.book, 1, None).ok()
}

/// Scores the books against an input, closest first.
///
/// The match runs over the book's name, its abbreviation, and the alternate
/// spellings, so a short form and a misspelling both land on the right book. A
/// reference the core parser already understands is boosted to the top, which
/// keeps this ranking consistent with what [`resolve`] accepts.
pub fn suggest(input: &str) -> Vec<Suggestion> {
    let query = normalize(input);
    if query.is_empty() {
        return Vec::new();
    }
    let mut suggestions: Vec<Suggestion> = Vec::new();
    for book in books::books() {
        if let Some(score) = score_book(&query, book.index) {
            suggestions.push(Suggestion {
                book: book.index,
                score,
            });
        }
    }
    if let Ok(reference) = Reference::parse(input.trim())
        && let Some(existing) = suggestions
            .iter_mut()
            .find(|suggestion| suggestion.book == reference.book)
    {
        existing.score = existing.score.max(1000);
    }
    suggestions.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.book.cmp(&right.book))
    });
    suggestions
}

/// Scores how well one book matches a normalized query.
///
/// The best of the book's keys wins. An exact name scores 1000, a prefix scores
/// just below it, a typo costs 140 per edit, and a subsequence match scores
/// lower again because it is the weakest evidence.
pub fn score_book(query: &str, book: u8) -> Option<i32> {
    let entry = books::book(book)?;
    let mut best: Option<i32> = None;
    let mut consider = |key: &str| {
        if let Some(score) = score_key(query, &normalize(key)) {
            best = Some(best.map_or(score, |current: i32| current.max(score)));
        }
    };
    consider(entry.name);
    consider(entry.abbreviation);
    for (alternate, index) in ALTERNATE_SPELLINGS {
        if *index == book {
            consider(alternate);
        }
    }
    best
}

/// Scores one query against one candidate key.
fn score_key(query: &str, key: &str) -> Option<i32> {
    if key.is_empty() {
        return None;
    }
    if query == key {
        return Some(1000);
    }
    if let Some(rest) = key.strip_prefix(query) {
        return Some(900 - rest.len() as i32);
    }
    if let Some(rest) = query.strip_prefix(key) {
        return Some(850 - rest.len() as i32);
    }
    let distance = edit_distance(query, key);
    let typo = (distance <= 3).then(|| 780 - distance as i32 * 140);
    let subsequence = subsequence_score(query, key);
    match (typo, subsequence) {
        (Some(typo), Some(sequence)) => Some(typo.max(sequence)),
        (Some(typo), None) => Some(typo),
        (None, Some(sequence)) => Some(sequence),
        (None, None) => None,
    }
}

/// Scores a query whose letters appear in order inside the key.
fn subsequence_score(query: &str, key: &str) -> Option<i32> {
    let mut chars = key.chars().enumerate();
    let mut last = None;
    for wanted in query.chars() {
        let mut found = None;
        for (index, candidate) in chars.by_ref() {
            if candidate == wanted {
                found = Some(index);
                break;
            }
        }
        last = Some(found?);
    }
    let last = last?;
    let gaps = last.saturating_sub(query.chars().count() - 1);
    Some(600 - gaps as i32 * 25 - (key.chars().count() - query.chars().count()) as i32 * 5)
}

/// Levenshtein distance between two normalized strings.
fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0usize; right.len() + 1];
    for (i, a) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(a != b);
            let insertion = current[j] + 1;
            let deletion = previous[j + 1] + 1;
            current[j + 1] = substitution.min(insertion).min(deletion);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// Lower-cases a name and drops everything that is not a letter or a digit.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Alternates and abbreviations readers type that are not the short name the
/// table stores. They are keys for scoring only; the core parser still decides
/// what a finished reference means.
static ALTERNATE_SPELLINGS: &[(&str, u8)] = &[
    ("gen", 0),
    ("ge", 0),
    ("gn", 0),
    ("ex", 1),
    ("exod", 1),
    ("lev", 2),
    ("lv", 2),
    ("num", 3),
    ("nm", 3),
    ("deut", 4),
    ("dt", 4),
    ("josh", 5),
    ("judg", 6),
    ("jdg", 6),
    ("ru", 7),
    ("rth", 7),
    ("1sa", 8),
    ("1sam", 8),
    ("2sa", 9),
    ("2sam", 9),
    ("1ki", 10),
    ("1kgs", 10),
    ("2ki", 11),
    ("2kgs", 11),
    ("1ch", 12),
    ("1chr", 12),
    ("2ch", 13),
    ("2chr", 13),
    ("ezr", 14),
    ("ne", 15),
    ("neh", 15),
    ("es", 16),
    ("esth", 16),
    ("jb", 17),
    ("ps", 18),
    ("psalm", 18),
    ("psalms", 18),
    ("pss", 18),
    ("pr", 19),
    ("prov", 19),
    ("prv", 19),
    ("ec", 20),
    ("ecc", 20),
    ("eccl", 20),
    ("qoh", 20),
    ("song", 21),
    ("sos", 21),
    ("isa", 22),
    ("is", 22),
    ("jer", 23),
    ("jr", 23),
    ("lam", 24),
    ("eze", 25),
    ("ezek", 25),
    ("dan", 26),
    ("dn", 26),
    ("hos", 27),
    ("ho", 27),
    ("joe", 28),
    ("jl", 28),
    ("am", 29),
    ("ob", 30),
    ("obad", 30),
    ("jon", 31),
    ("jnh", 31),
    ("mic", 32),
    ("mi", 32),
    ("nah", 33),
    ("na", 33),
    ("hab", 34),
    ("hb", 34),
    ("zep", 35),
    ("zeph", 35),
    ("hag", 36),
    ("hg", 36),
    ("zec", 37),
    ("zech", 37),
    ("mal", 38),
    ("ml", 38),
    ("mt", 39),
    ("matt", 39),
    ("mk", 40),
    ("mr", 40),
    ("lk", 41),
    ("luk", 41),
    ("jn", 42),
    ("joh", 42),
    ("ac", 43),
    ("ro", 44),
    ("rm", 44),
    ("1co", 45),
    ("1cor", 45),
    ("2co", 46),
    ("2cor", 46),
    ("gal", 47),
    ("ga", 47),
    ("eph", 48),
    ("php", 49),
    ("phil", 49),
    ("col", 50),
    ("1th", 51),
    ("1thess", 51),
    ("2th", 52),
    ("2thess", 52),
    ("1ti", 53),
    ("1tim", 53),
    ("2ti", 54),
    ("2tim", 54),
    ("tit", 55),
    ("phm", 56),
    ("philem", 56),
    ("heb", 57),
    ("jas", 58),
    ("jam", 58),
    ("1pe", 59),
    ("1pet", 59),
    ("2pe", 60),
    ("2pet", 60),
    ("1jn", 61),
    ("1john", 61),
    ("2jn", 62),
    ("2john", 62),
    ("3jn", 63),
    ("3john", 63),
    ("jud", 64),
    ("rev", 65),
    ("revelations", 65),
];
