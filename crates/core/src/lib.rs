// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Core of the Bible reader: the canonical grid, the translation model, the
//! binary format, and verse references.
//!
//! The importers, the search, and the reader all sit on this crate; none of it
//! depends on a GUI.

pub mod books;
pub mod format;
pub mod model;
