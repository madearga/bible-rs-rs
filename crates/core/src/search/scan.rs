// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The scanner that turns a query into matching verses.
//!
//! There is no index. The scanner builds one needle per term from the normalized
//! query, runs the SIMD substring search from `memchr` over the normalized corpus,
//! and maps every hit to its verse with a binary search. After a hit it jumps to
//! the next verse, so a term that fills a verse costs one match, not thousands.
//!
//! For a multi-term query the longest required term is scanned first, because a
//! longer needle is a stronger filter. The remaining terms and the excluded terms
//! are then checked against each candidate verse's wrapped line. Because every
//! verse line is space-wrapped, a term that is checked inside one line cannot
//! match across a verse boundary.

use memchr::memmem;

use crate::books::{self, Testament};

use super::corpus::Corpus;
use super::query::{MatchMode, Query, Scope};

/// One matching verse in canonical order.
///
/// The index is the flat verse index shared by every translation, so the reader
/// can map it to a book, chapter, and verse with the canonical grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Canonical verse index, zero for the first verse of Genesis.
    pub verse: u32,
}

/// The half-open range of canonical verse indices a scope covers.
struct VerseRange {
    start: usize,
    end: usize,
}

impl VerseRange {
    /// An empty range, used when a scope names a book that does not exist.
    fn empty() -> Self {
        Self { start: 0, end: 0 }
    }
}

impl Corpus {
    /// Finds every verse that satisfies a query, in canonical order.
    ///
    /// An empty query, or a query made only of excluded terms, returns nothing,
    /// because there is no required term to scan for.
    pub fn search(&self, query: &Query) -> Vec<Hit> {
        if query.is_empty() {
            return Vec::new();
        }

        let needles: Vec<String> = query
            .terms()
            .iter()
            .map(|term| needle(term.text(), query.mode()))
            .collect();

        let mut included = Vec::new();
        let mut excluded = Vec::new();
        for (index, term) in query.terms().iter().enumerate() {
            if term.is_excluded() {
                excluded.push(index);
            } else {
                included.push(index);
            }
        }
        if included.is_empty() {
            return Vec::new();
        }

        let first = included
            .iter()
            .copied()
            .max_by_key(|&index| needles[index].len())
            .unwrap_or(included[0]);
        let range = scope_range(query.scope());
        if range.start >= range.end {
            return Vec::new();
        }

        let bytes = self.normalized().as_bytes();
        let hay = &bytes[self.verse_start(range.start)..self.verse_start(range.end)];
        let finder = memmem::Finder::new(needles[first].as_bytes());
        let mut hits = Vec::new();
        let mut cursor = 0;
        while cursor < hay.len() {
            let Some(relative) = finder.find(&hay[cursor..]) else {
                break;
            };
            let verse = self.verse_at(self.verse_start(range.start) + cursor + relative);
            // A hit jumps to the next verse's leading space, so every verse is
            // visited at most once and the scan never rescans a matching verse.
            cursor = self.verse_start(verse + 1) - self.verse_start(range.start);
            if self.verse_matches(verse, &needles, &included, &excluded, first) {
                hits.push(Hit {
                    verse: u32::try_from(verse).unwrap_or(u32::MAX),
                });
            }
        }
        hits
    }

    /// Whether one verse satisfies every required and every excluded term.
    fn verse_matches(
        &self,
        verse: usize,
        needles: &[String],
        included: &[usize],
        excluded: &[usize],
        first: usize,
    ) -> bool {
        let line = self.verse_slice(verse).as_bytes();
        let found = |index: usize| memmem::find(line, needles[index].as_bytes()).is_some();
        included
            .iter()
            .filter(|&&index| index != first)
            .all(|&index| found(index))
            && !excluded.iter().any(|&index| found(index))
    }
}

/// Builds the needle a term is scanned for.
///
/// Every needle starts with a space because the corpus wraps each verse. Whole-word
/// mode adds the closing space; prefix mode leaves it off so a longer word matches.
fn needle(text: &str, mode: MatchMode) -> String {
    let mut needle = String::with_capacity(text.len() + 2);
    needle.push(' ');
    needle.push_str(text);
    if mode == MatchMode::WholeWord {
        needle.push(' ');
    }
    needle
}

/// The canonical verse range a scope covers.
fn scope_range(scope: Scope) -> VerseRange {
    match scope {
        Scope::WholeBible => VerseRange {
            start: 0,
            end: books::VERSE_COUNT,
        },
        Scope::OldTestament => VerseRange {
            start: 0,
            end: new_testament_start(),
        },
        Scope::NewTestament => VerseRange {
            start: new_testament_start(),
            end: books::VERSE_COUNT,
        },
        Scope::Book(index) => match books::book(index) {
            Some(book) => VerseRange {
                start: book.verse_start as usize,
                end: books::book(book.index + 1)
                    .map_or(books::VERSE_COUNT, |next| next.verse_start as usize),
            },
            None => VerseRange::empty(),
        },
    }
}

/// The canonical verse index where the New Testament begins.
fn new_testament_start() -> usize {
    books::books()
        .iter()
        .find(|book| book.testament == Testament::New)
        .map_or(books::VERSE_COUNT, |book| book.verse_start as usize)
}
