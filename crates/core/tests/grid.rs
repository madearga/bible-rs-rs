// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The canonical grid has to be exact: a wrong chapter count or offset would
//! misalign every translation the program ever opens.

use bible_core::books;

#[test]
fn grid_has_the_canonical_totals() {
    assert_eq!(books::BOOK_COUNT, 66);
    assert_eq!(books::CHAPTER_COUNT, 1189);
    assert_eq!(books::VERSE_COUNT, 31102);
    assert_eq!(books::BOOKS.len(), books::BOOK_COUNT);
    assert_eq!(books::VERSES_PER_CHAPTER.len(), books::CHAPTER_COUNT);
}

#[test]
fn chapter_offsets_agree_with_each_books_chapter_count() {
    let mut next_chapter = 0;
    let mut next_verse = 0usize;
    for book in &books::BOOKS {
        assert_eq!(
            usize::from(book.chapter_start),
            next_chapter,
            "{}",
            book.name
        );
        assert_eq!(usize::from(book.verse_start), next_verse, "{}", book.name);
        assert_eq!(
            usize::from(books::book_verse_count(book.index)),
            (0..book.chapters)
                .map(|offset| usize::from(
                    books::VERSES_PER_CHAPTER
                        [usize::from(book.chapter_start) + usize::from(offset)]
                ))
                .sum::<usize>(),
            "{}",
            book.name
        );
        for chapter in 1..=book.chapters {
            let offset = usize::from(book.chapter_start) + usize::from(chapter) - 1;
            let verses = usize::from(books::VERSES_PER_CHAPTER[offset]);
            assert_eq!(
                books::verses_in_chapter(book.index, chapter),
                Some(verses as u16),
                "{} {chapter}",
                book.name
            );
            assert_eq!(
                books::chapter_verse_start(book.index, chapter),
                Some(next_verse as u32),
                "{} {chapter}",
                book.name
            );
            next_verse += verses;
        }
        next_chapter += usize::from(book.chapters);
    }
    assert_eq!(next_chapter, books::CHAPTER_COUNT);
    assert_eq!(next_verse, books::VERSE_COUNT);
}

#[test]
fn the_canon_runs_from_genesis_to_revelation() {
    let first = books::book(0).expect("Genesis");
    let last = books::book(65).expect("Revelation");
    assert_eq!(first.name, "Genesis");
    assert_eq!(last.name, "Revelation");
    assert!(books::book(66).is_none());
    assert_eq!(books::verses_in_chapter(65, 22), Some(21));
    assert_eq!(books::verses_in_chapter(65, 23), None);
}
