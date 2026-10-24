// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The scrolling text surface, its chapter cache, and the line layout.
//!
//! The reader lays out one chapter at a time and caches the result, so scrolling
//! from Genesis to Revelation costs the same per frame as scrolling inside one
//! psalm. The scroll position is an anchor of a chapter plus an offset inside
//! that chapter, never a position in one enormous document, and only the
//! chapters that intersect the viewport are laid out.
//!
//! The layout is a plain data structure of lines and runs. It is built once per
//! chapter per size and mode, and it is what the tests assert on, so the
//! typesetting rules are checked without a display.

use std::collections::HashMap;

use bible_core::{MarkKind, Reference, Translation, Verse, books};

use crate::theme::Typography;
use crate::typography::{self, Measure, Run, TextRole};

/// What a line is for, which the painter and the tests both read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// The chapter title at the top of a chapter.
    Title,
    /// A section heading.
    Heading,
    /// A line of running prose, including a paragraph's first line.
    Prose,
    /// A line that keeps a poetry break.
    Poetry,
}

/// One run of text placed on a line.
#[derive(Clone, Debug, PartialEq)]
pub struct LaidRun {
    /// The characters.
    pub text: String,
    /// How the characters are set.
    pub role: TextRole,
    /// Left edge in points from the chapter's left edge.
    pub x: f32,
    /// Width in points.
    pub width: f32,
    /// Top edge in points from the chapter's top edge.
    pub top: f32,
    /// The baseline this run sits on, shared with every run on its line.
    pub baseline: f32,
    /// The verse the run's characters belong to, absent for a title, a heading,
    /// or a gap between two verses.
    ///
    /// A line that opens with one verse and continues into another carries runs
    /// of both, which is what lets the reading surface paint behind one verse's
    /// words without spilling onto its neighbours.
    pub verse: Option<Reference>,
}

/// One laid-out line of a chapter.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// What the line is for.
    pub kind: LineKind,
    /// The verse the line first draws, absent for a title or a heading.
    pub verse: Option<Reference>,
    /// Top edge in points from the chapter's top edge.
    pub top: f32,
    /// Line height in points.
    pub height: f32,
    /// The baseline shared by every run on the line.
    pub baseline: f32,
    /// The left edge of the line's first run, which is the paragraph or poetry indent.
    pub indent: f32,
    /// The placed runs, in reading order.
    pub runs: Vec<LaidRun>,
}

impl Line {
    /// Every character on the line, in reading order.
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

/// One chapter laid out into lines.
#[derive(Clone, Debug, PartialEq)]
pub struct ChapterLayout {
    /// Canonical book index.
    pub book: u8,
    /// 1-based chapter number.
    pub chapter: u8,
    /// The width the chapter was laid out at, in points.
    pub width: f32,
    /// Total height in points.
    pub height: f32,
    /// The lines, top to bottom.
    pub lines: Vec<Line>,
}

impl ChapterLayout {
    /// Every character in the layout, line by line.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(&line.text());
            out.push('\n');
        }
        out
    }

    /// The top of the first line that draws a verse.
    ///
    /// The search runs over the placed runs, because a verse that continues a
    /// paragraph opens no line of its own and still has a place to land.
    pub fn top_of_verse(&self, verse: u8) -> Option<f32> {
        self.lines
            .iter()
            .find(|line| {
                line.verse
                    .is_some_and(|reference| reference.verse == Some(verse))
                    || line.runs.iter().any(|run| {
                        run.verse
                            .is_some_and(|reference| reference.verse == Some(verse))
                    })
            })
            .map(|line| line.top)
    }

    /// The verse drawing a point measured from the chapter's top edge.
    pub fn verse_at(&self, y: f32) -> Option<Reference> {
        self.lines
            .iter()
            .find(|line| y >= line.top && y < line.top + line.height)
            .and_then(|line| line.verse)
    }
}

/// The identity of a layout, so a resize or a font change invalidates the cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayoutKey {
    width: u32,
    size: u32,
    line_height: u32,
    measure_em: u32,
    paragraph_mode: bool,
    verse_numbers: bool,
    headings: bool,
}

impl LayoutKey {
    fn of(ty: &Typography, width: f32) -> Self {
        Self {
            width: width.to_bits(),
            size: ty.size.to_bits(),
            line_height: ty.line_height.to_bits(),
            measure_em: ty.measure_em.to_bits(),
            paragraph_mode: ty.paragraph_mode,
            verse_numbers: ty.verse_numbers,
            headings: ty.headings,
        }
    }
}

/// A chapter placed on screen at a top offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Canonical book index.
    pub book: u8,
    /// 1-based chapter number.
    pub chapter: u8,
    /// Top of the chapter in points from the viewport's top edge.
    pub top: f32,
    /// Height of the chapter in points.
    pub height: f32,
}

/// The chapters intersecting one viewport, in reading order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The chapters to paint, top to bottom.
    pub placements: Vec<Placement>,
}

impl Frame {
    /// The chapter holding a point measured from the viewport's top edge.
    pub fn placement_at(&self, y: f32) -> Option<&Placement> {
        self.placements
            .iter()
            .find(|placement| y >= placement.top && y < placement.top + placement.height)
    }
}

/// The reading surface.
///
/// It owns the anchor and the chapter cache, and the caller owns the translation
/// and the metrics. Nothing here paints, so the same surface feeds the window and
/// the layout tests.
#[derive(Debug)]
pub struct Reader {
    anchor: Reference,
    offset: f32,
    cache: HashMap<(u8, u8), ChapterLayout>,
    key: Option<LayoutKey>,
    layout_calls: u64,
    /// The verse the next frame should land on, set by a jump whose reference
    /// names a verse and cleared once the offset has been placed there.
    land_on: Option<u8>,
}

impl Reader {
    /// Opens the surface at a chapter, which is where a new window starts.
    pub fn new(reference: Reference) -> Self {
        Self {
            anchor: reference.chapter_only(),
            offset: 0.0,
            cache: HashMap::new(),
            key: None,
            layout_calls: 0,
            land_on: None,
        }
    }

    /// The chapter the anchor sits in.
    pub fn anchor(&self) -> Reference {
        self.anchor
    }

    /// The offset of the viewport's top edge inside the anchor chapter.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Moves the surface to a chapter and puts its top at the viewport's top.
    /// Moves the surface to a place.
    ///
    /// A reference that names a verse makes the next frame land on that verse,
    /// so a jump shows the verse it was given instead of the top of its chapter.
    pub fn set_position(&mut self, reference: Reference) {
        self.anchor = reference.chapter_only();
        self.offset = 0.0;
        self.land_on = reference.verse;
    }

    /// How many chapter layouts have been computed since the last invalidation,
    /// which is what the cache test counts.
    pub fn layout_calls(&self) -> u64 {
        self.layout_calls
    }

    /// The cached layout of one chapter, if the viewport has needed it.
    pub fn layout(&self, book: u8, chapter: u8) -> Option<&ChapterLayout> {
        self.cache.get(&(book, chapter))
    }

    /// Scrolls the surface by a number of points, positive meaning down the page.
    pub fn scroll_by(&mut self, delta: f32) {
        self.offset += delta;
    }

    /// The chapters to paint for a viewport of a given height.
    ///
    /// This lays out the anchor chapter and any later chapter that reaches into
    /// the viewport, and steps the anchor across chapter boundaries so the
    /// offset never leaves the current chapter.
    pub fn frame(
        &mut self,
        translation: &Translation,
        ty: &Typography,
        width: f32,
        viewport: f32,
        measure: &dyn Measure,
    ) -> Frame {
        self.refresh(translation, ty, width, measure);
        self.normalize(translation, ty, width, measure);

        if let Some(verse) = self.land_on.take() {
            let top = self
                .ensure(
                    translation,
                    ty,
                    width,
                    measure,
                    self.anchor.book,
                    self.anchor.chapter,
                )
                .top_of_verse(verse);
            if let Some(top) = top {
                self.offset = (top - ty.line_spacing() * 0.6).max(0.0);
            }
        }

        let mut placements = Vec::new();
        let mut book = self.anchor.book;
        let mut chapter = self.anchor.chapter;
        let mut top = -self.offset;
        while top < viewport {
            let height = self
                .ensure(translation, ty, width, measure, book, chapter)
                .height;
            placements.push(Placement {
                book,
                chapter,
                top,
                height,
            });
            top += height;
            match next_chapter(book, chapter) {
                Some((next_book, next_chapter)) => {
                    book = next_book;
                    chapter = next_chapter;
                }
                None => break,
            }
        }
        Frame { placements }
    }

    /// Recomputes the cache when the size, the mode, or the width changed, and
    /// re-anchors to the verse that was being read.
    fn refresh(
        &mut self,
        translation: &Translation,
        ty: &Typography,
        width: f32,
        measure: &dyn Measure,
    ) {
        let key = LayoutKey::of(ty, width);
        if self.key == Some(key) {
            return;
        }
        let verse = if self.key.is_some() {
            self.cache
                .get(&(self.anchor.book, self.anchor.chapter))
                .and_then(|layout| layout.verse_at(self.offset))
                .and_then(|reference| reference.verse)
        } else {
            None
        };
        self.cache.clear();
        self.layout_calls = 0;
        self.key = Some(key);
        let layout = layout_chapter(
            translation,
            self.anchor.book,
            self.anchor.chapter,
            ty,
            width,
            measure,
        );
        self.layout_calls += 1;
        if let Some(top) = verse.and_then(|verse| layout.top_of_verse(verse)) {
            self.offset = top;
        }
        self.cache.insert((layout.book, layout.chapter), layout);
    }

    /// Keeps the anchor inside its chapter by stepping whole chapters.
    fn normalize(
        &mut self,
        translation: &Translation,
        ty: &Typography,
        width: f32,
        measure: &dyn Measure,
    ) {
        while self.offset < 0.0 {
            let Some((book, chapter)) = previous_chapter(self.anchor.book, self.anchor.chapter)
            else {
                self.offset = 0.0;
                break;
            };
            let height = self
                .ensure(translation, ty, width, measure, book, chapter)
                .height;
            self.offset += height;
            self.anchor = Reference {
                book,
                chapter,
                verse: None,
            };
            if self.offset >= 0.0 {
                break;
            }
        }
        loop {
            let height = self
                .ensure(
                    translation,
                    ty,
                    width,
                    measure,
                    self.anchor.book,
                    self.anchor.chapter,
                )
                .height;
            if self.offset < height {
                break;
            }
            let Some((book, chapter)) = next_chapter(self.anchor.book, self.anchor.chapter) else {
                self.offset = (height - 1.0).max(0.0);
                break;
            };
            self.offset -= height;
            self.anchor = Reference {
                book,
                chapter,
                verse: None,
            };
        }
    }

    /// The layout of one chapter, computing and caching it on a miss.
    fn ensure(
        &mut self,
        translation: &Translation,
        ty: &Typography,
        width: f32,
        measure: &dyn Measure,
        book: u8,
        chapter: u8,
    ) -> &ChapterLayout {
        if !self.cache.contains_key(&(book, chapter)) {
            let layout = layout_chapter(translation, book, chapter, ty, width, measure);
            self.layout_calls += 1;
            self.cache.insert((book, chapter), layout);
        }
        self.cache
            .get(&(book, chapter))
            .expect("the chapter was just inserted")
    }
}

/// The next chapter in canonical order, or `None` at Revelation's last chapter.
fn next_chapter(book: u8, chapter: u8) -> Option<(u8, u8)> {
    let entry = books::book(book)?;
    if chapter < entry.chapter_count {
        return Some((book, chapter + 1));
    }
    books::book(book + 1)?;
    Some((book + 1, 1))
}

/// The previous chapter in canonical order, or `None` at Genesis' first chapter.
fn previous_chapter(book: u8, chapter: u8) -> Option<(u8, u8)> {
    if chapter > 1 {
        return Some((book, chapter - 1));
    }
    let previous = book.checked_sub(1)?;
    Some((previous, books::book(previous)?.chapter_count))
}

/// Lays out one chapter at a width.
///
/// The caller caches the result, so this runs once per chapter per size and
/// mode. Every verse is walked once, marks before text, and the measure caps the
/// line length at the typography's width.
pub fn layout_chapter(
    translation: &Translation,
    book: u8,
    chapter: u8,
    ty: &Typography,
    width: f32,
    measure: &dyn Measure,
) -> ChapterLayout {
    let width = width.max(80.0);
    let mut writer = LineWriter::new(ty, width, measure);
    let title = books::book(book).map_or_else(
        || format!("{book} {chapter}"),
        |entry| format!("{} {chapter}", entry.name),
    );
    writer.start(LineKind::Title, 0.0, None);
    writer.text(&title, TextRole::Title, TextRole::Title.size(ty) * 1.4);
    writer.flush();
    writer.space(ty.line_spacing() * 0.45);

    let Some(chapter_verses) = translation.chapter(book, chapter) else {
        return ChapterLayout {
            book,
            chapter,
            width,
            height: writer.y,
            lines: writer.lines,
        };
    };

    let gutter = ty.size * 1.9;
    let mut after_heading = false;
    for verse in chapter_verses {
        let marks = translation.verse_marks(verse);
        let reference = Reference {
            book,
            chapter,
            verse: Some(verse.number),
        };

        if ty.headings {
            for mark in marks.iter().filter(|mark| mark.kind == MarkKind::Heading) {
                let payload = translation.mark_payload(mark);
                if payload.trim().is_empty() {
                    continue;
                }
                writer.verse = None;
                writer.space(ty.line_spacing() * 0.5);
                writer.start(LineKind::Heading, 0.0, None);
                writer.text(payload, TextRole::Heading, 0.0);
                writer.flush();
                writer.space(ty.line_spacing() * 0.2);
                after_heading = true;
            }
        }

        let text = translation.verse_text(verse);
        if text.trim().is_empty() {
            continue;
        }
        writer.verse = Some(reference);

        let label = if ty.verse_numbers {
            Some(typography::verse_label(verse))
        } else {
            None
        };
        let paragraph = marks.iter().any(|mark| mark.kind == MarkKind::Paragraph);

        for (index, segment) in poetry_segments(translation, verse, text.len())
            .into_iter()
            .enumerate()
        {
            let runs = typography::verse_runs_in(translation, verse, segment.start, segment.end);
            if segment.stanza {
                writer.space(ty.line_spacing() * 0.5);
            }

            if segment.poetry {
                let indent = ty.size * 2.0 * segment.level as f32;
                writer.start(LineKind::Poetry, indent, Some(reference));
                if index == 0 {
                    writer.number(label.as_deref());
                }
                writer.runs(&runs, indent + ty.size * 1.4);
                writer.flush();
                after_heading = false;
                continue;
            }

            if index == 0 {
                if ty.paragraph_mode {
                    let fresh = paragraph || writer.is_idle();
                    if fresh {
                        let indent = if paragraph && !after_heading {
                            ty.size * 1.1
                        } else {
                            0.0
                        };
                        writer.start(LineKind::Prose, indent, Some(reference));
                    } else {
                        writer.gap();
                    }
                    writer.number(label.as_deref());
                    writer.runs(&runs, 0.0);
                } else {
                    writer.start(LineKind::Prose, gutter, Some(reference));
                    writer.number_in_margin(label.as_deref(), 0.0);
                    writer.runs(&runs, gutter);
                    writer.flush();
                }
                after_heading = false;
            } else {
                if writer.is_idle() {
                    writer.start(LineKind::Prose, 0.0, Some(reference));
                }
                writer.runs(&runs, 0.0);
            }
        }

        if !ty.paragraph_mode {
            writer.flush();
        } else if !writer.is_idle() {
            writer.gap();
        }
    }
    writer.flush();

    ChapterLayout {
        book,
        chapter,
        width,
        height: writer.y,
        lines: writer.lines,
    }
}

/// One run of verse text between two poetry breaks.
struct Segment {
    start: usize,
    end: usize,
    level: u8,
    poetry: bool,
    stanza: bool,
}

/// Splits a verse at its poetry and stanza marks.
///
/// A stanza break adds space before the segment it starts, and a poetry indent
/// carries the level the segment is set in. A prose verse comes back as one
/// segment at level zero.
fn poetry_segments(translation: &Translation, verse: &Verse, len: usize) -> Vec<Segment> {
    let mut cuts: Vec<(usize, u8, bool)> = vec![(0, 0, false), (len, 0, false)];
    let mut stanza_at: Vec<usize> = Vec::new();
    for mark in translation.verse_marks(verse) {
        let offset = (mark.offset as usize).min(len);
        match mark.kind {
            MarkKind::PoetryLine => cuts.push((offset, 0, true)),
            MarkKind::PoetryIndent => {
                if let Some(entry) = cuts.iter_mut().find(|entry| entry.0 == offset) {
                    entry.1 = entry.1.max(mark.level);
                    entry.2 = true;
                } else {
                    cuts.push((offset, mark.level, true));
                }
            }
            MarkKind::StanzaBreak => stanza_at.push(offset),
            _ => {}
        }
    }
    cuts.sort_by_key(|entry| entry.0);
    let mut merged: Vec<(usize, u8, bool)> = Vec::new();
    for cut in cuts {
        if let Some(last) = merged.last_mut()
            && last.0 == cut.0
        {
            last.1 = last.1.max(cut.1);
            last.2 = last.2 || cut.2;
            continue;
        }
        merged.push(cut);
    }

    let mut segments = Vec::new();
    for pair in merged.windows(2) {
        let (start, end) = (pair[0].0, pair[1].0);
        if start == end {
            continue;
        }
        let (_, level, poetry) = pair[0];
        segments.push(Segment {
            start,
            end,
            level,
            poetry,
            stanza: stanza_at.contains(&start),
        });
    }
    segments
}

/// Builds the lines of one chapter, top to bottom.
struct LineWriter<'a> {
    ty: &'a Typography,
    width: f32,
    measure: &'a dyn Measure,
    lines: Vec<Line>,
    y: f32,
    draft: Option<Draft>,
    /// The verse whose characters are being added, which every run records so
    /// the reading surface can tell the verses of a shared line apart.
    verse: Option<Reference>,
}

/// A line being filled.
struct Draft {
    kind: LineKind,
    indent: f32,
    verse: Option<Reference>,
    runs: Vec<DraftRun>,
    x: f32,
}

/// A run being filled, before its position is fixed.
struct DraftRun {
    text: String,
    role: TextRole,
    x: f32,
    width: f32,
    verse: Option<Reference>,
}

impl<'a> LineWriter<'a> {
    fn new(ty: &'a Typography, width: f32, measure: &'a dyn Measure) -> Self {
        Self {
            ty,
            width,
            measure,
            lines: Vec::new(),
            y: 0.0,
            draft: None,
            verse: None,
        }
    }

    /// Whether no line is open, which is where a new verse may start cleanly.
    fn is_idle(&self) -> bool {
        self.draft.is_none()
    }

    /// Starts a line, finishing the open one.
    fn start(&mut self, kind: LineKind, indent: f32, verse: Option<Reference>) {
        self.flush();
        self.draft = Some(Draft {
            kind,
            indent,
            verse,
            runs: Vec::new(),
            x: indent,
        });
    }

    /// Adds vertical space, which is how a stanza break separates two stanzas.
    fn space(&mut self, gap: f32) {
        self.flush();
        self.y += gap;
    }

    /// Sets the verse number superscript at the start of the open line.
    fn number(&mut self, label: Option<&str>) {
        let Some(label) = label else { return };
        let role = TextRole::VerseNumber;
        let width = self.measure.width(label, role);
        self.push_run(label.to_owned(), role, self.cursor(), width);
    }

    /// Sets the verse number in the margin, away from the text column.
    fn number_in_margin(&mut self, label: Option<&str>, x: f32) {
        let Some(label) = label else { return };
        let role = TextRole::VerseNumber;
        let width = self.measure.width(label, role);
        self.push_run(label.to_owned(), role, x, width);
    }

    /// The x the next inline run starts at.
    fn cursor(&self) -> f32 {
        self.draft.as_ref().map_or(0.0, |draft| draft.x)
    }

    /// Appends a space between two verses flowing in the same paragraph.
    fn gap(&mut self) {
        let x = self.cursor();
        let width = self.measure.width(" ", TextRole::Body);
        self.push_run(" ".to_owned(), TextRole::Body, x, width);
    }

    /// Appends text in one role, wrapping at the measure.
    ///
    /// A run that arrives with no open line starts one in the continuation
    /// indent, which is what happens after a right-aligned Selah closes a line.
    fn text(&mut self, text: &str, role: TextRole, continuation: f32) {
        if self.is_idle() && !text.is_empty() {
            self.start(LineKind::Prose, continuation, None);
        }
        for token in text.split_inclusive(char::is_whitespace) {
            let trimmed = token.trim_end();
            if trimmed.is_empty() && self.cursor() == self.draft_indent() {
                continue;
            }
            let width = self.measure.width(token, role);
            if !token.trim().is_empty() && self.cursor() + width > self.width && !self.line_empty()
            {
                self.wrap(continuation);
            }
            let x = self.cursor();
            self.push_run(token.to_owned(), role, x, width);
        }
    }

    /// The indent of the open line.
    fn draft_indent(&self) -> f32 {
        self.draft.as_ref().map_or(0.0, |draft| draft.indent)
    }

    /// Whether the open line holds no runs yet.
    fn line_empty(&self) -> bool {
        self.draft
            .as_ref()
            .is_none_or(|draft| draft.runs.is_empty())
    }

    /// Appends styled runs, wrapping at the measure in the continuation indent.
    fn runs(&mut self, runs: &[Run], continuation: f32) {
        for run in runs {
            if run.role == TextRole::Selah {
                self.right(run);
                continue;
            }
            self.text(&run.text, run.role, continuation);
        }
    }

    /// Sets a Selah run at the right edge of the measure.
    fn right(&mut self, run: &Run) {
        self.flush();
        self.start(LineKind::Poetry, 0.0, None);
        let width = self.measure.width(&run.text, run.role);
        let x = (self.width - width).max(0.0);
        self.push_run(run.text.clone(), run.role, x, width);
        self.flush();
    }

    /// Ends the open line and starts a fresh one in the continuation indent.
    fn wrap(&mut self, continuation: f32) {
        let Some(draft) = self.draft.take() else {
            return;
        };
        let kind = draft.kind;
        let verse = draft.verse;
        let indent = draft.indent;
        self.commit(draft);
        self.draft = Some(Draft {
            kind,
            indent: indent.min(continuation),
            verse,
            runs: Vec::new(),
            x: continuation,
        });
    }

    /// Adds one run to the open line, merging with the previous run of the same
    /// role and the same verse so a line holds few runs.
    ///
    /// The verse takes part in the merge because two verses flowing in one
    /// paragraph share a role and an edge, and a run may not span them.
    fn push_run(&mut self, text: String, role: TextRole, x: f32, width: f32) {
        let verse = self.verse;
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        if let Some(last) = draft.runs.last_mut()
            && last.role == role
            && last.verse == verse
            && (last.x + last.width - x).abs() < 0.01
        {
            last.text.push_str(&text);
            last.width += width;
        } else {
            draft.runs.push(DraftRun {
                text,
                role,
                x,
                width,
                verse,
            });
        }
        draft.x = x + width;
    }

    /// Finishes the open line and appends it if it holds text.
    fn flush(&mut self) {
        if let Some(draft) = self.draft.take() {
            self.commit(draft);
        }
    }

    /// Turns a draft into a placed line at the current cursor.
    fn commit(&mut self, draft: Draft) {
        if draft.runs.is_empty() {
            return;
        }
        let ty = self.ty;
        let max_size = draft
            .runs
            .iter()
            .map(|run| run.role.size(ty))
            .fold(0.0_f32, f32::max);
        let height = ty.line_spacing().max(max_size * 1.28);
        let baseline = self.y + height - max_size * 0.22;
        let runs: Vec<LaidRun> = draft
            .runs
            .into_iter()
            .map(|run| LaidRun {
                text: run.text,
                role: run.role,
                x: run.x,
                width: run.width,
                top: baseline - run.role.ascent(ty) - run.role.superscript_lift(ty),
                baseline,
                verse: run.verse,
            })
            .collect();
        self.lines.push(Line {
            kind: draft.kind,
            verse: draft.verse,
            top: self.y,
            height,
            baseline,
            indent: draft.indent,
            runs,
        });
        self.y += height;
    }
}
