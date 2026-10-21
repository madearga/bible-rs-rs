// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Exporting the study data and reading it back.

use bible_core::Reference;
use bible_core::study::{Bookmark, HighlightColour, Store};

#[test]
fn json_export_round_trips_into_an_empty_store() {
    let mut source = Store::open_in_memory().expect("the source store opens");
    let highlighted = Reference::parse("John 3:16").expect("the reference resolves");
    let noted = Reference::parse("Ps 23:1").expect("the reference resolves");
    let bookmarked = Reference::parse("Rom 8:28").expect("the reference resolves");

    source
        .set_highlight(highlighted, Some(HighlightColour::Orange))
        .expect("the highlight is written");
    source
        .set_note(noted, "a synthetic note body")
        .expect("the note is written");
    source
        .set_bookmark(bookmarked, Some(Bookmark::with_label("come back")))
        .expect("the bookmark is written");
    let mut settings = source.settings().clone();
    settings.position = noted;
    source.set_settings(settings).expect("the settings are set");

    let json = source.export_json().expect("the export is written");
    assert!(json.contains("\"version\": 1"), "the export is versioned");

    let mut target = Store::open_in_memory().expect("the target store opens");
    target.import_json(&json).expect("the export imports");

    assert_eq!(
        target.snapshot(),
        source.snapshot(),
        "an import reproduces the store it came from"
    );
    assert_eq!(target.highlight(highlighted), Some(HighlightColour::Orange));
    assert_eq!(target.note(noted), Some("a synthetic note body"));
    assert_eq!(
        target.bookmark(bookmarked).and_then(|b| b.label.as_deref()),
        Some("come back")
    );
    assert_eq!(target.settings().position, noted);

    // A second export of the imported store is byte identical, so the format
    // does not depend on insertion order.
    assert_eq!(
        target.export_json().expect("the export is written"),
        json,
        "an export of an import is the same document"
    );
}

#[test]
fn malformed_json_errors_and_leaves_the_store_alone() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let verse = Reference::parse("John 3:16").expect("the reference resolves");
    store
        .set_highlight(verse, Some(HighlightColour::Green))
        .expect("the highlight is written");
    store
        .set_note(verse, "do not lose this")
        .expect("the note is written");
    let before = store.snapshot();

    let broken = [
        // Not JSON at all.
        "{ this is not json",
        // JSON, but not an export.
        "{\"hello\": \"world\"}",
        // Truncated mid-document.
        "{\"version\": 1, \"settings\": {",
        // A version this build does not read.
        "{\"version\": 99, \"settings\": {}, \"highlights\": [], \"notes\": [], \"bookmarks\": []}",
    ];
    for text in broken {
        let error = store
            .import_json(text)
            .expect_err("a broken file is refused");
        assert!(
            !error.to_string().is_empty(),
            "the refusal says why: {error}"
        );
        assert_eq!(
            store.snapshot(),
            before,
            "a refused import changes nothing, even for {text:?}"
        );
    }

    // Valid JSON that names a verse the canon does not have is refused too.
    let json = store.export_json().expect("the export is written");
    let outside = json.replace("\"book\": 0", "\"book\": 200");
    let error = store
        .import_json(&outside)
        .expect_err("a reference past Revelation is refused");
    assert!(
        error.to_string().contains("outside the canonical grid"),
        "the refusal names the problem: {error}"
    );
    assert_eq!(store.snapshot(), before);
}

#[test]
fn markdown_export_names_each_reference_and_the_notes() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let highlighted = Reference::parse("John 3:16").expect("the reference resolves");
    let noted = Reference::parse("Ps 23:1").expect("the reference resolves");
    let bookmarked = Reference::parse("Rom 8:28").expect("the reference resolves");

    store
        .set_highlight(highlighted, Some(HighlightColour::Yellow))
        .expect("the highlight is written");
    store
        .set_note(noted, "synthetic note text, nothing quoted")
        .expect("the note is written");
    store
        .set_bookmark(bookmarked, Some(Bookmark::unlabelled()))
        .expect("the bookmark is written");

    let markdown = store.export_markdown();

    assert!(
        markdown.contains(&highlighted.to_string()),
        "the highlighted verse is listed:\n{markdown}"
    );
    assert!(
        markdown.contains("yellow"),
        "the highlight colour is named:\n{markdown}"
    );
    assert!(
        markdown.contains("synthetic note text, nothing quoted"),
        "the note body is present:\n{markdown}"
    );
    assert!(
        markdown.contains(&noted.to_string()),
        "the note reference is present:\n{markdown}"
    );
    assert!(
        markdown.contains(&bookmarked.to_string()),
        "the bookmark is present:\n{markdown}"
    );
}

#[test]
fn an_empty_store_exports_cleanly() {
    let store = Store::open_in_memory().expect("the store opens");
    let json = store.export_json().expect("the export is written");
    assert!(json.contains("\"highlights\": []"));

    let markdown = store.export_markdown();
    assert!(markdown.contains("## Highlights"));
    assert!(markdown.contains("## Notes"));
    assert!(markdown.contains("## Bookmarks"));

    let mut target = Store::open_in_memory().expect("the target store opens");
    target.import_json(&json).expect("an empty export imports");
    assert_eq!(target.snapshot(), store.snapshot());
}

#[test]
fn the_export_carries_the_paragraph_mode() {
    let mut store = Store::open_in_memory().expect("the store opens");
    let mut settings = store.settings().clone();
    settings.paragraph_mode = false;
    store.set_settings(settings).expect("the settings are set");

    let json = store.export_json().expect("the export is written");
    assert!(
        json.contains("\"paragraph_mode\": false"),
        "the export should name the paragraph mode"
    );

    let mut target = Store::open_in_memory().expect("the target store opens");
    target.import_json(&json).expect("the export imports");
    assert!(!target.settings().paragraph_mode);
}

#[test]
fn an_export_without_the_paragraph_mode_imports_with_the_default() {
    let mut source = Store::open_in_memory().expect("the source store opens");
    source
        .set_settings(source.settings().clone())
        .expect("the settings are set");
    let json = source.export_json().expect("the export is written");
    let mut document: serde_json::Value = serde_json::from_str(&json).expect("the export is JSON");
    document["settings"]
        .as_object_mut()
        .expect("the settings are an object")
        .remove("paragraph_mode");
    let without = serde_json::to_string(&document).expect("the document is written");
    assert!(!without.contains("paragraph_mode"));

    let mut target = Store::open_in_memory().expect("the target store opens");
    target
        .import_json(&without)
        .expect("an export from before the field still imports");
    assert!(
        target.settings().paragraph_mode,
        "a missing field should fall back to the default"
    );
}
