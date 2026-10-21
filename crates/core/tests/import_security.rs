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
