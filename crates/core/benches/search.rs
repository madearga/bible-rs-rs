// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The search benchmark over a synthetic full-Bible corpus.
//!
//! The corpus has one generated verse per canonical slot. No Bible text is used.
//! Each query is parsed once and then searched many times in release mode; the
//! report gives the median, minimum, and maximum time per query in milliseconds.

use std::env;
use std::hint::black_box;
use std::time::Instant;

use bible_core::books;
use bible_core::model::{Translation, TranslationBuilder, TranslationMeta};
use bible_core::search::{Corpus, MatchMode, Query, Scope};

/// Consonants the generated words start with.
const ONSETS: [&str; 16] = [
    "b", "c", "d", "f", "g", "h", "l", "m", "n", "p", "r", "s", "t", "v", "w", "z",
];

/// Vowel groups the generated words carry.
const VOWELS: [&str; 8] = ["a", "e", "i", "o", "u", "ae", "ou", "ia"];

/// Endings the generated words close with.
const CODAS: [&str; 12] = ["n", "r", "s", "t", "l", "m", "d", "k", "p", "", "th", "nd"];

/// How many distinct words the generator can produce.
const VOCABULARY: usize = 1536;

fn main() {
    let args: Vec<String> = env::args().collect();
    let measurement = argument(&args, "--measurement-time").unwrap_or(3.0);
    let iterations = (measurement * 50.0).max(20.0) as usize;

    let translation = synthetic_bible();
    let build_start = Instant::now();
    let corpus = Corpus::from_translation(black_box(&translation));
    let build_ms = build_start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "corpus: {} verses, {} normalized bytes, built in {:.3} ms",
        corpus.verse_count(),
        corpus.normalized().len(),
        build_ms
    );
    println!("measurement: {iterations} iterations per query, reporting milliseconds\n");

    let queries = vec![
        ("single common word", Query::parse(&word(7))),
        ("single rare word", Query::parse(&word(900))),
        (
            "quoted phrase",
            Query::parse(&format!("\"{}\"", phrase(11, 23))),
        ),
        (
            "two required terms",
            Query::parse(&format!("{} {}", word(5), word(40))),
        ),
        (
            "required and excluded",
            Query::parse(&format!("{} -{}", word(5), word(3))),
        ),
        (
            "whole word mode",
            Query::parse(&word(7)).with_mode(MatchMode::WholeWord),
        ),
        (
            "old testament scope",
            Query::parse(&word(7)).with_scope(Scope::OldTestament),
        ),
        (
            "new testament scope",
            Query::parse(&word(7)).with_scope(Scope::NewTestament),
        ),
        (
            "one book scope",
            Query::parse(&word(7)).with_scope(Scope::Book(42)),
        ),
    ];

    let mut medians = Vec::with_capacity(queries.len());
    for (label, query) in &queries {
        let hits = corpus.search(query);
        let mut times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let found = corpus.search(black_box(query));
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            black_box(found.len());
        }
        times.sort_by(f64::total_cmp);
        let median = times[times.len() / 2];
        medians.push(median);
        println!(
            "{label:<24} {:>6} hits  median {median:>7.3} ms  min {:>7.3} ms  max {:>7.3} ms",
            hits.len(),
            times[0],
            times[times.len() - 1]
        );
    }

    medians.sort_by(f64::total_cmp);
    println!(
        "\nmedian across the query set: {:.3} ms",
        medians[medians.len() / 2]
    );
}

/// Reads a numeric command-line argument that cargo bench forwards to the binary.
fn argument(args: &[String], name: &str) -> Option<f64> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
}

/// One generated word at the given vocabulary index.
fn word(value: usize) -> String {
    let mut text = String::new();
    push_word(&mut text, value);
    text
}

/// Two generated words separated by one space.
fn phrase(first: usize, second: usize) -> String {
    format!("{} {}", word(first), word(second))
}

/// Appends the generated word at the given vocabulary index.
fn push_word(out: &mut String, value: usize) {
    out.push_str(ONSETS[value % ONSETS.len()]);
    out.push_str(VOWELS[(value / ONSETS.len()) % VOWELS.len()]);
    out.push_str(CODAS[(value / (ONSETS.len() * VOWELS.len())) % CODAS.len()]);
}

/// Builds a full synthetic translation, one generated verse per canonical slot.
fn synthetic_bible() -> Translation {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("bench", "Synthetic Bench"));
    let mut state = 0x2545_f491_4f6c_dd1du64;
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).expect("chapter exists");
            for verse in 1..=count {
                let length = 8 + next(&mut state) % 14;
                let mut text = String::with_capacity(length * 6);
                for position in 0..length {
                    if position > 0 {
                        text.push(' ');
                    }
                    // Most words come from a small set, so common terms are common
                    // and the rest of the vocabulary stays rare.
                    let value = if next(&mut state) % 100 < 80 {
                        next(&mut state) % 64
                    } else {
                        next(&mut state) % VOCABULARY
                    };
                    push_word(&mut text, value);
                }
                builder
                    .push_verse(verse as u8, &text)
                    .expect("canonical order");
            }
        }
    }
    builder.finish().expect("the whole grid is present")
}

/// Advances the generator and returns the next value.
fn next(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 33) as usize
}
