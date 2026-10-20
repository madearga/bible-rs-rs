// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The back and forward history of visited places.
//!
//! The history is a bounded list of references with a cursor. Visiting a place
//! drops everything ahead of the cursor, so forward history never points at a
//! place the reader abandoned, and the oldest entry is dropped when the list
//! reaches [`HISTORY_DEPTH`].

use std::collections::VecDeque;

use bible_core::Reference;

/// How many places the history keeps before it drops the oldest.
pub const HISTORY_DEPTH: usize = 32;

/// The places the reader has visited, with a cursor at the current one.
///
/// References are stored chapter-only because that is the unit the reader moves
/// by, so browsing within a chapter does not fill the history with noise.
#[derive(Clone, Debug)]
pub struct History {
    entries: VecDeque<Reference>,
    cursor: usize,
}

impl History {
    /// Starts a history with the place the reader opens at.
    pub fn new(reference: Reference) -> Self {
        let mut entries = VecDeque::with_capacity(HISTORY_DEPTH);
        entries.push_back(reference.chapter_only());
        Self { entries, cursor: 0 }
    }

    /// Records a visit and returns whether the history moved.
    ///
    /// Visiting the current place is a no-op, which keeps `back` from appearing
    /// to do nothing after a repeated jump. A new visit removes the entries
    /// ahead of the cursor and then appends.
    pub fn visit(&mut self, reference: Reference) -> bool {
        let reference = reference.chapter_only();
        if self.current() == Some(reference) {
            return false;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push_back(reference);
        self.cursor = self.entries.len() - 1;
        while self.entries.len() > HISTORY_DEPTH {
            self.entries.pop_front();
            self.cursor = self.cursor.saturating_sub(1);
        }
        true
    }

    /// The place the cursor points at.
    pub fn current(&self) -> Option<Reference> {
        self.entries.get(self.cursor).copied()
    }

    /// Moves back one place and returns it, or `None` at the oldest entry.
    pub fn back(&mut self) -> Option<Reference> {
        let previous = self.cursor.checked_sub(1)?;
        self.cursor = previous;
        self.current()
    }

    /// Moves forward one place and returns it, or `None` at the newest entry.
    pub fn forward(&mut self) -> Option<Reference> {
        if self.cursor + 1 >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        self.current()
    }

    /// Whether there is a place behind the cursor.
    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    /// Whether there is a place ahead of the cursor.
    pub fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// How many places the history holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history somehow holds nothing, which only happens before the
    /// first place is recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The places the history holds, oldest first.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = Reference> + '_ {
        self.entries.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chapter reference for a book and chapter, used to keep the tests short.
    fn place(book: u8, chapter: u8) -> Reference {
        Reference::new(book, chapter, None).expect("the place is canonical")
    }

    #[test]
    fn back_returns_to_the_previous_place() {
        let mut history = History::new(place(42, 3));
        history.visit(place(43, 1));
        history.visit(place(18, 23));
        assert_eq!(history.back(), Some(place(43, 1)));
        assert_eq!(history.back(), Some(place(42, 3)));
        assert_eq!(history.back(), None);
    }

    #[test]
    fn forward_returns_again_and_stops_at_the_end() {
        let mut history = History::new(place(42, 3));
        history.visit(place(43, 1));
        history.visit(place(18, 23));
        assert!(history.back().is_some());
        assert!(history.back().is_some());
        assert_eq!(history.forward(), Some(place(43, 1)));
        assert_eq!(history.forward(), Some(place(18, 23)));
        assert_eq!(history.forward(), None);
        assert!(!history.can_go_forward());
    }

    #[test]
    fn a_new_visit_drops_the_forward_places() {
        let mut history = History::new(place(42, 3));
        history.visit(place(43, 1));
        history.back();
        history.visit(place(19, 1));
        assert!(!history.can_go_forward());
        assert_eq!(history.current(), Some(place(19, 1)));
    }

    #[test]
    fn the_history_is_capped_and_drops_the_oldest() {
        let mut history = History::new(place(0, 1));
        for chapter in 2..=(HISTORY_DEPTH as u8 + 10) {
            history.visit(place(0, chapter));
        }
        assert_eq!(history.len(), HISTORY_DEPTH);
        assert_eq!(history.current(), Some(place(0, HISTORY_DEPTH as u8 + 10)));
        while history.back().is_some() {}
        let oldest = history.current().expect("the history still holds places");
        assert!(
            oldest.chapter > 1,
            "the first place should have been dropped"
        );
    }
}
