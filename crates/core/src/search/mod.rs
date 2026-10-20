// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Verse search over a translation.
//!
//! The engine is a scan, not an index. [`Corpus`] normalizes a whole translation
//! into one space-wrapped string, [`Query`] parses the query language into typed
//! terms with a match mode and a scope, and [`Corpus::search`] runs the SIMD
//! substring scan and returns the matching verses in canonical order.
//!
//! ```
//! use bible_core::search::{Corpus, Query, Scope};
//!
//! # fn example(corpus: &Corpus) {
//! let query = Query::parse("light -darkness").with_scope(Scope::NewTestament);
//! let hits = corpus.search(&query);
//! # let _ = hits;
//! # }
//! ```

mod corpus;
mod query;
mod scan;

pub use corpus::{Corpus, normalize};
pub use query::{MatchMode, Query, Scope, Term};
pub use scan::Hit;
