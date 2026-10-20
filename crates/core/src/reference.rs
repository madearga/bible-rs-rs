// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Parsing and formatting verse references, tolerant of how a reader types.
//!
//! A reference is accepted in the shapes the jump palette receives: a full book
//! name, an abbreviation, an ordinal prefix such as `1` for `1 John`, and a
//! chapter with an optional verse. Book names are matched exactly first and then
//! within two edits, so a misspelling still resolves, while a chapter or verse
//! past the end of the canon is an error rather than a silent clamp.

use std::fmt;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::books::{self, Book};

/// A place in the canon: a book, a chapter, and optionally one verse in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    /// Canonical book index, Genesis `0` through Revelation `65`.
    pub book: u8,
    /// 1-based chapter number.
    pub chapter: u16,
    /// 1-based verse number, `None` when the reference names a whole chapter.
    pub verse: Option<u16>,
}

impl Reference {
    /// A reference to one verse, refused when the canon has no such verse.
    pub fn at_verse(book: u8, chapter: u16, verse: u16) -> Result<Self> {
        let reference = Self {
            book,
            chapter,
            verse: Some(verse),
        };
        reference.check()?;
        Ok(reference)
    }

    /// A reference to a whole chapter, refused when the canon has no such chapter.
    pub fn at_chapter(book: u8, chapter: u16) -> Result<Self> {
        let reference = Self {
            book,
            chapter,
            verse: None,
        };
        reference.check()?;
        Ok(reference)
    }

    /// The book this reference points into, or `None` when the book index is
    /// outside the canon.
    pub fn book_name(&self) -> Option<&'static str> {
        books::book(self.book).map(|book| book.name)
    }

    fn check(&self) -> Result<()> {
        let book: &'static Book = books::book(self.book)
            .ok_or_else(|| anyhow!("book {} is outside the canon", self.book))?;
        ensure!(
            self.chapter >= 1 && self.chapter <= book.chapters,
            "{} has no chapter {}",
            book.name,
            self.chapter
        );
        if let Some(verse) = self.verse {
            let count = books::verses_in_chapter(self.book, self.chapter).unwrap_or(0);
            ensure!(
                verse >= 1 && verse <= count,
                "{} {} has no verse {verse}",
                book.name,
                self.chapter
            );
        }
        Ok(())
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let book = self.book_name().unwrap_or("Unknown");
        match self.verse {
            Some(verse) => write!(formatter, "{book} {}:{verse}", self.chapter),
            None => write!(formatter, "{book} {}", self.chapter),
        }
    }
}

impl FromStr for Reference {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        parse(text)
    }
}

/// Parses a reference the way a reader types one.
pub fn parse(input: &str) -> Result<Reference> {
    let trimmed = input.trim();
    ensure!(!trimmed.is_empty(), "a reference needs a book");

    let lowered = trimmed.to_lowercase();
    let (ordinal, body) = split_ordinal(&lowered);
    let digits_at = body
        .find(|character: char| character.is_ascii_digit())
        .ok_or_else(|| anyhow!("{trimmed:?} has no chapter number"))?;
    let name = body[..digits_at].trim();
    ensure!(
        !name.is_empty(),
        "{trimmed:?} starts with a number but no book name"
    );

    let mut book_name = String::with_capacity(name.len() + 1);
    if let Some(ordinal) = ordinal {
        book_name.push(ordinal);
    }
    book_name.push_str(name);
    let book = books::find_book(&book_name)
        .ok_or_else(|| anyhow!("{book_name:?} is not a book of the canon"))?;

    let (chapter, verse) = split_chapter_and_verse(&body[digits_at..])?;
    let reference = Reference {
        book: book.index,
        chapter,
        verse,
    };
    reference.check()?;
    Ok(reference)
}

/// Separates an ordinal prefix such as the `1` of `1 John` from the book name.
fn split_ordinal(text: &str) -> (Option<char>, &str) {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return (None, text);
    };
    if !('1'..='3').contains(&first) {
        return (None, text);
    }
    let rest = text[first.len_utf8()..].trim_start();
    if rest.starts_with(char::is_alphabetic) {
        (Some(first), rest)
    } else {
        (None, text)
    }
}

/// Splits a chapter number and an optional verse number, refusing anything left
/// over so a garbled reference is an error rather than a guess.
fn split_chapter_and_verse(tail: &str) -> Result<(u16, Option<u16>)> {
    let (chapter_digits, after_chapter) = split_digits(tail);
    let chapter = parse_number(chapter_digits, "chapter")?;

    let after_chapter = after_chapter.trim_start();
    if after_chapter.is_empty() {
        return Ok((chapter, None));
    }
    let after_separator = match memchr::memchr(b':', after_chapter.as_bytes()) {
        Some(index) => &after_chapter[index + 1..],
        None => after_chapter.strip_prefix('.').unwrap_or(after_chapter),
    };
    let after_separator = after_separator.trim_start();
    ensure!(
        !after_separator.is_empty(),
        "chapter {chapter} is followed by a separator but no verse"
    );

    let (verse_digits, trailing) = split_digits(after_separator);
    ensure!(!verse_digits.is_empty(), "expected a verse number");
    ensure!(
        trailing.trim().is_empty(),
        "{trailing:?} is not part of a reference"
    );
    Ok((chapter, Some(parse_number(verse_digits, "verse")?)))
}

fn split_digits(text: &str) -> (&str, &str) {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    text.split_at(end)
}

fn parse_number(digits: &str, what: &str) -> Result<u16> {
    if digits.is_empty() {
        bail!("expected a {what} number");
    }
    digits
        .parse::<u16>()
        .with_context(|| format!("{what} number {digits:?} is out of range"))
}
