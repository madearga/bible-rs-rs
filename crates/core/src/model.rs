// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The translation model: one text buffer, one verse table, and flat mark and
//! span arrays.
//!
//! A translation is built through [`TranslationBuilder`], which appends verses
//! in canonical order. The arrays stay flat so that loading a translation is a
//! handful of linear passes and rendering a chapter needs no per-verse
//! allocation.

use anyhow::{Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::books::{self, Book};

/// What a translation says about itself.
///
/// The metadata is the uncompressed JSON header of a translation file, so a
/// translation list can be read without decompressing any verse text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationMeta {
    /// Stable identifier used for file names and settings.
    pub id: String,
    /// Short label shown beside a verse number.
    pub abbreviation: String,
    /// Full name shown in lists.
    pub name: String,
    /// Language tag or language name of the verse text.
    pub language: String,
    /// One-line summary of the licence the text is distributed under.
    pub licence: String,
    /// Copyright notice that must travel with the text.
    pub copyright: String,
    /// Where the text came from, such as an archive name and digest.
    pub provenance: String,
    /// How the translation was obtained, such as `bundled` or `imported`.
    pub origin: String,
    /// Per-book titles in canonical order, empty when the translation keeps none.
    #[serde(default)]
    pub book_titles: Vec<String>,
    /// Whether the text marks the words of Christ for a red-letter treatment.
    pub red_letter: bool,
    /// Whether the text carries paragraph structure.
    pub paragraphs: bool,
    /// Whether the text carries section headings.
    pub headings: bool,
}

/// One verse of a translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verse {
    /// Verse number as printed, 1-based within its chapter.
    pub number: u8,
    /// Last verse number the text covers, equal to [`Verse::number`] unless a
    /// source merges consecutive verses into one entry.
    pub number_end: u8,
    /// Byte offset of the verse text in [`Translation::text`].
    pub text_start: u32,
    /// Byte length of the verse text.
    pub text_len: u16,
    /// Index of the verse's first mark in [`Translation::marks`].
    pub marks_start: u32,
    /// Number of marks attached to the verse.
    pub marks_len: u16,
    /// Index of the verse's first span in [`Translation::spans`].
    pub spans_start: u32,
    /// Number of spans attached to the verse.
    pub spans_len: u16,
}

/// A structural break inside or before a verse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Byte offset from the start of the verse text where the break applies.
    pub offset: u16,
    /// What kind of break this is.
    pub kind: MarkKind,
    /// Nesting or indent depth for kinds that carry one, otherwise zero.
    pub level: u8,
    /// Byte offset of the mark's payload in [`Translation::extra`].
    pub payload_start: u32,
    /// Byte length of the mark's payload, zero when the mark carries none.
    pub payload_len: u32,
}

/// The kinds of structural break a translation can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// Starts a new paragraph.
    Paragraph,
    /// Starts a line of poetry.
    Poetry,
    /// Separates stanzas.
    StanzaBreak,
    /// Introduces a section heading.
    Heading,
    /// The psalm title a psalter prints before verse 1.
    Superscription,
}

impl MarkKind {
    /// The stable on-disk code for this kind.
    pub fn code(self) -> u8 {
        match self {
            Self::Paragraph => 0,
            Self::Poetry => 1,
            Self::StanzaBreak => 2,
            Self::Heading => 3,
            Self::Superscription => 4,
        }
    }

    /// Reads a kind from its on-disk code, rejecting codes this version does not
    /// know rather than guessing.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Paragraph),
            1 => Some(Self::Poetry),
            2 => Some(Self::StanzaBreak),
            3 => Some(Self::Heading),
            4 => Some(Self::Superscription),
            _ => None,
        }
    }
}

/// A styled run of characters inside a verse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset from the start of the verse text where the style starts.
    pub start: u16,
    /// Byte offset from the start of the verse text where the style ends.
    pub end: u16,
    /// Which style applies.
    pub style: SpanStyle,
}

/// The character styles a translation can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    /// Words spoken by Christ, for a red-letter treatment.
    WordsOfChrist,
    /// Words a translator added for clarity.
    TranslatorAddition,
    /// The divine name, printed apart from the surrounding text.
    DivineName,
    /// The Selah marker, printed apart from the surrounding text.
    Selah,
}

impl SpanStyle {
    /// The stable on-disk code for this style.
    pub fn code(self) -> u8 {
        match self {
            Self::WordsOfChrist => 0,
            Self::TranslatorAddition => 1,
            Self::DivineName => 2,
            Self::Selah => 3,
        }
    }

    /// Reads a style from its on-disk code, rejecting codes this version does
    /// not know.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::WordsOfChrist),
            1 => Some(Self::TranslatorAddition),
            2 => Some(Self::DivineName),
            3 => Some(Self::Selah),
            _ => None,
        }
    }
}

/// A whole translation: its verse text, its verse table, and the structural
/// marks and character spans those verses point into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    meta: TranslationMeta,
    text: String,
    extra: String,
    verses: Vec<Verse>,
    marks: Vec<Mark>,
    spans: Vec<Span>,
}

impl Translation {
    /// Rebuilds a translation from parts the binary format already checked.
    pub(crate) fn from_parts(
        meta: TranslationMeta,
        text: String,
        extra: String,
        verses: Vec<Verse>,
        marks: Vec<Mark>,
        spans: Vec<Span>,
    ) -> Self {
        Self {
            meta,
            text,
            extra,
            verses,
            marks,
            spans,
        }
    }

    /// The translation's own description of itself.
    pub fn meta(&self) -> &TranslationMeta {
        &self.meta
    }

    /// Every verse's text, back to back in canonical order.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Heading and superscription payloads, indexed by [`Mark::payload_start`].
    pub fn extra(&self) -> &str {
        &self.extra
    }

    /// Every verse, in canonical order, one entry per verse of the canon this
    /// translation fills in.
    pub fn verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Every structural mark, grouped by verse.
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// Every character span, grouped by verse.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// The text of one verse.
    pub fn verse_text(&self, verse: &Verse) -> &str {
        let start = verse.text_start as usize;
        &self.text[start..start + usize::from(verse.text_len)]
    }

    /// The payload text of one mark, empty when the mark carries none.
    pub fn mark_text(&self, mark: &Mark) -> &str {
        let start = mark.payload_start as usize;
        &self.extra[start..start + mark.payload_len as usize]
    }

    /// One chapter, ready to be laid out, or an error when this translation does
    /// not reach that far into the canon.
    pub fn chapter(&self, book: u8, chapter: u16) -> Result<Chapter<'_>> {
        let book_ref: &'static Book =
            books::book(book).ok_or_else(|| anyhow!("book {book} is outside the canon"))?;
        let (start, count) = match (
            books::chapter_verse_start(book, chapter),
            books::verses_in_chapter(book, chapter),
        ) {
            (Some(start), Some(count)) => (start as usize, usize::from(count)),
            _ => bail!("{} has no chapter {chapter}", book_ref.name),
        };
        let end = start + count;
        ensure!(
            end <= self.verses.len(),
            "translation holds {} verses, {} {chapter} needs {end}",
            self.verses.len(),
            book_ref.name
        );
        Ok(Chapter {
            translation: self,
            book: book_ref,
            chapter,
            verses: &self.verses[start..end],
        })
    }
}

/// One chapter of a translation, borrowed so that laying it out allocates
/// nothing per verse.
#[derive(Debug, Clone, Copy)]
pub struct Chapter<'a> {
    translation: &'a Translation,
    book: &'static Book,
    chapter: u16,
    verses: &'a [Verse],
}

impl<'a> Chapter<'a> {
    /// The book this chapter belongs to.
    pub fn book(&self) -> &'static Book {
        self.book
    }

    /// The 1-based chapter number.
    pub fn chapter(&self) -> u16 {
        self.chapter
    }

    /// The chapter's verses, in order.
    pub fn verses(&self) -> &'a [Verse] {
        self.verses
    }

    /// The text of one of this chapter's verses.
    pub fn text(&self, verse: &Verse) -> &'a str {
        self.translation.verse_text(verse)
    }

    /// The marks attached to one of this chapter's verses.
    pub fn marks(&self, verse: &Verse) -> &'a [Mark] {
        self.translation.marks_of(verse)
    }

    /// The spans attached to one of this chapter's verses.
    pub fn spans(&self, verse: &Verse) -> &'a [Span] {
        self.translation.spans_of(verse)
    }

    /// The payload text of one of this chapter's marks.
    pub fn mark_text(&self, mark: &Mark) -> &'a str {
        self.translation.mark_text(mark)
    }

    /// Appends every verse's text to `out`, reusing its allocation.
    pub fn write_text(&self, out: &mut String) {
        for verse in self.verses {
            out.push_str(self.translation.verse_text(verse));
        }
    }
}

impl Translation {
    fn marks_of(&self, verse: &Verse) -> &[Mark] {
        let start = verse.marks_start as usize;
        &self.marks[start..start + usize::from(verse.marks_len)]
    }

    fn spans_of(&self, verse: &Verse) -> &[Span] {
        let start = verse.spans_start as usize;
        &self.spans[start..start + usize::from(verse.spans_len)]
    }
}

/// Builds a [`Translation`] verse by verse.
///
/// Verses are appended in canonical order, and each verse's marks and spans are
/// attached inside the scope of that verse, so a mark can never land on the
/// wrong verse and nothing has to be sorted afterwards.
#[derive(Debug)]
pub struct TranslationBuilder {
    meta: TranslationMeta,
    text: String,
    extra: String,
    verses: Vec<Verse>,
    marks: Vec<Mark>,
    spans: Vec<Span>,
}

impl TranslationBuilder {
    /// Starts an empty translation described by `meta`.
    pub fn new(meta: TranslationMeta) -> Self {
        Self {
            meta,
            text: String::new(),
            extra: String::new(),
            verses: Vec::new(),
            marks: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// Appends one verse and lets `build` attach its marks and spans.
    ///
    /// The closure runs only for this verse; on an error the partially attached
    /// marks and spans are dropped so the builder stays usable.
    pub fn push_verse<F>(&mut self, number: u8, text: &str, build: F) -> Result<()>
    where
        F: FnOnce(&mut VerseSink<'_>) -> Result<()>,
    {
        ensure!(number > 0, "verse numbers are 1-based");
        ensure!(
            text.len() <= usize::from(u16::MAX),
            "verse {number} has {} bytes, the format allows {}",
            text.len(),
            u16::MAX
        );
        let text_start = u32::try_from(self.text.len()).expect("verse text fits a u32 offset");
        let marks_start = u32::try_from(self.marks.len()).expect("mark table fits a u32 offset");
        let spans_start = u32::try_from(self.spans.len()).expect("span table fits a u32 offset");
        let extra_start = self.extra.len();

        let outcome = {
            let mut sink = VerseSink {
                text_len: text.len(),
                marks: &mut self.marks,
                spans: &mut self.spans,
                extra: &mut self.extra,
            };
            build(&mut sink)
        };
        if let Err(error) = outcome {
            self.marks.truncate(marks_start as usize);
            self.spans.truncate(spans_start as usize);
            self.extra.truncate(extra_start);
            return Err(error);
        }

        let marks_len = u16::try_from(self.marks.len() - marks_start as usize)
            .map_err(|_| anyhow!("verse {number} carries more than {} marks", u16::MAX))?;
        let spans_len = u16::try_from(self.spans.len() - spans_start as usize)
            .map_err(|_| anyhow!("verse {number} carries more than {} spans", u16::MAX))?;
        self.text.push_str(text);
        self.verses.push(Verse {
            number,
            number_end: number,
            text_start,
            text_len: u16::try_from(text.len()).expect("checked above"),
            marks_start,
            marks_len,
            spans_start,
            spans_len,
        });
        Ok(())
    }

    /// Appends one verse with no marks and no spans.
    pub fn push_plain_verse(&mut self, number: u8, text: &str) -> Result<()> {
        self.push_verse(number, text, |_| Ok(()))
    }

    /// The finished translation.
    pub fn finish(self) -> Translation {
        Translation {
            meta: self.meta,
            text: self.text,
            extra: self.extra,
            verses: self.verses,
            marks: self.marks,
            spans: self.spans,
        }
    }
}

/// The scope of one verse while [`TranslationBuilder::push_verse`] runs.
///
/// Offsets handed to this sink are byte offsets inside the verse text, which is
/// what the format stores.
#[derive(Debug)]
pub struct VerseSink<'a> {
    text_len: usize,
    marks: &'a mut Vec<Mark>,
    spans: &'a mut Vec<Span>,
    extra: &'a mut String,
}

impl VerseSink<'_> {
    /// Records a structural break with no payload and no level.
    pub fn mark(&mut self, kind: MarkKind, offset: u16) -> Result<()> {
        self.mark_with_level(kind, offset, 0)
    }

    /// Records a structural break at a nesting or indent level.
    pub fn mark_with_level(&mut self, kind: MarkKind, offset: u16, level: u8) -> Result<()> {
        self.check_offset(offset)?;
        self.marks.push(Mark {
            offset,
            kind,
            level,
            payload_start: 0,
            payload_len: 0,
        });
        Ok(())
    }

    /// Records a section heading that starts at `offset`.
    pub fn heading(&mut self, offset: u16, text: &str) -> Result<()> {
        self.mark_with_payload(MarkKind::Heading, offset, text)
    }

    /// Records a psalm superscription, which the format prints at verse start.
    pub fn superscription(&mut self, text: &str) -> Result<()> {
        self.mark_with_payload(MarkKind::Superscription, 0, text)
    }

    /// Records a styled run of `start..end` inside the verse.
    pub fn span(&mut self, style: SpanStyle, start: u16, end: u16) -> Result<()> {
        self.check_offset(start)?;
        self.check_offset(end)?;
        ensure!(start <= end, "span starts at {start} but ends at {end}");
        self.spans.push(Span { start, end, style });
        Ok(())
    }

    fn mark_with_payload(&mut self, kind: MarkKind, offset: u16, text: &str) -> Result<()> {
        self.check_offset(offset)?;
        let payload_start = u32::try_from(self.extra.len()).expect("payload fits a u32 offset");
        let payload_len = u32::try_from(text.len()).expect("payload fits a u32 length");
        self.extra.push_str(text);
        self.marks.push(Mark {
            offset,
            kind,
            level: 0,
            payload_start,
            payload_len,
        });
        Ok(())
    }

    fn check_offset(&self, offset: u16) -> Result<()> {
        ensure!(
            usize::from(offset) <= self.text_len,
            "offset {offset} is past the {} bytes of this verse",
            self.text_len
        );
        Ok(())
    }
}
