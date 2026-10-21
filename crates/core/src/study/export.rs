// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Taking the study data out of the store and putting it back.
//!
//! JSON is the archive format: it names every field explicitly, so a reader can
//! inspect it, and it round-trips through import without loss. Markdown is the
//! format for a human: it lists the highlighted verses, the notes, and the
//! bookmarks by reference. Neither format is versioned by the store's schema, so
//! both carry an explicit format version and refuse a file they do not know.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::Reference;
use crate::books;
use crate::study::record::{Bookmark, HighlightColour, Note, Settings, StudyData, Theme};

/// The only export format version this build writes and reads.
const EXPORT_VERSION: u32 = 1;

/// A verse position as it appears in the export, with no dependence on the
/// in-memory reference type.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Place {
    /// Canonical book index.
    book: u8,
    /// 1-based chapter number.
    chapter: u8,
    /// 1-based verse number, absent when only a chapter is meant.
    verse: Option<u8>,
}

impl Place {
    /// Describes a reference for the export.
    fn of(reference: Reference) -> Self {
        Self {
            book: reference.book,
            chapter: reference.chapter,
            verse: reference.verse,
        }
    }

    /// Resolves the exported position back into a reference, refusing a position
    /// the canonical grid does not contain.
    fn to_reference(self) -> Result<Reference> {
        Reference::new(self.book, self.chapter, self.verse).with_context(|| {
            format!(
                "exported position {}:{} is outside the canonical grid",
                self.chapter,
                self.verse
                    .map_or_else(|| "-".to_owned(), |verse| verse.to_string())
            )
        })
    }
}

/// One highlight in the export.
#[derive(Debug, Serialize, Deserialize)]
struct HighlightEntry {
    /// The highlighted verse.
    reference: Place,
    /// Which of the six colours it carries.
    colour: HighlightColour,
}

/// One note in the export.
#[derive(Debug, Serialize, Deserialize)]
struct NoteEntry {
    /// The verse the note hangs on.
    reference: Place,
    /// The note text.
    body: String,
}

/// One bookmark in the export.
#[derive(Debug, Serialize, Deserialize)]
struct BookmarkEntry {
    /// The bookmarked verse.
    reference: Place,
    /// The label the reader added, if any.
    label: Option<String>,
}

/// The settings as they appear in the export.
#[derive(Debug, Serialize, Deserialize)]
struct SettingsEntry {
    /// The verse the reader was last anchored on.
    position: Place,
    /// The theme that was showing.
    theme: Theme,
    /// Body text size in points.
    text_size: f32,
    /// Line height as a multiple of the text size.
    line_spacing: f32,
    /// Measure in em.
    line_length: f32,
    /// Whether body text was justified.
    justify: bool,
    /// Whether verse numbers were showing.
    show_verse_numbers: bool,
    /// Whether section headings were showing.
    show_headings: bool,
    /// Whether the words of Christ were red.
    words_of_christ_red: bool,
    /// Whether verses flowed inside paragraphs.
    ///
    /// An export written before this field existed omits it, so the default
    /// keeps the reader's earlier paragraph behavior rather than failing.
    #[serde(default = "default_paragraph_mode")]
    paragraph_mode: bool,
}

/// The paragraph mode an export or settings row written before the field existed
/// is read as.
fn default_paragraph_mode() -> bool {
    true
}

/// The smallest body text size the interface offers, in points.
const MIN_TEXT_SIZE: f32 = 12.0;

/// The largest body text size the interface offers, in points.
const MAX_TEXT_SIZE: f32 = 32.0;

/// The smallest line spacing the interface offers, as a multiple of the size.
const MIN_LINE_SPACING: f32 = 1.0;

/// The largest line spacing the interface offers, as a multiple of the size.
const MAX_LINE_SPACING: f32 = 2.6;

/// The narrowest measure the interface offers, in em.
const MIN_LINE_LENGTH: f32 = 18.0;

/// The widest measure the interface offers, in em.
const MAX_LINE_LENGTH: f32 = 60.0;

/// Checks one typography value against the range the interface supports.
fn check_range(name: &str, value: f32, min: f32, max: f32) -> Result<()> {
    if !value.is_finite() {
        bail!("the exported {name} is not a finite number");
    }
    if !(min..=max).contains(&value) {
        bail!("the exported {name} is {value}, which is outside {min} to {max}");
    }
    Ok(())
}

impl SettingsEntry {
    /// Flattens the live settings into their exported form.
    fn of(settings: &Settings) -> Self {
        Self {
            position: Place::of(settings.position),
            theme: settings.theme,
            text_size: settings.text_size,
            line_spacing: settings.line_spacing,
            line_length: settings.line_length,
            justify: settings.justify,
            show_verse_numbers: settings.show_verse_numbers,
            show_headings: settings.show_headings,
            words_of_christ_red: settings.words_of_christ_red,
            paragraph_mode: settings.paragraph_mode,
        }
    }

    /// Rebuilds the live settings from the exported form.
    ///
    /// Typography values are checked against the ranges the settings window
    /// offers, so a document cannot import a negative or extreme size.
    fn into_settings(self) -> Result<Settings> {
        check_range("text size", self.text_size, MIN_TEXT_SIZE, MAX_TEXT_SIZE)?;
        check_range(
            "line spacing",
            self.line_spacing,
            MIN_LINE_SPACING,
            MAX_LINE_SPACING,
        )?;
        check_range(
            "line length",
            self.line_length,
            MIN_LINE_LENGTH,
            MAX_LINE_LENGTH,
        )?;
        Ok(Settings {
            position: self.position.to_reference()?,
            theme: self.theme,
            text_size: self.text_size,
            line_spacing: self.line_spacing,
            line_length: self.line_length,
            justify: self.justify,
            show_verse_numbers: self.show_verse_numbers,
            show_headings: self.show_headings,
            words_of_christ_red: self.words_of_christ_red,
            paragraph_mode: self.paragraph_mode,
        })
    }
}

/// Encodes the settings as the single JSON column the store keeps, using the
/// same shape the archive uses so there is only one settings format to trust.
pub(crate) fn settings_to_json(settings: &Settings) -> Result<String> {
    serde_json::to_string(&SettingsEntry::of(settings)).context("could not encode the settings")
}

/// Reads the settings column back, refusing a row this build cannot understand.
pub(crate) fn settings_from_json(text: &str) -> Result<Settings> {
    let entry: SettingsEntry = serde_json::from_str(text)
        .context("the stored settings are not the shape this build writes")?;
    entry.into_settings()
}

/// The whole exported document.
#[derive(Debug, Serialize, Deserialize)]
struct ExportFile {
    /// The format version, so a future change can be detected rather than
    /// silently half-read.
    version: u32,
    /// The settings in force at export.
    settings: SettingsEntry,
    /// Every highlight.
    highlights: Vec<HighlightEntry>,
    /// Every note.
    notes: Vec<NoteEntry>,
    /// Every bookmark.
    bookmarks: Vec<BookmarkEntry>,
}

/// The order two verses sort in, which is the canonical reading order.
fn verse_key(reference: Reference) -> Result<u16> {
    let verse = reference
        .verse
        .ok_or_else(|| anyhow::anyhow!("{reference} names no verse"))?;
    books::verse_offset(reference.book, reference.chapter, verse)
        .ok_or_else(|| anyhow::anyhow!("{reference} is outside the canonical grid"))
}

/// Writes the study data as pretty-printed JSON.
pub(crate) fn to_json(data: &StudyData) -> Result<String> {
    let file = ExportFile {
        version: EXPORT_VERSION,
        settings: SettingsEntry::of(&data.settings),
        highlights: data
            .highlights
            .iter()
            .map(|(reference, colour)| HighlightEntry {
                reference: Place::of(*reference),
                colour: *colour,
            })
            .collect(),
        notes: data
            .notes
            .iter()
            .map(|(reference, note)| NoteEntry {
                reference: Place::of(*reference),
                body: note.body.clone(),
            })
            .collect(),
        bookmarks: data
            .bookmarks
            .iter()
            .map(|(reference, bookmark)| BookmarkEntry {
                reference: Place::of(*reference),
                label: bookmark.label.clone(),
            })
            .collect(),
    };
    let mut text =
        serde_json::to_string_pretty(&file).context("could not write the JSON export")?;
    text.push('\n');
    Ok(text)
}

/// Reads an export back into study data.
///
/// The whole document is validated before anything is returned, so a malformed
/// file rejects without the caller having applied half of it.
pub(crate) fn from_json(text: &str) -> Result<StudyData> {
    let file: ExportFile =
        serde_json::from_str(text).context("the study export is not valid JSON")?;
    if file.version != EXPORT_VERSION {
        bail!(
            "the study export is version {}, this build reads version {EXPORT_VERSION}",
            file.version
        );
    }

    let settings = file.settings.into_settings()?;

    let mut highlights = Vec::with_capacity(file.highlights.len());
    for entry in file.highlights {
        highlights.push((entry.reference.to_reference()?, entry.colour));
    }
    let mut notes = Vec::with_capacity(file.notes.len());
    for entry in file.notes {
        if entry.body.trim().is_empty() {
            continue;
        }
        notes.push((entry.reference.to_reference()?, Note::new(entry.body)));
    }
    let mut bookmarks = Vec::with_capacity(file.bookmarks.len());
    for entry in file.bookmarks {
        bookmarks.push((
            entry.reference.to_reference()?,
            Bookmark { label: entry.label },
        ));
    }

    sort_by_verse(&mut highlights)?;
    sort_by_verse(&mut notes)?;
    sort_by_verse(&mut bookmarks)?;

    Ok(StudyData {
        highlights,
        notes,
        bookmarks,
        settings,
    })
}

/// Puts a record list in canonical order and collapses a repeated verse onto the
/// first record, so an imported file that names a verse twice still yields one
/// record per verse like the store holds.
fn sort_by_verse<T>(records: &mut Vec<(Reference, T)>) -> Result<()> {
    let mut keyed = Vec::with_capacity(records.len());
    for record in records.drain(..) {
        keyed.push((verse_key(record.0)?, record));
    }
    keyed.sort_by_key(|(key, _)| *key);
    keyed.dedup_by_key(|(key, _)| *key);
    records.extend(keyed.into_iter().map(|(_, record)| record));
    Ok(())
}

/// Writes the study data as Markdown for a human to read or paste.
pub(crate) fn to_markdown(data: &StudyData) -> String {
    let mut out = String::from("# Study data\n");

    out.push_str("\n## Highlights\n\n");
    if data.highlights.is_empty() {
        out.push_str("_None._\n");
    }
    for (reference, colour) in &data.highlights {
        out.push_str(&format!("- {reference}, {colour}\n"));
    }

    out.push_str("\n## Notes\n");
    if data.notes.is_empty() {
        out.push_str("\n_None._\n");
    }
    for (reference, note) in &data.notes {
        out.push_str(&format!("\n### {reference}\n\n"));
        for line in note.body.lines() {
            out.push_str(line);
            out.push('\n');
        }
    }

    out.push_str("\n## Bookmarks\n\n");
    if data.bookmarks.is_empty() {
        out.push_str("_None._\n");
    }
    for (reference, bookmark) in &data.bookmarks {
        match &bookmark.label {
            Some(label) => out.push_str(&format!("- {reference} ({label})\n")),
            None => out.push_str(&format!("- {reference}\n")),
        }
    }

    out
}
