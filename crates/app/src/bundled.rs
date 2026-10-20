// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The translations compiled into the application binary.
//!
//! Each entry holds the compressed bytes of one file from
//! `data/translations`, pulled in with [`include_bytes!`] so the reader has
//! scripture on a machine that has never installed a translation. The bytes are
//! the files exactly as the data pipeline wrote them, and every display name,
//! abbreviation, and licence comes from the file's own uncompressed header
//! rather than a list maintained here. Only the identifier and the file are
//! named in this module, and [`tests/bundled.rs`] fails if the folder and the
//! table disagree.
//!
//! Nothing is decoded at startup. A translation is decompressed the first time
//! the reader or the command line asks for it, and the result is kept by the
//! library that asked.

use anyhow::{Context, Result};
use bible_core::{Translation, TranslationMeta, format};

/// One translation compiled into the binary.
#[derive(Clone, Copy, Debug)]
pub struct BundledTranslation {
    id: &'static str,
    bytes: &'static [u8],
}

impl BundledTranslation {
    /// Builds an entry from the identifier and the file's bytes.
    ///
    /// The constructor is constant so the table below can be a `const` and the
    /// compiler can inline the embedded bytes.
    const fn new(id: &'static str, bytes: &'static [u8]) -> Self {
        Self { id, bytes }
    }

    /// The stable identifier a caller passes to `--translation`, which is also
    /// the file stem under `data/translations`.
    pub fn id(&self) -> &'static str {
        self.id
    }

    /// The compressed translation file, as committed to the repository.
    pub fn bytes(&self) -> &'static [u8] {
        self.bytes
    }

    /// The metadata from the file's uncompressed header.
    ///
    /// This never decompresses the body, so listing the bundled translations is
    /// as cheap as listing the installed ones. Every bundled file is checked by
    /// the test suite, so a header that will not parse is a build bug rather
    /// than something a user can hit.
    pub fn meta(&self) -> Result<TranslationMeta> {
        format::read_meta(self.bytes)
            .with_context(|| format!("the bundled {} translation has no readable header", self.id))
    }

    /// Decodes the whole translation, which is what reading a verse needs.
    pub fn decode(&self) -> Result<Translation> {
        format::decode(self.bytes)
            .with_context(|| format!("the bundled {} translation does not decode", self.id))
    }
}

/// The identifier the reader opens when nothing else names a translation.
pub const DEFAULT_TRANSLATION_ID: &str = "kjv";

/// The translations the app ships with, in menu order.
///
/// The bytes are compiled in, so a file that disappears from
/// `data/translations` breaks the build rather than the reader. A file added to
/// that folder without an entry here fails the table test. The table is a
/// `static` rather than a `const` so the seven files are embedded once instead
/// of once per use site.
pub static BUNDLED_TRANSLATIONS: [BundledTranslation; 7] = [
    BundledTranslation::new("kjv", include_bytes!("../../../data/translations/kjv.bbl")),
    BundledTranslation::new("web", include_bytes!("../../../data/translations/web.bbl")),
    BundledTranslation::new("bsb", include_bytes!("../../../data/translations/bsb.bbl")),
    BundledTranslation::new("asv", include_bytes!("../../../data/translations/asv.bbl")),
    BundledTranslation::new("ylt", include_bytes!("../../../data/translations/ylt.bbl")),
    BundledTranslation::new(
        "darby",
        include_bytes!("../../../data/translations/darby.bbl"),
    ),
    BundledTranslation::new(
        "webster",
        include_bytes!("../../../data/translations/webster.bbl"),
    ),
];

/// Looks a compiled-in translation up by identifier.
pub fn find(id: &str) -> Option<&'static BundledTranslation> {
    BUNDLED_TRANSLATIONS.iter().find(|entry| entry.id == id)
}

/// The translation the reader opens when the caller names none.
pub fn default_translation() -> &'static BundledTranslation {
    find(DEFAULT_TRANSLATION_ID).expect("the default translation is in the bundled table")
}
