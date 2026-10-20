// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use bible_core::Reference;

#[test]
fn resolves_common_reference_shapes() {
    let john = Reference::parse("jn 3:16").expect("jn 3:16 resolves");
    assert_eq!(john.book, 42);
    assert_eq!(john.chapter, 3);
    assert_eq!(john.verse, Some(16));
    assert_eq!(john.to_string(), "John 3:16");

    let compact = Reference::parse("1co13").expect("1co13 resolves");
    assert_eq!(compact.book, 45);
    assert_eq!(compact.chapter, 13);
    assert_eq!(compact.verse, None);

    let spaced = Reference::parse("1co 13").expect("1co 13 resolves");
    assert_eq!(spaced, compact);
    assert_eq!(spaced.to_string(), "1 Corinthians 13");

    let psalm = Reference::parse("ps 23").expect("ps 23 resolves");
    assert_eq!(psalm.book, 18);
    assert_eq!(psalm.chapter, 23);
    assert_eq!(psalm.verse, None);
    assert_eq!(psalm.to_string(), "Psalms 23");

    let last = Reference::parse("rev 22:21").expect("rev 22:21 resolves");
    assert_eq!(last.book, 65);
    assert_eq!(last.chapter, 22);
    assert_eq!(last.verse, Some(21));
}

#[test]
fn tolerates_a_misspelled_book_name() {
    let revelation = Reference::parse("revalation 21").expect("the typo still resolves");
    assert_eq!(revelation.book, 65);
    assert_eq!(revelation.chapter, 21);
    assert_eq!(revelation.verse, None);

    let full = Reference::parse("  Revelation 21:4 ").expect("full name resolves");
    assert_eq!(full.book, 65);
    assert_eq!(full.verse, Some(4));
}

#[test]
fn rejects_references_outside_the_grid() {
    assert!(
        Reference::parse("rev 23").is_err(),
        "Revelation has 22 chapters"
    );
    assert!(Reference::parse("jn 3:99").is_err(), "John 3 has 36 verses");
    assert!(
        Reference::parse("gen 51").is_err(),
        "Genesis has 50 chapters"
    );
    assert!(
        Reference::parse("ps 119:177").is_err(),
        "Psalm 119 has 176 verses"
    );
}

#[test]
fn rejects_garbled_input() {
    assert!(Reference::parse("").is_err());
    assert!(Reference::parse("hello").is_err());
    assert!(Reference::parse("jn banana").is_err());
    assert!(Reference::parse("jn 3:16:2").is_err());
    assert!(Reference::parse("zzyzx 1").is_err());
    assert!(Reference::parse("66 1").is_err());
}

#[test]
fn reference_construction_is_validated() {
    assert!(Reference::new(0, 1, None).is_ok());
    assert!(Reference::new(0, 0, None).is_err());
    assert!(Reference::new(66, 1, None).is_err());
    assert!(Reference::new(42, 3, Some(0)).is_err());
    assert!(Reference::new(42, 3, Some(37)).is_err());
    assert_eq!(
        Reference::new(42, 3, Some(16))
            .expect("John 3:16 exists")
            .chapter_only()
            .verse,
        None
    );
}
