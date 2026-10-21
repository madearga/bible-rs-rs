// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Import security tests for the core crate. Each test builds a hostile file by
//! hand and asserts the importer refuses it instead of allocating what the file
//! asks for. Every fixture lives in a scratch folder under the system temp
//! folder, never in the reader's real data folder.

use std::io::Write;
use std::path::{Path, PathBuf};

use bible_core::{TranslationMeta, import_sword};

/// A folder under the system temp folder, removed again when the test ends.
struct Scratch {
    /// The folder itself, which is the module root.
    path: PathBuf,
}

impl Scratch {
    /// Creates a fresh module root for one test.
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("bible-security-{}-{name}", std::process::id()));
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
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The folder a synthetic zText module keeps its text in.
const ZTEXT_DIR: &str = "modules/texts/ztext/synthetic";

/// Writes a zText manifest that names the synthetic module.
fn write_manifest(scratch: &Scratch) {
    let conf = format!(
        "[Synthetic]\nDataPath=./{ZTEXT_DIR}/\nModDrv=zText\nBlockType=BOOK\nCompressType=ZIP\nVersification=KJV\n"
    );
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());
}

/// Wraps bytes in a zlib stream at the best compression level.
fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(data).expect("the stream is written");
    encoder.finish().expect("the stream is finished")
}

/// Writes the three files of one zText testament.
fn write_ztext(scratch: &Scratch, index: &[u8], compressed: &[u8], sizes: &[u8]) {
    scratch.write(&format!("{ZTEXT_DIR}/ot.bzv"), index);
    scratch.write(&format!("{ZTEXT_DIR}/ot.bzz"), compressed);
    scratch.write(&format!("{ZTEXT_DIR}/ot.bzs"), sizes);
}

/// One block table record for a block that starts at zero.
fn block_record(compressed_len: u32, declared: u32) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(&0u32.to_le_bytes());
    record.extend_from_slice(&compressed_len.to_le_bytes());
    record.extend_from_slice(&declared.to_le_bytes());
    record
}

/// One verse index record.
fn verse_record(block: u32, offset: u32, length: u16) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(&block.to_le_bytes());
    record.extend_from_slice(&offset.to_le_bytes());
    record.extend_from_slice(&length.to_le_bytes());
    record
}

/// Imports a module from a scratch folder.
fn imported(path: &Path) -> anyhow::Result<bible_core::Translation> {
    import_sword(path, TranslationMeta::new("synthetic", "Synthetic"), None)
}

#[test]
fn a_block_that_decompresses_past_the_limit_is_refused() {
    let scratch = Scratch::new("block-bomb");
    write_manifest(&scratch);

    // The table declares a tiny block, but the stream expands to five megabytes.
    let plain = vec![b'a'; 5 * 1024 * 1024];
    let compressed = deflate(&plain);
    let index = verse_record(0, 0, 1);
    let sizes = block_record(compressed.len() as u32, 64);
    write_ztext(&scratch, &index, &compressed, &sizes);

    let error = imported(&scratch.path).expect_err("the block bomb is refused");
    let message = format!("{error:#}");
    assert!(message.contains("block 0"), "{message}");
    assert!(message.contains("limit"), "{message}");
}

#[test]
fn a_block_that_declares_a_huge_size_is_refused_before_decoding() {
    let scratch = Scratch::new("declared-bomb");
    write_manifest(&scratch);

    let compressed = deflate(b"Alpha one.");
    let index = verse_record(0, 0, 1);
    let sizes = block_record(compressed.len() as u32, 4 * 1024 * 1024 + 1);
    write_ztext(&scratch, &index, &compressed, &sizes);

    let error = imported(&scratch.path).expect_err("the declared size is refused");
    assert!(format!("{error:#}").contains("declares"), "{error:#}");
}

#[test]
fn an_oversized_verse_index_is_refused() {
    let scratch = Scratch::new("index-bomb");
    write_manifest(&scratch);

    // 48,001 ten byte records is under half a megabyte of input.
    let mut index = Vec::new();
    for _ in 0..48_001 {
        index.extend_from_slice(&verse_record(0, 0, 1));
    }
    let compressed = deflate(b"A");
    let sizes = block_record(compressed.len() as u32, 1);
    write_ztext(&scratch, &index, &compressed, &sizes);

    let error = imported(&scratch.path).expect_err("the oversized index is refused");
    let message = format!("{error:#}");
    assert!(message.contains("48001 records"), "{message}");
    assert!(message.contains("limit"), "{message}");
}

#[test]
fn an_oversized_raw_text_index_is_refused() {
    let scratch = Scratch::new("raw-index-bomb");
    let conf = "[Synthetic]\nDataPath=./modules/texts/rawtext/synthetic/\nModDrv=RawText\nVersification=KJV\n";
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());
    scratch.write("modules/texts/rawtext/synthetic/ot", b"Alpha one.\n");
    let index = vec![0u8; 6 * 48_001];
    scratch.write("modules/texts/rawtext/synthetic/ot.vss", &index);

    let error = imported(&scratch.path).expect_err("the oversized index is refused");
    let message = format!("{error:#}");
    assert!(message.contains("48001 records"), "{message}");
    assert!(message.contains("limit"), "{message}");
}

/// A translation file header with a chosen metadata length and body length.
fn bbl_header(meta_len: u32, meta: &[u8], body_len: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"BIBL");
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&meta_len.to_le_bytes());
    bytes.extend_from_slice(meta);
    bytes.extend_from_slice(&body_len.to_le_bytes());
    bytes
}

#[test]
fn a_body_that_declares_a_huge_length_is_refused_before_decompressing() {
    let meta = serde_json::to_vec(&TranslationMeta::new("x", "X")).expect("metadata");
    let mut bytes = bbl_header(meta.len() as u32, &meta, u32::MAX);
    // A single zstd frame is enough for the decoder to start; the declared
    // length alone must be refused before any output is produced.
    bytes.extend_from_slice(&[0x28, 0xb5, 0x2f, 0xfd]);

    let error = bible_core::format::decode(&bytes).expect_err("the huge body is refused");
    let message = format!("{error:#}");
    assert!(message.contains("body declares"), "{message}");
    assert!(message.contains("limit"), "{message}");
}

#[test]
fn a_metadata_length_past_the_limit_is_refused() {
    let bytes = bbl_header(u32::MAX, &[], 0);
    let error = bible_core::format::decode(&bytes).expect_err("the huge header is refused");
    let message = format!("{error:#}");
    assert!(message.contains("metadata header"), "{message}");
    assert!(message.contains("limit"), "{message}");

    let error = bible_core::format::read_meta(&bytes).expect_err("listing refuses it too");
    assert!(
        format!("{error:#}").contains("metadata header"),
        "{error:#}"
    );
}

#[test]
fn a_data_path_that_traverses_upward_is_refused() {
    let scratch = Scratch::new("data-path-parent");
    let conf = "[Synthetic]\nDataPath=../../outside/\nModDrv=zText\nCompressType=ZIP\n";
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());

    let error = imported(&scratch.path).expect_err("a traversing DataPath is refused");
    let message = format!("{error:#}");
    assert!(message.contains("DataPath"), "{message}");
    assert!(message.contains("relative"), "{message}");
}

#[test]
fn an_absolute_data_path_is_refused() {
    let scratch = Scratch::new("data-path-absolute");
    let conf = "[Synthetic]\nDataPath=/tmp/outside/\nModDrv=zText\nCompressType=ZIP\n";
    scratch.write("mods.d/synthetic.conf", conf.as_bytes());

    let error = imported(&scratch.path).expect_err("an absolute DataPath is refused");
    assert!(format!("{error:#}").contains("DataPath"), "{error:#}");
}

#[test]
fn a_verse_record_past_its_block_is_refused() {
    let scratch = Scratch::new("record-offset");
    write_manifest(&scratch);

    let compressed = deflate(b"Alpha");
    // The record starts two bytes in and claims a hundred, past a five byte block.
    let index = verse_record(0, 2, 100);
    let sizes = block_record(compressed.len() as u32, 5);
    write_ztext(&scratch, &index, &compressed, &sizes);

    let error = imported(&scratch.path).expect_err("the record is not clamped");
    let message = format!("{error:#}");
    assert!(message.contains("past the end of block"), "{message}");
    assert!(message.contains('0'), "{message}");
}

#[test]
fn a_source_file_past_the_size_limit_is_refused() {
    let scratch = Scratch::new("source-size");
    let path = scratch.path.join("huge.usfm");
    let file = std::fs::File::create(&path).expect("create the source");
    file.set_len(65 * 1024 * 1024).expect("set the length");
    drop(file);

    let error = bible_core::import_path(&path, TranslationMeta::new("x", "X"), None)
        .expect_err("a source past the size limit is refused");
    let message = format!("{error:#}");
    assert!(message.contains("limit for one source file"), "{message}");
}

#[test]
fn xml_that_nests_past_the_depth_limit_is_refused() {
    let mut xml = String::from("<osis>");
    for _ in 0..300 {
        xml.push_str("<div>");
    }
    xml.push_str("</osis>");

    let error = bible_core::import_osis(&xml, TranslationMeta::new("x", "X"))
        .expect_err("deep nesting is refused");
    let message = format!("{error:#}");
    assert!(message.contains("nests deeper"), "{message}");
}

#[test]
fn a_verse_stops_at_the_representable_text_limit() {
    let scratch = Scratch::new("verse-text");
    let path = scratch.path.join("long.usfm");
    let word = "a".repeat(1_000);
    let line = vec![word.as_str(); 100].join(" ");
    let source = format!("\\id GEN\n\\c 1\n\\v 1 {line}\n");
    scratch.write("long.usfm", source.as_bytes());

    let translation = bible_core::import_path(&path, TranslationMeta::new("x", "X"), None)
        .expect("the verse is imported with the bound applied");
    let verse = translation.verse_at(0, 1, 1).expect("the verse exists");
    assert!(
        translation.verse_text(verse).len() <= 65_535,
        "the accumulated text stops at the u16 limit"
    );
}

#[test]
fn repeated_verse_records_cannot_pass_the_text_budget() {
    let scratch = Scratch::new("text-budget");
    write_manifest(&scratch);

    // Nine four megabyte blocks fit the block budget, but six hundred records
    // that each take 65,535 bytes ask for more verse text than the module may
    // produce, so the budget stops them before the strings are made.
    let plain = vec![b'a'; 4 * 1024 * 1024];
    let compressed = deflate(&plain);
    let mut index = Vec::new();
    for offset in 0..600u32 {
        let block = offset % 9;
        index.extend_from_slice(&verse_record(block, 0, 65_535));
    }
    let mut sizes = Vec::new();
    for _ in 0..9 {
        sizes.extend_from_slice(&block_record(compressed.len() as u32, 4 * 1024 * 1024));
    }
    write_ztext(&scratch, &index, &compressed, &sizes);

    let error = imported(&scratch.path).expect_err("the text budget is enforced");
    let message = format!("{error:#}");
    assert!(message.contains("verse text"), "{message}");
    assert!(message.contains("limit"), "{message}");
}
