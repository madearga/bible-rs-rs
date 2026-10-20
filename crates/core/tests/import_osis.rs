// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! OSIS import tests. Every word here is synthetic.

use bible_core::{MarkKind, SpanStyle, Translation, TranslationMeta, import_osis};

/// A document that states verses as elements and as milestones, splits one verse's
/// text across child elements, and hides a note inside a verse.
const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<osis xmlns="http://www.bibletechnologies.net/2003/OSIS/namespace">
  <osisText osisIDWork="Synthetic">
    <div type="book" osisID="Gen">
      <chapter osisID="Gen.1">
        <title type="section">A Heading</title>
        <verse osisID="Gen.1.1">Alpha <w>beta</w> gamma.</verse>
        <verse osisID="Gen.1.2">Delta <transChange>added words</transChange> omega.</verse>
        <verse sID="Gen.1.3"/>
        Epsilon <note type="study">a note nobody sees</note> zeta.
        <verse eID="Gen.1.3"/>
        <verse sID="Gen.1.4" eID="Gen.1.4"/>
      </chapter>
      <chapter osisID="Gen.2">
        <verse osisID="Gen.2.1">Eta theta.</verse>
      </chapter>
    </div>
  </osisText>
</osis>
"#;

/// Imports the fixture.
fn imported() -> Translation {
    import_osis(DOCUMENT, TranslationMeta::new("synthetic", "Synthetic"))
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

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha beta gamma.");
    assert_eq!(
        verse_text(&translation, 0, 1, 2),
        "Delta added words omega."
    );
    assert_eq!(verse_text(&translation, 0, 1, 3), "Epsilon zeta.");
    assert_eq!(verse_text(&translation, 0, 1, 4), "");
    assert_eq!(verse_text(&translation, 0, 2, 1), "Eta theta.");
    assert_eq!(verse_text(&translation, 0, 2, 2), "");
}

#[test]
fn a_note_is_dropped_with_its_content() {
    let translation = imported();
    assert!(!translation.text().contains("nobody sees"));
}

#[test]
fn a_translator_addition_becomes_a_span() {
    let translation = imported();
    let verse = translation.verse_at(0, 1, 2).expect("the slot exists");
    let text = translation.verse_text(verse);
    let spans = translation
        .verse_spans(verse)
        .iter()
        .map(|span| (span.style, &text[span.start as usize..span.end as usize]))
        .collect::<Vec<_>>();
    assert_eq!(spans, vec![(SpanStyle::TranslatorAddition, "added words")]);
}

#[test]
fn a_title_becomes_a_heading_mark() {
    let translation = imported();
    let verse = translation.verse_at(0, 1, 1).expect("the slot exists");
    let heading = translation
        .verse_marks(verse)
        .iter()
        .find(|mark| mark.kind == MarkKind::Heading)
        .expect("the heading is attached to the verse it precedes");
    assert_eq!(translation.mark_payload(heading), "A Heading");
}

#[test]
fn the_grid_is_complete() {
    let translation = imported();
    assert_eq!(translation.verses().len(), bible_core::VERSE_COUNT);
}
