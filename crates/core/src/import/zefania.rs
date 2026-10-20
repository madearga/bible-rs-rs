// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The Zefania XML parser.
//!
//! Zefania is the same shape as OSIS with its own element names. Books, chapters,
//! and verses are elements that nest, each carrying its own number attribute, so
//! the parser can keep the current book and chapter as it walks down. Zefania also
//! allows notes and styles inside a verse; notes are apparatus and are dropped,
//! and the styles that have a counterpart in the model become spans.

use anyhow::{Context, Result, bail};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::model::{SpanStyle, Translation, TranslationMeta};

use super::{Document, VerseAccumulator, assemble, book_from_code};

/// Parses a Zefania file into a translation.
pub(crate) fn import(xml: &str, meta: TranslationMeta) -> Result<Translation> {
    let mut document = Document::new();
    Parser::new(&mut document).run(xml)?;
    if document.is_empty() {
        bail!("the Zefania file holds no verse the canonical grid knows");
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
        }
    }

    /// Parses a whole document.
    fn run(mut self, xml: &str) -> Result<()> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(false);

        loop {
            match reader.read_event() {
                Err(error) => {
                    return Err(error).context("the Zefania file is not well-formed XML");
                }
                Ok(Event::Eof) => break,
                Ok(Event::Start(element)) => self.start(&element, false),
                Ok(Event::Empty(element)) => self.start(&element, true),
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
            bail!("the Zefania file ends inside an element that never closes");
        }
        Ok(())
    }

    /// Whether the cursor is inside apparatus that must be dropped.
    fn skipping(&self) -> bool {
        self.stack.iter().any(|frame| matches!(frame, Frame::Skip))
    }

    /// Appends decoded text to the open verse.
    fn text(&mut self, segment: &str) {
        if self.skipping() {
            return;
        }
        if let Some(open) = self.open.as_mut() {
            open.push_text(segment);
        }
    }

    /// Handles an opening element, including the self-closing form.
    fn start(&mut self, element: &BytesStart<'_>, empty: bool) {
        let closing = |parser: &mut Self, frame: Frame| {
            if empty {
                parser.close_frame(frame);
            } else {
                parser.stack.push(frame);
            }
        };

        match element.local_name().as_ref() {
            b"XMLBIBLE" => {
                if let Some(name) = attribute(element, b"biblename") {
                    self.document.set_title(&name);
                }
                closing(self, Frame::Other);
            }
            b"BIBLEBOOK" => {
                if !self.skipping() {
                    self.close_verse();
                    self.book = book_of(element);
                }
                closing(self, Frame::Other);
            }
            b"CHAPTER" => {
                if !self.skipping()
                    && let Some(number) = attribute(element, b"cnumber")
                        .and_then(|value| value.trim().parse::<u8>().ok())
                {
                    self.close_verse();
                    self.chapter = number;
                }
                closing(self, Frame::Other);
            }
            b"VERS" => {
                if self.skipping() {
                    closing(self, Frame::Other);
                    return;
                }
                self.verse_from(element);
                closing(self, Frame::Verse);
            }
            b"NOTE" | b"XREF" => closing(self, Frame::Skip),
            b"STYLE" => {
                let style = attribute(element, b"css")
                    .or_else(|| attribute(element, b"name"))
                    .map(|name| name.to_ascii_lowercase());
                let style = match style.as_deref() {
                    Some("red" | "words_of_christ" | "jesus") => Some(SpanStyle::WordsOfChrist),
                    Some("italic-add" | "addition" | "transchange") => {
                        Some(SpanStyle::TranslatorAddition)
                    }
                    Some("divine" | "divinename") => Some(SpanStyle::DivineName),
                    Some("selah") => Some(SpanStyle::Selah),
                    _ => None,
                };
                match style {
                    Some(style) => {
                        if style == SpanStyle::WordsOfChrist {
                            self.document.note_red_letters();
                        }
                        let start = self.open.as_ref().map_or(0, VerseAccumulator::offset);
                        closing(self, Frame::Style(style, start));
                    }
                    None => closing(self, Frame::Other),
                }
            }
            _ => closing(self, Frame::Other),
        }
    }

    /// Handles a closing element, finishing whatever its frame was doing.
    fn end(&mut self) {
        if let Some(frame) = self.stack.pop() {
            self.close_frame(frame);
        }
    }

    /// Finishes one frame, closing a verse or a style.
    fn close_frame(&mut self, frame: Frame) {
        match frame {
            Frame::Verse => self.close_verse(),
            Frame::Style(style, start) => {
                if let Some(open) = self.open.as_mut() {
                    open.close_span(style, start);
                }
            }
            Frame::Skip | Frame::Other => {}
        }
    }

    /// Opens the verse a `VERS` element names, closing the previous one.
    fn verse_from(&mut self, element: &BytesStart<'_>) {
        let chapter = attribute(element, b"cnumber")
            .and_then(|value| value.trim().parse::<u8>().ok())
            .unwrap_or(self.chapter);
        let number =
            attribute(element, b"vnumber").and_then(|value| value.trim().parse::<u8>().ok());
        let (Some(book), Some(number)) = (self.book, number) else {
            return;
        };
        self.close_verse();
        self.chapter = chapter;
        self.open = Some(VerseAccumulator::new(book, chapter, number, number));
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

/// Resolves the book a `BIBLEBOOK` element names.
///
/// The name wins over the number because a Zefania file that carries the
/// deuterocanonical books numbers them too, and the numbers would shift the
/// canonical grid. The number is used only when the name says nothing.
fn book_of(element: &BytesStart<'_>) -> Option<u8> {
    if let Some(name) = attribute(element, b"bname")
        && let Some(book) = book_from_code(&name)
    {
        return Some(book);
    }
    let number =
        attribute(element, b"bnumber").and_then(|value| value.trim().parse::<u16>().ok())?;
    if (1..=66).contains(&number) {
        Some((number - 1) as u8)
    } else {
        None
    }
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
