// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The CLI tests. They run the release quality binary with real arguments and
//! assert on its output and exit code, which is how a core change is verified.
//!
//! The data folder holds a synthetic translation built in the test, so no
//! scripture is committed or read.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bible_core::study::{Bookmark, HighlightColour, Store, Theme};
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

/// The study export document the core store writes and reads, as a literal.
///
/// John 3:16 carries a blue highlight and a note; Genesis 1:1 carries a
/// labelled bookmark. Every field is explicit so the test fails if the CLI
/// silently falls back to a different schema.
fn study_export_literal() -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "settings": {
            "position": {"book": 0, "chapter": 1, "verse": 1},
            "theme": "light",
            "text_size": 17.0,
            "line_spacing": 1.62,
            "line_length": 34.0,
            "justify": false,
            "show_verse_numbers": true,
            "show_headings": true,
            "words_of_christ_red": true,
            "paragraph_mode": true
        },
        "highlights": [
            {"reference": {"book": 42, "chapter": 3, "verse": 16}, "colour": "blue"}
        ],
        "notes": [
            {"reference": {"book": 42, "chapter": 3, "verse": 16}, "body": "a generated note"}
        ],
        "bookmarks": [
            {"reference": {"book": 0, "chapter": 1, "verse": 1}, "label": "start"}
        ]
    })
}

/// Seeds the same study database the GUI opens, then closes it.
fn seed_study(dir: &Path) {
    let mut store = Store::open(dir.join("study.db")).expect("open the study store");
    store
        .set_highlight(
            Reference::new(42, 3, Some(16)).unwrap(),
            Some(HighlightColour::Blue),
        )
        .expect("set a highlight");
    store
        .set_note(Reference::new(42, 3, Some(16)).unwrap(), "a generated note")
        .expect("set a note");
    store
        .set_bookmark(
            Reference::new(0, 1, Some(1)).unwrap(),
            Some(Bookmark::with_label("start")),
        )
        .expect("set a bookmark");
    let mut settings = store.settings().clone();
    settings.position = Reference::new(18, 23, Some(1)).unwrap();
    settings.theme = Theme::Sepia;
    settings.text_size = 19.5;
    store.set_settings(settings).expect("set study settings");
    store.close().expect("close the study store");
}

#[test]
fn study_export_reads_the_gui_store_as_json_and_markdown() {
    let dir = scratch("study-export");
    seed_study(&dir);

    let json = dir.join("out.json");
    let exported = run(
        &dir,
        &["export", json.to_str().expect("a path"), "--format", "json"],
    );
    assert!(exported.status.success(), "{}", stderr(&exported));

    let text = std::fs::read_to_string(&json).expect("the export was written");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the export is JSON");
    let mut expected = study_export_literal();
    expected["settings"]["position"] = serde_json::json!({"book": 18, "chapter": 23, "verse": 1});
    expected["settings"]["theme"] = serde_json::json!("sepia");
    expected["settings"]["text_size"] = serde_json::json!(19.5);
    assert_eq!(document, expected, "{text}");

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
    assert!(text.contains("John 3:16, blue"), "{text}");
    assert!(text.contains("### John 3:16"), "{text}");
    assert!(text.contains("a generated note"), "{text}");
    assert!(text.contains("Genesis 1:1 (start)"), "{text}");
}

#[test]
fn study_import_writes_the_gui_store() {
    let dir = scratch("study-import");
    let source = dir.join("study-in.json");
    let mut document = study_export_literal();
    document["highlights"][0]["colour"] = serde_json::json!("green");
    document["notes"][0]["body"] = serde_json::json!("an imported note");
    document["bookmarks"][0]["label"] = serde_json::json!(null);
    std::fs::write(&source, serde_json::to_vec_pretty(&document).unwrap())
        .expect("write the study file");

    let imported = run(&dir, &["import-study", source.to_str().expect("a path")]);
    assert!(imported.status.success(), "{}", stderr(&imported));

    let store = Store::open(dir.join("study.db")).expect("reopen the study store");
    let john = Reference::new(42, 3, Some(16)).unwrap();
    let genesis = Reference::new(0, 1, Some(1)).unwrap();
    assert_eq!(store.highlight(john), Some(HighlightColour::Green));
    assert_eq!(store.note(john), Some("an imported note"));
    assert_eq!(store.bookmark(genesis), Some(&Bookmark::unlabelled()));
    assert_eq!(store.settings().theme, Theme::Light);
    assert_eq!(
        store.settings().position,
        Reference::new(0, 1, Some(1)).unwrap()
    );
    store.close().expect("close the study store");
}

#[test]
fn a_malformed_study_import_fails_and_preserves_the_store() {
    let dir = scratch("study-import-bad");
    seed_study(&dir);

    let source = dir.join("bad.json");
    std::fs::write(&source, "{\"version\": 1, \"settings\":").expect("write a broken file");
    let imported = run(&dir, &["import-study", source.to_str().expect("a path")]);
    assert_eq!(imported.status.code(), Some(1), "{}", stderr(&imported));

    let wrong_version = dir.join("wrong-version.json");
    std::fs::write(&wrong_version, r#"{"version": 99}"#).expect("write a future file");
    let imported = run(
        &dir,
        &["import-study", wrong_version.to_str().expect("a path")],
    );
    assert_eq!(imported.status.code(), Some(1), "{}", stderr(&imported));

    let store = Store::open(dir.join("study.db")).expect("reopen the study store");
    let john = Reference::new(42, 3, Some(16)).unwrap();
    let genesis = Reference::new(0, 1, Some(1)).unwrap();
    assert_eq!(store.highlight(john), Some(HighlightColour::Blue));
    assert_eq!(store.note(john), Some("a generated note"));
    assert_eq!(
        store.bookmark(genesis),
        Some(&Bookmark::with_label("start"))
    );
    store.close().expect("close the study store");
}

#[test]
fn export_refuses_to_overwrite_the_live_study_database() {
    let dir = scratch("study-export-guard");
    seed_study(&dir);

    let live = dir.join("study.db");
    let exported = run(&dir, &["export", live.to_str().expect("a path")]);
    assert_eq!(exported.status.code(), Some(1), "{}", stderr(&exported));
    assert!(
        stderr(&exported).contains("live study database"),
        "{}",
        stderr(&exported)
    );

    let store = Store::open(&live).expect("the live store still opens");
    assert_eq!(
        store.note(Reference::new(42, 3, Some(16)).unwrap()),
        Some("a generated note")
    );
    store.close().expect("close the study store");
}

#[test]
fn study_cli_rejects_access_while_another_store_is_open() {
    let dir = scratch("study-busy");
    let store = Store::open(dir.join("study.db")).expect("open GUI-like store");

    let output = run(&dir, &["export", dir.join("out.json").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("busy"), "{}", stderr(&output));

    let source = dir.join("study-in.json");
    std::fs::write(
        &source,
        serde_json::to_vec_pretty(&study_export_literal()).unwrap(),
    )
    .expect("write the study file");
    let output = run(&dir, &["import-study", source.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("busy"), "{}", stderr(&output));
    assert_eq!(
        store.note(Reference::new(42, 3, Some(16)).unwrap()),
        None,
        "the refused import did not touch the open store"
    );

    store.close().expect("close GUI-like store");
}

#[cfg(unix)]
#[test]
fn export_rejects_database_and_side_file_aliases_without_corruption() {
    use std::os::unix::fs::symlink;

    let dir = scratch("study-export-aliases");
    seed_study(&dir);
    let database = dir.join("study.db");

    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", database.display()));
        std::fs::write(&side, format!("owned {suffix} sentinel")).expect("create side file");
        let alias = dir.join(format!("alias{suffix}"));
        std::fs::hard_link(&side, &alias).expect("hard-link side file");
        let before = std::fs::read(&side).unwrap();
        let output = run(&dir, &["export", alias.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert_eq!(std::fs::read(&side).unwrap(), before);
    }

    let lock = PathBuf::from(format!("{}.lock", database.display()));
    std::fs::hard_link(&lock, dir.join("alias.lock")).expect("hard-link lock file");
    let before = std::fs::read(&lock).unwrap();
    let output = run(&dir, &["export", dir.join("alias.lock").to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(std::fs::read(&lock).unwrap(), before);

    let hardlink = dir.join("hardlink.db");
    std::fs::hard_link(&database, &hardlink).expect("hard-link database");
    let output = run(&dir, &["export", hardlink.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));

    let link = dir.join("symlink.db");
    symlink(&database, &link).expect("symlink database");
    let output = run(&dir, &["export", link.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));

    let store = Store::open(&database).expect("database remains readable");
    assert_eq!(
        store.note(Reference::new(42, 3, Some(16)).unwrap()),
        Some("a generated note")
    );
    store.close().unwrap();
}

#[cfg(unix)]
#[test]
fn export_rejects_a_dangling_symlink_to_the_database() {
    use std::os::unix::fs::symlink;

    let dir = scratch("study-export-dangling");
    let database = dir.join("study.db");
    let alias = dir.join("dangling.db");
    symlink(&database, &alias).expect("create dangling database symlink");

    let output = run(&dir, &["export", alias.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    Store::open(&database)
        .expect("database was not replaced")
        .close()
        .unwrap();
}

#[test]
fn a_reference_without_a_translation_reads_the_bundled_default() {
    let dir = scratch("default");
    let reference = Reference::new(0, 1, Some(1)).expect("Genesis 1:1 is canonical");
    let output = run(&dir, &["read", &reference.to_string()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("(KJV)"), "{}", stdout(&output));
}
