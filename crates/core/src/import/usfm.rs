// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The USFM parser.
//!
//! USFM is a marker language: a backslash and a name introduce structure, style,
//! or apparatus, and a few markers carry a payload on the rest of the line. The
//! parser walks a document a line at a time because that is where USFM puts its
//! structure, then routes each run of literal text to whichever destination the
//! marker in front of it chose.
//!
//! Two rules keep the reader's model honest. Only verse text reaches a verse, so
//! headings, titles, footnotes, and cross references never leak into it. And a
//! structural break found while a verse is open is attached to that verse at the
//! offset where it appeared, which for the common case of a paragraph marker
//! after a verse puts it exactly at the boundary before the next verse.

use anyhow::{Result, bail};

use crate::model::{MarkKind, SpanStyle, Translation, TranslationMeta};

use super::{Document, RawMark, VerseAccumulator, assemble, book_from_code};

/// Character markers that style a range of verse text.
fn span_style(name: &str) -> Option<SpanStyle> {
    Some(match name {
        "wj" => SpanStyle::WordsOfChrist,
        "add" | "addpn" => SpanStyle::TranslatorAddition,
        "nd" | "ndx" => SpanStyle::DivineName,
        "sls" => SpanStyle::Selah,
        _ => return None,
    })
}

/// Markers whose whole content is apparatus the reader never shows.
fn is_apparatus(name: &str) -> bool {
    matches!(
        name,
        "f" | "x"
            | "fe"
            | "ef"
            | "fig"
            | "fm"
            | "fr"
            | "ft"
            | "fk"
            | "fq"
            | "fqa"
            | "fv"
            | "fdc"
            | "fl"
            | "xo"
            | "xt"
            | "xk"
            | "xq"
            | "xot"
            | "xnt"
            | "xdc"
            | "rq"
            | "rem"
            | "sts"
    )
}

/// Where the literal text after a marker belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dest {
    /// Verse text, the only destination a reader sees.
    Verse,
    /// The number of a `\v` marker.
    VerseNumber,
    /// The number of a `\c` marker.
    Chapter,
    /// The book code of an `\id` marker.
    Id,
    /// The heading or superscription text of a marker that carries one.
    Payload,
    /// A book title marker such as `\mt`.
    BookTitle,
    /// The translation title marker `\ide`.
    TranslationTitle,
    /// Apparatus, dropped.
    Skip,
}

/// A marker opened but not yet closed, so its `*` closes the right one.
#[derive(Clone, Copy, Debug)]
struct Opened {
    /// The style to record when this closes, if any.
    style: Option<SpanStyle>,
    /// Offset in the verse text where the styled range began.
    start: u16,
    /// Whether closing this marker ends an apparatus run.
    skip: bool,
    /// Whether the marker's content carries `|` attributes that are not text.
    attributes: bool,
    /// The destination an apparatus run returns to when it closes.
    resume: Dest,
}

impl Opened {
    /// A transparent marker, which styles nothing and only needs closing.
    fn plain() -> Self {
        Self {
            style: None,
            start: 0,
            skip: false,
            attributes: false,
            resume: Dest::Verse,
        }
    }
}

/// Parses a set of USFM documents into one translation.
///
/// Every document contributes to the same translation, so a source that splits
/// its books across several files imports as one.
pub(crate) fn import(documents: &[&str], meta: TranslationMeta) -> Result<Translation> {
    let mut document = Document::new();
    for source in documents {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        Parser::new(&mut document).run(source)?;
    }
    if document.is_empty() {
        bail!("the USFM source holds no verse the canonical grid knows");
    }
    assemble(document, meta)
}

/// One pass of the parser over one document.
struct Parser<'d> {
    document: &'d mut Document,
    book: Option<u8>,
    skipping: bool,
    chapter: u8,
    chapter_opened: bool,
    open: Option<VerseAccumulator>,
    stack: Vec<Opened>,
    deferred: Vec<RawMark>,
    dest: Dest,
    buffer: String,
    payload: Option<(MarkKind, u8, u16)>,
}

impl<'d> Parser<'d> {
    /// Starts a parser that appends to `document`.
    fn new(document: &'d mut Document) -> Self {
        Self {
            document,
            book: None,
            skipping: false,
            chapter: 1,
            chapter_opened: false,
            open: None,
            stack: Vec::new(),
            deferred: Vec::new(),
            dest: Dest::Verse,
            buffer: String::new(),
            payload: None,
        }
    }

    /// Parses a whole document, tolerating a marker left unclosed at the end.
    fn run(mut self, source: &str) -> Result<()> {
        for line in source.split('\n') {
            self.line(line.trim_end_matches('\r'))?;
        }
        self.close_verse();
        Ok(())
    }

    /// Parses one line.
    fn line(&mut self, line: &str) -> Result<()> {
        let mut pos = 0;

        if let Some((consumed, number, number_end)) = bare_verse_number(line) {
            self.flush()?;
            self.start_verse(number, number_end);
            pos = consumed;
        }

        while pos < line.len() {
            match memchr::memchr(b'\\', &line.as_bytes()[pos..]) {
                Some(offset) => {
                    let at = pos + offset;
                    if at > pos {
                        self.text_segment(&line[pos..at])?;
                    }
                    let (name, starred, after) = read_marker(line, at);
                    self.marker(&name, starred)?;
                    pos = after.max(at + 1);
                }
                None => {
                    self.text_segment(&line[pos..])?;
                    break;
                }
            }
        }

        self.flush()?;
        self.dest = Dest::Verse;
        Ok(())
    }

    /// Routes a run of literal text to the destination the last marker chose.
    fn text_segment(&mut self, segment: &str) -> Result<()> {
        if segment.is_empty() {
            return Ok(());
        }
        // A word marker carries its lexicon attributes after a `|`, which are not text.
        let attributes = self.stack.last().is_some_and(|opened| opened.attributes);
        let segment = if attributes {
            match segment.split_once('|') {
                Some((before, _)) => {
                    if let Some(opened) = self.stack.last_mut() {
                        opened.attributes = false;
                    }
                    before
                }
                None => return Ok(()),
            }
        } else {
            segment
        };
        if segment.is_empty() {
            return Ok(());
        }
        match self.dest {
            Dest::Verse => {
                if let Some(open) = self.open.as_mut() {
                    open.push_text(segment);
                }
            }
            Dest::Skip => {}
            Dest::VerseNumber | Dest::Chapter => {
                self.buffer.push_str(segment);
                // The number ends at the first whitespace, and anything after it is
                // text, so the verse opens before the rest of the line is read.
                if self.buffer.contains(char::is_whitespace) {
                    self.flush()?;
                }
            }
            Dest::Payload | Dest::BookTitle | Dest::TranslationTitle | Dest::Id => {
                self.buffer.push_str(segment)
            }
        }
        Ok(())
    }

    /// Applies one marker, which may change the destination, open or close a style,
    /// or attach a structural break.
    fn marker(&mut self, name: &str, starred: bool) -> Result<()> {
        if name.is_empty() {
            return Ok(());
        }
        let name = name.to_ascii_lowercase();
        if starred {
            self.close_style();
            return Ok(());
        }

        match name.as_str() {
            "id" => {
                self.end_destination()?;
                self.dest = Dest::Id;
            }
            "c" => {
                self.end_destination()?;
                self.close_verse();
                self.dest = Dest::Chapter;
            }
            "v" => {
                self.end_destination()?;
                self.close_verse();
                self.dest = Dest::VerseNumber;
            }
            "h" | "toc" | "toc1" | "toc2" | "toc3" | "mt" | "mt1" | "mt2" | "mt3" | "ms"
            | "ms1" | "ms2" | "ms3" => {
                self.end_destination()?;
                self.dest = Dest::BookTitle;
            }
            "ide" => {
                self.end_destination()?;
                self.dest = Dest::TranslationTitle;
            }
            "s" | "s1" => {
                self.end_destination()?;
                self.heading(0);
            }
            "s2" => {
                self.end_destination()?;
                self.heading(1);
            }
            "s3" => {
                self.end_destination()?;
                self.heading(2);
            }
            "s4" => {
                self.end_destination()?;
                self.heading(3);
            }
            "d" => {
                self.end_destination()?;
                self.superscription();
            }
            "r" => {
                self.end_destination()?;
                self.heading(0);
            }
            "p" | "m" | "nb" | "cls" | "pc" | "pr" | "ph" | "li" | "sp" | "pi" | "pi1" | "pi2"
            | "pi3" | "pi4" => {
                self.end_destination()?;
                self.add_mark(MarkKind::Paragraph, 0);
            }
            "b" => {
                self.end_destination()?;
                self.add_mark(MarkKind::StanzaBreak, 0);
            }
            "q" | "q1" | "qr" | "qc" | "qa" | "qm" | "l" => {
                self.end_destination()?;
                self.add_mark(MarkKind::PoetryLine, 0);
            }
            "q2" => {
                self.end_destination()?;
                self.add_mark(MarkKind::PoetryIndent, 1);
            }
            "q3" => {
                self.end_destination()?;
                self.add_mark(MarkKind::PoetryIndent, 2);
            }
            "q4" => {
                self.end_destination()?;
                self.add_mark(MarkKind::PoetryIndent, 3);
            }
            "mi" | "mq" => {
                self.end_destination()?;
                self.add_mark(MarkKind::PoetryLine, 1);
            }
            _ => {
                if is_apparatus(&name) {
                    // The markers inside a note are not closed one by one, so only
                    // the marker that starts the run suspends the destination.
                    if self.dest == Dest::Skip {
                        self.stack.push(Opened::plain());
                    } else {
                        self.open_skip();
                    }
                } else if let Some(style) = span_style(&name) {
                    if style == SpanStyle::WordsOfChrist {
                        self.document.note_red_letters();
                    }
                    self.open_style(style);
                } else if name == "w" {
                    self.stack.push(Opened {
                        attributes: true,
                        ..Opened::plain()
                    });
                } else {
                    // An unknown marker is treated as a transparent character marker
                    // so its `*` closes here instead of unbalancing the stack.
                    self.stack.push(Opened::plain());
                }
            }
        }
        Ok(())
    }

    /// Finishes the text of the destination in front of the cursor, if it has one.
    ///
    /// Apparatus runs keep their own text out of the verse, so nothing opens or
    /// closes inside one until the marker that started it closes.
    fn end_destination(&mut self) -> Result<()> {
        if self.dest == Dest::Skip {
            return Ok(());
        }
        self.flush()
    }

    /// Starts a heading mark whose text is the rest of the line.
    fn heading(&mut self, level: u8) {
        let offset = self.open.as_ref().map_or(0, VerseAccumulator::offset);
        self.payload = Some((MarkKind::Heading, level, offset));
        self.dest = Dest::Payload;
    }

    /// Starts a Psalm superscription, which is muted italic text before the verse.
    fn superscription(&mut self) {
        let offset = self.open.as_ref().map_or(0, VerseAccumulator::offset);
        self.payload = Some((MarkKind::Superscription, 0, offset));
        self.dest = Dest::Payload;
    }

    /// Attaches a structural break, to the open verse or to the next one to open.
    fn add_mark(&mut self, kind: MarkKind, level: u8) {
        self.document.note_feature(kind);
        match self.open.as_mut() {
            Some(open) => open.add_mark(kind, level, ""),
            None => self.deferred.push(RawMark {
                offset: 0,
                kind,
                level,
                payload: String::new(),
            }),
        }
    }

    /// Opens a styled range at the current end of the verse text.
    fn open_style(&mut self, style: SpanStyle) {
        let start = self.open.as_ref().map_or(0, VerseAccumulator::offset);
        self.stack.push(Opened {
            style: Some(style),
            start,
            ..Opened::plain()
        });
    }

    /// Opens an apparatus run, dropping everything until its closing `*`.
    fn open_skip(&mut self) {
        self.stack.push(Opened {
            skip: true,
            resume: self.dest,
            ..Opened::plain()
        });
        self.dest = Dest::Skip;
    }

    /// Closes the innermost open marker, recording a style when it had one.
    fn close_style(&mut self) {
        let dest = self.dest;
        if dest == Dest::Skip {
            // The markers inside a note are not closed one by one, so a close marker
            // in apparatus ends the run that started it.
            while let Some(opened) = self.stack.pop() {
                if opened.skip {
                    self.dest = opened.resume;
                    break;
                }
            }
            return;
        }
        let Some(opened) = self.stack.pop() else {
            return;
        };
        if opened.skip {
            self.dest = opened.resume;
            return;
        }
        if dest != Dest::Verse {
            return;
        }
        if let (Some(style), Some(open)) = (opened.style, self.open.as_mut()) {
            open.close_span(style, opened.start);
        }
    }

    /// Finishes the marker in front of the cursor and returns to plain verse text.
    fn flush(&mut self) -> Result<()> {
        match self.dest {
            Dest::Verse | Dest::Skip => {}
            Dest::Id => {
                let code = self
                    .buffer
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                self.open_book(&code);
            }
            Dest::Chapter => {
                let number = self.buffer.split_whitespace().next().unwrap_or_default();
                if let Ok(chapter) = number.parse::<u8>()
                    && chapter > 0
                {
                    self.chapter = chapter;
                    self.chapter_opened = false;
                }
            }
            Dest::VerseNumber => {
                let buffer = std::mem::take(&mut self.buffer);
                self.open_verse_from(&buffer)?;
            }
            Dest::Payload => {
                if let Some((kind, level, offset)) = self.payload.take() {
                    let text = fold_whitespace(&self.buffer);
                    self.document.note_feature(kind);
                    match self.open.as_mut() {
                        Some(open) => open.add_mark_at(offset, kind, level, &text),
                        None => self.deferred.push(RawMark {
                            offset: 0,
                            kind,
                            level,
                            payload: text,
                        }),
                    }
                }
            }
            Dest::BookTitle => {
                let title = fold_whitespace(&self.buffer);
                if let Some(book) = self.book {
                    self.document.set_book_title(book, &title);
                }
            }
            Dest::TranslationTitle => {
                let title = fold_whitespace(&self.buffer);
                self.document.set_title(&title);
            }
        }
        self.buffer.clear();
        self.dest = Dest::Verse;
        Ok(())
    }

    /// Sets the book the following verses belong to, skipping books outside the grid.
    fn open_book(&mut self, code: &str) {
        match book_from_code(code) {
            Some(book) => {
                self.book = Some(book);
                self.skipping = false;
                self.chapter = 1;
                self.chapter_opened = false;
            }
            None => {
                self.book = None;
                self.skipping = true;
                self.deferred.clear();
            }
        }
    }

    /// Reads a `\v` payload such as `13-14 text` and opens that verse.
    fn open_verse_from(&mut self, buffer: &str) -> Result<()> {
        let buffer = buffer.trim_start();
        let split = buffer.find(char::is_whitespace).unwrap_or(buffer.len());
        let (head, rest) = buffer.split_at(split);
        let (number, number_end) = parse_verse_number(head)?;
        self.start_verse(number, number_end);
        if let Some(open) = self.open.as_mut() {
            open.push_text(rest);
        }
        Ok(())
    }

    /// Opens a verse, moving the chapter on when a source prints no `\c` markers.
    fn start_verse(&mut self, number: u8, number_end: u8) {
        if number == 1 && self.chapter_opened {
            self.chapter = self.chapter.saturating_add(1);
            self.chapter_opened = false;
        }
        self.close_verse();
        self.stack.clear();

        if self.skipping {
            return;
        }
        let Some(book) = self.book else {
            return;
        };
        self.chapter_opened = true;

        let mut open = VerseAccumulator::new(book, self.chapter, number, number_end);
        let mut marks = std::mem::take(&mut self.deferred);
        for mark in &mut marks {
            mark.offset = 0;
        }
        open.marks = marks;
        self.open = Some(open);
    }

    /// Closes the open verse, if any, into the document.
    fn close_verse(&mut self) {
        let Some(mut open) = self.open.take() else {
            return;
        };
        while let Some(opened) = self.stack.pop() {
            if let Some(style) = opened.style {
                open.close_span(style, opened.start);
            }
        }
        if self.dest == Dest::Skip {
            self.dest = Dest::Verse;
            self.buffer.clear();
            self.payload = None;
        }

        let book = open.book;
        let chapter = open.chapter;
        let number = open.number;
        self.document.insert(book, chapter, number, open.finish());
    }
}

/// Folds every run of whitespace in a marker's payload to one space.
fn fold_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads the marker that starts at `at`, returning its name, whether it closes, and
/// where its content begins.
fn read_marker(line: &str, at: usize) -> (String, bool, usize) {
    let bytes = line.as_bytes();
    let mut index = at + 1;
    while index < bytes.len() && bytes[index] == b'+' {
        index += 1;
    }
    let start = index;
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    let name = line[start..index].to_owned();
    let starred = index < bytes.len() && bytes[index] == b'*';
    if starred {
        index += 1;
    }
    (name, starred, index)
}

/// Reads `13-14` or `3` into a first and last verse number.
fn parse_verse_number(head: &str) -> Result<(u8, u8)> {
    let head = head.trim();
    if head.is_empty() {
        bail!("a verse marker carries no verse number");
    }
    let (first, last) = match head.split_once('-') {
        Some((first, last)) => (first, Some(last)),
        None => (head, None),
    };
    let number =
        leading_number(first).ok_or_else(|| anyhow::anyhow!("{head:?} is not a verse number"))?;
    let number_end = match last {
        Some(last) => leading_number(last).unwrap_or(number).max(number),
        None => number,
    };
    Ok((number, number_end))
}

/// Reads the digits at the start of a token, ignoring a letter suffix such as `3a`.
fn leading_number(token: &str) -> Option<u8> {
    let digits: String = token.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Recognises a line that opens a verse with a bare number, as paragraph-style sources do.
fn bare_verse_number(line: &str) -> Option<(usize, u8, u8)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('\\') {
        return None;
    }
    let token_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let token = &trimmed[..token_end];
    let stripped = token.trim_end_matches(|character: char| character.is_ascii_alphabetic());
    if stripped.is_empty() || !stripped.starts_with(|character: char| character.is_ascii_digit()) {
        return None;
    }
    let (number, number_end) = parse_verse_number(stripped).ok()?;
    let consumed = line.len() - trimmed.len() + token_end;
    Some((consumed, number, number_end))
}
