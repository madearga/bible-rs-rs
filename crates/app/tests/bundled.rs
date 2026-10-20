// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The bundled translation tests. They prove the seven files under
//! `data/translations` really are compiled into the binary, that each decodes to
//! a complete grid, and that the reader, the search, and the command line show
//! the text those files carry rather than the synthetic sample.
//!
//! The tests read the committed `.bbl` files and run the real binary. Nothing
//! here writes scripture into the repository.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use bible_app::app::{ShotSpec, render_shot};
use bible_app::bundled::{BUNDLED_TRANSLATIONS, DEFAULT_TRANSLATION_ID, find};
use bible_app::theme::Theme;
use bible_app::{Library, SAMPLE_TRANSLATION_ID};

/// The binary under test, built by cargo before the integration tests run.
const BINARY: &str = env!("CARGO_BIN_EXE_bible");

/// The verse count every translation on the canonical grid carries.
const VERSE_COUNT: usize = 31_102;

/// The identifiers the app is required to ship, in menu order.
const EXPECTED_IDS: [&str; 7] = ["kjv", "web", "bsb", "asv", "ylt", "darby", "webster"];

/// wgpu device creation is serialised inside this test binary, the way the
/// render tests do it, so the two shots do not fight over the Metal device.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch data folder no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-bundled-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("translations")).expect("scratch folder");
    dir
}

/// The committed translations folder, which the embedded table must match.
fn data_translations() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/translations")
}

/// Runs the CLI against a data folder and returns its standard output.
fn run(dir: &Path, args: &[&str]) -> String {
    let output = Command::new(BINARY)
        .args(args)
        .env("BIBLE_DATA_DIR", dir)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "bible {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_embedded_table_lists_the_seven_committed_translations() {
    let ids: Vec<&str> = BUNDLED_TRANSLATIONS
        .iter()
        .map(|entry| entry.id())
        .collect();
    assert_eq!(ids, EXPECTED_IDS);
    for entry in &BUNDLED_TRANSLATIONS {
        assert_eq!(
            &entry.bytes()[..4],
            b"BIBL",
            "{} does not start with the translation magic",
            entry.id()
        );
        assert_eq!(
            entry.meta().expect("a readable header").id,
            entry.id(),
            "the file header disagrees with the table entry"
        );
    }
}

#[test]
fn the_table_and_the_data_folder_cannot_drift() {
    let mut files: Vec<String> = std::fs::read_dir(data_translations())
        .expect("the data folder exists")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bbl"))
        .map(|path| {
            path.file_stem()
                .expect("a file has a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    files.sort();
    let mut ids: Vec<String> = BUNDLED_TRANSLATIONS
        .iter()
        .map(|entry| entry.id().to_owned())
        .collect();
    ids.sort();
    assert_eq!(
        files, ids,
        "data/translations and the embedded table disagree, so one of them has a stray file"
    );
}

#[test]
fn every_embedded_translation_decodes_to_the_full_grid() {
    for entry in &BUNDLED_TRANSLATIONS {
        let translation = entry
            .decode()
            .unwrap_or_else(|error| panic!("{} does not decode: {error:#}", entry.id()));
        assert_eq!(
            translation.verses().len(),
            VERSE_COUNT,
            "{} does not carry every verse",
            entry.id()
        );
    }
}

#[test]
fn the_default_is_the_king_james_version_and_switching_round_trips() {
    assert_eq!(DEFAULT_TRANSLATION_ID, "kjv");
    assert_eq!(
        find(DEFAULT_TRANSLATION_ID)
            .expect("the default is embedded")
            .id(),
        "kjv"
    );

    let library = Library::discover_in(scratch("switch"));
    let default = library.load(None).expect("the default loads");
    assert_eq!(default.meta().id, "kjv");

    let web = library.load(Some("web")).expect("web loads");
    assert_eq!(web.meta().id, "web");

    let back = library.load(None).expect("the default loads again");
    assert_eq!(back.meta().id, "kjv");
    assert!(
        Arc::ptr_eq(&default, &back),
        "the second load decoded the same translation again"
    );

    let sample = library
        .load(Some(SAMPLE_TRANSLATION_ID))
        .expect("the sample loads");
    assert_eq!(sample.meta().id, SAMPLE_TRANSLATION_ID);
    assert_ne!(
        sample.meta().id,
        default.meta().id,
        "the sample must not stand in for scripture"
    );
}

#[test]
fn the_cli_reads_the_real_king_james_verse() {
    let dir = scratch("read");
    let text = run(&dir, &["read", "john 3:16"]);
    assert!(text.contains("John 3:16 (KJV)"), "{text}");
    assert!(
        text.contains(
            "For God so loved the world, that he gave his only begotten Son, \
             that whosoever believeth in him should not perish, but have everlasting life."
        ),
        "{text}"
    );
}

#[test]
fn the_cli_searches_real_text() {
    let dir = scratch("search");
    let text = run(&dir, &["search", "beginning", "--limit", "3"]);
    assert!(text.contains("Genesis 1:1"), "{text}");
    assert!(text.contains("In the beginning God created"), "{text}");

    let limited = run(&dir, &["search", "beginning", "--limit", "1"]);
    assert_eq!(
        limited.lines().count(),
        1,
        "--limit did not stop the search: {limited}"
    );
}

#[test]
fn the_cli_translations_verb_lists_the_seven_names() {
    let dir = scratch("translations");
    let text = run(&dir, &["translations"]);
    for (id, name) in [
        ("kjv", "King James Version"),
        ("web", "World English Bible"),
        ("bsb", "Berean Standard Bible"),
        ("asv", "American Standard Version"),
        ("ylt", "Young's Literal Translation"),
        ("darby", "Darby Bible"),
        ("webster", "Webster Bible"),
    ] {
        assert!(text.contains(id), "{id} is missing from:\n{text}");
        assert!(text.contains(name), "{name} is missing from:\n{text}");
    }
    assert!(
        !text.contains(SAMPLE_TRANSLATION_ID),
        "the synthetic sample is listed as a translation:\n{text}"
    );
}

#[test]
fn a_bundled_shot_differs_from_the_sample_shot() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("shot");
    let reference = bible_core::Reference::parse("psalm 23").expect("Psalm 23 is canonical");

    let bundled_path = dir.join("psalm23-bundled.png");
    let bundled = ShotSpec {
        path: bundled_path.clone(),
        size: [640, 480],
        theme: Theme::Light,
        reference,
        translation: Some(DEFAULT_TRANSLATION_ID.to_owned()),
        ..ShotSpec::default()
    };
    render_shot(&bundled).expect("the bundled shot renders");

    let sample_path = dir.join("psalm23-sample.png");
    let sample = ShotSpec {
        path: sample_path.clone(),
        size: [640, 480],
        theme: Theme::Light,
        reference,
        translation: Some(SAMPLE_TRANSLATION_ID.to_owned()),
        ..ShotSpec::default()
    };
    render_shot(&sample).expect("the sample shot renders");

    let bundled_bytes = std::fs::read(&bundled_path).expect("the bundled PNG was written");
    let sample_bytes = std::fs::read(&sample_path).expect("the sample PNG was written");
    assert!(
        bundled_bytes != sample_bytes,
        "the bundled render is identical to the sample render, so no real text reached the page"
    );
}
