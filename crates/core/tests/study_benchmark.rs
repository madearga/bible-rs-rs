// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The two numbers the study store is built for: how long a read takes, and how
//! long a write takes.
//!
//! Run it with `cargo test --release -- --nocapture store_benchmark` to see the
//! figures this reports.

use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bible_core::Reference;
use bible_core::books;
use bible_core::study::{HighlightColour, Store};

/// The number of verses the read measurement walks through.
const VERSES: usize = 1000;

/// A path in the system temporary folder that is removed when the test ends.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    /// Reserves a database path no other test in this run will pick.
    fn new(test: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bib-core-study-{}-{test}-{unique}.sqlite",
            std::process::id()
        ));
        Self { path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut name = self.path.as_os_str().to_owned();
            name.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(name));
        }
    }
}

/// Collects `VERSES` consecutive verse references in reading order, starting at
/// the first verse of the psalm book so the walk crosses chapters.
fn consecutive_verses() -> Vec<Reference> {
    let mut references = Vec::with_capacity(VERSES);
    let start = books::book(18).expect("the psalter is in the canon");
    'books: for book in books::books().iter().skip(start.index as usize) {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).unwrap_or(0);
            for verse in 1..=count {
                references.push(Reference {
                    book: book.index,
                    chapter,
                    verse: Some(verse as u8),
                });
                if references.len() == VERSES {
                    break 'books;
                }
            }
        }
    }
    assert_eq!(references.len(), VERSES, "the canon has enough verses");
    references
}

#[test]
fn store_benchmark() {
    let db = TempDb::new("bench");
    let mut store = Store::open(&db.path).expect("the store opens");
    let references = consecutive_verses();

    // Give every measured verse a highlight, so the read finds a value and not
    // only an empty slot.
    for reference in &references {
        store
            .set_highlight(*reference, Some(HighlightColour::Yellow))
            .expect("the highlight is written");
    }

    // Warm the mirror's cache lines so the number is the lookup, not the first
    // touch of 1000 separate addresses.
    let mut seen = 0usize;
    for reference in &references {
        if store.highlight(black_box(*reference)).is_some() {
            seen += 1;
        }
    }
    assert_eq!(seen, VERSES, "every measured verse reads back its colour");

    let read_start = Instant::now();
    let mut found = 0usize;
    for reference in &references {
        if store.highlight(black_box(*reference)).is_some() {
            found += 1;
        }
    }
    let read_time = read_start.elapsed();
    black_box(found);
    assert_eq!(found, VERSES);

    let per_read = read_time / u32::try_from(VERSES).expect("the count fits a u32");

    // One warm-up write, then the write that is reported.
    let warm = references[0];
    store
        .set_highlight(warm, Some(HighlightColour::Blue))
        .expect("the warm-up write lands");

    let write_start = Instant::now();
    store
        .set_highlight(warm, Some(HighlightColour::Green))
        .expect("the measured write lands");
    let write_time = write_start.elapsed();

    println!("study store benchmark");
    println!("read: {VERSES} mirrored highlights in {read_time:?}, {per_read:?} per verse");
    println!("write: one highlight to the WAL database in {write_time:?}");

    // The budget is a release-mode number; a debug build runs several times
    // slower without the code being wrong, so only assert it where it was measured.
    #[cfg(not(debug_assertions))]
    assert!(
        per_read < std::time::Duration::from_micros(1),
        "a mirrored read must stay under 1 us per verse, measured {per_read:?}"
    );
    assert_eq!(store.highlight(warm), Some(HighlightColour::Green));

    store.close().expect("the store closes");
}
