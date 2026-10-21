// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reader application: the window, the themes, the render harness, and the CLI.
//!
//! The crate is split three ways. [`theme`] owns every colour and every font the
//! reader paints with, [`app`] owns the window and the reading surface, and
//! [`cli`] owns the command line that scripts the same core crate. The data
//! layer below is what the window and the command line share: the translations
//! compiled into the binary, the installed translations on this machine, and the
//! study file.

#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use bible_core::{Translation, format};
use serde::{Deserialize, Serialize};

pub mod app;
pub mod book_picker;
pub mod bundled;
pub mod cli;
pub mod history;
#[cfg(target_os = "macos")]
pub mod macos_menu;
pub mod palette;
pub mod parallel;
pub mod reader;
pub mod search_panel;
pub mod selection;
pub mod settings;
pub mod shortcuts;
pub mod sidebar;
pub mod theme;
pub mod typography;
pub mod verse_menu;

#[cfg(test)]
mod palette_test;
#[cfg(test)]
mod reader_tests;

pub use app::ReaderApp;
pub use bundled::{
    BUNDLED_TRANSLATIONS, BundledTranslation, DEFAULT_TRANSLATION_ID, default_translation,
};

/// The identifier that selects the synthetic sample rather than scripture.
///
/// The sample is generated words only. It exists so the render harness has a
/// page to paint that no translation produces, which is how a shot of real text
/// is proven to differ from a shot of the placeholder.
pub const SAMPLE_TRANSLATION_ID: &str = "sample";

/// The sample translation, generated from non-scripture words, so the harness
/// has a page that is visibly not scripture.
const SAMPLE_BYTES: &[u8] = include_bytes!("../../../assets/sample/passage.bbl");

/// The folder the app reads translations and writes study data to.
///
/// `BIBLE_DATA_DIR` overrides the platform location so a test can point the app
/// at a scratch folder.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIBLE_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home {
            return home.join("Library/Application Support/BibleReader");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(dir).join("bible-reader");
        }
        if let Some(home) = home {
            return home.join(".local/share/bible-reader");
        }
    }
    PathBuf::from(".bible-reader")
}

/// The folder translated files are installed into.
pub fn translations_dir(dir: &Path) -> PathBuf {
    dir.join("translations")
}

/// The longest translation identifier the installer accepts.
///
/// The identifier becomes a file name, so it is deliberately short. The bound
/// keeps a hostile header from producing a name no file system will take.
pub const MAX_TRANSLATION_ID: usize = 64;

/// Checks that a translation identifier is safe to use as a file name.
///
/// An installed file is named after the identifier in its own header, so a
/// hostile file must not be able to choose a path. Only ASCII letters, digits,
/// underscore, and hyphen are accepted, the identifier may not be empty, and it
/// may not be longer than [`MAX_TRANSLATION_ID`] bytes. Callers reject the file
/// with the returned message rather than repairing the identifier.
pub fn check_translation_id(id: &str) -> Result<()> {
    if id.is_empty() {
        bail!("a translation needs an identifier before it can be installed");
    }
    if id.len() > MAX_TRANSLATION_ID {
        bail!(
            "the translation identifier is {} bytes, the limit is {MAX_TRANSLATION_ID}",
            id.len()
        );
    }
    if let Some(byte) = id
        .bytes()
        .find(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-'))
    {
        bail!(
            "the translation identifier {id:?} contains {byte:?}, only ASCII letters, digits, underscore, and hyphen are allowed"
        );
    }
    Ok(())
}

/// The embedded sample translation, decoded.
///
/// The sample holds generated words only. It exists so the render harness can
/// paint a page that is not scripture and prove a real translation reaches the
/// screen.
pub fn sample_translation() -> Result<Translation> {
    format::decode(SAMPLE_BYTES).context("the embedded sample translation does not decode")
}

/// One translation installed on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledTranslation {
    /// The identity read from the file's uncompressed header.
    pub meta: bible_core::TranslationMeta,
    /// The binary file itself.
    pub path: PathBuf,
}

/// The translations this machine can read.
///
/// Discovery reads the uncompressed header of every `.bbl` file, so listing the
/// translations never decompresses a body. The seven compiled-in translations
/// are always available and an installed file of the same identifier takes
/// precedence over them. A translation is decoded the first time it is asked
/// for and kept in memory afterwards, so a switch back to one already read is
/// instant.
#[derive(Debug, Default)]
pub struct Library {
    dir: PathBuf,
    installed: Vec<InstalledTranslation>,
    loaded: Mutex<BTreeMap<String, Arc<Translation>>>,
}

impl Library {
    /// Reads the translations installed in a given folder.
    pub fn discover_in(dir: PathBuf) -> Self {
        let mut installed = Vec::new();
        if let Ok(entries) = fs::read_dir(translations_dir(&dir)) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|extension| extension != "bbl") {
                    continue;
                }
                let Ok(bytes) = fs::read(&path) else {
                    continue;
                };
                let Ok(meta) = format::read_meta(&bytes) else {
                    continue;
                };
                installed.push(InstalledTranslation { meta, path });
            }
        }
        installed.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
        Self {
            dir,
            installed,
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    /// Reads the translations installed in the folder [`data_dir`] names.
    pub fn discover() -> Self {
        Self::discover_in(data_dir())
    }

    /// The data folder this library reads.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every installed translation, sorted by identifier.
    pub fn installed(&self) -> &[InstalledTranslation] {
        &self.installed
    }

    /// Looks an installed translation up by identifier.
    pub fn find(&self, id: &str) -> Option<&InstalledTranslation> {
        self.installed.iter().find(|entry| entry.meta.id == id)
    }

    /// Loads a translation by identifier, decoding it the first time it is
    /// asked for.
    ///
    /// With no identifier the default bundled translation is used, and the
    /// sample is used only when a caller asks for it by its own identifier. An
    /// installed file wins over a bundled one with the same identifier, so a
    /// translation the user installed replaces the compiled-in copy.
    pub fn load(&self, id: Option<&str>) -> Result<Arc<Translation>> {
        let id = id.unwrap_or(DEFAULT_TRANSLATION_ID);
        if let Some(cached) = self
            .loaded
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(id)
            .cloned()
        {
            return Ok(cached);
        }
        let translation = Arc::new(self.decode(id)?);
        let mut loaded = self
            .loaded
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Ok(loaded
            .entry(id.to_owned())
            .or_insert_with(|| Arc::clone(&translation))
            .clone())
    }

    /// Decodes one translation without consulting the cache.
    fn decode(&self, id: &str) -> Result<Translation> {
        if id == SAMPLE_TRANSLATION_ID {
            return sample_translation();
        }
        if let Some(entry) = self.find(id) {
            let bytes = fs::read(&entry.path).with_context(|| {
                format!("cannot read the translation at {}", entry.path.display())
            })?;
            return format::decode(&bytes).with_context(|| {
                format!(
                    "the translation at {} does not decode",
                    entry.path.display()
                )
            });
        }
        if let Some(entry) = bundled::find(id) {
            return entry.decode();
        }
        bail!("no translation called {id} is bundled or installed")
    }

    /// Writes a translation's binary form into the library folder.
    ///
    /// The file is named after the translation's own identifier, so installing
    /// the same translation twice replaces it rather than duplicating it. The
    /// identifier is checked first, because it becomes a file name. The bytes
    /// are written through a sibling temporary file and renamed into place, so
    /// an existing symlink or hard link at the destination is replaced rather
    /// than followed.
    pub fn install(&mut self, translation: &Translation) -> Result<PathBuf> {
        let id = translation.meta().id.as_str();
        check_translation_id(id)?;
        let folder = translations_dir(&self.dir);
        fs::create_dir_all(&folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
        check_folder_containment(&self.dir, &folder)?;
        let path = folder.join(format!("{id}.bbl"));
        let bytes = format::encode(translation)?;
        write_installed(&path, &bytes)?;
        self.installed
            .retain(|entry| entry.meta.id != translation.meta().id);
        self.loaded
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(id);
        self.installed.push(InstalledTranslation {
            meta: translation.meta().clone(),
            path: path.clone(),
        });
        self.installed.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
        Ok(path)
    }
}

/// Verifies that the translations folder is really inside the data folder.
///
/// The folder is created before this check, so a link that was planted there
/// resolves to its target and is caught. Without it a `translations` symlink
/// would send every installed file outside the data folder.
fn check_folder_containment(dir: &Path, folder: &Path) -> Result<()> {
    let dir = fs::canonicalize(dir).with_context(|| format!("cannot resolve {}", dir.display()))?;
    let folder =
        fs::canonicalize(folder).with_context(|| format!("cannot resolve {}", folder.display()))?;
    if !folder.starts_with(&dir) {
        bail!(
            "the translations folder {} is outside the data folder {}",
            folder.display(),
            dir.display()
        );
    }
    Ok(())
}

/// Writes installed bytes through a new file of our own and renames them into
/// place.
///
/// Exclusive creation means the temporary name is never a file this call did
/// not make, and the rename replaces the destination entry itself, so a symlink
/// or hard link already sitting at the destination is removed rather than
/// followed. A temporary file is removed when the write fails.
fn write_installed(path: &Path, bytes: &[u8]) -> Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("{} has no file name", path.display()))?;
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut temporary_name = OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".tmp-{}-{unique}", std::process::id()));
    let temporary = parent.join(temporary_name);

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("cannot create {}", temporary.display()))?;
    let write = file
        .write_all(bytes)
        .with_context(|| format!("cannot write {}", temporary.display()))
        .and_then(|()| {
            file.sync_all()
                .with_context(|| format!("cannot flush {}", temporary.display()))
        });
    drop(file);

    let result = write.and_then(|()| {
        fs::rename(&temporary, path).with_context(|| format!("cannot write {}", path.display()))
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// One highlight in the study data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Highlight {
    /// The verse the highlight belongs to, written as the reader shows it.
    pub reference: String,
    /// Which of the six highlight colours is used, one to six.
    pub colour: u8,
}

/// One note in the study data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// The verse the note is attached to.
    pub reference: String,
    /// The note itself.
    pub body: String,
}

/// One bookmark in the study data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    /// The verse the bookmark points at.
    pub reference: String,
    /// An optional label shown beside the bookmark.
    #[serde(default)]
    pub label: Option<String>,
}

/// Everything the reader keeps about a text, keyed by canonical reference rather
/// than by translation so it follows the reader across translations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StudyData {
    /// Highlighted verses.
    pub highlights: Vec<Highlight>,
    /// Notes attached to verses.
    pub notes: Vec<Note>,
    /// Bookmarked verses.
    pub bookmarks: Vec<Bookmark>,
}

impl StudyData {
    /// Reads study data from a JSON file.
    ///
    /// A missing file reads as empty study data rather than as an error, so a
    /// fresh install starts with nothing and no complaint.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("{} is not a study file", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    /// Writes study data to a JSON file, creating the folder it lives in.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).context("cannot serialise the study data")?;
        fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
    }

    /// Writes the study data as Markdown, one section per kind, for export.
    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Study\n");
        if !self.highlights.is_empty() {
            out.push_str("\n## Highlights\n");
            for highlight in &self.highlights {
                out.push_str(&format!(
                    "\n- {} (colour {})\n",
                    highlight.reference, highlight.colour
                ));
            }
        }
        if !self.notes.is_empty() {
            out.push_str("\n## Notes\n");
            for note in &self.notes {
                out.push_str(&format!("\n### {}\n\n{}\n", note.reference, note.body));
            }
        }
        if !self.bookmarks.is_empty() {
            out.push_str("\n## Bookmarks\n");
            for bookmark in &self.bookmarks {
                match &bookmark.label {
                    Some(label) => out.push_str(&format!("\n- {} {}\n", bookmark.reference, label)),
                    None => out.push_str(&format!("\n- {}\n", bookmark.reference)),
                }
            }
        }
        out
    }
}
