// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! USFM import tests. Every word here is synthetic, so no copyrighted text is
//! bundled and no licence question can arise from a fixture.

use std::path::PathBuf;

use bible_core::{ImportFormat, MarkKind, Translation, TranslationMeta, import_path, import_usfm};

/// A small document with two chapters, a heading, a paragraph, a range, a red
/// letter span, an italic addition, and a book that is not on the grid.
const DOCUMENT: &str = r"\id TOB
\c 1
\v 1 A book that is not on the grid.
\id GEN
\toc1 Genesis
\mt1 Genesis
\ide A Synthetic Version
\c 1
\s1 A Heading
\p
\v 1 Alpha \add added words\add* omega.
\v 2 Delta \wj spoken words\wj* tail.
\v 4 Epsilon.
\c 2
\q1
\v 1 Zeta.
\v 3-4 Eta range.
";

/// A document whose last marker is never closed.
const UNCLOSED: &str = r"\id GEN
\c 1
\v 1 Alpha beta \wj and then the file ends
";

/// Imports the main fixture with metadata the caller supplies.
fn imported() -> Translation {
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    import_usfm(&[DOCUMENT], meta).expect("the fixture imports")
}

/// The text of a verse, with an empty string when the source printed nothing.
fn verse_text(translation: &Translation, book: u8, chapter: u8, verse: u8) -> String {
    translation
        .verse_at(book, chapter, verse)
        .map(|verse| translation.verse_text(verse).to_owned())
        .expect("the canonical slot exists")
}

/// The spans of a verse, so a test can look for a style over some words.
fn spans(translation: &Translation, book: u8, chapter: u8, verse: u8) -> Vec<String> {
    let slot = translation
        .verse_at(book, chapter, verse)
        .expect("the canonical slot exists");
    let text = translation.verse_text(slot);
    translation
        .verse_spans(slot)
        .iter()
        .map(|span| {
            format!(
                "{:?}:{}",
                span.style,
                &text[span.start as usize..span.end as usize]
            )
        })
        .collect()
}

#[test]
fn verses_carry_their_text_and_numbers() {
    let translation = imported();

    assert_eq!(
        verse_text(&translation, 0, 1, 1),
        "Alpha added words omega."
    );
    assert_eq!(
        verse_text(&translation, 0, 1, 2),
        "Delta spoken words tail."
    );
    assert_eq!(verse_text(&translation, 0, 1, 3), "");
    assert_eq!(verse_text(&translation, 0, 1, 4), "Epsilon.");
    assert_eq!(verse_text(&translation, 0, 2, 1), "Zeta.");
    assert_eq!(verse_text(&translation, 0, 2, 2), "");
}

#[test]
fn a_verse_range_keeps_both_numbers() {
    let translation = imported();
    let verse = translation.verse_at(0, 2, 3).expect("the slot exists");
    assert_eq!(translation.verse_text(verse), "Eta range.");
    assert_eq!(verse.number, 3);
    assert_eq!(verse.number_end, 4);
    assert_eq!(verse_text(&translation, 0, 2, 4), "");
}

#[test]
fn a_red_letter_span_and_an_italic_addition_become_spans() {
    let translation = imported();

    assert_eq!(
        spans(&translation, 0, 1, 1),
        vec!["TranslatorAddition:added words".to_owned()]
    );
    assert_eq!(
        spans(&translation, 0, 1, 2),
        vec!["WordsOfChrist:spoken words".to_owned()]
    );
    assert!(translation.meta().red_letters);
}

#[test]
fn a_heading_becomes_a_mark_with_its_text() {
    let translation = imported();
    let verse = translation.verse_at(0, 1, 1).expect("the slot exists");
    let heading = translation
        .verse_marks(verse)
        .iter()
        .find(|mark| mark.kind == MarkKind::Heading)
        .expect("the heading is attached to the verse it precedes");
    assert_eq!(translation.mark_payload(heading), "A Heading");
    assert!(translation.meta().headings);
}

#[test]
fn a_paragraph_and_a_poetry_line_become_marks() {
    let translation = imported();

    let verse = translation.verse_at(0, 1, 1).expect("the slot exists");
    assert!(
        translation
            .verse_marks(verse)
            .iter()
            .any(|mark| mark.kind == MarkKind::Paragraph)
    );

    let poetry = translation.verse_at(0, 2, 1).expect("the slot exists");
    assert!(
        translation
            .verse_marks(poetry)
            .iter()
            .any(|mark| mark.kind == MarkKind::PoetryLine)
    );
    assert!(translation.meta().paragraphs);
}

#[test]
fn a_book_outside_the_grid_is_skipped_and_the_next_book_starts_clean() {
    let translation = imported();

    // The book before Genesis is not on the grid, so its text must not appear anywhere.
    assert!(
        !translation.text().contains("not on the grid"),
        "a book outside the grid leaked into the translation"
    );
    assert_eq!(
        verse_text(&translation, 0, 1, 1),
        "Alpha added words omega."
    );
    assert_eq!(translation.meta().book_titles[0], "Genesis");
}

#[test]
fn the_translation_title_is_used_when_the_caller_supplies_none() {
    let meta = TranslationMeta::new("synthetic", "");
    let translation = import_usfm(&[DOCUMENT], meta).expect("the fixture imports");
    assert_eq!(translation.meta().name, "A Synthetic Version");
}

#[test]
fn caller_metadata_is_not_overwritten_by_the_source() {
    let translation = imported();
    assert_eq!(translation.meta().id, "synthetic");
    assert_eq!(translation.meta().name, "Synthetic");
}

#[test]
fn an_unclosed_marker_at_the_end_of_the_file_does_not_panic() {
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[UNCLOSED], meta).expect("the fixture imports");
    assert_eq!(
        verse_text(&translation, 0, 1, 1),
        "Alpha beta and then the file ends"
    );
}

#[test]
fn the_grid_is_complete_even_when_the_source_is_not() {
    let translation = imported();
    assert_eq!(translation.verses().len(), bible_core::VERSE_COUNT);
    assert_eq!(verse_text(&translation, 65, 22, 21), "");
}

#[test]
fn a_footnote_and_a_cross_reference_are_not_shown() {
    const WITH_APPARATUS: &str = r"\id GEN
\c 1
\v 1 Alpha\f + \fr 1:1 \ft a note nobody sees\f* beta\x - \xo 1:2 \xt elsewhere\x* gamma.
";
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[WITH_APPARATUS], meta).expect("the fixture imports");
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha beta gamma.");
}

#[test]
fn a_nested_custom_marker_in_a_footnote_does_not_end_the_note() {
    // The marker sequence is the World English Bible's Genesis 1:1 footnote, kept
    // verbatim because it is the shape that used to break the importer.
    const NESTED: &str = r#"\id GEN
\c 1
\v 1 Alpha\f + \fr 1:1 \ft The Hebrew word rendered "God" is "\+wh אֱלֹהִ֑ים\+wh*" (Elohim).\f* omega.
"#;
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[NESTED], meta).expect("the fixture imports");

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha omega.");
    assert!(
        spans(&translation, 0, 1, 1).is_empty(),
        "a style from inside the footnote reached the verse"
    );
}

#[test]
fn a_nested_custom_marker_in_a_cross_reference_does_not_end_the_note() {
    const NESTED: &str = r"\id GEN
\c 1
\v 1 Alpha\x - \xo 1:2 \xt see \+nd the word\+nd* elsewhere\x* omega.
";
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[NESTED], meta).expect("the fixture imports");

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha omega.");
    assert!(
        spans(&translation, 0, 1, 1).is_empty(),
        "a style from inside the cross reference reached the verse"
    );
}

#[test]
fn a_plain_footnote_still_ends_at_its_own_star() {
    const PLAIN: &str = r"\id GEN
\c 1
\v 1 Alpha\f + \fr 1:1 \ft a note nobody sees\f* omega.
";
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[PLAIN], meta).expect("the fixture imports");

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha omega.");
}

#[test]
fn a_styled_range_outside_apparatus_still_closes() {
    const STYLED: &str = r"\id GEN
\c 1
\v 1 Alpha \add added words\add* omega.
";
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[STYLED], meta).expect("the fixture imports");

    assert_eq!(
        verse_text(&translation, 0, 1, 1),
        "Alpha added words omega."
    );
    assert_eq!(
        spans(&translation, 0, 1, 1),
        vec!["TranslatorAddition:added words".to_owned()]
    );
}

#[test]
fn a_marker_delimiter_space_does_not_reach_the_verse() {
    // The Darby shape: a word marker opens directly after a bracket, so its
    // delimiter space must not become verse text.
    const DARBY: &str = r#"\id REV
\c 22
\v 21 Alpha[\w be|strong="G3956"\w*] \w with|strong="G3326"\w* omega.
"#;
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[DARBY], meta).expect("the fixture imports");

    let text = verse_text(&translation, 65, 22, 21);
    assert!(
        text.contains("[be]"),
        "the delimiter space leaked: {text:?}"
    );
    assert!(
        !text.contains("[ be]"),
        "the delimiter space leaked: {text:?}"
    );
    assert_eq!(text, "Alpha[be] with omega.");
}

#[test]
fn a_real_space_between_two_markers_survives() {
    const SPACED: &str = r#"\id GEN
\c 1
\v 1 Alpha \w one|strong="G1520"\w* \w the|strong="G3588"\w* omega.
"#;
    let meta = TranslationMeta::new("synthetic", "Synthetic");
    let translation = import_usfm(&[SPACED], meta).expect("the fixture imports");

    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one the omega.");
}

/// A folder under the system temp folder, removed again when the test ends.
struct Scratch {
    /// The folder itself.
    path: PathBuf,
}

impl Scratch {
    /// Creates a fresh folder for one test.
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bible-usfm-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("the scratch folder is created");
        Self { path }
    }

    /// Writes a document into the folder.
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, text).expect("the document is written");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn a_folder_of_documents_imports_as_one_translation() {
    let scratch = Scratch::new("folder");
    scratch.write("01-gen.usfm", "\\id GEN\n\\c 1\n\\v 1 Alpha one.\n");
    scratch.write("02-matt.usfm", "\\id MAT\n\\c 1\n\\v 1 Delta four.\n");

    let translation = import_path(
        &scratch.path,
        TranslationMeta::new("synthetic", "Synthetic"),
        None,
    )
    .expect("the folder imports");

    assert_eq!(translation.meta().id, "synthetic");
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one.");
    assert_eq!(verse_text(&translation, 39, 1, 1), "Delta four.");
}

#[test]
fn the_format_is_detected_from_a_file_name() {
    let scratch = Scratch::new("detect");
    let file = scratch.write("gen.usfm", "\\id GEN\n\\c 1\n\\v 1 Alpha one.\n");
    assert_eq!(ImportFormat::detect(&file), Some(ImportFormat::Usfm));
    let translation =
        import_path(&file, TranslationMeta::new("synthetic", "S"), None).expect("the file imports");
    assert_eq!(verse_text(&translation, 0, 1, 1), "Alpha one.");
}
