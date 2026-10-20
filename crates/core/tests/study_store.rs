// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The store behaves like a durable keyed map over the canonical grid, and like
//! it is already in memory when it is read.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use bible_core::Reference;
use bible_core::study::{Bookmark, HighlightColour, Note, Settings, Store, Theme};

/// A path in the system temporary folder that is removed when the test ends.
///
/// SQLite in WAL mode leaves `-wal` and `-shm` beside the database, so all three
/// are removed together.
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
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    /// The database path itself.
    fn path(&self) -> &Path {
        &self.path
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

/// A verse reference in the middle of the canon, so the test is not reading slot 0.
fn sample_verse() -> Reference {
    Reference::parse("John 3:16").expect("the reference resolves")
}

/// A second verse, used where a test needs two records side by side.
fn other_verse() -> Reference {
    Reference::parse("Ps 23:1").expect("the reference resolves")
}

#[test]
fn a_file_backed_store_has_one_writer_for_its_lifetime() {
    let db = TempDb::new("lifetime-lock");
    let first = Store::open(db.path()).expect("the first store opens");

    let error = Store::open(db.path())
        .err()
        .expect("a second store is rejected while the first is alive");
    assert!(error.to_string().contains("busy"), "{error:#}");

    first.close().expect("the first store closes");
    Store::open(db.path()).expect("the lock is released on close");
}

#[test]
fn store_opens_in_wal_mode() {
    let db = TempDb::new("wal");
    let store = Store::open(db.path()).expect("the store opens");
    assert_eq!(store.journal_mode().expect("the mode is readable"), "wal");
}

#[test]
fn a_highlight_can_be_set_changed_and_cleared() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let verse = sample_verse();

    assert_eq!(store.highlight(verse), None, "a fresh verse has no colour");

    store
        .set_highlight(verse, Some(HighlightColour::Yellow))
        .expect("the highlight is written");
    assert_eq!(store.highlight(verse), Some(HighlightColour::Yellow));

    store
        .set_highlight(verse, Some(HighlightColour::Blue))
        .expect("the highlight is rewritten");
    assert_eq!(store.highlight(verse), Some(HighlightColour::Blue));

    store
        .set_highlight(verse, None)
        .expect("the highlight is cleared");
    assert_eq!(store.highlight(verse), None);

    // The other five colours are all addressable.
    for colour in HighlightColour::ALL {
        store
            .set_highlight(verse, Some(colour))
            .expect("every colour is writable");
        assert_eq!(store.highlight(verse), Some(colour));
    }
}

#[test]
fn a_write_is_readable_without_a_flush() {
    let db = TempDb::new("mirror");
    let mut store = Store::open(db.path()).expect("the store opens");
    let verse = other_verse();

    store
        .set_highlight(verse, Some(HighlightColour::Green))
        .expect("the highlight is written");
    // Deliberately no flush, no close, no reopen: the mirror is the read path.
    assert_eq!(store.highlight(verse), Some(HighlightColour::Green));

    store
        .set_note(verse, "a synthetic note")
        .expect("the note is written");
    assert_eq!(store.note(verse), Some("a synthetic note"));
}

#[test]
fn a_note_can_be_set_and_an_empty_note_removes_it() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let verse = sample_verse();

    assert_eq!(store.note(verse), None);
    store
        .set_note(verse, "remember this verse")
        .expect("the note is written");
    assert_eq!(store.note(verse), Some("remember this verse"));
    assert_eq!(store.snapshot().notes.len(), 1);

    // Blank is the same as cleared, so a whitespace-only note does not linger.
    store.set_note(verse, "   ").expect("the note is cleared");
    assert_eq!(store.note(verse), None);
    assert!(store.snapshot().notes.is_empty());
}

#[test]
fn a_bookmark_can_be_set_read_and_toggled() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let verse = sample_verse();

    assert_eq!(store.bookmark(verse), None);
    assert!(
        store.toggle_bookmark(verse).expect("the toggle writes"),
        "toggling an unmarked verse marks it"
    );
    assert_eq!(store.bookmark(verse), Some(&Bookmark::unlabelled()));

    assert!(
        !store.toggle_bookmark(verse).expect("the toggle writes"),
        "toggling a marked verse unmarks it"
    );
    assert_eq!(store.bookmark(verse), None);

    let labelled = Bookmark::with_label("the verse to come back to");
    store
        .set_bookmark(verse, Some(labelled.clone()))
        .expect("the bookmark is written");
    assert_eq!(store.bookmark(verse), Some(&labelled));
    assert_eq!(
        store.bookmark(verse).and_then(|b| b.label.as_deref()),
        Some("the verse to come back to")
    );

    store
        .set_bookmark(verse, None)
        .expect("the bookmark is removed");
    assert_eq!(store.bookmark(verse), None);
}

#[test]
fn the_study_data_survives_a_close_and_a_reopen() {
    let db = TempDb::new("reopen");
    let verse = sample_verse();

    {
        let mut store = Store::open(db.path()).expect("the store opens");
        store
            .set_highlight(verse, Some(HighlightColour::Pink))
            .expect("the highlight is written");
        store
            .set_note(verse, "still here after a restart")
            .expect("the note is written");
        store
            .set_bookmark(verse, Some(Bookmark::with_label("keep")))
            .expect("the bookmark is written");
        store.close().expect("the store closes");
    }

    let store = Store::open(db.path()).expect("the store reopens");
    assert_eq!(store.highlight(verse), Some(HighlightColour::Pink));
    assert_eq!(store.note(verse), Some("still here after a restart"));
    assert_eq!(
        store.bookmark(verse).and_then(|b| b.label.as_deref()),
        Some("keep")
    );
}

#[test]
fn the_reading_position_is_written_and_read_back() {
    let db = TempDb::new("position");
    let position = Reference::parse("Rom 8:28").expect("the reference resolves");

    {
        let mut store = Store::open(db.path()).expect("the store opens");
        let mut settings = store.settings().clone();
        settings.position = position;
        settings.theme = Theme::Sepia;
        store.set_settings(settings).expect("the settings are set");

        // The mirror already holds the new settings, before any write is due.
        assert_eq!(store.settings().position, position);
        assert_eq!(store.settings().theme, Theme::Sepia);

        store.flush().expect("the settings are flushed");
        store.close().expect("the store closes");
    }

    let store = Store::open(db.path()).expect("the store reopens");
    assert_eq!(store.settings().position, position);
    assert_eq!(store.settings().theme, Theme::Sepia);
}

#[test]
fn settings_writes_are_throttled_but_read_at_once() {
    let db = TempDb::new("throttle");
    let first = Reference::parse("Gen 2:1").expect("the reference resolves");
    let second = Reference::parse("Isa 55:1").expect("the reference resolves");

    {
        let mut store = Store::open(db.path()).expect("the store opens");

        let mut settings = store.settings().clone();
        settings.position = first;
        store.set_settings(settings).expect("the settings are set");

        // Every read sees the newest value even though nothing was written.
        let mut settings = store.settings().clone();
        settings.position = second;
        store.set_settings(settings).expect("the settings are set");
        assert_eq!(store.settings().position, second);
    }

    // Dropped without a flush, so the throttled write never happened and the
    // reopened store still holds the defaults.
    let store = Store::open(db.path()).expect("the store reopens");
    assert_eq!(store.settings().position, Settings::default().position);
}

#[test]
fn a_chapter_reference_is_not_study_data() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let chapter = Reference::parse("Rom 8").expect("the reference resolves");

    assert_eq!(store.highlight(chapter), None);
    assert_eq!(store.note(chapter), None);
    assert!(
        store
            .set_highlight(chapter, Some(HighlightColour::Green))
            .is_err()
    );
    assert!(store.set_note(chapter, "nowhere to hang this").is_err());
    assert!(store.toggle_bookmark(chapter).is_err());
}

#[test]
fn a_note_round_trips_through_a_snapshot() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let verse = other_verse();
    store
        .set_note(verse, "first line\nsecond line")
        .expect("the note is written");
    store
        .set_highlight(verse, Some(HighlightColour::Purple))
        .expect("the highlight is written");

    let data = store.snapshot();
    assert_eq!(
        data.notes,
        vec![(verse, Note::new("first line\nsecond line"))]
    );
    assert_eq!(data.highlights, vec![(verse, HighlightColour::Purple)]);
}
