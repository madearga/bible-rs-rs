// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The translation model shared by every importer and every reader.
//!
//! A translation is one UTF-8 string holding all its verse text plus flat arrays
//! laid out in canonical verse order. Chapter and verse lookups are slice indexes
//! into those arrays, so materialising a chapter allocates nothing.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::books::{self, CHAPTER_COUNT, VERSE_COUNT};
use crate::reference::Reference;

/// Metadata that describes a translation without touching its text.
///
/// The whole struct is serialised into the uncompressed header of the binary
/// format, so listing installed translations never decompresses the body.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationMeta {
    /// Stable identifier used for file names and settings, such as `kjv`.
    pub id: String,
    /// Short label for tabs and menus, such as `KJV`.
    pub abbreviation: String,
    /// Full display name, such as `King James Version`.
    pub name: String,
    /// BCP 47 language tag or a plain language name.
    pub language: String,
    /// One-line licence summary shown in the translation list.
    pub licence: String,
    /// Copyright notice that must travel with the text.
    pub copyright: String,
    /// Where the text came from, such as a file name or a download URL.
    pub provenance: String,
    /// How the translation reached this machine, such as `bundled` or `imported`.
    pub origin: String,
    /// Per-book display titles in canonical order, empty when the source had none.
    pub book_titles: Vec<String>,
    /// Whether the source marks the words of Christ.
    pub red_letters: bool,
    /// Whether the source marks paragraph breaks.
    pub paragraphs: bool,
    /// Whether the source carries section headings.
    pub headings: bool,
}

impl TranslationMeta {
    /// Creates metadata with the two fields every caller has, leaving the rest empty.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Self::default()
        }
    }
}

/// A structural break attached to a verse.
///
/// A mark records where a paragraph, poetry line, stanza, heading, or Psalm
/// superscription begins. The offset is in bytes from the start of the verse text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    /// A paragraph begins here.
    Paragraph = 0,
    /// A poetry line begins here.
    PoetryLine = 1,
    /// A poetry line is indented; the level carries the depth.
    PoetryIndent = 2,
    /// A stanza break precedes this position.
    StanzaBreak = 3,
    /// A section heading precedes the verse.
    Heading = 4,
    /// A Psalm superscription precedes the verse text.
    Superscription = 5,
}

impl MarkKind {
    /// The on-disk code for this kind.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Recovers a kind from its on-disk code, rejecting unknown codes.
    pub fn from_u8(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Paragraph,
            1 => Self::PoetryLine,
            2 => Self::PoetryIndent,
            3 => Self::StanzaBreak,
            4 => Self::Heading,
            5 => Self::Superscription,
            _ => return None,
        })
    }
}

/// One structural break inside a verse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// Byte offset from the start of the verse text.
    pub offset: u16,
    /// What kind of break this is.
    pub kind: MarkKind,
    /// Depth for an indented poetry line, zero otherwise.
    pub level: u8,
    /// Start of the mark's payload in the translation's extra string.
    pub payload_start: u32,
    /// Length of the payload in bytes, zero when the mark carries no text.
    pub payload_len: u32,
}

/// A character style applied to a byte range inside a verse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpanStyle {
    /// Words spoken by Christ.
    WordsOfChrist = 0,
    /// Words a translator supplied to make the sense clear.
    TranslatorAddition = 1,
    /// The divine name.
    DivineName = 2,
    /// The word Selah.
    Selah = 3,
}

impl SpanStyle {
    /// The on-disk code for this style.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Recovers a style from its on-disk code, rejecting unknown codes.
    pub fn from_u8(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::WordsOfChrist,
            1 => Self::TranslatorAddition,
            2 => Self::DivineName,
            3 => Self::Selah,
            _ => return None,
        })
    }
}

/// One styled byte range inside a verse; `end` is exclusive and relative to the verse start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// First byte of the styled range.
    pub start: u16,
    /// Byte just past the styled range.
    pub end: u16,
    /// The style applied to the range.
    pub style: SpanStyle,
}

/// One canonical verse slot in a translation.
///
/// The slot always exists for every verse on the grid, even when the translation
/// prints nothing for it. Text and side arrays are addressed by the stored ranges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verse {
    /// Printed verse number, usually the canonical number of the slot.
    pub number: u8,
    /// Last number of a printed range such as `13-14`; equal to `number` for a single verse.
    pub number_end: u8,
    /// Byte offset of the verse text in the translation's text string.
    pub text_start: u32,
    /// Length of the verse text in bytes.
    pub text_len: u16,
    /// Index of the verse's first mark in the translation's mark array.
    pub marks_start: u32,
    /// Number of marks belonging to this verse.
    pub marks_len: u16,
    /// Index of the verse's first span in the translation's span array.
    pub spans_start: u32,
    /// Number of spans belonging to this verse.
    pub spans_len: u16,
}

/// A complete translation laid out against the canonical grid.
///
/// The text is one allocation, the side arrays are flat, and every lookup returns
/// a borrowed slice. Instances are built through [`TranslationBuilder`] or by
/// decoding the binary format, never assembled field by field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translation {
    meta: TranslationMeta,
    text: String,
    extra: String,
    verses: Vec<Verse>,
    chapter_starts: Vec<u32>,
    marks: Vec<Mark>,
    spans: Vec<Span>,
}

impl Translation {
    /// Builds a translation from already-flattened parts.
    ///
    /// This is the seam the binary decoder uses. Every range is checked against
    /// its string or array so a malformed file becomes an error rather than a
    /// panic later in the reader. Callers normally use [`TranslationBuilder`].
    pub fn from_parts(
        meta: TranslationMeta,
        text: String,
        extra: String,
        verses: Vec<Verse>,
        marks: Vec<Mark>,
        spans: Vec<Span>,
    ) -> Result<Self> {
        if verses.len() != VERSE_COUNT {
            bail!(
                "translation has {} verses, the canonical grid has {VERSE_COUNT}",
                verses.len()
            );
        }

        let mut chapter_starts = Vec::with_capacity(CHAPTER_COUNT + 1);
        chapter_starts.push(0);
        let mut total = 0u32;
        for count in books::VERSES_PER_CHAPTER {
            total += u32::from(count);
            chapter_starts.push(total);
        }

        let translation = Self {
            meta,
            text,
            extra,
            verses,
            chapter_starts,
            marks,
            spans,
        };
        translation.validate()?;
        Ok(translation)
    }

    /// Checks every stored range against the arrays it points into.
    fn validate(&self) -> Result<()> {
        for (index, verse) in self.verses.iter().enumerate() {
            let text_end = verse.text_start as usize + verse.text_len as usize;
            if self.text.get(verse.text_start as usize..text_end).is_none() {
                bail!("verse {index} text range is out of bounds or not a character boundary");
            }

            let marks_end = verse.marks_start as usize + verse.marks_len as usize;
            if marks_end > self.marks.len() {
                bail!("verse {index} mark range is out of bounds");
            }
            for mark in &self.marks[verse.marks_start as usize..marks_end] {
                let payload_end = mark.payload_start as usize + mark.payload_len as usize;
                if mark.offset as usize > verse.text_len as usize
                    || !self
                        .text
                        .is_char_boundary(verse.text_start as usize + mark.offset as usize)
                {
                    bail!("verse {index} mark offset is outside the verse text");
                }
                if self
                    .extra
                    .get(mark.payload_start as usize..payload_end)
                    .is_none()
                {
                    bail!("verse {index} mark payload is out of bounds");
                }
            }

            let spans_end = verse.spans_start as usize + verse.spans_len as usize;
            if spans_end > self.spans.len() {
                bail!("verse {index} span range is out of bounds");
            }
            for span in &self.spans[verse.spans_start as usize..spans_end] {
                if span.start >= span.end || span.end as usize > verse.text_len as usize {
                    bail!("verse {index} span range is invalid");
                }
                let base = verse.text_start as usize;
                if !self.text.is_char_boundary(base + span.start as usize)
                    || !self.text.is_char_boundary(base + span.end as usize)
                {
                    bail!("verse {index} span range splits a character");
                }
            }
        }
        Ok(())
    }

    /// The translation's metadata.
    pub fn meta(&self) -> &TranslationMeta {
        &self.meta
    }

    /// The single string holding every verse's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The side string holding heading and other mark payloads.
    pub fn extra(&self) -> &str {
        &self.extra
    }

    /// Every verse slot in canonical order.
    pub fn verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Every mark in the translation, flattened in verse order.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Every span in the translation, flattened in verse order.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The text of one verse.
    pub fn verse_text(&self, verse: &Verse) -> &str {
        let start = verse.text_start as usize;
        // Ranges were checked by `from_parts`, so this slice cannot fail.
        &self.text[start..start + verse.text_len as usize]
    }

    /// The marks attached to one verse.
    pub fn verse_marks(&self, verse: &Verse) -> &[Mark] {
        let start = verse.marks_start as usize;
        &self.marks[start..start + verse.marks_len as usize]
    }

    /// The spans attached to one verse.
    pub fn verse_spans(&self, verse: &Verse) -> &[Span] {
        let start = verse.spans_start as usize;
        &self.spans[start..start + verse.spans_len as usize]
    }

    /// The payload text a mark carries, empty for marks without text.
    pub fn mark_payload(&self, mark: &Mark) -> &str {
        let start = mark.payload_start as usize;
        &self.extra[start..start + mark.payload_len as usize]
    }

    /// The verses of a 1-based chapter, borrowed from the flat verse array.
    ///
    /// This is the hot path for rendering. It allocates nothing and does two
    /// index lookups.
    pub fn chapter(&self, book: u8, chapter: u8) -> Option<&[Verse]> {
        let index = books::chapter_offset(book, chapter)? as usize;
        let start = *self.chapter_starts.get(index)? as usize;
        let end = *self.chapter_starts.get(index + 1)? as usize;
        self.verses.get(start..end)
    }

    /// The verse a reference points at, or `None` when the reference names only a chapter.
    pub fn verse(&self, reference: &Reference) -> Option<&Verse> {
        let verse = reference.verse?;
        self.verse_at(reference.book, reference.chapter, verse)
    }

    /// The verse at a book, 1-based chapter, and 1-based verse.
    pub fn verse_at(&self, book: u8, chapter: u8, verse: u8) -> Option<&Verse> {
        let index = books::verse_offset(book, chapter, verse)? as usize;
        self.verses.get(index)
    }

    /// The verse text a reference points at, or `None` when it names only a chapter.
    pub fn reference_text(&self, reference: &Reference) -> Option<&str> {
        self.verse(reference).map(|verse| self.verse_text(verse))
    }
}

/// Builds a translation verse by verse in canonical order.
///
/// The builder holds the only mutable state in the model. It tracks the next
/// canonical slot so an importer cannot write verses out of order or with the
/// wrong number, and it refuses to finish until every slot on the grid exists.
/// Marks and spans are appended after the verse they belong to and are flattened
/// into the translation's arrays as they arrive.
#[derive(Clone, Debug)]
pub struct TranslationBuilder {
    meta: TranslationMeta,
    text: String,
    extra: String,
    verses: Vec<Verse>,
    marks: Vec<Mark>,
    spans: Vec<Span>,
    book: u8,
    chapter: u8,
    verse: u8,
}

impl TranslationBuilder {
    /// Starts a translation with room reserved for the whole canonical grid.
    pub fn new(meta: TranslationMeta) -> Self {
        Self {
            meta,
            text: String::new(),
            extra: String::new(),
            verses: Vec::with_capacity(VERSE_COUNT),
            marks: Vec::new(),
            spans: Vec::new(),
            book: 0,
            chapter: 1,
            verse: 1,
        }
    }

    /// Appends the next canonical verse.
    ///
    /// The number must be the verse number the grid expects at this position.
    /// Push an empty string for a verse the translation does not print.
    pub fn push_verse(&mut self, number: u8, text: &str) -> Result<()> {
        self.push_verse_range(number, number, text)
    }

    /// Appends a verse whose printed number is a range, such as `13-14`.
    ///
    /// The slot is still one canonical verse; `number_end` only affects display.
    pub fn push_verse_range(&mut self, number: u8, number_end: u8, text: &str) -> Result<()> {
        if self.verses.len() >= VERSE_COUNT {
            bail!("translation already holds all {VERSE_COUNT} verses");
        }
        if number != self.verse {
            bail!(
                "expected verse {} but got {number} at {} {}",
                self.verse,
                books::book(self.book).map_or("unknown book", |book| book.name),
                self.chapter
            );
        }
        if number_end < number {
            bail!("verse range {number}-{number_end} runs backwards");
        }
        let text_len = u16::try_from(text.len())
            .map_err(|_| anyhow::anyhow!("verse text is longer than 65535 bytes"))?;

        self.text.push_str(text);
        let verse = Verse {
            number,
            number_end,
            text_start: u32::try_from(self.text.len() - text.len())
                .map_err(|_| anyhow::anyhow!("translation text is longer than 4 GiB"))?,
            text_len,
            marks_start: u32::try_from(self.marks.len())
                .map_err(|_| anyhow::anyhow!("too many marks"))?,
            marks_len: 0,
            spans_start: u32::try_from(self.spans.len())
                .map_err(|_| anyhow::anyhow!("too many spans"))?,
            spans_len: 0,
        };
        self.verses.push(verse);
        self.advance();
        Ok(())
    }

    /// Advances the cursor to the next canonical slot.
    fn advance(&mut self) {
        let Some(book) = books::book(self.book) else {
            return;
        };
        let count = books::verses_in_chapter(self.book, self.chapter).unwrap_or(0);
        if self.verse < count as u8 {
            self.verse += 1;
            return;
        }
        self.verse = 1;
        self.chapter += 1;
        if self.chapter > book.chapter_count {
            self.chapter = 1;
            self.book += 1;
        }
    }

    /// The verse just appended, or `None` before the first verse.
    fn last(&mut self) -> Result<&mut Verse> {
        self.verses
            .last_mut()
            .ok_or_else(|| anyhow::anyhow!("a mark or span needs a verse first"))
    }

    /// Attaches a structural mark to the verse just appended.
    ///
    /// The offset is in bytes from the start of that verse. A heading adds its
    /// text to the translation's extra string; other kinds pass an empty payload.
    pub fn mark(&mut self, offset: u16, kind: MarkKind, level: u8, payload: &str) -> Result<()> {
        let payload_start = u32::try_from(self.extra.len())
            .map_err(|_| anyhow::anyhow!("extra text is longer than 4 GiB"))?;
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| anyhow::anyhow!("mark payload is longer than 4 GiB"))?;
        self.extra.push_str(payload);

        let verse = self.last()?;
        if offset as usize > verse.text_len as usize {
            bail!("mark offset {offset} is past the end of the verse");
        }
        let marks_len = u16::try_from(verse.marks_len as usize + 1)
            .map_err(|_| anyhow::anyhow!("too many marks on one verse"))?;
        verse.marks_len = marks_len;
        self.marks.push(Mark {
            offset,
            kind,
            level,
            payload_start,
            payload_len,
        });
        Ok(())
    }

    /// Attaches a character style to a byte range of the verse just appended.
    ///
    /// The range is relative to the verse start and must sit inside its text.
    pub fn span(&mut self, style: SpanStyle, start: u16, end: u16) -> Result<()> {
        let verse = self.last()?;
        let text_len = verse.text_len;
        if start >= end || end > text_len {
            bail!("span {start}..{end} does not fit a verse of {text_len} bytes");
        }
        let base = verse.text_start as usize;
        if !self.text.is_char_boundary(base + start as usize)
            || !self.text.is_char_boundary(base + end as usize)
        {
            bail!("span {start}..{end} splits a character");
        }
        let verse = self.last()?;
        let spans_len = u16::try_from(verse.spans_len as usize + 1)
            .map_err(|_| anyhow::anyhow!("too many spans on one verse"))?;
        verse.spans_len = spans_len;
        self.spans.push(Span { start, end, style });
        Ok(())
    }

    /// Number of verses appended so far.
    pub fn len(&self) -> usize {
        self.verses.len()
    }

    /// Whether no verse has been appended yet.
    pub fn is_empty(&self) -> bool {
        self.verses.is_empty()
    }

    /// Freezes the builder into a translation, checking the grid is complete.
    pub fn finish(self) -> Result<Translation> {
        if self.verses.len() != VERSE_COUNT {
            bail!(
                "translation has {} of {VERSE_COUNT} verses",
                self.verses.len()
            );
        }
        Translation::from_parts(
            self.meta,
            self.text,
            self.extra,
            self.verses,
            self.marks,
            self.spans,
        )
    }
}
