// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The palette tests.
//!
//! They drive the fuzzy matcher and the palette's rows with synthetic data only.
//! No scripture appears here; the references are grid positions.

use bible_core::Reference;

use crate::palette::{Entry, Palette, resolve, suggest};
use crate::shortcuts::Action;

/// Builds a reference the test expects.
fn place(book: u8, chapter: u8, verse: Option<u8>) -> Reference {
    Reference::new(book, chapter, verse).expect("the reference is canonical")
}

#[test]
fn a_short_book_and_verse_resolves() {
    assert_eq!(resolve("jn 3:16"), Some(place(42, 3, Some(16))));
}

#[test]
fn a_joined_ordinal_and_chapter_resolves() {
    assert_eq!(resolve("1co13"), Some(place(45, 13, None)));
}

#[test]
fn a_misspelled_book_resolves() {
    assert_eq!(resolve("revalation 21"), Some(place(65, 21, None)));
}

#[test]
fn a_psalm_resolves() {
    assert_eq!(resolve("ps 23"), Some(place(18, 23, None)));
}

#[test]
fn a_romans_reference_resolves() {
    assert_eq!(resolve("Rom 8"), Some(place(44, 8, None)));
}

#[test]
fn a_book_name_without_a_chapter_resolves_to_its_first_chapter() {
    assert_eq!(resolve("john"), Some(place(42, 1, None)));
}

#[test]
fn an_exact_name_ranks_above_a_fuzzy_one() {
    let ranked = suggest("john");
    let john = ranked
        .iter()
        .find(|suggestion| suggestion.book == 42)
        .expect("John is suggested");
    let jonah = ranked
        .iter()
        .find(|suggestion| suggestion.book == 31)
        .expect("Jonah is suggested");
    assert!(
        john.score > jonah.score,
        "John scored {} and Jonah scored {}",
        john.score,
        jonah.score
    );
    assert_eq!(ranked.first().map(|suggestion| suggestion.book), Some(42));
}

#[test]
fn an_unknown_input_resolves_to_nothing() {
    assert_eq!(resolve("zzzzq"), None);
    assert_eq!(resolve("zzzzq 3:16"), None);
}

#[test]
fn the_palette_lists_recents_translations_and_commands() {
    let mut palette = Palette::new();
    palette.open();
    let recent = [place(42, 3, None)];
    let translations = [("kjv".to_string(), "KJV".to_string())];
    palette.refresh(&recent, &translations);
    assert!(palette.entries().contains(&Entry::Recent(recent[0])));
    assert!(palette.entries().iter().any(|entry| matches!(
        entry,
        Entry::Translation { id, .. } if id == "kjv"
    )));
    assert!(
        palette
            .entries()
            .contains(&Entry::Command(Action::JumpPalette))
    );
}

#[test]
fn the_palette_accepts_the_resolved_reference() {
    let mut palette = Palette::new();
    palette.open();
    palette.set_input("jn 3:16");
    palette.refresh(&[], &[]);
    assert_eq!(
        palette.accept(),
        Some(crate::palette::Choice::Jump(place(42, 3, Some(16))))
    );
}

#[test]
fn the_fuzzy_table_ranks_the_books_by_how_close_they_are() {
    // The table is printed in the report, so the test keeps it honest.
    let cases = [
        ("john", 42),
        ("jn", 42),
        ("revalation", 65),
        ("revelation", 65),
        ("rom", 44),
        ("ps", 18),
        ("1co", 45),
        ("jon", 31),
        ("joel", 28),
    ];
    for (input, expected) in cases {
        let ranked = suggest(input);
        let best = ranked.first().expect("a book is suggested");
        assert_eq!(
            best.book,
            expected,
            "{input} ranked {:?} first",
            ranked.iter().map(|s| s.book).collect::<Vec<u8>>()
        );
    }
}

#[test]
fn the_selection_moves_and_stops_at_the_ends() {
    let mut palette = Palette::new();
    palette.open();
    palette.set_input("john");
    palette.refresh(&[], &[]);
    palette.move_selection(1);
    assert_eq!(palette.selected(), 1);
    palette.move_selection(-5);
    assert_eq!(palette.selected(), 0);
    palette.move_selection(1000);
    assert_eq!(palette.selected(), palette.entries().len() - 1);
}
