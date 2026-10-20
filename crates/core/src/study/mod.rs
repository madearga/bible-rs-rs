// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The study store: highlights, notes, bookmarks, and settings.
//!
//! The store is one SQLite file in WAL mode, which keeps a crash from losing a
//! write that already returned. Every record is keyed by the global verse index
//! of the canonical grid, so the data is translation independent: a highlight
//! made in one translation shows in all of them.
//!
//! The reader asks for the study state of every verse on screen on every frame,
//! so the store keeps a second copy of the data in memory, addressed by that same
//! verse index. A read is an array lookup in the mirror and never touches the
//! database. A write goes to the database first, then to the mirror, so the disk
//! and the mirror cannot disagree after a failed write: if SQLite rejects the
//! write, the call returns the error and the mirror still holds the old value.
//!
//! Settings are the exception, because they carry the reading position and that
//! changes many times a second while scrolling. [`Store::set_settings`] always
//! updates the mirror and writes the file at most once every 1.5 seconds, so
//! scrolling never waits on the disk. [`Store::flush`] writes a pending change
//! on demand, and [`Store::close`] does it on the way out.

mod export;
pub mod record;

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};

use crate::Reference;
use crate::books;

pub use record::{Bookmark, HighlightColour, Note, Settings, StudyData, Theme};

/// How long the store waits between two writes of the settings row.
///
/// A second and a half is slow enough that a scroll storm coalesces into one
/// write and fast enough that a quit right after a move loses nothing that
/// matters.
pub const SETTINGS_WRITE_INTERVAL: Duration = Duration::from_millis(1500);

/// The number of verse slots the mirror covers, one per verse in the canon.
const MIRROR_SLOTS: usize = books::VERSE_COUNT;

/// The in-memory copy of the study data, indexed by global verse index.
struct Mirror {
    /// The colour of each verse, or `None` where the verse has no highlight.
    highlights: Vec<Option<HighlightColour>>,
    /// The note on each verse, or `None` where there is none.
    notes: Vec<Option<Note>>,
    /// The bookmark on each verse, or `None` where there is none.
    bookmarks: Vec<Option<Bookmark>>,
}

impl Mirror {
    /// An empty mirror with a slot for every verse in the canon.
    fn empty() -> Self {
        Self {
            highlights: vec![None; MIRROR_SLOTS],
            notes: (0..MIRROR_SLOTS).map(|_| None).collect(),
            bookmarks: (0..MIRROR_SLOTS).map(|_| None).collect(),
        }
    }

    /// Rebuilds the mirror from a snapshot, refusing a reference off the grid.
    fn from_data(data: &StudyData) -> Result<Self> {
        let mut mirror = Self::empty();
        for (reference, colour) in &data.highlights {
            mirror.highlights[verse_index(*reference)?] = Some(*colour);
        }
        for (reference, note) in &data.notes {
            mirror.notes[verse_index(*reference)?] = Some(note.clone());
        }
        for (reference, bookmark) in &data.bookmarks {
            mirror.bookmarks[verse_index(*reference)?] = Some(bookmark.clone());
        }
        Ok(mirror)
    }
}

/// The study data of one reader, backed by one SQLite file and mirrored in memory.
pub struct Store {
    /// The database connection. Every write commits before the mirror changes.
    connection: Connection,
    /// The exclusive lifetime lock for a file-backed store.
    _lock: Option<File>,
    /// The in-memory copy every read is served from.
    mirror: Mirror,
    /// The settings in force, which may be newer than the stored row.
    settings: Settings,
    /// When the settings row was last written, for the write throttle.
    settings_written_at: Instant,
    /// Whether the settings in the mirror are newer than the stored row.
    settings_pending: bool,
}

impl Store {
    /// Opens the store at `path`, which is created along with its schema when
    /// the file is not there yet.
    ///
    /// The database is switched to WAL mode so a write does not block a reader,
    /// and the whole of it is read into the mirror so nothing the reader shows
    /// has to wait on a query.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let lock_path = lock_path(path);
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| {
                format!(
                    "could not open the study store lock at {}",
                    lock_path.display()
                )
            })?;
        lock.try_lock().with_context(|| {
            format!(
                "the study store at {} is busy; close the other Bible window or command and try again",
                path.display()
            )
        })?;
        let connection = Connection::open(path)
            .with_context(|| format!("could not open the study store at {}", path.display()))?;
        Self::from_connection(connection, Some(lock))
    }

    /// Opens a store that lives only as long as the handle, for tests and for a
    /// preview that must not touch the reader's real data.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?, None)
    }

    /// Sets up the schema and the mirror on an open connection.
    fn from_connection(connection: Connection, lock: Option<File>) -> Result<Self> {
        // WAL is a property of the file, not of the connection, but it is
        // idempotent and cheap to reassert on every open.
        let _mode: String = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .context("could not switch the study store to WAL mode")?;
        connection
            .execute_batch(
                "PRAGMA synchronous = NORMAL;
                 CREATE TABLE IF NOT EXISTS highlights (
                     verse  INTEGER PRIMARY KEY,
                     colour INTEGER NOT NULL
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS notes (
                     verse INTEGER PRIMARY KEY,
                     body  TEXT NOT NULL
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS bookmarks (
                     verse INTEGER PRIMARY KEY,
                     label TEXT
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS settings (
                     id   INTEGER PRIMARY KEY CHECK (id = 1),
                     json TEXT NOT NULL
                 ) STRICT;",
            )
            .context("could not create the study store schema")?;

        let mirror = Self::load_mirror(&connection)?;
        let settings = Self::load_settings(&connection)?;

        Ok(Self {
            connection,
            _lock: lock,
            mirror,
            settings,
            settings_written_at: Instant::now(),
            settings_pending: false,
        })
    }

    /// Reads every record out of the database into a fresh mirror.
    fn load_mirror(connection: &Connection) -> Result<Mirror> {
        let mut mirror = Mirror::empty();

        let mut statement = connection.prepare("SELECT verse, colour FROM highlights")?;
        let rows =
            statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, u8>(1)?)))?;
        for row in rows {
            let (verse, code) = row?;
            let index = slot(verse)?;
            let colour = HighlightColour::from_code(code)
                .ok_or_else(|| anyhow!("the study store holds unknown highlight colour {code}"))?;
            mirror.highlights[index] = Some(colour);
        }

        let mut statement = connection.prepare("SELECT verse, body FROM notes")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (verse, body) = row?;
            mirror.notes[slot(verse)?] = Some(Note::new(body));
        }

        let mut statement = connection.prepare("SELECT verse, label FROM bookmarks")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        for row in rows {
            let (verse, label) = row?;
            mirror.bookmarks[slot(verse)?] = Some(Bookmark { label });
        }

        Ok(mirror)
    }

    /// Reads the settings row, falling back to the defaults when the store has
    /// never written one or when the row is a shape this build cannot read.
    fn load_settings(connection: &Connection) -> Result<Settings> {
        let json: String =
            match connection.query_row("SELECT json FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            }) {
                Ok(json) => json,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Settings::default()),
                Err(error) => return Err(error.into()),
            };
        // Settings are a preference, not study data: a row from a future build
        // is replaced rather than blocking the reader from opening at all.
        Ok(export::settings_from_json(&json).unwrap_or_default())
    }

    /// Reports the journal mode SQLite is using, which is `wal` for a store on
    /// disk and `memory` for one that lives only in this process.
    pub fn journal_mode(&self) -> Result<String> {
        Ok(self
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
    }

    /// The highlight colour of a verse, or `None` when it has none.
    ///
    /// This is the call the reader makes for every verse on screen, so it reads
    /// the mirror and never runs a query.
    pub fn highlight(&self, reference: Reference) -> Option<HighlightColour> {
        let index = verse_index(reference).ok()?;
        self.mirror.highlights[index]
    }

    /// Sets a verse's highlight colour, or clears it when `colour` is `None`.
    pub fn set_highlight(
        &mut self,
        reference: Reference,
        colour: Option<HighlightColour>,
    ) -> Result<()> {
        let index = verse_index(reference)?;
        let verse = index as i64;
        match colour {
            Some(colour) => {
                self.connection.execute(
                    "INSERT INTO highlights (verse, colour) VALUES (?1, ?2)
                     ON CONFLICT(verse) DO UPDATE SET colour = excluded.colour",
                    params![verse, colour.code()],
                )?;
            }
            None => {
                self.connection
                    .execute("DELETE FROM highlights WHERE verse = ?1", params![verse])?;
            }
        }
        self.mirror.highlights[index] = colour;
        Ok(())
    }

    /// The body of the note on a verse, or `None` when it has no note.
    pub fn note(&self, reference: Reference) -> Option<&str> {
        let index = verse_index(reference).ok()?;
        self.mirror.notes[index]
            .as_ref()
            .map(|note| note.body.as_str())
    }

    /// Sets the note on a verse. A blank body removes the note instead of storing
    /// a note with nothing in it.
    pub fn set_note(&mut self, reference: Reference, body: &str) -> Result<()> {
        let index = verse_index(reference)?;
        let verse = index as i64;
        if body.trim().is_empty() {
            self.connection
                .execute("DELETE FROM notes WHERE verse = ?1", params![verse])?;
            self.mirror.notes[index] = None;
        } else {
            self.connection.execute(
                "INSERT INTO notes (verse, body) VALUES (?1, ?2)
                 ON CONFLICT(verse) DO UPDATE SET body = excluded.body",
                params![verse, body],
            )?;
            self.mirror.notes[index] = Some(Note::new(body));
        }
        Ok(())
    }

    /// The bookmark on a verse, label and all, or `None` when it is not marked.
    pub fn bookmark(&self, reference: Reference) -> Option<&Bookmark> {
        let index = verse_index(reference).ok()?;
        self.mirror.bookmarks[index].as_ref()
    }

    /// Puts a bookmark on a verse, replacing any earlier one, or removes it when
    /// `bookmark` is `None`.
    pub fn set_bookmark(&mut self, reference: Reference, bookmark: Option<Bookmark>) -> Result<()> {
        let index = verse_index(reference)?;
        let verse = index as i64;
        match &bookmark {
            Some(bookmark) => {
                self.connection.execute(
                    "INSERT INTO bookmarks (verse, label) VALUES (?1, ?2)
                     ON CONFLICT(verse) DO UPDATE SET label = excluded.label",
                    params![verse, bookmark.label],
                )?;
            }
            None => {
                self.connection
                    .execute("DELETE FROM bookmarks WHERE verse = ?1", params![verse])?;
            }
        }
        self.mirror.bookmarks[index] = bookmark;
        Ok(())
    }

    /// Marks a verse if it is unmarked and unmarks it if it is marked, returning
    /// whether the verse is bookmarked afterwards.
    pub fn toggle_bookmark(&mut self, reference: Reference) -> Result<bool> {
        if self.bookmark(reference).is_some() {
            self.set_bookmark(reference, None)?;
            Ok(false)
        } else {
            self.set_bookmark(reference, Some(Bookmark::unlabelled()))?;
            Ok(true)
        }
    }

    /// The settings currently in force, which include where the reader was.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Replaces the settings, writing them to the file at most every
    /// [`SETTINGS_WRITE_INTERVAL`].
    ///
    /// The mirror takes the new settings at once, so a read straight afterwards
    /// sees them. A write that is due goes to the database now; one that is not
    /// stays pending until the next interval elapses or [`Store::flush`] is
    /// called. A failed write keeps the change pending and returns the error.
    pub fn set_settings(&mut self, settings: Settings) -> Result<()> {
        self.settings = settings;
        self.settings_pending = true;
        if self.settings_written_at.elapsed() >= SETTINGS_WRITE_INTERVAL {
            self.write_settings()?;
        }
        Ok(())
    }

    /// Writes the settings to the file if a change is still pending.
    pub fn flush(&mut self) -> Result<()> {
        if self.settings_pending {
            self.write_settings()?;
        }
        Ok(())
    }

    /// Writes the settings row and clears the pending flag, leaving the flag set
    /// when SQLite refuses so the next flush tries again.
    fn write_settings(&mut self) -> Result<()> {
        let json = export::settings_to_json(&self.settings)?;
        self.connection.execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            params![json],
        )?;
        self.settings_pending = false;
        self.settings_written_at = Instant::now();
        Ok(())
    }

    /// Takes everything the store holds at this instant, in canonical order.
    pub fn snapshot(&self) -> StudyData {
        let mut data = StudyData {
            highlights: Vec::new(),
            notes: Vec::new(),
            bookmarks: Vec::new(),
            settings: self.settings.clone(),
        };
        for (index, reference) in verse_references().enumerate() {
            if let Some(colour) = self.mirror.highlights[index] {
                data.highlights.push((reference, colour));
            }
            if let Some(note) = &self.mirror.notes[index] {
                data.notes.push((reference, note.clone()));
            }
            if let Some(bookmark) = &self.mirror.bookmarks[index] {
                data.bookmarks.push((reference, bookmark.clone()));
            }
        }
        data
    }

    /// Writes the study data as JSON, the archive format that
    /// [`Store::import_json`] reads back.
    pub fn export_json(&self) -> Result<String> {
        export::to_json(&self.snapshot())
    }

    /// Replaces the whole study data with a JSON export.
    ///
    /// The document is parsed and checked before the database is touched, and
    /// the replacement is one transaction, so a malformed file leaves the store
    /// exactly as it was.
    pub fn import_json(&mut self, json: &str) -> Result<()> {
        let data = export::from_json(json)?;
        let mirror = Mirror::from_data(&data)?;

        let transaction = self.connection.transaction()?;
        transaction.execute("DELETE FROM highlights", [])?;
        transaction.execute("DELETE FROM notes", [])?;
        transaction.execute("DELETE FROM bookmarks", [])?;
        for (reference, colour) in &data.highlights {
            transaction.execute(
                "INSERT INTO highlights (verse, colour) VALUES (?1, ?2)",
                params![verse_index(*reference)? as i64, colour.code()],
            )?;
        }
        for (reference, note) in &data.notes {
            transaction.execute(
                "INSERT INTO notes (verse, body) VALUES (?1, ?2)",
                params![verse_index(*reference)? as i64, note.body],
            )?;
        }
        for (reference, bookmark) in &data.bookmarks {
            transaction.execute(
                "INSERT INTO bookmarks (verse, label) VALUES (?1, ?2)",
                params![verse_index(*reference)? as i64, bookmark.label],
            )?;
        }
        let settings_json = export::settings_to_json(&data.settings)?;
        transaction.execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json",
            params![settings_json],
        )?;
        transaction.commit()?;

        self.mirror = mirror;
        self.settings = data.settings;
        self.settings_pending = false;
        self.settings_written_at = Instant::now();
        Ok(())
    }

    /// Writes the study data as Markdown, for reading or for pasting elsewhere.
    pub fn export_markdown(&self) -> String {
        export::to_markdown(&self.snapshot())
    }

    /// Closes the store, writing any settings change that was still waiting.
    pub fn close(mut self) -> Result<()> {
        self.flush()
    }
}

/// Returns the separate file used to lock a file-backed store for its lifetime.
fn lock_path(database: &Path) -> PathBuf {
    let mut path = database.as_os_str().to_owned();
    path.push(".lock");
    PathBuf::from(path)
}

/// Turns a reference into its global verse index, which is the key of every
/// record in the store and every slot in the mirror.
fn verse_index(reference: Reference) -> Result<usize> {
    let verse = reference
        .verse
        .ok_or_else(|| anyhow!("{reference} names no verse"))?;
    let offset = books::verse_offset(reference.book, reference.chapter, verse)
        .ok_or_else(|| anyhow!("{reference} is outside the canonical grid"))?;
    Ok(usize::from(offset))
}

/// Checks a verse index read from the database before it is used as a slot.
fn slot(verse: i64) -> Result<usize> {
    let index = usize::try_from(verse)
        .map_err(|_| anyhow!("the study store holds verse index {verse}, which is negative"))?;
    if index >= MIRROR_SLOTS {
        return Err(anyhow!(
            "the study store holds verse index {verse}, past the last verse of the canon"
        ));
    }
    Ok(index)
}

/// Walks every verse of the canon in reading order.
fn verse_references() -> impl Iterator<Item = Reference> {
    books::books().iter().flat_map(|book| {
        (1..=book.chapter_count).flat_map(move |chapter| {
            let count = books::verses_in_chapter(book.index, chapter).unwrap_or(0);
            (1..=count).map(move |verse| Reference {
                book: book.index,
                chapter,
                verse: Some(verse as u8),
            })
        })
    })
}
