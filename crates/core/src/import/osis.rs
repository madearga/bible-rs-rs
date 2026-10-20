// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The OSIS XML parser.
//!
//! OSIS states a verse either as an element wrapping its text or as a pair of
//! empty milestone elements joined by an `sID` and an `eID`. Both shapes appear
//! in the wild and both are read here. A verse's text is everything between the
//! two, so a verse whose text is split across several child elements such as
//! `<w>` becomes one string.
//!
//! Notes, readings, and cross references are apparatus and are dropped with their
//! content, because the reader does not show them.

use anyhow::{Context, Result, bail};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::model::{MarkKind, SpanStyle, Translation, TranslationMeta};

use super::{Document, RawMark, VerseAccumulator, assemble, book_from_code};

/// Parses an OSIS file into a translation.
pub(crate) fn import(xml: &str, meta: TranslationMeta) -> Result<Translation> {
    let mut document = Document::new();
    Parser::new(&mut document).run(xml)?;
    if document.is_empty() {
        bail!("the OSIS file holds no verse the canonical grid knows");
    }
    assemble(document, meta)
}

/// What one open element is doing to the text inside it.
#[derive(Clone, Copy, Debug)]
enum Frame {
    /// A verse element wrapping its own text.
    Verse,
    /// A styled range that closes with this element.
    Style(SpanStyle, u16),
    /// A heading whose text becomes a mark payload.
    Heading(u16),
    /// Apparatus, dropped with everything inside it.
    Skip,
    /// An element that does nothing to the text.
    Other,
}

/// One pass of the parser over one document.
struct Parser<'d> {
    document: &'d mut Document,
    book: Option<u8>,
    chapter: u8,
    open: Option<VerseAccumulator>,
    stack: Vec<Frame>,
    heading: String,
    deferred: Vec<RawMark>,
}

impl<'d> Parser<'d> {
    /// Starts a parser that appends to `document`.
    fn new(document: &'d mut Document) -> Self {
        Self {
            document,
            book: None,
            chapter: 1,
            open: None,
            stack: Vec::new(),
            heading: String::new(),
            deferred: Vec::new(),
        }
    }

    /// Parses a whole document.
    fn run(mut self, xml: &str) -> Result<()> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);

        loop {
            match reader.read_event() {
                Err(error) => {
                    return Err(error).context("the OSIS file is not well-formed XML");
                }
                Ok(Event::Eof) => break,
                Ok(Event::Start(element)) => self.start(&element),
                Ok(Event::Empty(element)) => self.empty(&element),
                Ok(Event::End(_)) => self.end(),
                Ok(Event::Text(text)) => self.text(&text.xml10_content()?),
                Ok(Event::CData(text)) => {
                    let text = String::from_utf8_lossy(&text);
                    let text = text.into_owned();
                    self.text(&text);
                }
                Ok(_) => {}
            }
        }

        self.close_verse();
        if !self.stack.is_empty() {
            bail!("the OSIS file ends inside an element that never closes");
        }
        Ok(())
    }

    /// Whether the cursor is inside apparatus that must be dropped.
    fn skipping(&self) -> bool {
        self.stack.iter().any(|frame| matches!(frame, Frame::Skip))
    }

    /// Appends decoded text to whatever the open element wants it for.
    fn text(&mut self, segment: &str) {
        if self.skipping() {
            return;
        }
        match self.stack.last() {
            Some(Frame::Heading(_)) => self.heading.push_str(segment),
            _ => {
                if let Some(open) = self.open.as_mut() {
                    open.push_text(segment);
                }
            }
        }
    }

    /// Handles an element that opens a range.
    fn start(&mut self, element: &BytesStart<'_>) {
        let name = element.local_name();
        match name.as_ref() {
            b"verse" => {
                if let Some(id) = attribute(element, b"osisID") {
                    self.verse_from(&id);
                }
                self.stack.push(Frame::Verse);
            }
            b"chapter" => {
                if let Some(id) = attribute(element, b"osisID") {
                    self.chapter_from(&id);
                }
                self.stack.push(Frame::Other);
            }
            b"div" => {
                if let Some(id) = attribute(element, b"osisID")
                    && !id.contains('.')
                    && let Some(book) = book_from_code(&id)
                {
                    self.close_verse();
                    self.book = Some(book);
                }
                self.stack.push(Frame::Other);
            }
            b"note" | b"rdg" | b"rdgGrp" | b"ref" => self.stack.push(Frame::Skip),
            b"title" => {
                let offset = self.open.as_ref().map_or(0, VerseAccumulator::offset);
                self.heading.clear();
                self.stack.push(Frame::Heading(offset));
            }
            b"transChange" => self.open_style(SpanStyle::TranslatorAddition),
            b"q" => {
                let who = attribute(element, b"who").unwrap_or_default();
                let style = (who.eq_ignore_ascii_case("jesus")
                    || who.eq_ignore_ascii_case("christ"))
                .then_some(SpanStyle::WordsOfChrist);
                match style {
                    Some(style) => {
                        self.document.note_red_letters();
                        self.open_style(style);
                    }
                    None => self.stack.push(Frame::Other),
                }
            }
            b"divineName" => self.open_style(SpanStyle::DivineName),
            _ => self.stack.push(Frame::Other),
        }
    }

    /// Handles an element that carries no content, which is how a milestone states
    /// where a verse starts and where it ends.
    fn empty(&mut self, element: &BytesStart<'_>) {
        match element.local_name().as_ref() {
            b"verse" => {
                if let Some(id) =
                    attribute(element, b"osisID").or_else(|| attribute(element, b"sID"))
                {
                    self.verse_from(&id);
                }
                if attribute(element, b"eID").is_some() {
                    self.close_verse();
                }
            }
            b"chapter" => {
                if let Some(id) = attribute(element, b"osisID") {
                    self.chapter_from(&id);
                }
            }
            _ => {}
        }
    }

    /// Handles a closing element, finishing whatever its frame was doing.
    fn end(&mut self) {
        match self.stack.pop() {
            Some(Frame::Verse) => self.close_verse(),
            Some(Frame::Style(style, start)) => {
                if let Some(open) = self.open.as_mut() {
                    open.close_span(style, start);
                }
            }
            Some(Frame::Heading(offset)) => {
                let text = std::mem::take(&mut self.heading);
                self.document.note_feature(MarkKind::Heading);
                match self.open.as_mut() {
                    Some(open) => open.add_mark_at(offset, MarkKind::Heading, 0, &text),
                    None => self.deferred.push(RawMark {
                        offset: 0,
                        kind: MarkKind::Heading,
                        level: 0,
                        payload: text.trim().to_owned(),
                    }),
                }
            }
            Some(Frame::Skip | Frame::Other) | None => {}
        }
    }

    /// Opens a styled range at the current end of the verse text.
    fn open_style(&mut self, style: SpanStyle) {
        let start = self.open.as_ref().map_or(0, VerseAccumulator::offset);
        self.stack.push(Frame::Style(style, start));
    }

    /// Opens the verse an `osisID` names, closing the previous one.
    fn verse_from(&mut self, id: &str) {
        let Some((book, chapter, verse, verse_end)) = reference(id) else {
            return;
        };
        self.close_verse();
        self.book = Some(book);
        self.chapter = chapter;
        let mut open = VerseAccumulator::new(book, chapter, verse, verse_end);
        open.marks = std::mem::take(&mut self.deferred);
        self.open = Some(open);
    }

    /// Sets the chapter a `chapter` element names.
    fn chapter_from(&mut self, id: &str) {
        if let Some(chapter) = id
            .split('.')
            .nth(1)
            .and_then(|part| part.trim().parse::<u8>().ok())
        {
            self.close_verse();
            self.chapter = chapter;
        }
    }

    /// Closes the open verse, if any, into the document.
    fn close_verse(&mut self) {
        let Some(open) = self.open.take() else {
            return;
        };
        let book = open.book;
        let chapter = open.chapter;
        let number = open.number;
        self.document.insert(book, chapter, number, open.finish());
    }
}

/// Splits an OSIS identifier such as `Gen.1.1` or `Gen.1.1-Gen.1.2`.
///
/// The book, chapter, and verse numbers are returned only when the identifier
/// actually names them, so a chapter milestone does not invent a verse.
fn reference(id: &str) -> Option<(u8, u8, u8, u8)> {
    let id = id.trim();
    let mut parts = id.split('.');
    let book = book_from_code(parts.next()?)?;
    let chapter = parts
        .next()
        .and_then(|part| part.trim().parse::<u8>().ok())
        .filter(|chapter| *chapter > 0)?;
    let number = parts.next()?;
    let verse = leading_number(number)?;
    let verse_end = parts
        .next_back()
        .and_then(leading_number)
        .unwrap_or(verse)
        .max(verse);
    Some((book, chapter, verse, verse_end))
}

/// Reads the digits a token starts with, ignoring a letter suffix such as `3a`.
fn leading_number(token: &str) -> Option<u8> {
    let digits: String = token.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Reads one attribute of an element as text.
fn attribute(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element
        .try_get_attribute(name)
        .ok()
        .flatten()
        .and_then(|attribute| attribute.unescape_value().ok())
        .map(|value| value.into_owned())
}
