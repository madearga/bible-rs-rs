// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::books;
use bible_core::model::{MarkKind, SpanStyle, TranslationBuilder, TranslationMeta};

#[test]
fn builder_rejects_verses_out_of_order() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    assert!(builder.push_verse(1, "In the beginning").is_ok());
    assert!(builder.push_verse(3, "skipped a verse").is_err());
    assert!(builder.push_verse(2, "the second verse").is_ok());
    assert!(!builder.is_empty());
    assert_eq!(builder.len(), 2);
}

#[test]
fn builder_refuses_an_incomplete_grid() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    builder
        .push_verse(1, "In the beginning")
        .expect("first verse");
    assert!(builder.finish().is_err(), "one verse is not a translation");
}

#[test]
fn builder_checks_mark_and_span_ranges() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    builder
        .push_verse(1, "In the beginning")
        .expect("first verse");
    assert!(builder.mark(999, MarkKind::Paragraph, 0, "").is_err());
    assert!(builder.span(SpanStyle::WordsOfChrist, 3, 3).is_err());
    assert!(builder.span(SpanStyle::WordsOfChrist, 0, 999).is_err());
    assert!(builder.mark(0, MarkKind::Paragraph, 0, "").is_ok());
    assert!(builder.span(SpanStyle::WordsOfChrist, 0, 2).is_ok());
}

#[test]
fn builder_rejects_a_mark_before_any_verse() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    assert!(builder.mark(0, MarkKind::Paragraph, 0, "").is_err());
    assert!(builder.span(SpanStyle::Selah, 0, 1).is_err());
}

#[test]
fn builder_tracks_the_canonical_cursor_across_books() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).expect("chapter exists");
            for verse in 1..=count {
                builder
                    .push_verse(verse as u8, "")
                    .expect("canonical order");
            }
        }
    }
    assert_eq!(builder.len(), 31102);
    assert!(builder.push_verse(1, "past the end of Revelation").is_err());
    let translation = builder.finish().expect("the whole grid is present");
    assert_eq!(translation.verses().len(), 31102);
}

#[test]
fn a_reference_reaches_the_built_text() {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("t", "Test"));
    let target = books::verse_offset(42, 3, 16).expect("John 3:16 exists");
    for index in 0..=target {
        let (_, _, verse) = split_index(index).expect("every earlier verse is on the grid");
        let text = if index == target {
            "the synthetic witness"
        } else {
            ""
        };
        builder.push_verse(verse, text).expect("canonical order");
    }
    builder
        .push_verse(17, "the next verse")
        .expect("the grid continues");

    let translation = builder.finish();
    assert!(
        translation.is_err(),
        "a partial grid is not a finished translation"
    );
}

/// Finds the book, chapter, and verse number of a global verse index.
fn split_index(index: u16) -> Option<(u8, u8, u8)> {
    for book in books::books() {
        let end = book.verse_start
            + books::VERSES_PER_CHAPTER[book.chapter_start as usize
                ..(book.chapter_start + book.chapter_count as u16) as usize]
                .iter()
                .sum::<u16>();
        if index < end {
            let mut chapter_start = book.verse_start;
            for chapter in 1..=book.chapter_count {
                let count = books::verses_in_chapter(book.index, chapter)?;
                if index < chapter_start + count {
                    return Some((book.index, chapter, (index - chapter_start) as u8 + 1));
                }
                chapter_start += count;
            }
        }
    }
    None
}
