// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Reference parsing is what the jump palette leans on, so it has to accept how
//! a reader actually types and still refuse a place the canon does not have.

use bible_core::reference::{Reference, parse};

fn resolved(input: &str) -> Reference {
    parse(input).unwrap_or_else(|error| panic!("{input:?} should resolve: {error:#}"))
}

#[test]
fn resolves_the_shapes_a_reader_types() {
    assert_eq!(
        resolved("jn 3:16"),
        Reference {
            book: 42,
            chapter: 3,
            verse: Some(16)
        }
    );
    assert_eq!(
        resolved("1co13"),
        Reference {
            book: 45,
            chapter: 13,
            verse: None
        }
    );
    assert_eq!(
        resolved("1co 13"),
        Reference {
            book: 45,
            chapter: 13,
            verse: None
        }
    );
    assert_eq!(
        resolved("revalation 21"),
        Reference {
            book: 65,
            chapter: 21,
            verse: None
        }
    );
    assert_eq!(
        resolved("ps 23"),
        Reference {
            book: 18,
            chapter: 23,
            verse: None
        }
    );
    assert_eq!(
        resolved("rev 22:21"),
        Reference {
            book: 65,
            chapter: 22,
            verse: Some(21)
        }
    );
}

#[test]
fn resolves_full_names_ordinals_and_whitespace() {
    assert_eq!(
        resolved("  Genesis 1:1 "),
        Reference {
            book: 0,
            chapter: 1,
            verse: Some(1)
        }
    );
    assert_eq!(
        resolved("1 John 3:16"),
        Reference {
            book: 61,
            chapter: 3,
            verse: Some(16)
        }
    );
    assert_eq!(
        resolved("1jn 3.16"),
        Reference {
            book: 61,
            chapter: 3,
            verse: Some(16)
        }
    );
    assert_eq!(
        resolved("psalm 119"),
        Reference {
            book: 18,
            chapter: 119,
            verse: None
        }
    );
    assert_eq!(
        resolved("Rom 8:28"),
        Reference {
            book: 44,
            chapter: 8,
            verse: Some(28)
        }
    );
}

#[test]
fn rejects_a_chapter_the_canon_does_not_have() {
    let error = parse("jn 22").expect_err("John has 21 chapters");
    assert!(error.to_string().contains("no chapter"), "{error:#}");
    assert!(parse("gen 51").is_err());
    assert!(parse("rev 0").is_err());
}

#[test]
fn rejects_a_verse_past_the_end_of_its_chapter() {
    let error = parse("jn 3:37").expect_err("John 3 has 36 verses");
    assert!(error.to_string().contains("no verse"), "{error:#}");
    assert!(parse("ps 117:3").is_err());
    assert!(parse("jn 3:0").is_err());
}

#[test]
fn rejects_a_garbled_reference() {
    for input in [
        "",
        "   ",
        "jn",
        "3:16",
        "1:2",
        "jn 3:16-17",
        "jn 3:16x",
        "xq 4",
    ] {
        assert!(parse(input).is_err(), "{input:?} should not resolve");
    }
}

#[test]
fn formats_back_the_way_a_reader_reads_it() {
    assert_eq!(resolved("jn 3:16").to_string(), "John 3:16");
    assert_eq!(resolved("ps 23").to_string(), "Psalms 23");
    assert_eq!(resolved("1co13").to_string(), "1 Corinthians 13");

    let parsed: Reference = "rev 22:21".parse().expect("FromStr");
    assert_eq!(parsed, resolved("rev 22:21"));
}

#[test]
fn refuses_a_reference_built_out_of_range() {
    assert!(Reference::at_verse(42, 3, 37).is_err());
    assert!(Reference::at_chapter(42, 22).is_err());
    assert!(Reference::at_verse(42, 3, 16).is_ok());
    assert!(Reference::at_chapter(42, 21).is_ok());
}
