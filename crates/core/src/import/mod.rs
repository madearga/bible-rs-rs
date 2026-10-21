// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Importers that turn a translation file the user owns into a [`Translation`].
//!
//! Four sources are accepted: USFM, OSIS XML, Zefania XML, and SWORD modules.
//! Every importer parses its source into a common [`Document`] keyed by canonical
//! book, chapter, and verse, and the document is then laid onto the canonical grid
//! so a source that omits a verse still leaves an empty slot for it.
//!
//! The entry point most callers want is [`import_path`], which detects the format
//! from the file or folder it is handed.
//!
//! ```no_run
//! use std::path::Path;
//!
//! use bible_core::{TranslationMeta, import::import_path};
//!
//! let meta = TranslationMeta::new("mine", "My Translation");
//! let translation = import_path(Path::new("bible.usfm"), meta, None).expect("a translation");
//! println!("{} verses", translation.verses().len());
//! ```

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::books::{self, BOOK_COUNT};
use crate::model::{MarkKind, SpanStyle, Translation, TranslationBuilder, TranslationMeta};

mod osis;
mod sword;
mod usfm;
mod zefania;

pub mod zip;

/// The largest source file one import reads, in bytes.
const MAX_SOURCE_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// The largest total of source bytes one import reads, in bytes.
const MAX_SOURCE_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

/// The most source files one import reads.
const MAX_SOURCE_FILES: usize = 256;

/// The largest verse text the model can store, in bytes.
///
/// The format keeps a `u16` length per verse, so text past this cannot be
/// represented. The accumulator stops there rather than growing without bound.
pub(crate) const MAX_VERSE_TEXT_BYTES: usize = u16::MAX as usize;

/// The deepest XML element nesting an import accepts.
const MAX_XML_DEPTH: usize = 256;

/// The most structural breaks one verse may carry.
const MAX_VERSE_MARKS: usize = 1024;

/// The most character styles one verse may carry.
const MAX_VERSE_SPANS: usize = 4096;

/// The input formats an import can come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImportFormat {
    /// A `.usfm` or `.sfm` file, a folder holding them, or a zip of them.
    Usfm,
    /// An OSIS XML file.
    Osis,
    /// A Zefania XML file.
    Zefania,
    /// A SWORD module zip, folder, or `mods.d/*.conf` manifest.
    Sword,
}

impl ImportFormat {
    /// Guesses the format from a path's extension, falling back to a content sniff.
    ///
    /// `None` means the file does not look like any accepted format, so the
    /// caller can say so instead of guessing wrongly.
    pub fn detect(path: &Path) -> Option<Self> {
        if path.is_dir() {
            let lower = path.to_string_lossy().to_ascii_lowercase();
            if lower.contains("modules") || path.join("mods.d").is_dir() {
                return Some(Self::Sword);
            }
            return Some(Self::Usfm);
        }

        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        let extension = name.rsplit_once('.').map(|(_, extension)| extension);

        match extension {
            Some("usfm" | "sfm") => Some(Self::Usfm),
            Some("osis") => Some(Self::Osis),
            Some("conf") => Some(Self::Sword),
            Some("sword") => Some(Self::Sword),
            Some("zip") => {
                // A zip holds either a folder of USFM documents or a SWORD module.
                let names = zip::entry_names(path).unwrap_or_default();
                if names.iter().any(|name| is_usfm_name(name)) {
                    Some(Self::Usfm)
                } else {
                    Some(Self::Sword)
                }
            }
            Some("xml") => {
                let head = read_head(path)?;
                if head.contains("<XMLBIBLE") || head.contains("<BIBLEBOOK") {
                    Some(Self::Zefania)
                } else {
                    Some(Self::Osis)
                }
            }
            _ => None,
        }
    }
}

/// The first few kilobytes of a file, used only to sniff an XML dialect.
fn read_head(path: &Path) -> Option<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; 4096];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

/// Imports a translation from whatever `path` holds.
///
/// The format is detected from the path, and the returned translation already
/// sits on the canonical grid, so a verse the source leaves out is present with
/// empty text. `unlock_key` is only used by locked SWORD modules.
pub fn import_path(
    path: &Path,
    meta: TranslationMeta,
    unlock_key: Option<&str>,
) -> Result<Translation> {
    let format = ImportFormat::detect(path).with_context(|| {
        format!(
            "{} is not a USFM, OSIS, Zefania, or SWORD source",
            path.display()
        )
    })?;
    import_with(format, path, meta, unlock_key)
}

/// Imports USFM that the caller already holds as text.
///
/// Pass every document of one translation, because a source that splits its books
/// across files is still one translation.
pub fn import_usfm(documents: &[&str], meta: TranslationMeta) -> Result<Translation> {
    usfm::import(documents, meta)
}

/// Imports an OSIS XML document from a string.
pub fn import_osis(xml: &str, meta: TranslationMeta) -> Result<Translation> {
    osis::import(xml, meta)
}

/// Imports a Zefania XML document from a string.
pub fn import_zefania(xml: &str, meta: TranslationMeta) -> Result<Translation> {
    zefania::import(xml, meta)
}

/// Imports a SWORD module from a zip, a folder, or its `mods.d` manifest.
///
/// `unlock_key` is the key a user owns for a locked module and is ignored when the
/// module is not locked.
pub fn import_sword(
    path: &Path,
    meta: TranslationMeta,
    unlock_key: Option<&str>,
) -> Result<Translation> {
    sword::import(path, meta, unlock_key)
}

/// Imports a translation with the format named explicitly, skipping detection.
pub fn import_with(
    format: ImportFormat,
    path: &Path,
    meta: TranslationMeta,
    unlock_key: Option<&str>,
) -> Result<Translation> {
    match format {
        ImportFormat::Usfm => import_usfm_path(path, meta),
        ImportFormat::Osis => {
            let source = read_text(path)?;
            osis::import(&source, meta)
        }
        ImportFormat::Zefania => {
            let source = read_text(path)?;
            zefania::import(&source, meta)
        }
        ImportFormat::Sword => sword::import(path, meta, unlock_key),
    }
}

/// Imports USFM from one file, a folder of them, or a zip of them.
///
/// Several documents merge into one translation because a source that splits its
/// books across files is still one translation.
fn import_usfm_path(path: &Path, meta: TranslationMeta) -> Result<Translation> {
    let files = usfm_sources(path)?;
    let mut documents = Vec::with_capacity(files.len());
    for (name, bytes) in files {
        let text = decode_text(&bytes)
            .with_context(|| format!("{name} is not readable as UTF-8 or Latin-1 text"))?;
        documents.push(text);
    }
    let borrowed = documents.iter().map(String::as_str).collect::<Vec<_>>();
    usfm::import(&borrowed, meta)
}

/// How a USFM source is named to the diagnostics, paired with its bytes.
type SourceFile = (String, Vec<u8>);

/// Collects the USFM documents behind a file, a folder, or a zip.
fn usfm_sources(path: &Path) -> Result<Vec<SourceFile>> {
    if path.is_dir() {
        return usfm_folder(path);
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if name.to_ascii_lowercase().ends_with(".zip") || name.to_ascii_lowercase().ends_with(".sword")
    {
        let archive = zip::read_archive(path)?;
        let mut files = Vec::new();
        for entry in archive {
            if is_usfm_name(&entry.name) {
                files.push((entry.name, entry.bytes));
            }
        }
        if files.is_empty() {
            bail!("{} holds no .usfm or .sfm file", path.display());
        }
        check_source_count(files.len())?;
        let mut total = 0usize;
        for (_, bytes) in &files {
            total = total.saturating_add(bytes.len());
        }
        if total as u64 > MAX_SOURCE_TOTAL_BYTES {
            bail!(
                "the archive holds {total} bytes of source, the limit is {MAX_SOURCE_TOTAL_BYTES}"
            );
        }
        return Ok(files);
    }
    Ok(vec![(name, read_source_file(path)?)])
}

/// Reads a source file under a hard size limit.
fn read_source_file(path: &Path) -> Result<Vec<u8>> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot read {}", path.display()))?;
    let length = file
        .metadata()
        .with_context(|| format!("cannot inspect {}", path.display()))?
        .len();
    if length > MAX_SOURCE_FILE_BYTES {
        bail!(
            "{} is {length} bytes, the limit for one source file is {MAX_SOURCE_FILE_BYTES}",
            path.display()
        );
    }
    let mut bytes = Vec::with_capacity(length.min(1024 * 1024) as usize);
    file.take(MAX_SOURCE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("cannot read {}", path.display()))?;
    if bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
        bail!(
            "{} grew past the {MAX_SOURCE_FILE_BYTES} byte limit while it was read",
            path.display()
        );
    }
    Ok(bytes)
}

/// Refuses a source that names more files than an import may read.
fn check_source_count(files: usize) -> Result<()> {
    if files > MAX_SOURCE_FILES {
        bail!("the source names {files} files, the limit is {MAX_SOURCE_FILES}");
    }
    Ok(())
}

/// Collects every `.usfm` and `.sfm` file directly inside a folder, in name order.
fn usfm_folder(path: &Path) -> Result<Vec<SourceFile>> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(path)
        .with_context(|| format!("cannot list {}", path.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|entry| entry.is_file() && is_usfm_name(&entry.to_string_lossy()))
        .collect();
    names.sort();

    if names.is_empty() {
        bail!("{} holds no .usfm or .sfm file", path.display());
    }
    check_source_count(names.len())?;

    let mut total = 0u64;
    let mut sources = Vec::with_capacity(names.len());
    for name in names {
        let label = name
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let bytes = read_source_file(&name)?;
        total = total.saturating_add(bytes.len() as u64);
        if total > MAX_SOURCE_TOTAL_BYTES {
            bail!(
                "the folder holds {total} bytes of source, the limit is {MAX_SOURCE_TOTAL_BYTES}"
            );
        }
        sources.push((label, bytes));
    }
    Ok(sources)
}

/// Whether a file name looks like a USFM document by its extension.
fn is_usfm_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".usfm") || lower.ends_with(".sfm")
}

/// Reads a text file, accepting UTF-8 and falling back to Latin-1.
fn read_text(path: &Path) -> Result<String> {
    let bytes = read_source_file(path)?;
    decode_text(&bytes).with_context(|| format!("{} is not text", path.display()))
}

/// Decodes bytes as UTF-8, stripping a byte-order mark, and falls back to Latin-1.
pub(crate) fn decode_text(bytes: &[u8]) -> Result<String> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_owned()),
        Err(_) => Ok(bytes.iter().map(|byte| char::from(*byte)).collect()),
    }
}

/// One verse as the source states it, before it is laid on the canonical grid.
#[derive(Clone, Debug, Default)]
pub(crate) struct RawVerse {
    /// Last printed number of a range such as `13-14`, equal to the first for a single verse.
    pub(crate) number_end: u8,
    /// The verse text with source markup removed.
    pub(crate) text: String,
    /// Structural breaks, with offsets relative to the verse text.
    pub(crate) marks: Vec<RawMark>,
    /// Character styles, with offsets relative to the verse text.
    pub(crate) spans: Vec<RawSpan>,
}

/// One structural break found in a source verse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RawMark {
    /// Byte offset from the verse start.
    pub(crate) offset: u16,
    /// What kind of break this is.
    pub(crate) kind: MarkKind,
    /// Depth for an indented poetry line.
    pub(crate) level: u8,
    /// Heading or superscription text, empty for breaks that carry none.
    pub(crate) payload: String,
}

/// One character style found in a source verse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawSpan {
    /// First byte of the styled range.
    pub(crate) start: u16,
    /// Byte just past the styled range.
    pub(crate) end: u16,
    /// The style applied to the range.
    pub(crate) style: SpanStyle,
}

impl RawVerse {
    /// Trims trailing whitespace and drops or clamps anything that pointed past it.
    ///
    /// Sources trail spaces and newlines, and a style that was opened and never
    /// closed can end past the text; both are corrected here rather than being
    /// allowed to fail the whole import.
    pub(crate) fn tidy(&mut self) {
        let trimmed = self.text.trim_end().len();
        self.text.truncate(trimmed);
        let len = trimmed as u16;

        for mark in &mut self.marks {
            mark.offset = mark.offset.min(len);
        }
        self.spans.retain_mut(|span| {
            span.end = span.end.min(len);
            span.start < span.end
        });
    }
}

/// Accumulates one verse while a source is being read.
///
/// Every importer speaks to this type instead of assembling verses itself, so the
/// whitespace folding, the mark offsets, and the span clamping are written once.
#[derive(Debug)]
pub(crate) struct VerseAccumulator {
    /// Canonical book the verse belongs to.
    pub(crate) book: u8,
    /// Chapter the verse belongs to.
    pub(crate) chapter: u8,
    /// Printed verse number, zero for a chapter superscription.
    pub(crate) number: u8,
    /// Last printed number of a range.
    pub(crate) number_end: u8,
    /// Verse text collected so far.
    pub(crate) text: String,
    /// Whether the next text appended needs a space before it.
    space_pending: bool,
    /// Structural breaks collected so far.
    pub(crate) marks: Vec<RawMark>,
    /// Character styles collected so far.
    pub(crate) spans: Vec<RawSpan>,
}

impl VerseAccumulator {
    /// Starts an empty verse.
    pub(crate) fn new(book: u8, chapter: u8, number: u8, number_end: u8) -> Self {
        Self {
            book,
            chapter,
            number,
            number_end: number_end.max(number),
            text: String::new(),
            space_pending: false,
            marks: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// The byte offset of the end of the verse text so far.
    pub(crate) fn offset(&self) -> u16 {
        u16::try_from(self.text.len()).unwrap_or(u16::MAX)
    }

    /// Appends text, folding every run of whitespace to one space.
    ///
    /// Sources split a verse across markers, so a run of text has to join the
    /// previous one the way the source's own whitespace says. A marker that closes
    /// right before a full stop therefore yields `earth.` and not `earth .`.
    pub(crate) fn push_text(&mut self, segment: &str) {
        // A paragraph glyph the source prints carries nothing the model does not
        // already record as a paragraph mark, so it is dropped where it opens a verse.
        let segment = if self.text.is_empty() {
            segment.trim_start_matches(['\u{00b6}', ' ', '\t'])
        } else {
            segment
        };
        if segment.is_empty() {
            return;
        }
        let mut separator = self.space_pending || segment.starts_with(char::is_whitespace);
        for word in segment.split_whitespace() {
            let needed = word.len() + usize::from(!self.text.is_empty() && separator);
            if self.text.len() + needed > MAX_VERSE_TEXT_BYTES {
                // The format stores a `u16` length per verse, so text past this
                // point could never be written. Stop rather than grow forever.
                return;
            }
            if !self.text.is_empty() && separator {
                self.text.push(' ');
            }
            self.text.push_str(word);
            separator = true;
        }
        self.space_pending = segment.chars().next_back().is_some_and(char::is_whitespace);
    }

    /// Attaches a structural break at the current end of the verse text.
    pub(crate) fn add_mark(&mut self, kind: MarkKind, level: u8, payload: &str) {
        self.add_mark_at(self.offset(), kind, level, payload);
    }

    /// Attaches a structural break at an offset already recorded.
    pub(crate) fn add_mark_at(&mut self, offset: u16, kind: MarkKind, level: u8, payload: &str) {
        if self.marks.len() >= MAX_VERSE_MARKS {
            return;
        }
        let offset = offset.min(self.offset());
        self.marks.push(RawMark {
            offset,
            kind,
            level,
            payload: payload.trim().to_owned(),
        });
    }

    /// Closes a styled range that started at `start`, dropping it when it is empty.
    pub(crate) fn close_span(&mut self, style: SpanStyle, start: u16) {
        if self.spans.len() >= MAX_VERSE_SPANS {
            return;
        }
        let end = self.offset();
        if end > start {
            self.spans.push(RawSpan { start, end, style });
        }
    }

    /// Finishes the verse, trimming trailing whitespace and clamping what pointed past it.
    pub(crate) fn finish(mut self) -> RawVerse {
        trim_spans(&self.text, &mut self.spans);
        let mut raw = RawVerse {
            number_end: self.number_end,
            text: self.text,
            marks: self.marks,
            spans: self.spans,
        };
        raw.tidy();
        raw
    }
}

/// Pulls style ranges in off the spaces a marker's own separator left behind.
fn trim_spans(text: &str, spans: &mut [RawSpan]) {
    let bytes = text.as_bytes();
    for span in spans.iter_mut() {
        if span.end as usize > bytes.len() {
            span.end = bytes.len() as u16;
        }
        while (span.start as usize) < span.end as usize && bytes[span.start as usize] == b' ' {
            span.start += 1;
        }
        while (span.end as usize) > span.start as usize && bytes[span.end as usize - 1] == b' ' {
            span.end -= 1;
        }
    }
}

/// The verses of one source, keyed by canonical book, chapter, and verse.
///
/// A key of verse zero holds a chapter superscription, which belongs to the
/// chapter rather than to any one verse.
#[derive(Debug, Default)]
pub(crate) struct Document {
    verses: BTreeMap<(u8, u8, u8), RawVerse>,
    book_titles: Vec<Option<String>>,
    title: Option<String>,
    red_letters: bool,
    paragraphs: bool,
    headings: bool,
}

impl Document {
    /// Creates an empty document with a title slot for every canonical book.
    pub(crate) fn new() -> Self {
        Self {
            verses: BTreeMap::new(),
            book_titles: vec![None; BOOK_COUNT],
            title: None,
            red_letters: false,
            paragraphs: false,
            headings: false,
        }
    }

    /// Stores one verse, keeping a later reading of the same slot only if it has text.
    pub(crate) fn insert(&mut self, book: u8, chapter: u8, verse: u8, raw: RawVerse) {
        if raw.text.is_empty() && raw.marks.is_empty() && raw.spans.is_empty() {
            return;
        }
        self.verses.insert((book, chapter, verse), raw);
    }

    /// Records the display title of a canonical book.
    pub(crate) fn set_book_title(&mut self, book: u8, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        if let Some(slot) = self.book_titles.get_mut(book as usize)
            && slot.is_none()
        {
            *slot = Some(title.to_owned());
        }
    }

    /// Records the source's own title for the translation, such as a `\ide` line.
    pub(crate) fn set_title(&mut self, title: &str) {
        let title = title.trim();
        if !title.is_empty() && self.title.is_none() {
            self.title = Some(title.to_owned());
        }
    }

    /// Notes that the source marks a feature the model records as a flag.
    pub(crate) fn note_feature(&mut self, kind: MarkKind) {
        match kind {
            MarkKind::Paragraph => self.paragraphs = true,
            MarkKind::Heading => self.headings = true,
            _ => {}
        }
    }

    /// Marks the source as carrying words of Christ.
    pub(crate) fn note_red_letters(&mut self) {
        self.red_letters = true;
    }

    /// Whether the source stated no verse at all.
    pub(crate) fn is_empty(&self) -> bool {
        self.verses.is_empty()
    }

    /// Lays the source onto the canonical grid, one slot per canonical verse.
    ///
    /// Metadata supplied by the caller wins; the source only fills fields the
    /// caller left empty, so an import cannot silently rename a translation.
    pub(crate) fn assemble(mut self, mut meta: TranslationMeta) -> Result<Translation> {
        if meta.name.trim().is_empty()
            && let Some(title) = &self.title
        {
            meta.name = title.clone();
        }
        if meta.book_titles.is_empty() {
            meta.book_titles = self
                .book_titles
                .iter()
                .map(|title| title.clone().unwrap_or_default())
                .collect();
        } else if meta.book_titles.len() != BOOK_COUNT {
            bail!(
                "metadata has {} book titles, the canonical grid has {BOOK_COUNT}",
                meta.book_titles.len()
            );
        } else {
            for (slot, title) in meta.book_titles.iter_mut().zip(&self.book_titles) {
                if let (true, Some(title)) = (slot.is_empty(), title) {
                    *slot = title.clone();
                }
            }
        }
        meta.red_letters |= self.red_letters;
        meta.paragraphs |= self.paragraphs;
        meta.headings |= self.headings;

        let mut builder = TranslationBuilder::new(meta);
        for book in 0..BOOK_COUNT as u8 {
            let Some(entry) = books::book(book) else {
                continue;
            };
            for chapter in 1..=entry.chapter_count {
                let superscription = self.verses.remove(&(book, chapter, 0)).map(|raw| raw.text);
                let verse_count = books::verses_in_chapter(book, chapter).unwrap_or(0);
                let mut covered_to = 0u8;

                for verse in 1..=verse_count as u8 {
                    let raw = if verse <= covered_to {
                        None
                    } else {
                        self.verses.remove(&(book, chapter, verse))
                    };
                    let text = raw.as_ref().map(|raw| raw.text.as_str()).unwrap_or("");
                    let mut number_end = verse;
                    if let Some(raw) = &raw
                        && raw.number_end > verse
                        && !self.verses.contains_key(&(book, chapter, verse + 1))
                    {
                        number_end = raw.number_end.min(verse_count as u8);
                    }
                    covered_to = covered_to.max(number_end);

                    builder.push_verse_range(verse, number_end, text)?;

                    if verse == 1
                        && let Some(text) = &superscription
                    {
                        builder.mark(0, MarkKind::Superscription, 0, text)?;
                    }
                    if let Some(raw) = &raw {
                        for mark in &raw.marks {
                            builder.mark(mark.offset, mark.kind, mark.level, &mark.payload)?;
                        }
                        for span in &raw.spans {
                            builder.span(span.style, span.start, span.end)?;
                        }
                    }
                }
            }
        }
        builder.finish()
    }
}

/// Lays a parsed source onto the canonical grid, keeping the caller's metadata.
pub(crate) fn assemble(document: Document, meta: TranslationMeta) -> Result<Translation> {
    document.assemble(meta)
}

/// The USFM book codes, in canonical order.
static USFM_CODES: [&str; BOOK_COUNT] = [
    "GEN", "EXO", "LEV", "NUM", "DEU", "JOS", "JDG", "RUT", "1SA", "2SA", "1KI", "2KI", "1CH",
    "2CH", "EZR", "NEH", "EST", "JOB", "PSA", "PRO", "ECC", "SNG", "ISA", "JER", "LAM", "EZK",
    "DAN", "HOS", "JOL", "AMO", "OBA", "JON", "MIC", "NAM", "HAB", "ZEP", "HAG", "ZEC", "MAL",
    "MAT", "MRK", "LUK", "JHN", "ACT", "ROM", "1CO", "2CO", "GAL", "EPH", "PHP", "COL", "1TH",
    "2TH", "1TI", "2TI", "TIT", "PHM", "HEB", "JAS", "1PE", "2PE", "1JN", "2JN", "3JN", "JUD",
    "REV",
];

/// The OSIS book identifiers, in canonical order.
static OSIS_IDS: [&str; BOOK_COUNT] = [
    "Gen", "Exod", "Lev", "Num", "Deut", "Josh", "Judg", "Ruth", "1Sam", "2Sam", "1Kgs", "2Kgs",
    "1Chr", "2Chr", "Ezra", "Neh", "Esth", "Job", "Ps", "Prov", "Eccl", "Song", "Isa", "Jer",
    "Lam", "Ezek", "Dan", "Hos", "Joel", "Amos", "Obad", "Jonah", "Mic", "Nah", "Hab", "Zeph",
    "Hag", "Zech", "Mal", "Matt", "Mark", "Luke", "John", "Acts", "Rom", "1Cor", "2Cor", "Gal",
    "Eph", "Phil", "Col", "1Thess", "2Thess", "1Tim", "2Tim", "Titus", "Phlm", "Heb", "Jas",
    "1Pet", "2Pet", "1John", "2John", "3John", "Jude", "Rev",
];

/// Folds a code to the form the tables use: lowercase, letters and digits only.
fn fold(code: &str) -> String {
    code.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Finds the canonical book a source code, identifier, or name refers to.
///
/// Sources name books differently, so the USFM codes, the OSIS identifiers, the
/// canonical names, and the canonical abbreviations all resolve. Anything else,
/// such as a deuterocanonical book, returns `None` and is skipped by the
/// importers so the canonical grid never shifts.
pub(crate) fn book_from_code(code: &str) -> Option<u8> {
    let folded = fold(code);
    if folded.is_empty() {
        return None;
    }

    for (index, candidate) in USFM_CODES.iter().enumerate() {
        if fold(candidate) == folded {
            return Some(index as u8);
        }
    }
    for (index, candidate) in OSIS_IDS.iter().enumerate() {
        if fold(candidate) == folded {
            return Some(index as u8);
        }
    }
    for index in 0..BOOK_COUNT {
        let Some(entry) = books::book(index as u8) else {
            continue;
        };
        if fold(entry.name) == folded || fold(entry.abbreviation) == folded {
            return Some(index as u8);
        }
    }
    // The spellings sources use where the canonical name is not what they print.
    let alias = match folded.as_str() {
        "songofsongs" | "canticles" => 21,
        "psalm" | "thepsalms" => 18,
        "revelations" | "apocalypse" => 65,
        _ => return None,
    };
    Some(alias)
}
