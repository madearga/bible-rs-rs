// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Zefania import tests. Every word here is synthetic.

use bible_core::{SpanStyle, Translation, TranslationMeta, import_zefania};

/// A document with its own element names, a note inside a verse, and a book that
/// is not on the grid after the canonical one.
const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<XMLBIBLE version="1.0" biblename="A Synthetic Version">
  <INFORMATION>
    <title>A Synthetic Version</title>
    <language>en</language>
  </INFORMATION>
  <BIBLEBOOK bnumber="1" bname="Genesis">
    <CHAPTER cnumber="1">
      <VERS vnumber="1">Alpha beta.</VERS>
      <VERS vnumber="2">Gamma delta.</VERS>
      <VERS vnumber="3">Epsilon <NOTE>a note nobody sees</NOTE> zeta.</VERS>
      <VERS vnumber="4"><STYLE css="red">spoken words</STYLE> eta.</VERS>
    </CHAPTER>
    <CHAPTER cnumber="2">
      <VERS vnumber="1">Theta.</VERS>
    </CHAPTER>
  </BIBLEBOOK>
  <BIBLEBOOK bnumber="99" bname="Tobit">
    <CHAPTER cnumber="1">
      <VERS vnumber="1">A book that is not on the grid.</VERS>
    </CHAPTER>
  </BIBLEBOOK>
</XMLBIBLE>
"#;

/// Imports the fixture.
fn imported() -> Translation {
    import_zefania(DOCUMENT, TranslationMeta::new("synthetic", "Synthetic"))
        .expect("the fixture imports")
}

/// The text of a verse, with an empty string when the source printed nothing.
fn verse_text(translation: &Translation, book: u8, chapter: u8, verse: u8) -> String {
    translation
        .verse_at(book, chapter, verse)
        .map(|verse| translation.verse_text(verse).to_owned())
        .expect("the canonical slot exists")
}

#[test]
fn verses_carry_their_text() {
    let translation = imported();

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha beta.");
    assert_eq!(verse_text(&translation, 0, 1, 2), "Gamma delta.");
    assert_eq!(verse_text(&translation, 0, 2, 1), "Theta.");
    assert_eq!(verse_text(&translation, 0, 2, 2), "");
}

#[test]
fn a_note_is_dropped_with_its_content() {
    let translation = imported();
    assert_eq!(verse_text(&translation, 0, 1, 3), "Epsilon zeta.");
    assert!(!translation.text().contains("nobody sees"));
}

#[test]
fn a_book_outside_the_grid_is_skipped() {
    let translation = imported();
    assert!(!translation.text().contains("not on the grid"));
}

#[test]
fn a_styled_range_becomes_a_span() {
    let translation = imported();
    let verse = translation.verse_at(0, 1, 4).expect("the slot exists");
    let text = translation.verse_text(verse);
    let spans = translation
        .verse_spans(verse)
        .iter()
        .map(|span| (span.style, &text[span.start as usize..span.end as usize]))
        .collect::<Vec<_>>();
    assert_eq!(spans, vec![(SpanStyle::WordsOfChrist, "spoken words")]);
}

#[test]
fn the_translation_title_is_used_when_the_caller_supplies_none() {
    let translation = import_zefania(DOCUMENT, TranslationMeta::new("synthetic", ""))
        .expect("the fixture imports");
    assert_eq!(translation.meta().name, "A Synthetic Version");
}

#[test]
fn the_grid_is_complete() {
    let translation = imported();
    assert_eq!(translation.verses().len(), bible_core::VERSE_COUNT);
}
