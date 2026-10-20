// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reader application: the window, the themes, the render harness, and the CLI.
//!
//! The crate is split three ways. [`theme`] owns every colour and every font the
//! reader paints with, [`app`] owns the window and the reading surface, and
//! [`cli`] owns the command line that scripts the same core crate. The data
//! layer below is what the window and the command line share: the catalog of
//! translations that ship with the app, the installed translations on this
//! machine, and the study file.

#![warn(missing_docs)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bible_core::{Translation, format};
use serde::{Deserialize, Serialize};

pub mod app;
pub mod cli;
pub mod theme;

pub use app::ReaderApp;

/// One translation that ships with the app.
///
/// Only the identity is compiled in. The text is built from the source archives
/// by the data pipeline and lives in `data/translations`, so the repository
/// never carries scripture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundledTranslation {
    /// The stable identifier, also the file stem.
    pub id: &'static str,
    /// The short label shown in the translation list.
    pub abbreviation: &'static str,
    /// The full display name.
    pub name: &'static str,
    /// The language name.
    pub language: &'static str,
    /// The licence the text is distributed under.
    pub licence: &'static str,
}

/// The translations the app is built to ship with, in menu order.
pub const BUNDLED_TRANSLATIONS: [BundledTranslation; 8] = [
    BundledTranslation {
        id: "kjv",
        abbreviation: "KJV",
        name: "King James Version",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "web",
        abbreviation: "WEB",
        name: "World English Bible",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "bsb",
        abbreviation: "BSB",
        name: "Berean Standard Bible",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "asv",
        abbreviation: "ASV",
        name: "American Standard Version",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "ylt",
        abbreviation: "YLT",
        name: "Young's Literal Translation",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "darby",
        abbreviation: "DARBY",
        name: "Darby Bible",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "webster",
        abbreviation: "WEBSTER",
        name: "Webster Bible",
        language: "English",
        licence: "Public domain",
    },
    BundledTranslation {
        id: "sample",
        abbreviation: "SAMPLE",
        name: "Synthetic Sample",
        language: "English",
        licence: "Generated text, not scripture",
    },
];

/// The sample translation, generated from non-scripture words, so the window and
/// the harness have a page to paint before any translation is installed.
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

/// The embedded sample translation, decoded.
///
/// The sample holds generated words only. It exists so the reader has a
/// typographic page to paint on a machine with no translation installed.
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
/// installed translations never decompresses a body. A translation is loaded
/// only when the reader opens it.
#[derive(Clone, Debug, Default)]
pub struct Library {
    dir: PathBuf,
    installed: Vec<InstalledTranslation>,
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
        Self { dir, installed }
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

    /// Loads a translation by identifier.
    ///
    /// With no identifier the first installed translation is used, and with
    /// none installed the embedded sample is used, so the reader always has a
    /// page to paint.
    pub fn load(&self, id: Option<&str>) -> Result<Translation> {
        let path = match id {
            Some(id) => {
                let entry = self
                    .find(id)
                    .with_context(|| format!("no translation called {id} is installed"))?;
                entry.path.clone()
            }
            None => match self.installed.first() {
                Some(entry) => entry.path.clone(),
                None => return sample_translation(),
            },
        };
        let bytes = fs::read(&path)
            .with_context(|| format!("cannot read the translation at {}", path.display()))?;
        format::decode(&bytes)
            .with_context(|| format!("the translation at {} does not decode", path.display()))
    }

    /// Writes a translation's binary form into the library folder.
    ///
    /// The file is named after the translation's own identifier, so installing
    /// the same translation twice replaces it rather than duplicating it.
    pub fn install(&mut self, translation: &Translation) -> Result<PathBuf> {
        let id = translation.meta().id.as_str();
        if id.is_empty() {
            bail!("a translation needs an identifier before it can be installed");
        }
        let folder = translations_dir(&self.dir);
        fs::create_dir_all(&folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
        let path = folder.join(format!("{id}.bbl"));
        let bytes = format::encode(translation)?;
        fs::write(&path, &bytes).with_context(|| format!("cannot write {}", path.display()))?;
        self.installed
            .retain(|entry| entry.meta.id != translation.meta().id);
        self.installed.push(InstalledTranslation {
            meta: translation.meta().clone(),
            path: path.clone(),
        });
        self.installed.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
        Ok(path)
    }
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
