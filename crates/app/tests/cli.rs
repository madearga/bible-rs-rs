// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The CLI tests. They run the release quality binary with real arguments and
//! assert on its output and exit code, which is how a core change is verified.
//!
//! The data folder holds a synthetic translation built in the test, so no
//! scripture is committed or read.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bible_core::{Reference, Translation, TranslationBuilder, TranslationMeta, books, format};

/// The binary under test, built by cargo before the tests run.
const BINARY: &str = env!("CARGO_BIN_EXE_bible");

/// The synthetic words every generated verse carries, and the needle the search
/// test looks for.
const WITNESS: &str = "synthetic witness words";

/// A scratch data folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("translations")).expect("scratch folder");
    dir
}

/// Builds a full-grid translation from generated words, with one witness verse
/// that carries a searchable phrase.
fn synthetic(id: &str) -> Translation {
    let mut meta = TranslationMeta::new(id, format!("Synthetic {id}"));
    meta.abbreviation = id.to_uppercase();
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
                    (42, 3, 16) => format!("{WITNESS} in a generated verse for the test."),
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

/// Installs a synthetic translation into a data folder.
fn install(dir: &Path, id: &str) {
    let translation = synthetic(id);
    let bytes = format::encode(&translation).expect("encode");
    std::fs::write(dir.join("translations").join(format!("{id}.bbl")), bytes)
        .expect("write the translation");
}

/// Runs the CLI against a data folder.
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(BINARY)
        .args(args)
        .env("BIBLE_DATA_DIR", dir)
        .output()
        .expect("the binary runs")
}

/// The standard output of a finished run, as text.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The standard error of a finished run, as text.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn read_prints_a_verse_and_exits_zero() {
    let dir = scratch("read");
    install(&dir, "synth");
    let output = run(&dir, &["read", "John 3:16", "--translation", "synth"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("John 3:16"), "{text}");
    assert!(text.contains(WITNESS), "{text}");
}

#[test]
fn read_prints_a_chapter_and_exits_zero() {
    let dir = scratch("chapter");
    install(&dir, "synth");
    let output = run(&dir, &["read", "Genesis 1", "--translation", "synth"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("alpha beta gamma"));
}

#[test]
fn read_rejects_a_bad_reference() {
    let dir = scratch("bad-reference");
    install(&dir, "synth");
    let output = run(&dir, &["read", "Nowhere 99:1"]);
    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}

#[test]
fn translations_lists_the_bundled_translations() {
    let dir = scratch("translations");
    let output = run(&dir, &["translations"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    for name in [
        "King James Version",
        "World English Bible",
        "Berean Standard Bible",
        "American Standard Version",
        "Young's Literal Translation",
        "Darby Bible",
        "Webster Bible",
    ] {
        assert!(text.contains(name), "{name} is missing from:\n{text}");
    }
}

#[test]
fn search_prints_matches_and_exits_zero() {
    let dir = scratch("search");
    install(&dir, "synth");
    let output = run(&dir, &["search", "witness", "--translation", "synth"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("John 3:16"), "{text}");
    assert!(text.contains(WITNESS), "{text}");
}

#[test]
fn an_unknown_verb_prints_usage_and_exits_non_zero() {
    let dir = scratch("usage");
    let output = run(&dir, &["frobnicate"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("usage: bible"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn import_converts_a_text_file_and_installs_it() {
    let dir = scratch("import");
    let source = dir.join("witness.txt");
    std::fs::write(
        &source,
        "# generated test words, no scripture\nJohn 3:16\tsynthetic witness words from a file\nGenesis 1:1\talpha beta gamma from a file\n",
    )
    .expect("write the source");

    let output = run(
        &dir,
        &["import", source.to_str().expect("a path"), "--id", "mine"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("installed mine"));

    let listed = run(&dir, &["translations"]);
    assert!(stdout(&listed).contains("mine"), "{}", stdout(&listed));

    let read = run(&dir, &["read", "John 3:16", "--translation", "mine"]);
    assert!(read.status.success(), "{}", stderr(&read));
    assert!(stdout(&read).contains("synthetic witness words from a file"));
}

#[test]
fn study_data_exports_and_imports_round_trip() {
    let dir = scratch("study");
    let source = dir.join("study-in.json");
    std::fs::write(
        &source,
        r#"{
            "highlights": [{"reference": "John 3:16", "colour": 2}],
            "notes": [{"reference": "John 3:16", "body": "a generated note"}],
            "bookmarks": [{"reference": "Genesis 1:1", "label": "start"}]
        }"#,
    )
    .expect("write the study file");

    let imported = run(&dir, &["import-study", source.to_str().expect("a path")]);
    assert!(imported.status.success(), "{}", stderr(&imported));

    let json = dir.join("out.json");
    let exported = run(
        &dir,
        &["export", json.to_str().expect("a path"), "--format", "json"],
    );
    assert!(exported.status.success(), "{}", stderr(&exported));
    let text = std::fs::read_to_string(&json).expect("the export was written");
    assert!(text.contains("John 3:16"), "{text}");
    assert!(text.contains("a generated note"), "{text}");

    let markdown = dir.join("out.md");
    let exported = run(
        &dir,
        &[
            "export",
            markdown.to_str().expect("a path"),
            "--format",
            "markdown",
        ],
    );
    assert!(exported.status.success(), "{}", stderr(&exported));
    let text = std::fs::read_to_string(&markdown).expect("the export was written");
    assert!(text.contains("## Highlights"), "{text}");
    assert!(text.contains("a generated note"), "{text}");
}

#[test]
fn a_reference_without_a_translation_reads_the_bundled_default() {
    let dir = scratch("default");
    let reference = Reference::new(0, 1, Some(1)).expect("Genesis 1:1 is canonical");
    let output = run(&dir, &["read", &reference.to_string()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("(KJV)"), "{}", stdout(&output));
}
