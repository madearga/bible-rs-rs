// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The import security tests. They feed the CLI hostile files and assert that
//! nothing lands outside the data folder, that a link at the destination is
//! replaced rather than followed, and that the identifier check is enforced.
//!
//! Every test runs in its own scratch folder, so the reader's real data folder
//! is never read or written.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bible_core::{Translation, TranslationBuilder, TranslationMeta, books, format};

/// The binary under test, built by cargo before the tests run.
const BINARY: &str = env!("CARGO_BIN_EXE_bible");

/// A scratch data folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-import-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("translations")).expect("scratch folder");
    dir
}

/// Builds a full-grid translation from generated words, named by `id`.
fn synthetic(id: &str) -> Translation {
    let mut meta = TranslationMeta::new(id, format!("Synthetic {id}"));
    meta.abbreviation = "SYN".to_owned();
    meta.language = "en".to_owned();
    meta.licence = "Generated test words".to_owned();
    meta.origin = "synthetic".to_owned();
    meta.book_titles = books::books()
        .iter()
        .map(|book| book.name.to_owned())
        .collect();

    let mut builder = TranslationBuilder::new(meta);
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).expect("a real chapter");
            for verse in 1..=count {
                let text = match (book.index, chapter, verse) {
                    (0, 1, 1) => "alpha beta gamma, generated words only.".to_owned(),
                    _ => String::new(),
                };
                builder
                    .push_verse(verse as u8, &text)
                    .expect("verse in order");
            }
        }
    }
    builder.finish().expect("the grid is complete")
}

/// Writes an encoded translation under a fixed source name.
fn source_file(dir: &Path, name: &str, translation: &Translation) -> PathBuf {
    let path = dir.join(name);
    let bytes = format::encode(translation).expect("encode");
    std::fs::write(&path, bytes).expect("write the source");
    path
}

/// Runs the CLI against a data folder.
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BINARY)
        .args(args)
        .env("BIBLE_DATA_DIR", dir)
        .output()
        .expect("the binary runs")
}

/// The standard error of a finished run, as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_header_identifier_with_a_parent_component_is_refused() {
    let dir = scratch("parent-id");
    let source = source_file(&dir, "hostile.bbl", &synthetic("../escaped"));
    let outside = dir.join("escaped.bbl");

    let output = run(&dir, &["import", source.to_str().expect("a path")]);
    assert!(!output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("identifier"),
        "the message names the identifier: {}",
        stderr(&output)
    );
    assert!(!outside.exists(), "nothing was written outside the folder");
    assert!(
        !dir.join("translations").join("../escaped.bbl").exists(),
        "no file escaped the translations folder"
    );
}

#[test]
fn an_absolute_header_identifier_is_refused() {
    let dir = scratch("absolute-id");
    let target = dir.join("outside").join("absolute.bbl");
    let id = target.to_str().expect("a path").to_owned();
    let source = source_file(&dir, "hostile.bbl", &synthetic(&id));

    let output = run(&dir, &["import", source.to_str().expect("a path")]);
    assert!(!output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("identifier"),
        "the message names the identifier: {}",
        stderr(&output)
    );
    assert!(!target.exists(), "nothing was written at the absolute path");
}

#[test]
fn a_link_at_the_destination_is_replaced_not_followed() {
    let dir = scratch("destination-link");
    let source = source_file(&dir, "safe.bbl", &synthetic("safe"));
    let outside = dir.join("outside.bbl");
    std::fs::write(&outside, "untouched sentinel").expect("write the sentinel");
    let destination = dir.join("translations").join("safe.bbl");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &destination).expect("plant the link");

    let output = run(&dir, &["import", source.to_str().expect("a path")]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        std::fs::read_to_string(&outside).expect("the sentinel remains"),
        "untouched sentinel",
        "the link target was not written through"
    );
    #[cfg(unix)]
    assert!(
        !std::fs::symlink_metadata(&destination)
            .expect("the destination exists")
            .file_type()
            .is_symlink(),
        "the destination is a real file now"
    );
}
