// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The normalized corpus that every search scans.
//!
//! One translation becomes one string of space-wrapped verses. Each verse's text
//! is lower-cased, stripped of diacritics, stripped of apostrophes, and has every
//! other punctuation mark turned into a single space. The verse text is surrounded
//! by one space on each side, so concatenating verses leaves two spaces between
//! them and a needle can never match across a verse boundary.
//!
//! The corpus keeps the byte offset where each verse starts. A scan reports a byte
//! position and a binary search turns it into a canonical verse index.

use crate::model::Translation;

/// Normalizes text into the form the corpus stores.
///
/// This is the single definition of matching: lower case, folded diacritics,
/// apostrophes removed so the parts of a contraction join, every other
/// non-alphanumeric character turned into one space, and runs of spaces collapsed.
/// A caller uses it directly when it needs to compare a display string with a
/// query term.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_apostrophe(ch) {
            continue;
        }
        let mut produced = false;
        let mut separator = false;
        for lower in ch.to_lowercase() {
            match fold(lower) {
                Some("") => {}
                Some(folded) => {
                    out.push_str(folded);
                    produced = true;
                }
                None if lower.is_alphanumeric() => {
                    out.push(lower);
                    produced = true;
                }
                None => separator = true,
            }
        }
        if !produced && separator {
            push_space(&mut out);
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Whether the character is an apostrophe that should join the text around it.
fn is_apostrophe(ch: char) -> bool {
    matches!(ch, '\'' | '\u{2019}' | '\u{02bc}' | '\u{2018}')
}

/// Adds one space unless the string is empty or already ends with a space.
fn push_space(out: &mut String) {
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}

/// Maps a lower-case character with a diacritic to its unaccented form.
///
/// Returns `Some("")` for a combining mark that should disappear, `Some(folded)`
/// for a character that folds to one or more plain letters, and `None` for a
/// character that needs no folding.
fn fold(ch: char) -> Option<&'static str> {
    Some(match ch {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ţ' | 'ť' | 'ŧ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'ß' => "ss",
        'þ' => "th",
        '\u{0300}'..='\u{036f}' => "",
        _ => return None,
    })
}

/// The normalized text of a whole translation, ready to scan.
///
/// Build it once per translation and keep it for the lifetime of the reader. A
/// search never allocates a per-verse string and never builds an index.
#[derive(Clone, Debug)]
pub struct Corpus {
    normalized: String,
    verse_starts: Vec<u32>,
}

impl Corpus {
    /// Builds the corpus for one translation.
    ///
    /// Every verse is normalized and wrapped in single spaces, so the string holds
    /// exactly one leading space per verse and two spaces at every verse boundary.
    pub fn from_translation(translation: &Translation) -> Self {
        let verses = translation.verses();
        let mut normalized = String::with_capacity(translation.text().len() + verses.len() * 2);
        let mut verse_starts = Vec::with_capacity(verses.len() + 1);
        for verse in verses {
            verse_starts.push(u32::try_from(normalized.len()).unwrap_or(u32::MAX));
            normalized.push(' ');
            normalized.push_str(&normalize(translation.verse_text(verse)));
            normalized.push(' ');
        }
        verse_starts.push(u32::try_from(normalized.len()).unwrap_or(u32::MAX));
        Self {
            normalized,
            verse_starts,
        }
    }

    /// The whole normalized string, for tests and for tools that need the shape.
    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    /// How many verses the corpus covers, one entry per canonical slot.
    pub fn verse_count(&self) -> usize {
        self.verse_starts.len().saturating_sub(1)
    }

    /// The byte offset where a verse's wrapped line starts.
    pub(crate) fn verse_start(&self, verse: usize) -> usize {
        self.verse_starts[verse] as usize
    }

    /// The wrapped normalized line of one verse, including its two boundary spaces.
    pub(crate) fn verse_slice(&self, verse: usize) -> &str {
        let start = self.verse_starts[verse] as usize;
        let end = self.verse_starts[verse + 1] as usize;
        &self.normalized[start..end]
    }

    /// The verse a normalized byte position belongs to.
    ///
    /// A match always starts on the leading space of the verse it belongs to,
    /// because a match cannot begin on a trailing boundary space, so a binary
    /// search over the verse starts is exact.
    pub(crate) fn verse_at(&self, position: usize) -> usize {
        self.verse_starts
            .partition_point(|&start| start as usize <= position)
            - 1
    }
}
