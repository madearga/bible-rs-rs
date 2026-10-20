// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The core of the Bible reader.
//!
//! This crate owns the canonical grid, the reference type, the translation model,
//! and the binary on-disk format. It has no GUI dependency so the importers, the
//! search, and the study store can build on it without dragging in a toolkit.

#![warn(missing_docs)]

pub mod books;
pub mod format;
pub mod import;
pub mod model;
pub mod reference;

pub use books::{BOOK_COUNT, Book, CHAPTER_COUNT, Testament, VERSE_COUNT};
pub use format::{FORMAT_VERSION, MAGIC, decode, encode};
pub use import::{
    ImportFormat, import_osis, import_path, import_sword, import_usfm, import_with, import_zefania,
};
pub use model::{
    Mark, MarkKind, Span, SpanStyle, Translation, TranslationBuilder, TranslationMeta, Verse,
};
pub use reference::Reference;
