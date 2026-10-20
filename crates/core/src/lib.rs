// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The core of the Bible reader.
//!
//! This crate owns the canonical grid, the reference type, the translation model,
//! the binary on-disk format, and the study store. It has no GUI dependency, so
//! the importers, the search, and the study store can build on it without
//! dragging in a toolkit.

#![warn(missing_docs)]

pub mod books;
pub mod format;
pub mod model;
pub mod reference;
pub mod search;
pub mod study;

pub use books::{BOOK_COUNT, Book, CHAPTER_COUNT, Testament, VERSE_COUNT};
pub use format::{FORMAT_VERSION, MAGIC, decode, encode};
pub use model::{
    Mark, MarkKind, Span, SpanStyle, Translation, TranslationBuilder, TranslationMeta, Verse,
};
pub use reference::Reference;
pub use search::{Corpus, Hit, MatchMode, Query, Scope, Term, normalize};
pub use study::{
    Bookmark, HighlightColour, Note, SETTINGS_WRITE_INTERVAL, Settings, Store, StudyData, Theme,
};
