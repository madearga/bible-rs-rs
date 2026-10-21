// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The parallel translations view.
//!
//! Parallel mode shows one chapter in two or three translations side by side.
//! It is a state over the reader rather than a replacement. The single-column
//! [`crate::reader::Reader`] is left untouched while the view is open, so
//! leaving the view returns the reader to the exact page and offset it had.
//!
//! There is one layout engine. Every column is laid out by
//! [`crate::reader::layout_chapter`] at its own width, and this module does no
//! typesetting of its own. What it adds is the shared scroll anchor and the
//! verse grid. Row numbers come from the canonical grid in [`bible_core::books`],
//! so every column carries the same verse numbers in the same order even where
//! one translation prints a verse differently. A row is as tall as the tallest
//! column's block for that verse, so the columns stay aligned at every verse
//! start, and a scroll in any column moves the one anchor every column paints
//! from.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bible_core::{Reference, Translation, books};

use crate::reader::{ChapterLayout, layout_chapter};
use crate::theme::Typography;
use crate::typography::Measure;

/// The fewest columns a parallel view may show.
pub const MIN_COLUMNS: usize = 2;

/// The most columns a parallel view may show.
///
/// Three is where a column still holds a readable measure at a normal window
/// width. A fourth would fall below the smallest measure the layout accepts.
pub const MAX_COLUMNS: usize = 3;

/// One translation on the page.
#[derive(Clone, Debug)]
pub struct Column {
    id: Arc<str>,
    translation: Arc<Translation>,
}

impl Column {
    /// The stable identifier a translation menu selects this column by.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The short label painted above the column, such as `KJV`.
    pub fn abbreviation(&self) -> &str {
        &self.translation.meta().abbreviation
    }

    /// The full display name, which a translation menu shows beside the label.
    pub fn name(&self) -> &str {
        &self.translation.meta().name
    }

    /// The translation this column reads, so the painter and the tests share it.
    pub fn translation(&self) -> &Translation {
        self.translation.as_ref()
    }
}

/// One verse's place on the shared grid.
///
/// The row carries the verse number the canonical grid gives it and one line
/// range per column. A range indexes into that column's [`ChapterLayout`], so a
/// row never copies text and a frame allocates only the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct ParallelRow {
    /// The canonical verse number, which every column shares.
    pub verse: u8,
    /// The reference a press on this row selects.
    pub reference: Reference,
    /// Top edge of the row in points from the chapter's top edge.
    pub top: f32,
    /// The row's height, set by the tallest column's block for this verse.
    pub height: f32,
    /// One line range per column, empty where a column has no such verse.
    pub cells: Vec<Range<usize>>,
}

/// One chapter laid out in every column and aligned on the canonical grid.
#[derive(Clone, Debug, PartialEq)]
pub struct ParallelGrid {
    /// Canonical book index.
    pub book: u8,
    /// 1-based chapter number.
    pub chapter: u8,
    /// The width each column was laid out at, in points.
    pub width: f32,
    /// The shared offset this grid is painted at, in points from the chapter's
    /// top edge. Every column paints from this one value.
    pub offset: f32,
    /// The total height of the grid in points.
    pub height: f32,
    /// The rows, top to bottom, one per canonical verse.
    pub rows: Vec<ParallelRow>,
    /// Each column's laid-out chapter, in column order.
    layouts: Vec<Arc<ChapterLayout>>,
}

impl ParallelGrid {
    /// Each column's laid-out chapter, in column order.
    pub fn layouts(&self) -> &[Arc<ChapterLayout>] {
        &self.layouts
    }

    /// The top edge of a row measured from the viewport's top edge.
    ///
    /// Every column paints from this one value, which is what keeps a scroll in
    /// any column moving all of them together.
    pub fn row_top(&self, row: usize, offset: f32) -> Option<f32> {
        self.rows.get(row).map(|row| row.top - offset)
    }

    /// The verse numbers the rows carry, top to bottom.
    pub fn verse_numbers(&self) -> Vec<u8> {
        self.rows.iter().map(|row| row.verse).collect()
    }
}

/// The typography and width a layout was built against, so a resize or a font
/// change rebuilds the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Style {
    width: u32,
    size: u32,
    line_height: u32,
    measure_em: u32,
    paragraph_mode: bool,
    verse_numbers: bool,
    headings: bool,
}

impl Style {
    fn of(ty: &Typography, width: f32) -> Self {
        Self {
            width: width.to_bits(),
            size: ty.size.to_bits(),
            line_height: ty.line_height.to_bits(),
            measure_em: ty.measure_em.to_bits(),
            // Parallel columns always start each verse on its own line, so the
            // reader's paragraph mode does not change a parallel layout and must
            // not invalidate its cache.
            paragraph_mode: false,
            verse_numbers: ty.verse_numbers,
            headings: ty.headings,
        }
    }
}

/// The typography one column is laid out with.
///
/// Paragraph mode flows several verses into one line, and a line records only
/// the verse it starts with. Rows are verse-aligned across columns, so parallel
/// columns lay every verse out from its own line. Paragraph breaks, poetry, and
/// headings still come from the marks, so only the flow of prose changes.
fn column_typography(ty: &Typography) -> Typography {
    Typography {
        paragraph_mode: false,
        ..*ty
    }
}

/// The shared scroll anchor, in the same shape the reader uses.
///
/// One anchor serves every column, so there is nothing per column to keep in
/// step. The chapter is chapter-only, and the offset is a distance in points
/// from that chapter's top edge. Parallel mode is entered with the reader's
/// anchor and offset, which is why leaving it is exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// The chapter the viewport's top edge sits in.
    pub reference: Reference,
    /// The offset of the viewport's top edge inside that chapter.
    pub offset: f32,
}

/// The parallel reading surface.
///
/// It owns the columns, the shared anchor, and the layout cache. The caller
/// keeps the selection and the study data, exactly as it does for the single
/// column reader, so both modes share one model.
#[derive(Debug)]
pub struct ParallelView {
    columns: Vec<Column>,
    anchor: Reference,
    offset: f32,
    style: Option<Style>,
    grid: Option<ParallelGrid>,
    cache: HashMap<(Arc<str>, u8, u8, Style), Arc<ChapterLayout>>,
}

impl ParallelView {
    /// Builds a view over a list of translations, starting from an anchor.
    ///
    /// Fewer than [`MIN_COLUMNS`] translations cannot fill the view and more
    /// than [`MAX_COLUMNS`] cannot stay readable, so the list is rejected in the
    /// first case and truncated in the second.
    pub fn new(
        translations: Vec<Arc<Translation>>,
        anchor: Reference,
        offset: f32,
    ) -> Option<Self> {
        if translations.len() < MIN_COLUMNS {
            return None;
        }
        let columns: Vec<Column> = translations
            .into_iter()
            .take(MAX_COLUMNS)
            .map(|translation| Column {
                id: Arc::from(translation.meta().id.as_str()),
                translation,
            })
            .collect();
        Some(Self {
            columns,
            anchor: anchor.chapter_only(),
            offset,
            style: None,
            grid: None,
            cache: HashMap::new(),
        })
    }

    /// The columns on the page, left to right.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// How many columns are on the page.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// The chapter the shared anchor sits in.
    pub fn anchor(&self) -> Reference {
        self.anchor
    }

    /// The offset of the viewport's top edge inside the anchor chapter.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Moves the shared anchor to a chapter and puts its top at the viewport's
    /// top, which is what a chapter jump does.
    pub fn set_reference(&mut self, reference: Reference) {
        self.anchor = reference.chapter_only();
        self.offset = 0.0;
        self.grid = None;
    }

    /// Scrolls every column by one shared distance, positive meaning down the
    /// page.
    pub fn scroll_by(&mut self, delta: f32) {
        self.offset += delta;
    }

    /// Reads a column in another translation, keeping every other column and
    /// the shared anchor where they were.
    ///
    /// The identifier is taken from the translation itself, so a caller cannot
    /// leave a column whose label disagrees with its text.
    pub fn set_translation(&mut self, index: usize, translation: Arc<Translation>) -> bool {
        let Some(column) = self.columns.get_mut(index) else {
            return false;
        };
        column.id = Arc::from(translation.meta().id.as_str());
        column.translation = translation;
        self.grid = None;
        true
    }

    /// Lays out the anchor chapter in every column and returns the aligned grid.
    ///
    /// The grid is cached until the size, the mode, the width, or a column's
    /// translation changes, so scrolling and painting cost the same per frame as
    /// the single column reader.
    pub fn frame(
        &mut self,
        ty: &Typography,
        column_width: f32,
        measure: &dyn Measure,
    ) -> &ParallelGrid {
        let width = column_width.max(80.0);
        let style = Style::of(ty, width);
        if self.style != Some(style) {
            self.cache.clear();
            self.style = Some(style);
            self.grid = None;
        }
        self.normalize(ty, width, measure);
        let stale = self.grid.as_ref().is_none_or(|grid| {
            grid.book != self.anchor.book || grid.chapter != self.anchor.chapter
        });
        if stale {
            self.grid = Some(self.build(ty, width, measure, self.anchor.book, self.anchor.chapter));
        }
        let offset = self.offset;
        let grid = self.grid.as_mut().expect("the grid was just built");
        grid.offset = offset;
        grid
    }

    /// Keeps the anchor inside its chapter by stepping whole chapters, so the
    /// columns scroll continuously from Genesis to Revelation.
    fn normalize(&mut self, ty: &Typography, width: f32, measure: &dyn Measure) {
        while self.offset < 0.0 {
            let Some((book, chapter)) = previous_chapter(self.anchor.book, self.anchor.chapter)
            else {
                self.offset = 0.0;
                break;
            };
            let height = self.chapter_height(ty, width, measure, book, chapter);
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
            let height =
                self.chapter_height(ty, width, measure, self.anchor.book, self.anchor.chapter);
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

    /// The height of one chapter's grid, which is what a chapter step needs.
    fn chapter_height(
        &mut self,
        ty: &Typography,
        width: f32,
        measure: &dyn Measure,
        book: u8,
        chapter: u8,
    ) -> f32 {
        self.build(ty, width, measure, book, chapter).height
    }

    /// Lays out one chapter in every column and aligns it on the canonical grid.
    fn build(
        &mut self,
        ty: &Typography,
        width: f32,
        measure: &dyn Measure,
        book: u8,
        chapter: u8,
    ) -> ParallelGrid {
        let style = self.style.expect("a style is set before the grid is built");
        let column_ty = column_typography(ty);
        let mut layouts: Vec<Arc<ChapterLayout>> = Vec::with_capacity(self.columns.len());
        for column in &self.columns {
            let key = (Arc::clone(&column.id), book, chapter, style);
            let layout = self
                .cache
                .entry(key)
                .or_insert_with(|| {
                    Arc::new(layout_chapter(
                        &column.translation,
                        book,
                        chapter,
                        &column_ty,
                        width,
                        measure,
                    ))
                })
                .clone();
            layouts.push(layout);
        }

        let count = books::verses_in_chapter(book, chapter).unwrap_or(0);
        let blocks: Vec<Vec<Option<Range<usize>>>> = layouts
            .iter()
            .map(|layout| verse_blocks(layout, count))
            .collect();

        let mut rows = Vec::with_capacity(count as usize);
        let mut top = 0.0_f32;
        for verse in 1..=count as u8 {
            let mut height = 0.0_f32;
            let mut cells = Vec::with_capacity(layouts.len());
            for (index, layout) in layouts.iter().enumerate() {
                let Some(range) = blocks[index][verse as usize].clone() else {
                    cells.push(0..0);
                    continue;
                };
                let block_top = layout.lines[range.start].top;
                let last = &layout.lines[range.end - 1];
                height = height.max(last.top + last.height - block_top);
                cells.push(range);
            }
            // A verse no column has still takes a line, so the grid never
            // collapses and a row number always has a row.
            let height = if height > 0.0 {
                height
            } else {
                ty.line_spacing()
            };
            rows.push(ParallelRow {
                verse,
                reference: Reference {
                    book,
                    chapter,
                    verse: Some(verse),
                },
                top,
                height,
                cells,
            });
            top += height;
        }

        ParallelGrid {
            book,
            chapter,
            width,
            offset: self.offset,
            height: top,
            rows,
            layouts,
        }
    }
}

/// Groups one column's lines by the verse they belong to.
///
/// A title or a heading carries no verse of its own, so it joins the verse that
/// follows it. That is what keeps a heading above the verse it introduces on the
/// same row in every column.
fn verse_blocks(layout: &ChapterLayout, count: u16) -> Vec<Option<Range<usize>>> {
    let mut blocks: Vec<Option<Range<usize>>> = vec![None; count as usize + 1];
    let mut index = 0;
    while index < layout.lines.len() {
        let mut number = layout.lines[index]
            .verse
            .and_then(|reference| reference.verse);
        if number.is_none() {
            number = layout.lines[index..]
                .iter()
                .find_map(|line| line.verse.and_then(|reference| reference.verse));
        }
        let Some(number) = number else {
            index += 1;
            continue;
        };
        let start = index;
        let mut end = index + 1;
        while end < layout.lines.len() {
            match layout.lines[end]
                .verse
                .and_then(|reference| reference.verse)
            {
                Some(other) if other != number => break,
                Some(_) => end += 1,
                None => {
                    let ahead = layout.lines[end..]
                        .iter()
                        .find_map(|line| line.verse.and_then(|reference| reference.verse));
                    if ahead.is_some_and(|other| other != number) {
                        break;
                    }
                    end += 1;
                }
            }
        }
        if let Some(slot) = blocks.get_mut(number as usize)
            && slot.is_none()
        {
            *slot = Some(start..end);
        }
        index = end;
    }
    blocks
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
