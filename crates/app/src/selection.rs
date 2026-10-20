// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The verse selection model the study features hang off.
//!
//! A selection is a pair of verse references. One press sets both ends to the
//! same verse, a shift-press moves the far end to make a range, and a keypress
//! moves the whole selection one verse at a time without leaving the chapter.
//! The model holds no geometry, so it survives a resize and a font change.

use bible_core::{Reference, Verse};

/// Which way a keypress moves the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The next verse in the chapter, which the J key asks for.
    Next,
    /// The previous verse in the chapter, which the K key asks for.
    Previous,
}

/// A span of selected verses, from the anchor to the moving end.
///
/// The anchor is the verse the selection began on and the focus is the verse the
/// pointer or the keyboard last reached. Reporting a start and an end orders
/// them canonically, so a backwards range reads the same as a forwards one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    anchor: Option<Reference>,
    focus: Option<Reference>,
}

impl Selection {
    /// An empty selection, which is how a fresh window starts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.anchor.is_none()
    }

    /// Drops the selection.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.focus = None;
    }

    /// Selects one verse, which is what a plain click does.
    pub fn select(&mut self, reference: Reference) {
        self.anchor = Some(reference);
        self.focus = Some(reference);
    }

    /// Moves the far end of the selection to a verse, which is what a
    /// shift-click does. With nothing selected it selects the verse outright.
    pub fn extend_to(&mut self, reference: Reference) {
        match self.anchor {
            Some(_) => self.focus = Some(reference),
            None => self.select(reference),
        }
    }

    /// The anchor, the verse the selection began on.
    pub fn anchor(&self) -> Option<Reference> {
        self.anchor
    }

    /// The far end, the verse last reached.
    pub fn focus(&self) -> Option<Reference> {
        self.focus
    }

    /// The first verse of the range in canonical order, absent when nothing is
    /// selected.
    pub fn start(&self) -> Option<Reference> {
        self.range().map(|(start, _)| start)
    }

    /// The last verse of the range in canonical order, absent when nothing is
    /// selected.
    pub fn end(&self) -> Option<Reference> {
        self.range().map(|(_, end)| end)
    }

    /// The range as a start and an end, ordered so it reads forwards.
    pub fn range(&self) -> Option<(Reference, Reference)> {
        let (anchor, focus) = (self.anchor?, self.focus?);
        if key(anchor) <= key(focus) {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }

    /// Whether a verse falls inside the selection.
    pub fn contains(&self, reference: Reference) -> bool {
        self.range()
            .is_some_and(|(start, end)| key(reference) >= key(start) && key(reference) <= key(end))
    }

    /// Moves the selection one verse inside a chapter, which is what the J and K
    /// keys do.
    ///
    /// A range collapses to its focus before moving, and a move past the first or
    /// last verse of the chapter leaves the selection alone and returns `false`,
    /// so the selection stops at the chapter boundary.
    pub fn step(&mut self, direction: Step, chapter: &[Verse]) -> bool {
        let Some(focus) = self.focus else {
            return false;
        };
        let Some(index) = chapter
            .iter()
            .position(|verse| verse.number == focus.verse.unwrap_or(0))
        else {
            return false;
        };
        let next = match direction {
            Step::Next => index + 1,
            Step::Previous => match index.checked_sub(1) {
                Some(previous) => previous,
                None => return false,
            },
        };
        let Some(verse) = chapter.get(next) else {
            return false;
        };
        let moved = Reference {
            book: focus.book,
            chapter: focus.chapter,
            verse: Some(verse.number),
        };
        self.select(moved);
        true
    }
}

/// The canonical order of a reference, used to compare two ends of a range.
fn key(reference: Reference) -> (u8, u8, u8) {
    (
        reference.book,
        reference.chapter,
        reference.verse.unwrap_or(0),
    )
}
