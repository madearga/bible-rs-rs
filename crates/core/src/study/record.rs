// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The records the study store keeps.
//!
//! Every record is keyed by a [`Reference`] into the canonical grid, never by a
//! translation, so a highlight or a note made while reading one translation is
//! still there in every other. The types here are plain data: they know nothing
//! about SQLite or about a file, which lets the store and the exporters share
//! them without either one reaching into the other.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Reference;

/// One of the six highlight colours a verse can carry.
///
/// A verse with no highlight is represented by `None` rather than by a colour,
/// so clearing a highlight and never having set one produce the same state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HighlightColour {
    /// The first colour, chosen by the `1` key.
    Yellow,
    /// The second colour, chosen by the `2` key.
    Green,
    /// The third colour, chosen by the `3` key.
    Blue,
    /// The fourth colour, chosen by the `4` key.
    Pink,
    /// The fifth colour, chosen by the `5` key.
    Orange,
    /// The sixth colour, chosen by the `6` key.
    Purple,
}

impl HighlightColour {
    /// Every colour, in the order the reader's `1` to `6` keys select them.
    pub const ALL: [Self; 6] = [
        Self::Yellow,
        Self::Green,
        Self::Blue,
        Self::Pink,
        Self::Orange,
        Self::Purple,
    ];

    /// The stable name this colour is written as in JSON and in Markdown.
    ///
    /// The name is part of the export format, so it must not change even if the
    /// palette is restyled.
    pub fn name(self) -> &'static str {
        match self {
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Pink => "pink",
            Self::Orange => "orange",
            Self::Purple => "purple",
        }
    }

    /// The small integer the database row stores for this colour.
    pub fn code(self) -> u8 {
        match self {
            Self::Yellow => 1,
            Self::Green => 2,
            Self::Blue => 3,
            Self::Pink => 4,
            Self::Orange => 5,
            Self::Purple => 6,
        }
    }

    /// Recovers a colour from a stored code, rejecting codes this build does not
    /// know so a newer database cannot be read as if it were current.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|colour| colour.code() == code)
    }
}

impl fmt::Display for HighlightColour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A note attached to one verse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// The note text exactly as the reader wrote it.
    pub body: String,
}

impl Note {
    /// Wraps a body in a note. An empty or blank body is not a note, and the
    /// store rejects it rather than keeping a note nobody can see.
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

/// A marker on one verse, with an optional label.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmark {
    /// A short label the reader added, or `None` for a bare bookmark.
    pub label: Option<String>,
}

impl Bookmark {
    /// A bookmark with no label, which is what the `B` key creates.
    pub fn unlabelled() -> Self {
        Self { label: None }
    }

    /// A bookmark carrying a label the reader typed.
    pub fn with_label(label: impl Into<String>) -> Self {
        let label = label.into();
        if label.trim().is_empty() {
            return Self::unlabelled();
        }
        Self { label: Some(label) }
    }
}

/// The looks the reader offers: three chosen for long reading, plus the
/// automatic one that follows the system appearance.
///
/// A settings row written before the automatic choice existed names a concrete
/// theme, so an older row reads exactly as it was saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Warm paper.
    Light,
    /// Aged paper, easier in low light than white.
    Sepia,
    /// Dimmed, but not pure black.
    Dark,
    /// Whichever of light and dark the system is showing.
    Auto,
}

/// Where the reader was and how it looked.
///
/// These are preferences rather than study data, so the store writes them at
/// most every 1.5 seconds: scrolling updates the position many times a second
/// and none of those updates should wait on the disk.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The verse the reader was last anchored on.
    pub position: Reference,
    /// Which theme is showing.
    pub theme: Theme,
    /// Body text size in points.
    pub text_size: f32,
    /// Line height as a multiple of the text size.
    pub line_spacing: f32,
    /// Measure in em, capped so a line stays readable.
    pub line_length: f32,
    /// Whether body text is justified rather than left aligned.
    pub justify: bool,
    /// Whether verse numbers show in the margin.
    pub show_verse_numbers: bool,
    /// Whether section headings show.
    pub show_headings: bool,
    /// Whether the words of Christ are drawn in red.
    pub words_of_christ_red: bool,
    /// Whether verses flow inside paragraphs rather than starting a new line.
    ///
    /// The default is on, which is what the reader already did, so a settings
    /// row written before this field existed keeps its old behavior.
    pub paragraph_mode: bool,
    /// Whether the sidebar was open when the reader last stopped.
    ///
    /// The default is closed, which is what a settings row written before this
    /// field existed reads as, so an older row opens with the sidebar shut
    /// until the reader opens it again.
    pub sidebar_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            position: Reference {
                book: 0,
                chapter: 1,
                verse: Some(1),
            },
            theme: Theme::Light,
            text_size: 17.0,
            line_spacing: 1.62,
            line_length: 34.0,
            justify: false,
            show_verse_numbers: true,
            show_headings: true,
            words_of_christ_red: true,
            paragraph_mode: true,
            sidebar_open: false,
        }
    }
}

/// Everything the store holds at one instant, in canonical order.
///
/// A snapshot is the unit of comparison and the input to the exporters. The
/// vectors are sorted by verse, so two snapshots of equal content compare equal
/// and an export is byte stable.
#[derive(Clone, Debug, PartialEq)]
pub struct StudyData {
    /// Every highlight, ordered by verse.
    pub highlights: Vec<(Reference, HighlightColour)>,
    /// Every note, ordered by verse.
    pub notes: Vec<(Reference, Note)>,
    /// Every bookmark, ordered by verse.
    pub bookmarks: Vec<(Reference, Bookmark)>,
    /// The settings in force.
    pub settings: Settings,
}
