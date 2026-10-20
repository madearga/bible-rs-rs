// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The search query and its parser.
//!
//! A query is not a bag of strings. It is one [`Query`] value carrying the
//! normalized terms, whether each one is required or excluded, the match mode,
//! and the scope. Keeping those as typed fields means the scanner never has to
//! re-read the user's text and the display layer can render the terms without
//! guessing what a leading minus meant.

use super::corpus::normalize;

/// How a term matches against a word in the corpus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchMode {
    /// A term matches the start of a word, so `light` finds `lightning`.
    #[default]
    Prefix,
    /// A term must match the whole word, so `light` finds only `light`.
    WholeWord,
}

/// The part of the canon a search looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// Every book, Genesis through Revelation.
    #[default]
    WholeBible,
    /// Genesis through Malachi.
    OldTestament,
    /// Matthew through Revelation.
    NewTestament,
    /// One book, named by its canonical index.
    Book(u8),
}

/// One normalized search term.
///
/// A term keeps its internal spaces, so a quoted phrase is a single term and the
/// scanner treats it as one longer needle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    text: String,
    excluded: bool,
}

impl Term {
    /// The normalized needle text, with single spaces between the words of a phrase.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether a matching verse must be removed from the results.
    pub fn is_excluded(&self) -> bool {
        self.excluded
    }
}

/// A parsed search request.
///
/// The parser fills the terms, and the caller may change the mode and the scope
/// before handing the query to the corpus. A query with no terms matches nothing
/// and the scanner returns an empty result immediately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    terms: Vec<Term>,
    mode: MatchMode,
    scope: Scope,
}

impl Query {
    /// Parses the query language into terms, with prefix matching over the whole Bible.
    ///
    /// Words are separated by whitespace, double quotes group a phrase into one
    /// term, and a leading minus marks the next term as excluded.
    pub fn parse(input: &str) -> Self {
        Self {
            terms: parse_terms(input),
            mode: MatchMode::Prefix,
            scope: Scope::WholeBible,
        }
    }

    /// Returns the query with a different match mode.
    pub fn with_mode(mut self, mode: MatchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns the query with a different scope.
    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }

    /// The normalized terms in the order the user typed them.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// The match mode the scanner applies to every term.
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// The part of the canon the scanner looks at.
    pub fn scope(&self) -> Scope {
        self.scope
    }

    /// Whether the query has no terms, so it matches nothing.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Splits the raw query text into normalized terms.
fn parse_terms(input: &str) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut rest = input.chars().peekable();
    loop {
        while matches!(rest.peek(), Some(ch) if ch.is_whitespace()) {
            rest.next();
        }
        let Some(&first) = rest.peek() else {
            break;
        };
        let mut excluded = false;
        if first == '-' {
            rest.next();
            excluded = true;
            while matches!(rest.peek(), Some(ch) if ch.is_whitespace()) {
                rest.next();
            }
            if rest.peek().is_none() {
                break;
            }
        }

        let mut raw = String::new();
        if rest.peek() == Some(&'"') {
            rest.next();
            for ch in rest.by_ref() {
                if ch == '"' {
                    break;
                }
                raw.push(ch);
            }
        } else {
            while let Some(&ch) = rest.peek() {
                if ch.is_whitespace() {
                    break;
                }
                rest.next();
                raw.push(ch);
            }
        }

        let text = normalize(&raw);
        if !text.is_empty() {
            terms.push(Term { text, excluded });
        }
    }
    terms
}
