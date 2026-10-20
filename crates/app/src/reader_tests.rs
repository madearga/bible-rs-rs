// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reader tests. They lay out synthetic text that is not scripture and
//! assert on the layout and the selection, so the typography rules are checked
//! without a window.

use std::time::Instant;

use bible_core::{
    MarkKind, Reference, SpanStyle, Translation, TranslationBuilder, TranslationMeta, books,
};

use crate::reader::{ChapterLayout, LineKind, Reader, layout_chapter};
use crate::selection::{Selection, Step};
use crate::theme::Typography;
use crate::typography::{MonoMeasure, TextRole};

/// A prose chapter with several verses, used for the paragraph mode comparison.
const PROSE: (u8, u8) = (42, 3);

/// A chapter carrying a heading, a divine name, poetry, and a stanza break.
const MARKED: (u8, u8) = (19, 23);

/// The body text of one synthetic verse, generated rather than quoted.
fn synthetic_text(book: u8, chapter: u8, verse: u8) -> String {
    format!("alpha{book} beta{chapter} gamma{verse} delta")
}

/// Builds a whole synthetic translation, with the marked chapter carrying one
/// mark of each kind the reader understands.
fn synthetic_bible() -> Translation {
    let mut builder = TranslationBuilder::new(TranslationMeta::new("synthetic", "Synthetic"));
    for book in books::books() {
        for chapter in 1..=book.chapter_count {
            let count = books::verses_in_chapter(book.index, chapter).unwrap_or(0);
            for number in 1..=count as u8 {
                let text = synthetic_text(book.index, chapter, number);
                builder
                    .push_verse(number, &text)
                    .expect("the grid accepts it");
                if (book.index, chapter) == MARKED {
                    add_marks(&mut builder, number, &text);
                }
            }
        }
    }
    builder.finish().expect("the synthetic grid is complete")
}

/// Attaches the marks and spans the marked chapter needs.
fn add_marks(builder: &mut TranslationBuilder, number: u8, text: &str) {
    match number {
        1 => {
            builder
                .mark(0, MarkKind::Heading, 0, "A synthetic section heading")
                .expect("a heading fits the verse");
            builder
                .mark(0, MarkKind::Paragraph, 0, "")
                .expect("a paragraph mark fits the verse");
            builder
                .span(SpanStyle::DivineName, 0, 5)
                .expect("the first word is long enough");
        }
        2 => {
            builder
                .mark(0, MarkKind::Paragraph, 0, "")
                .expect("a paragraph mark fits the verse");
            builder
                .mark(0, MarkKind::PoetryLine, 0, "")
                .expect("a poetry mark fits the verse");
            builder
                .mark(0, MarkKind::PoetryIndent, 2, "")
                .expect("a poetry indent fits the verse");
        }
        3 => {
            builder
                .mark(0, MarkKind::StanzaBreak, 0, "")
                .expect("a stanza break fits the verse");
            builder
                .mark(0, MarkKind::PoetryLine, 0, "")
                .expect("a poetry mark fits the verse");
            builder
                .mark(0, MarkKind::PoetryIndent, 1, "")
                .expect("a poetry indent fits the verse");
        }
        4 => {
            let end = text.len().min(u16::MAX as usize) as u16;
            builder
                .mark(end, MarkKind::PoetryLine, 0, "")
                .expect("a poetry mark fits the verse");
            builder
                .mark(end, MarkKind::PoetryIndent, 1, "")
                .expect("a poetry indent fits the verse");
        }
        _ => {}
    }
}

/// The metrics and measure the layout tests use.
fn typography(paragraph_mode: bool) -> Typography {
    Typography {
        paragraph_mode,
        ..Typography::default()
    }
}

/// Lays out one chapter of the synthetic translation.
fn layout(translation: &Translation, ty: &Typography, chapter: (u8, u8)) -> ChapterLayout {
    let measure = MonoMeasure::new(*ty);
    layout_chapter(
        translation,
        chapter.0,
        chapter.1,
        ty,
        ty.measure(),
        &measure,
    )
}

#[test]
fn layout_has_a_positive_height_and_carries_the_verse_text() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let layout = layout(&translation, &ty, PROSE);
    assert!(layout.height > 0.0, "the chapter has no height");
    let rendered = layout.text();
    for verse in translation
        .chapter(PROSE.0, PROSE.1)
        .expect("the chapter exists")
    {
        if verse.number > 3 {
            break;
        }
        for word in synthetic_text(PROSE.0, PROSE.1, verse.number).split_whitespace() {
            assert!(
                rendered.contains(word),
                "the word {word} of verse {} is missing from the layout",
                verse.number
            );
        }
    }
}

#[test]
fn verse_per_line_produces_more_lines_than_paragraph_mode() {
    let translation = synthetic_bible();
    let paragraph = layout(&translation, &typography(true), PROSE);
    let verse_per_line = layout(&translation, &typography(false), PROSE);
    assert!(
        verse_per_line.lines.len() > paragraph.lines.len(),
        "verse-per-line has {} lines and paragraph has {}",
        verse_per_line.lines.len(),
        paragraph.lines.len()
    );
}

#[test]
fn poetry_is_indented_and_a_stanza_break_adds_space() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let layout = layout(&translation, &ty, MARKED);

    let prose = layout
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Prose && line.text().contains("beta23"))
        .expect("the prose verse is laid out");
    let poetry = layout
        .lines
        .iter()
        .find(|line| {
            line.kind == LineKind::Poetry && line.verse.is_some_and(|r| r.verse == Some(2))
        })
        .expect("the poetry verse is laid out");
    assert!(
        poetry.indent > prose.indent,
        "poetry indent {} is not more than prose indent {}",
        poetry.indent,
        prose.indent
    );

    let stanza = layout
        .lines
        .iter()
        .find(|line| {
            line.verse
                .is_some_and(|reference| reference.verse == Some(3))
        })
        .expect("the stanza verse is laid out");
    let index = layout
        .lines
        .iter()
        .position(|line| std::ptr::eq(line, stanza))
        .expect("the stanza line is in the layout");
    let previous = &layout.lines[index - 1];
    let gap = stanza.top - (previous.top + previous.height);
    assert!(
        gap >= ty.line_spacing() * 0.4,
        "the stanza break adds {gap} points, not a stanza's space"
    );
}

#[test]
fn a_heading_is_set_in_its_own_style() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let layout = layout(&translation, &ty, MARKED);
    let heading = layout
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Heading)
        .expect("the heading is laid out");
    assert!(!heading.text().trim().is_empty());
    let heading_role = heading.runs[0].role;
    assert_eq!(heading_role, TextRole::Heading);
    assert_ne!(heading_role, TextRole::Body);
    assert_ne!(heading_role.size(&ty), TextRole::Body.size(&ty));
    assert!(heading_role.weight() > TextRole::Body.weight());

    let body = layout
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Prose && !line.runs.is_empty())
        .expect("the body text is laid out");
    assert!(
        body.runs.iter().any(|run| run.role == TextRole::Body),
        "the body line holds no body text"
    );
}

#[test]
fn the_divine_name_keeps_the_body_baseline() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let layout = layout(&translation, &ty, MARKED);
    let line = layout
        .lines
        .iter()
        .find(|line| line.runs.iter().any(|run| run.role == TextRole::DivineName))
        .expect("the divine name is laid out");
    let divine = line
        .runs
        .iter()
        .find(|run| run.role == TextRole::DivineName)
        .expect("the divine name run");
    let body = line
        .runs
        .iter()
        .find(|run| run.role == TextRole::Body)
        .expect("the body run");
    assert_eq!(divine.text, "ALPHA", "the divine name is not capitalised");
    assert!(
        (divine.baseline - body.baseline).abs() < f32::EPSILON,
        "the divine name sits on baseline {} and the body on {}",
        divine.baseline,
        body.baseline
    );
    assert!(
        divine.top > body.top,
        "the smaller capitals start at the same top as the body"
    );
    assert!(
        TextRole::DivineName.size(&ty) < TextRole::Body.size(&ty),
        "the divine name is not set smaller than the body"
    );
}

#[test]
fn a_chapter_is_laid_out_once_and_then_read_from_the_cache() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let measure = MonoMeasure::new(ty);
    let mut reader = Reader::new(Reference::new(PROSE.0, PROSE.1, None).expect("a chapter"));
    let first = reader.frame(&translation, &ty, ty.measure(), 120.0, &measure);
    assert!(!first.placements.is_empty());
    let calls = reader.layout_calls();
    let second = reader.frame(&translation, &ty, ty.measure(), 120.0, &measure);
    assert_eq!(first, second);
    assert_eq!(
        reader.layout_calls(),
        calls,
        "the second frame recomputed a chapter layout"
    );
    assert_eq!(calls, 1, "one chapter was needed but {calls} were laid out");
}

#[test]
fn a_resize_re_anchors_to_the_verse_being_read() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let measure = MonoMeasure::new(ty);
    let mut reader = Reader::new(Reference::new(PROSE.0, PROSE.1, None).expect("a chapter"));
    reader.scroll_by(ty.line_spacing() * 4.0);
    reader.frame(&translation, &ty, ty.measure(), 200.0, &measure);
    let offset = reader.offset();
    assert!(offset > 0.0, "the scroll did not move the anchor");
    reader.frame(&translation, &ty, ty.measure() * 0.8, 200.0, &measure);
    assert!(
        reader.offset() > 0.0,
        "the re-anchor dropped the reading position"
    );
}

#[test]
fn scrolling_steps_the_anchor_into_the_next_chapter() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let measure = MonoMeasure::new(ty);
    let mut reader = Reader::new(Reference::new(PROSE.0, PROSE.1, None).expect("a chapter"));
    reader.frame(&translation, &ty, ty.measure(), 200.0, &measure);
    let height = reader
        .layout(PROSE.0, PROSE.1)
        .expect("the chapter is cached")
        .height;
    reader.scroll_by(height + 20.0);
    reader.frame(&translation, &ty, ty.measure(), 200.0, &measure);
    assert_eq!(reader.anchor().chapter, PROSE.1 + 1);
}

#[test]
fn a_click_and_a_shift_click_build_a_range() {
    let chapter = synthetic_bible()
        .chapter(PROSE.0, PROSE.1)
        .expect("the chapter exists")
        .to_vec();
    let mut selection = Selection::new();
    assert!(selection.is_empty());

    let first = Reference::new(PROSE.0, PROSE.1, Some(chapter[4].number)).expect("a verse");
    selection.select(first);
    assert_eq!(selection.start(), Some(first));
    assert_eq!(selection.end(), Some(first));

    let far = Reference::new(PROSE.0, PROSE.1, Some(chapter[7].number)).expect("a verse");
    selection.extend_to(far);
    assert_eq!(selection.start(), Some(first));
    assert_eq!(selection.end(), Some(far));
    assert!(selection.contains(first));
    assert!(selection.contains(far));
}

#[test]
fn the_keys_move_one_verse_and_stop_at_the_chapter_boundary() {
    let translation = synthetic_bible();
    let chapter = translation
        .chapter(PROSE.0, PROSE.1)
        .expect("the chapter exists");
    let ty = typography(true);
    let measure = MonoMeasure::new(ty);
    let mut reader = Reader::new(Reference::new(PROSE.0, PROSE.1, None).expect("a chapter"));
    reader.frame(&translation, &ty, ty.measure(), 200.0, &measure);

    let mut selection = Selection::new();
    let start = Reference::new(PROSE.0, PROSE.1, Some(chapter[0].number)).expect("a verse");
    selection.select(start);
    assert!(
        !selection.step(Step::Previous, chapter),
        "the selection moved above the first verse"
    );
    assert_eq!(selection.focus(), Some(start));

    assert!(selection.step(Step::Next, chapter));
    assert_eq!(
        selection.focus().and_then(|reference| reference.verse),
        Some(2)
    );

    let last = *chapter.last().expect("the chapter has verses");
    selection.select(Reference {
        book: PROSE.0,
        chapter: PROSE.1,
        verse: Some(last.number),
    });
    assert!(
        !selection.step(Step::Next, chapter),
        "the selection moved past the last verse"
    );
    assert_eq!(
        selection.focus().and_then(|reference| reference.verse),
        Some(last.number)
    );

    selection.clear();
    assert!(selection.is_empty());
    assert_eq!(selection.range(), None);
    assert!(!selection.step(Step::Next, chapter));
}

#[test]
fn scroll_benchmark() {
    let translation = synthetic_bible();
    let ty = typography(true);
    let measure = MonoMeasure::new(ty);
    let mut reader = Reader::new(Reference::new(0, 1, None).expect("Genesis 1"));

    let frames = 2400;
    let viewport = 800.0;
    let mut placements = 0usize;
    let started = Instant::now();
    for _ in 0..frames {
        let frame = reader.frame(&translation, &ty, ty.measure(), viewport, &measure);
        placements += frame.placements.len();
        reader.scroll_by(viewport);
    }
    let elapsed = started.elapsed();
    let per_frame = elapsed.as_secs_f64() * 1_000_000.0 / f64::from(frames);
    println!(
        "scroll_benchmark: {per_frame:.2} microseconds of CPU per frame over {frames} frames, \
         {} chapters placed, {} chapter layouts",
        placements,
        reader.layout_calls()
    );
    assert_eq!(
        reader.anchor().book,
        books::BOOK_COUNT as u8 - 1,
        "the scroll did not reach the last book of the Bible"
    );
    assert!(
        per_frame < 1000.0,
        "scrolling costs {per_frame:.2} microseconds per frame, over the 1000 microsecond budget"
    );
}
