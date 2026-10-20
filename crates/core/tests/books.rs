// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::books::{self, BOOK_COUNT, CHAPTER_COUNT, VERSE_COUNT};

#[test]
fn grid_has_the_canonical_totals() {
    assert_eq!(books::books().len(), BOOK_COUNT);
    assert_eq!(BOOK_COUNT, 66);

    let chapters: usize = books::books()
        .iter()
        .map(|book| book.chapter_count as usize)
        .sum();
    assert_eq!(chapters, CHAPTER_COUNT);
    assert_eq!(CHAPTER_COUNT, 1189);

    let verses: usize = books::VERSES_PER_CHAPTER.iter().map(|n| *n as usize).sum();
    assert_eq!(verses, VERSE_COUNT);
    assert_eq!(books::VERSES_PER_CHAPTER.len(), CHAPTER_COUNT);
    assert_eq!(VERSE_COUNT, 31102);
}

#[test]
fn chapter_offsets_follow_each_books_chapter_count() {
    let mut chapter_start = 0u16;
    let mut verse_start = 0u16;
    for (index, book) in books::books().iter().enumerate() {
        assert_eq!(book.index as usize, index);
        assert_eq!(
            book.chapter_start, chapter_start,
            "{} chapter start",
            book.name
        );
        assert_eq!(book.verse_start, verse_start, "{} verse start", book.name);

        let book_verses: u16 = books::VERSES_PER_CHAPTER
            [chapter_start as usize..(chapter_start + book.chapter_count as u16) as usize]
            .iter()
            .sum();
        assert_eq!(
            books::verse_offset(book.index, book.chapter_count, u8::MAX),
            None,
            "{} has no verse 255 in its last chapter",
            book.name
        );
        assert_eq!(
            books::chapter_offset(book.index, book.chapter_count + 1),
            None,
            "{} rejects a chapter past the end",
            book.name
        );

        chapter_start += book.chapter_count as u16;
        verse_start += book_verses;
    }
    assert_eq!(chapter_start as usize, CHAPTER_COUNT);
    assert_eq!(verse_start as usize, VERSE_COUNT);

    let psalms = books::book(18).expect("Psalms is book 18");
    assert_eq!(psalms.name, "Psalms");
    assert_eq!(psalms.chapter_count, 150);
    assert_eq!(books::verses_in_chapter(18, 119), Some(176));
    assert_eq!(books::verses_in_chapter(18, 0), None);
    assert_eq!(books::verses_in_chapter(18, 151), None);

    assert_eq!(books::verse_offset(0, 1, 1), Some(0));
    assert_eq!(books::verse_offset(65, 22, 21), Some(31101));
    assert_eq!(books::verse_offset(42, 3, 16), Some(26136));
}
