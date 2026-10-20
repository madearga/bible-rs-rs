// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Malformed input tests. Every format must answer a truncated file with an error
//! and never with a panic.

use bible_core::{TranslationMeta, import_osis, import_sword, import_usfm, import_zefania};

/// Metadata for a test import.
fn meta() -> TranslationMeta {
    TranslationMeta::new("synthetic", "Synthetic")
}

#[test]
fn a_usfm_file_truncated_before_its_verse_number_is_an_error() {
    let truncated = "\\id GEN\n\\c 1\n\\v ";
    assert!(import_usfm(&[truncated], meta()).is_err());
}

#[test]
fn a_usfm_file_truncated_before_its_book_code_is_an_error() {
    // Nothing on the grid, so there is no translation to build.
    assert!(import_usfm(&["\\id "], meta()).is_err());
}

#[test]
fn an_osis_file_truncated_inside_an_attribute_is_an_error() {
    let truncated =
        r#"<osis><osisText><div osisID="Gen"><chapter osisID="Gen.1"><verse osisID="Gen.1"#;
    assert!(import_osis(truncated, meta()).is_err());
}

#[test]
fn a_zefania_file_truncated_inside_a_verse_is_an_error() {
    let truncated = r#"<XMLBIBLE biblename="S"><BIBLEBOOK bnumber="1" bname="Genesis"><CHAPTER cnumber="1"><VERS vnumber="1">Alpha"#;
    assert!(import_zefania(truncated, meta()).is_err());
}

#[test]
fn an_empty_xml_document_is_an_error() {
    assert!(import_osis("", meta()).is_err());
    assert!(import_zefania("", meta()).is_err());
    assert!(import_osis("<osis></osis>", meta()).is_err());
}

#[test]
fn a_sword_manifest_truncated_before_its_driver_is_an_error() {
    let root = std::env::temp_dir().join(format!("bible-malformed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("mods.d")).expect("the folder is created");
    std::fs::write(
        root.join("mods.d/synthetic.conf"),
        b"[Synthetic]\nDataPath=./modules/te",
    )
    .expect("the manifest is written");

    assert!(import_sword(&root, meta(), None).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_malformed_archive_is_an_error_and_not_a_panic() {
    use bible_core::import::zip::read_archive_bytes;

    assert!(read_archive_bytes(b"").is_err());
    assert!(read_archive_bytes(b"PK").is_err());
    assert!(read_archive_bytes(&vec![0u8; 4096]).is_err());
}
