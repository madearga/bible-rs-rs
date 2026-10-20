// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! SWORD module tests. The modules here are built by hand out of synthetic words.

use std::path::{Path, PathBuf};

use bible_core::{SpanStyle, Translation, TranslationMeta, import_sword};

/// A folder under the system temp folder, removed again when the test ends.
struct Scratch {
    /// The folder itself, which is the module root.
    path: PathBuf,
}

impl Scratch {
    /// Creates a fresh module root for one test.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bible-sword-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the scratch folder is created");
        Self { path }
    }

    /// Writes a file under the module root, creating its folders.
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("the folder is created");
        }
        std::fs::write(&path, bytes).expect("the file is written");
        path
    }

    /// The folder a raw text module keeps its testaments in.
    fn raw_text_dir(&self) -> String {
        "modules/texts/rawtext/synthetic".to_string()
    }

    /// Writes a manifest for a raw text module.
    fn manifest(&self, extra: &str) -> PathBuf {
        let conf = format!(
            "[Synthetic]\nDataPath=./{}/\nModDrv=RawText\nVersification=KJV\nLang=en\nDescription=Synthetic Module\n{extra}",
            self.raw_text_dir()
        );
        self.write("mods.d/synthetic.conf", conf.as_bytes())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The records that sit in front of the first verse of a testament.
///
/// A module writes two records for the testament itself, then one for the book, then
/// one for each chapter, so a verse's place in the index is not its ordinal.
const LEADING: [&str; 4] = ["module", "testament", "book", "chapter"];

/// A raw text testament and its verse index, which is six bytes per verse.
fn raw_text(verses: &[&str]) -> (Vec<u8>, Vec<u8>) {
    let mut body = Vec::new();
    let mut index = Vec::new();
    for verse in LEADING.into_iter().chain(verses.iter().copied()) {
        let start = u32::try_from(body.len()).expect("the testament fits");
        index.extend_from_slice(&start.to_le_bytes());
        index.extend_from_slice(
            &(u16::try_from(verse.len()).expect("the verse fits")).to_le_bytes(),
        );
        body.extend_from_slice(verse.as_bytes());
        body.push(b'\n');
    }
    (body, index)
}

/// Writes one raw text testament and its index under the module root.
fn write_raw_text(scratch: &Scratch, directory: &str, testament: &str, verses: &[&str]) {
    let (body, index) = raw_text(verses);
    scratch.write(&format!("{directory}/{testament}"), &body);
    scratch.write(&format!("{directory}/{testament}.vss"), &index);
}

/// Imports a module from a scratch folder.
fn imported(path: &Path) -> Translation {
    import_sword(path, TranslationMeta::new("synthetic", "Synthetic"), None)
        .expect("the module imports")
}

/// The text of a verse, with an empty string when the module printed nothing.
fn verse_text(translation: &Translation, book: u8, chapter: u8, verse: u8) -> String {
    translation
        .verse_at(book, chapter, verse)
        .map(|verse| translation.verse_text(verse).to_owned())
        .expect("the canonical slot exists")
}

#[test]
fn a_raw_text_module_imports_its_verses_in_order() {
    let scratch = Scratch::new("rawtext");
    scratch.manifest("");
    let directory = scratch.raw_text_dir();
    write_raw_text(
        &scratch,
        &directory,
        "ot",
        &["Alpha one.", "Beta two.", "Gamma three."],
    );
    write_raw_text(&scratch, &directory, "nt", &["Delta four."]);

    let translation = imported(&scratch.path);
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one.");
    assert_eq!(verse_text(&translation, 0, 1, 2), "Beta two.");
    assert_eq!(verse_text(&translation, 0, 1, 3), "Gamma three.");
    assert_eq!(verse_text(&translation, 0, 1, 4), "");
    assert_eq!(verse_text(&translation, 39, 1, 1), "Delta four.");
    assert_eq!(translation.meta().language, "en");
}

#[test]
fn a_manifest_file_can_be_the_source() {
    let scratch = Scratch::new("manifest");
    let conf = scratch.manifest("");
    let directory = scratch.raw_text_dir();
    write_raw_text(&scratch, &directory, "ot", &["Alpha one."]);

    let translation = imported(&conf);
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one.");
}

#[test]
fn a_z_text_module_decompresses_its_blocks() {
    let scratch = Scratch::new("ztext");
    let directory = "modules/texts/ztext/synthetic";
    let conf = format!(
        "[Synthetic]\nDataPath=./{directory}/\nModDrv=zText\nBlockType=BOOK\nCompressType=ZIP\nVersification=KJV\n"
    );
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());

    let verses = ["Alpha one.", "Beta two.", "Gamma three."];
    let mut block = String::new();
    let mut records = Vec::new();
    for verse in LEADING.into_iter().chain(verses.iter().copied()) {
        let offset = block.len() as u16;
        block.push_str(verse);
        records.push((offset, verse.len() as u16));
    }

    let compressed = deflate_stored(block.as_bytes());
    let mut index = Vec::new();
    for (offset, length) in records {
        index.extend_from_slice(&0u32.to_le_bytes());
        index.extend_from_slice(&(u32::from(offset)).to_le_bytes());
        index.extend_from_slice(&length.to_le_bytes());
    }
    let mut sizes = Vec::new();
    sizes.extend_from_slice(&0u32.to_le_bytes());
    sizes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    sizes.extend_from_slice(&(block.len() as u32).to_le_bytes());

    scratch.write(&format!("{directory}/ot.bzz"), &compressed);
    scratch.write(&format!("{directory}/ot.bzv"), &index);
    scratch.write(&format!("{directory}/ot.bzs"), &sizes);

    let translation = imported(&scratch.path);
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one.");
    assert_eq!(verse_text(&translation, 0, 1, 2), "Beta two.");
    assert_eq!(verse_text(&translation, 0, 1, 3), "Gamma three.");
}

/// Wraps bytes in a zlib stream of uncompressed deflate blocks.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01, 0x01];
    let length = u16::try_from(data.len()).expect("the block fits");
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(&(!length).to_le_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The Adler-32 checksum a zlib stream ends with.
fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[test]
fn markup_is_stripped_and_its_styles_become_spans() {
    let scratch = Scratch::new("markup");
    scratch.manifest("");
    let directory = scratch.raw_text_dir();
    write_raw_text(
        &scratch,
        &directory,
        "ot",
        &[
            "Alpha <q who=\"Jesus\">spoken words</q> omega.",
            "Beta <note>a note nobody sees</note> gamma.",
            "Delta <transChange>added words</transChange> epsilon.",
        ],
    );

    let translation = imported(&scratch.path);
    assert_eq!(
        verse_text(&translation, 0, 1, 1),
        "Alpha spoken words omega."
    );
    assert_eq!(verse_text(&translation, 0, 1, 2), "Beta gamma.");
    assert_eq!(
        verse_text(&translation, 0, 1, 3),
        "Delta added words epsilon."
    );
    assert!(!translation.text().contains("nobody sees"));

    let verse = translation.verse_at(0, 1, 1).expect("the slot exists");
    let text = translation.verse_text(verse);
    let styles = translation
        .verse_spans(verse)
        .iter()
        .map(|span| (span.style, &text[span.start as usize..span.end as usize]))
        .collect::<Vec<_>>();
    assert_eq!(styles, vec![(SpanStyle::WordsOfChrist, "spoken words")]);
    assert!(translation.meta().red_letters);
}

/// Writes a raw text module that declares its own markup dialect.
fn module_with_markup(scratch: &Scratch, name: &str, dialect: &str, verse: &str) {
    let conf = format!(
        "[{name}]\nDataPath=./modules/texts/rawtext/{name}/\nModDrv=RawText\nVersification=KJV\nMarkup={dialect}\n"
    );
    scratch.write(&format!("mods.d/{name}.conf"), conf.as_bytes());
    write_raw_text(
        scratch,
        &format!("modules/texts/rawtext/{name}"),
        "ot",
        &[verse],
    );
}

#[test]
fn thml_and_gbf_markup_are_stripped_too() {
    let scratch = Scratch::new("dialects");
    module_with_markup(
        &scratch,
        "thml",
        "ThML",
        "Alpha <span class=\"add\">added words</span> <scripRef>a reference</scripRef> omega.",
    );
    module_with_markup(
        &scratch,
        "gbf",
        "GBF",
        "Beta <W>spoken words</W> <FI>a note</FI> gamma.",
    );

    let thml = import_sword(
        &scratch.path.join("mods.d/thml.conf"),
        TranslationMeta::new("thml", "ThML"),
        None,
    )
    .expect("the ThML module imports");
    assert_eq!(verse_text(&thml, 0, 1, 1), "Alpha added words omega.");

    let gbf = import_sword(
        &scratch.path.join("mods.d/gbf.conf"),
        TranslationMeta::new("gbf", "GBF"),
        None,
    )
    .expect("the GBF module imports");
    assert_eq!(verse_text(&gbf, 0, 1, 1), "Beta spoken words gamma.");
}

#[test]
fn a_locked_module_is_named_as_such() {
    let scratch = Scratch::new("locked");
    // A manifest locks a module by leaving its cipher key empty.
    scratch.manifest("CipherKey=\n");
    let directory = scratch.raw_text_dir();
    write_raw_text(&scratch, &directory, "ot", &["Alpha one."]);

    let meta = || TranslationMeta::new("synthetic", "Synthetic");

    let error = import_sword(&scratch.path, meta(), None).expect_err("a locked module needs a key");
    assert!(error.to_string().contains("locked"), "{error}");

    let error = import_sword(&scratch.path, meta(), Some("open sesame"))
        .expect_err("an enciphered module cannot be read");
    assert!(error.to_string().contains("enciphered"), "{error}");
}

#[test]
fn a_raw_text_testament_without_an_index_is_an_error() {
    let scratch = Scratch::new("noindex");
    scratch.manifest("");
    let directory = scratch.raw_text_dir();
    scratch.write(&format!("{directory}/ot"), b"Alpha one.\n");

    let error = import_sword(&scratch.path, TranslationMeta::new("s", "S"), None)
        .expect_err("a testament without its index cannot be read");
    assert!(error.to_string().contains("no verse index"), "{error}");
}

#[test]
fn a_manifest_without_a_driver_is_an_error() {
    let scratch = Scratch::new("nodriver");
    scratch.write("mods.d/synthetic.conf", b"[Synthetic]\nDataPath=x\n");
    assert!(import_sword(&scratch.path, TranslationMeta::new("s", "S"), None).is_err());
}

#[test]
fn a_truncated_verse_index_is_an_error() {
    let scratch = Scratch::new("badindex");
    let directory = "modules/texts/ztext/synthetic";
    let conf = format!("[Synthetic]\nDataPath=./{directory}/\nModDrv=zText\n");
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());
    scratch.write(
        &format!("{directory}/ot.bzz"),
        &deflate_stored(b"Alpha one."),
    );
    scratch.write(&format!("{directory}/ot.bzv"), &[0u8; 7]);
    scratch.write(&format!("{directory}/ot.czs"), &(5u32).to_le_bytes());
    assert!(import_sword(&scratch.path, TranslationMeta::new("s", "S"), None).is_err());
}

#[test]
fn a_module_that_is_not_there_is_an_error() {
    let scratch = Scratch::new("missing");
    assert!(import_sword(&scratch.path, TranslationMeta::new("s", "S"), None).is_err());
}
