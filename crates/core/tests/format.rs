// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

use std::time::Instant;

use bible_core::books;
use bible_core::format::{self, MAGIC};
use bible_core::model::{MarkKind, SpanStyle, Translation, TranslationBuilder, TranslationMeta};

/// Builds a full-grid translation from generated text, with marks and spans.
fn synthetic_translation(repeat: usize) -> Translation {
    let mut meta = TranslationMeta::new("synth", "Synthetic Witness");
    meta.abbreviation = "SYN".to_owned();
    meta.language = "en".to_owned();
    meta.licence = "Public domain test words".to_owned();
    meta.provenance = "generated in a test".to_owned();
    meta.origin = "synthetic".to_owned();
    meta.book_titles = books::books()
        .iter()
        .map(|book| book.name.to_owned())
        .collect();
    meta.red_letters = true;
    meta.paragraphs = true;
    meta.headings = true;

    let filler = "lorem ipsum ".repeat(repeat);
    let mut builder = TranslationBuilder::new(meta);
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).expect("canonical chapter");
            for verse in 1..=count {
                let text =
                    format!("Witness {chapter}:{verse} λόγος ✓ δοκιμή {filler}synthetic tail.");
                builder
                    .push_verse(verse as u8, &text)
                    .expect("verse in order");
                if verse % 7 == 0 {
                    builder
                        .mark(0, MarkKind::Paragraph, 0, "")
                        .expect("paragraph mark");
                }
                if verse % 11 == 0 {
                    builder
                        .mark(
                            4,
                            MarkKind::Heading,
                            0,
                            &format!("Heading {chapter}:{verse}"),
                        )
                        .expect("heading mark");
                }
                if verse % 13 == 0 {
                    builder
                        .mark(9, MarkKind::PoetryLine, 0, "")
                        .expect("poetry mark");
                }
                builder
                    .span(SpanStyle::WordsOfChrist, 0, 7)
                    .expect("words of Christ span");
                if verse % 5 == 0 {
                    builder
                        .span(SpanStyle::DivineName, 8, 12)
                        .expect("divine name span");
                }
            }
        }
    }
    builder.finish().expect("the grid is complete")
}

#[test]
fn round_trip_preserves_every_field() {
    let original = synthetic_translation(1);
    let bytes = format::encode(&original).expect("encode");
    let decoded = format::decode(&bytes).expect("decode");

    assert_eq!(decoded.meta(), original.meta());
    assert_eq!(decoded.text(), original.text());
    assert_eq!(decoded.extra(), original.extra());
    assert_eq!(decoded.verses().len(), original.verses().len());
    for (index, (left, right)) in decoded
        .verses()
        .iter()
        .zip(original.verses().iter())
        .enumerate()
    {
        assert_eq!(left, right, "verse {index} differs");
    }
    assert_eq!(decoded.marks(), original.marks());
    assert_eq!(decoded.spans(), original.spans());
    assert_eq!(decoded, original);

    let first = decoded.verse_at(0, 1, 1).expect("Genesis 1:1 exists");
    assert_eq!(decoded.verse_text(first), original.verse_text(first));
    assert_eq!(decoded.verse_marks(first).len(), first.marks_len as usize);
    assert_eq!(decoded.verse_spans(first).len(), first.spans_len as usize);

    let heading = decoded
        .verses()
        .iter()
        .flat_map(|verse| decoded.verse_marks(verse))
        .find(|mark| mark.kind == MarkKind::Heading)
        .expect("the synthetic text has a heading");
    assert!(decoded.mark_payload(heading).starts_with("Heading "));
}

#[test]
fn encoded_bytes_start_with_the_magic() {
    let bytes = format::encode(&synthetic_translation(0)).expect("encode");
    assert_eq!(&bytes[..4], b"BIBL");
    assert_eq!(&bytes[..4], &MAGIC);
    assert_eq!(
        u16::from_le_bytes([bytes[4], bytes[5]]),
        format::FORMAT_VERSION
    );
}

#[test]
fn round_trip_keeps_multibyte_text_exact() {
    let original = synthetic_translation(0);
    let verse = original.verse_at(42, 3, 16).expect("John 3:16 exists");
    let text = original.verse_text(verse);
    assert!(text.contains('λ'));
    assert!(text.contains('✓'));

    let bytes = format::encode(&original).expect("encode");
    let decoded = format::decode(&bytes).expect("decode");
    let decoded_verse = decoded.verse_at(42, 3, 16).expect("John 3:16 survives");
    assert_eq!(
        decoded.verse_text(decoded_verse).as_bytes(),
        text.as_bytes()
    );
    assert_eq!(decoded.verse_text(decoded_verse), text);
    assert_eq!(
        decoded.reference_text(&bible_core::Reference::parse("jn 3:16").unwrap()),
        Some(text)
    );
}

#[test]
fn a_chapter_is_a_borrowed_slice_and_allocates_nothing_per_verse() {
    let translation = synthetic_translation(0);
    let chapter = translation.chapter(18, 119).expect("Psalm 119 exists");
    assert_eq!(chapter.len(), 176);
    let mut total = 0;
    let mut marks = 0;
    for verse in chapter {
        total += translation.verse_text(verse).len();
        marks += translation.verse_marks(verse).len();
    }
    assert!(total > 0);
    assert!(marks > 0);
}

#[test]
fn read_meta_never_touches_the_compressed_body() {
    let bytes = format::encode(&synthetic_translation(0)).expect("encode");
    let meta = format::read_meta(&bytes).expect("header reads");
    assert_eq!(meta.id, "synth");
    assert_eq!(meta.name, "Synthetic Witness");
}

#[test]
fn damaged_files_are_rejected_without_panicking() {
    let bytes = format::encode(&synthetic_translation(0)).expect("encode");
    assert!(format::decode(b"BIBL").is_err());
    assert!(format::decode(b"NOPE\x01\x00\x00\x00\x00\x00").is_err());

    let mut wrong_version = bytes.clone();
    wrong_version[4] = 9;
    assert!(format::decode(&wrong_version).is_err());

    let mut truncated = bytes;
    truncated.truncate(truncated.len() - 16);
    assert!(format::decode(&truncated).is_err());
}

#[test]
fn decode_benchmark() {
    let translation = synthetic_translation(6);
    let bytes = format::encode(&translation).expect("encode");
    let decoded = format::decode(&bytes).expect("the first decode works");
    assert_eq!(decoded, translation);

    let start = Instant::now();
    let again = format::decode(&bytes).expect("the timed decode works");
    let elapsed = start.elapsed();
    assert_eq!(again.verses().len(), 31102);

    println!(
        "decode_benchmark: decoded {} bytes ({} compressed) in {:.3} ms",
        translation.text().len(),
        bytes.len(),
        elapsed.as_secs_f64() * 1000.0
    );
    // The budget is a release-mode number; a debug build runs several times slower
    // without the code being wrong, so only assert it where it was measured.
    #[cfg(not(debug_assertions))]
    assert!(
        elapsed.as_secs_f64() < 0.050,
        "decode took {:.3} ms, the budget is 50 ms",
        elapsed.as_secs_f64() * 1000.0
    );
}
