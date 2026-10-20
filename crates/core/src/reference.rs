// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Parsing and formatting of verse references.
//!
//! The parser accepts what a reader actually types: a full book name, a common
//! abbreviation, an ordinal prefix such as `1` in `1co13`, and a chapter with an
//! optional verse. Book names are matched tolerantly so a typo still resolves.
//! Every resolved reference is checked against the canonical grid.

use std::fmt;

use anyhow::{Result, bail};

use crate::books;

/// A place in the canon, with an optional verse.
///
/// A reference is translation independent. A chapter-only reference is how the
/// reader asks for a whole chapter; a verse reference names one verse slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    /// Canonical book index, Genesis at 0.
    pub book: u8,
    /// 1-based chapter number.
    pub chapter: u8,
    /// 1-based verse number, absent when the reference names only a chapter.
    pub verse: Option<u8>,
}

impl Reference {
    /// Builds a reference after checking the book, chapter, and verse exist.
    pub fn new(book: u8, chapter: u8, verse: Option<u8>) -> Result<Self> {
        let entry = books::book(book).ok_or_else(|| anyhow::anyhow!("no book at index {book}"))?;
        if chapter == 0 || chapter > entry.chapter_count {
            bail!("{} has no chapter {chapter}", entry.name);
        }
        if let Some(verse) = verse {
            let count = books::verses_in_chapter(book, chapter).unwrap_or(0);
            if verse == 0 || verse > count as u8 {
                bail!("{} {chapter} has no verse {verse}", entry.name);
            }
        }
        Ok(Self {
            book,
            chapter,
            verse,
        })
    }

    /// Parses a reference such as `jn 3:16`, `1co13`, `Rom 8`, or `revalation 21`.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            bail!("empty reference");
        }

        // Keep a leading ordinal with the book name, so `1co13` splits into
        // book `1co` and chapter `13` rather than book `` and numbers.
        let bytes = trimmed.as_bytes();
        let search_from = match (bytes.first(), bytes.get(1)) {
            (Some(first @ b'1'..=b'3'), Some(second)) if !second.is_ascii_digit() => {
                let _ = first;
                1
            }
            _ => 0,
        };

        let first_digit = bytes[search_from..]
            .iter()
            .position(u8::is_ascii_digit)
            .map(|offset| offset + search_from)
            .ok_or_else(|| anyhow::anyhow!("reference has no chapter: {input:?}"))?;

        let name = normalize(&trimmed[..first_digit]);
        let rest = &trimmed[first_digit..];
        if name.is_empty() {
            bail!("reference has no book: {input:?}");
        }
        let book = resolve_book(&name).ok_or_else(|| {
            anyhow::anyhow!("unknown book {:?} in {input:?}", &trimmed[..first_digit])
        })?;

        let (chapter, verse) = parse_numbers(rest)?;
        Reference::new(book, chapter, verse)
    }

    /// The first chapter of the same book, handy for chapter navigation.
    pub fn chapter_only(self) -> Self {
        Self {
            verse: None,
            ..self
        }
    }

    /// Whether the reference names a single verse rather than a whole chapter.
    pub fn is_verse(self) -> bool {
        self.verse.is_some()
    }
}

impl fmt::Display for Reference {
    /// Formats the reference the way the reader shows it, such as `John 3:16`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = books::book(self.book).map_or("Unknown", |book| book.name);
        match self.verse {
            Some(verse) => write!(f, "{name} {}:{verse}", self.chapter),
            None => write!(f, "{name} {}", self.chapter),
        }
    }
}

/// Splits the numeric tail into a chapter and an optional verse.
fn parse_numbers(rest: &str) -> Result<(u8, Option<u8>)> {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        bail!("chapter number missing in {rest:?}");
    }
    let chapter = parse_number(&rest[..digits], "chapter")?;

    let tail = &rest[digits..];
    if tail.trim().is_empty() {
        return Ok((chapter, None));
    }

    // A verse follows once the separator is consumed. `memchr` finds the colon
    // in the common `chapter:verse` form; spaces and dots are accepted too.
    let mut tail = tail.trim_start();
    if let Some(first) = tail.as_bytes().first() {
        if matches!(first, b':' | b'.' | b',') {
            tail = &tail[1..];
        } else if memchr::memchr(b':', tail.as_bytes()).is_some() {
            bail!("unexpected separator in {tail:?}");
        }
    }
    tail = tail.trim();
    let verse_digits = tail.bytes().take_while(u8::is_ascii_digit).count();
    if verse_digits == 0 || verse_digits != tail.len() {
        bail!("verse number missing or malformed in {tail:?}");
    }
    let verse = parse_number(tail, "verse")?;
    Ok((chapter, Some(verse)))
}

/// Parses a decimal number that must fit a chapter or verse slot.
fn parse_number(text: &str, what: &str) -> Result<u8> {
    let value: u32 = text
        .parse()
        .map_err(|_| anyhow::anyhow!("{what} {text:?} is not a number"))?;
    u8::try_from(value).map_err(|_| anyhow::anyhow!("{what} {value} is out of range"))
}

/// Lower-cases a book name and drops everything that is not alphanumeric.
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

/// Common spellings and abbreviations that fuzzy matching alone would confuse.
static ALIASES: &[(&str, u8)] = &[
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
    ("canticles", 21),
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
];

/// Finds the book a normalized name refers to, exact matches first and typos second.
fn resolve_book(name: &str) -> Option<u8> {
    if let Some((_, index)) = ALIASES.iter().find(|(alias, _)| *alias == name) {
        return Some(*index);
    }
    for book in books::books() {
        if normalize(book.name) == name || normalize(book.abbreviation) == name {
            return Some(book.index);
        }
    }
    if name.len() < 2 {
        return None;
    }

    // Typo tolerance: take the single closest key, but only when it is clearly
    // the best candidate. A name that is equally close to two books is ambiguous.
    let mut best_distance = 3;
    let mut best: Option<u8> = None;
    let mut tied = false;
    let mut consider = |key: &str, index: u8| {
        let distance = edit_distance(name, key);
        if distance < best_distance {
            best_distance = distance;
            best = Some(index);
            tied = false;
        } else if distance == best_distance && best != Some(index) {
            tied = true;
        }
    };
    for (alias, index) in ALIASES {
        consider(alias, *index);
    }
    for book in books::books() {
        consider(&normalize(book.name), book.index);
        consider(&normalize(book.abbreviation), book.index);
    }
    if tied { None } else { best }
}

/// Levenshtein distance between two ASCII strings, capped by the caller's threshold.
fn edit_distance(left: &str, right: &str) -> usize {
    let left = left.as_bytes();
    let right = right.as_bytes();
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
