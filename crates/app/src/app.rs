// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reader window and the offscreen render harness.
//!
//! [`ReaderApp`] owns the reading position, the theme, and the loaded
//! translation, and paints one frame at a time. [`render_shot`] drives the same
//! app through the toolkit's test harness and writes a PNG, which is how every
//! change to this file is verified on a machine with no display.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bible_core::{MarkKind, Reference, SpanStyle, Translation, Verse, books};
use eframe::egui;
use egui::epaint::text::VariationCoords;
use egui::text::{LayoutJob, TextFormat};
use egui::{Align, FontId, Pos2, Rect, RichText, Stroke, pos2, vec2};

use crate::Library;
use crate::theme::{self, Palette, Theme, Typography};

/// The reading application.
///
/// The state is flat on purpose. One translation is loaded at a time, the
/// position is a canonical reference rather than a scroll offset, and the verse
/// rectangles of the last frame are kept so a pointer press resolves to a verse
/// without laying the chapter out twice.
pub struct ReaderApp {
    library: Library,
    translation: Translation,
    reference: Reference,
    theme: Theme,
    typography: Typography,
    applied_theme: Option<Theme>,
    selected: Option<Reference>,
    hovered: Option<Reference>,
    verse_rects: Vec<(Reference, Rect)>,
    status: String,
}

impl ReaderApp {
    /// Opens the reader at a reference with a theme already chosen.
    ///
    /// This is the constructor the window and the harness share. The first
    /// installed translation is used, and with none installed the embedded
    /// sample is used, so the first frame always has a page to paint.
    pub fn open(cc: &eframe::CreationContext<'_>, theme: Theme, reference: Reference) -> Self {
        theme::install(&cc.egui_ctx, theme);
        let library = Library::discover();
        let mut status = String::new();
        let translation = match library.load(None) {
            Ok(translation) => translation,
            Err(error) => {
                status = format!("{error:#}");
                crate::sample_translation().expect("the embedded sample translation is valid")
            }
        };
        Self {
            library,
            translation,
            reference: reference.chapter_only(),
            theme,
            typography: Typography::default(),
            applied_theme: Some(theme),
            selected: None,
            hovered: None,
            verse_rects: Vec::new(),
            status,
        }
    }

    /// The library this reader reads installed translations from, which a
    /// translation menu switches between.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// The translation currently on screen.
    pub fn translation(&self) -> &Translation {
        &self.translation
    }

    /// The chapter the reader is showing.
    pub fn reference(&self) -> Reference {
        self.reference
    }

    /// The theme currently painted.
    pub fn theme(&self) -> Theme {
        self.theme
    }

    /// Paints the toolbar, the reading surface, and the status bar.
    fn paint(&mut self, ui: &mut egui::Ui) {
        let palette = self.theme.palette();
        self.paint_toolbar(ui, palette);

        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE.fill(palette.paper).inner_margin(8))
            .show(ui, |ui| self.paint_status(ui, palette));

        let chapter = {
            let this = &*self;
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(palette.paper))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| this.paint_chapter(ui, palette))
                        .inner
                })
                .inner
        };
        self.verse_rects = chapter.0;
        self.hovered = chapter.1;
        let ctx = ui.ctx().clone();
        self.resolve_pointer(&ctx);
    }

    /// Paints the top bar with the current reference and the theme control.
    fn paint_toolbar(&mut self, ui: &mut egui::Ui, palette: Palette) {
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::NONE
                    .fill(palette.paper)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Bible reader")
                            .font(FontId::new(13.0, theme::sans()))
                            .color(palette.muted),
                    );
                    ui.label(
                        RichText::new(self.reference.to_string())
                            .font(FontId::new(14.0, theme::sans()))
                            .color(palette.ink),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let label = format!("Theme: {}", self.theme.name());
                        if ui.button(label).clicked() {
                            self.theme = self.theme.next();
                        }
                        ui.label(
                            RichText::new(self.translation.meta().abbreviation.clone())
                                .font(FontId::new(13.0, theme::sans()))
                                .color(palette.muted),
                        );
                    });
                });
            });
    }

    /// Paints the bottom line with the translation, the theme, and the pointer's
    /// target, which is how a harness shot proves a click landed.
    fn paint_status(&self, ui: &mut egui::Ui, palette: Palette) {
        ui.horizontal(|ui| {
            let line = if self.status.is_empty() {
                self.translation.meta().name.clone()
            } else {
                format!("{} ({})", self.translation.meta().name, self.status)
            };
            ui.label(
                RichText::new(line)
                    .font(FontId::new(11.0, theme::sans()))
                    .color(palette.muted),
            );
            if let Some(selected) = self.selected {
                ui.label(
                    RichText::new(format!("selected {selected}"))
                        .font(FontId::new(11.0, theme::sans()))
                        .color(palette.accent),
                );
            }
            if let Some(hovered) = self.hovered {
                ui.label(
                    RichText::new(format!("pointer {hovered}"))
                        .font(FontId::new(11.0, theme::sans()))
                        .color(palette.muted),
                );
            }
        });
    }

    /// Paints the chapter title and every verse of the chapter, returning the
    /// verse rectangles and the verse under the pointer.
    fn paint_chapter(
        &self,
        ui: &mut egui::Ui,
        palette: Palette,
    ) -> (Vec<(Reference, Rect)>, Option<Reference>) {
        let mut rects = Vec::new();
        let mut hovered = None;
        let Some(book) = books::book(self.reference.book) else {
            return (rects, hovered);
        };
        let margin = 24.0;
        let measure = self
            .typography
            .measure()
            .min((ui.available_width() - margin * 2.0).max(200.0));

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - measure) / 2.0).max(0.0));
            ui.vertical(|ui| {
                ui.set_max_width(measure);
                self.paint_title(ui, book.name, palette);
                ui.add_space(10.0);
                self.paint_verses(ui, palette, measure, &mut rects, &mut hovered);
                ui.add_space(48.0);
            });
        });
        (rects, hovered)
    }

    /// Paints the chapter title in the body face at semibold weight, with the
    /// accent rule the reader uses under a new chapter.
    fn paint_title(&self, ui: &mut egui::Ui, book: &str, palette: Palette) {
        let mut job = LayoutJob::default();
        job.append(
            &format!("{book} {}", self.reference.chapter),
            0.0,
            TextFormat {
                font_id: FontId::new(self.typography.chapter_title_size(), theme::body_family()),
                color: palette.ink,
                coords: VariationCoords::new([(b"wght", 600.0)]),
                line_height: Some(self.typography.line_spacing()),
                ..Default::default()
            },
        );
        ui.label(job);
        let width = ui.available_width().min(self.typography.measure());
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), egui::Sense::hover());
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, vec2(width * 0.14, 1.0)),
            0.0,
            palette.accent,
        );
    }

    /// Paints every verse of the chapter, honouring headings, poetry, and the
    /// two paragraph modes.
    fn paint_verses(
        &self,
        ui: &mut egui::Ui,
        palette: Palette,
        measure: f32,
        rects: &mut Vec<(Reference, Rect)>,
        hovered: &mut Option<Reference>,
    ) {
        let Some(chapter) = self
            .translation
            .chapter(self.reference.book, self.reference.chapter)
        else {
            return;
        };
        for verse in chapter {
            let reference = Reference {
                book: self.reference.book,
                chapter: self.reference.chapter,
                verse: Some(verse.number),
            };
            let text = self.translation.verse_text(verse);
            if text.trim().is_empty() {
                continue;
            }
            if self.typography.headings {
                for mark in self.translation.verse_marks(verse) {
                    if mark.kind == MarkKind::Heading {
                        let heading = self.translation.mark_payload(mark);
                        if !heading.trim().is_empty() {
                            self.paint_heading(ui, heading, palette);
                        }
                    }
                }
            }
            let job = self.verse_job(verse, text, palette, measure);
            let response = ui.add(egui::Label::new(job).wrap());
            let rect = response.rect;
            if response.hovered() {
                *hovered = Some(reference);
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.0, palette.rule),
                    egui::StrokeKind::Inside,
                );
            }
            if self.selected == Some(reference) {
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(1.5, palette.accent),
                    egui::StrokeKind::Inside,
                );
            }
            rects.push((reference, rect));
            if self.typography.paragraph_mode {
                ui.add_space(self.typography.line_spacing() * 0.18);
            }
        }
    }

    /// Paints a section heading above the verse it introduces.
    fn paint_heading(&self, ui: &mut egui::Ui, heading: &str, palette: Palette) {
        ui.add_space(self.typography.line_spacing() * 0.4);
        let mut job = LayoutJob::default();
        job.append(
            heading,
            0.0,
            TextFormat {
                font_id: FontId::new(self.typography.heading_size(), theme::sans()),
                color: palette.accent,
                coords: VariationCoords::new([(b"wght", 600.0)]),
                line_height: Some(self.typography.line_spacing()),
                ..Default::default()
            },
        );
        ui.label(job);
        ui.add_space(2.0);
    }

    /// Builds the laid-out text of one verse.
    ///
    /// The verse number is a superscript in the sans face followed by a
    /// non-breaking space, so it can never be separated from its verse by a line
    /// break. Character styles inside the verse come from the core crate's span
    /// ranges, and poetry breaks come from its mark ranges.
    fn verse_job(&self, verse: &Verse, text: &str, palette: Palette, measure: f32) -> LayoutJob {
        let ty = &self.typography;
        let line_height = Some(ty.line_spacing());
        let mut job = LayoutJob::default();
        job.wrap.max_width = measure;
        job.justify = ty.justify;

        let base = TextFormat {
            font_id: FontId::new(ty.size, theme::body_family()),
            color: palette.ink,
            line_height,
            ..Default::default()
        };

        let mut prefix = String::new();
        if ty.paragraph_mode && self.starts_paragraph(verse) {
            prefix.push('\u{2003}');
        }
        for mark in self.translation.verse_marks(verse) {
            if mark.kind == MarkKind::StanzaBreak {
                prefix.push('\n');
            }
        }
        if !prefix.is_empty() {
            job.append(&prefix, 0.0, base.clone());
        }

        if ty.verse_numbers {
            let number = if verse.number_end > verse.number {
                format!("{}\u{2013}{}\u{00A0}", verse.number, verse.number_end)
            } else {
                format!("{}\u{00A0}", verse.number)
            };
            job.append(
                &number,
                0.0,
                TextFormat {
                    font_id: FontId::new(ty.verse_number_size(), theme::sans()),
                    color: palette.verse_number,
                    valign: Align::TOP,
                    line_height,
                    ..Default::default()
                },
            );
        }

        for (chunk, format) in self.verse_chunks(verse, text, palette, &base) {
            job.append(&chunk, 0.0, format);
        }
        job
    }

    /// Whether the verse opens a new paragraph.
    fn starts_paragraph(&self, verse: &Verse) -> bool {
        self.translation
            .verse_marks(verse)
            .iter()
            .any(|mark| mark.kind == MarkKind::Paragraph)
    }

    /// Splits a verse's text into styled chunks for layout.
    ///
    /// The split points are the poetry line breaks from the mark array and the
    /// character style ranges from the span array. Every chunk carries the exact
    /// format it is painted with.
    fn verse_chunks(
        &self,
        verse: &Verse,
        text: &str,
        palette: Palette,
        base: &TextFormat,
    ) -> Vec<(String, TextFormat)> {
        let ty = &self.typography;
        let line_height = Some(ty.line_spacing());
        let italic = TextFormat {
            font_id: FontId::new(ty.size, theme::body_italic_family()),
            color: palette.ink,
            line_height,
            ..Default::default()
        };
        let divine = TextFormat {
            font_id: FontId::new(ty.divine_name_size(), theme::body_family()),
            color: palette.ink,
            line_height,
            ..Default::default()
        };
        let red = TextFormat {
            color: if ty.red_letters {
                palette.words_of_christ
            } else {
                palette.ink
            },
            ..base.clone()
        };
        let superscription = TextFormat {
            font_id: FontId::new(ty.size, theme::body_italic_family()),
            color: palette.muted,
            line_height,
            ..Default::default()
        };
        let superscript = self
            .translation
            .verse_marks(verse)
            .iter()
            .any(|mark| mark.kind == MarkKind::Superscription);

        let mut breaks: Vec<(usize, u8)> = Vec::new();
        for mark in self.translation.verse_marks(verse) {
            match mark.kind {
                MarkKind::PoetryLine => breaks.push((mark.offset as usize, 0)),
                MarkKind::PoetryIndent => {
                    if let Some(entry) = breaks
                        .iter_mut()
                        .find(|entry| entry.0 == mark.offset as usize)
                    {
                        entry.1 = mark.level;
                    } else {
                        breaks.push((mark.offset as usize, mark.level));
                    }
                }
                _ => {}
            }
        }
        breaks.sort_by_key(|entry| entry.0);
        breaks.dedup_by_key(|entry| entry.0);

        let mut cuts: Vec<usize> = vec![0, text.len()];
        for (offset, _) in &breaks {
            cuts.push(*offset);
        }
        for span in self.translation.verse_spans(verse) {
            cuts.push(span.start as usize);
            cuts.push(span.end as usize);
        }
        cuts.retain(|cut| *cut <= text.len() && text.is_char_boundary(*cut));
        cuts.sort_unstable();
        cuts.dedup();

        let mut chunks = Vec::new();
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            if start == end {
                continue;
            }
            let mut chunk = String::new();
            if let Some((_, level)) = breaks.iter().find(|entry| entry.0 == start) {
                chunk.push('\n');
                for _ in 0..*level {
                    chunk.push_str("\u{2003}\u{2003}");
                }
            }
            let slice = &text[start..end];
            let style = self
                .translation
                .verse_spans(verse)
                .iter()
                .filter(|span| start >= span.start as usize && start < span.end as usize)
                .min_by_key(|span| span.end - span.start)
                .map(|span| span.style);
            let format = match style {
                Some(SpanStyle::WordsOfChrist) => red.clone(),
                Some(SpanStyle::TranslatorAddition) => italic.clone(),
                Some(SpanStyle::DivineName) => {
                    chunk.push_str(&slice.to_uppercase());
                    divine.clone()
                }
                Some(SpanStyle::Selah) => italic.clone(),
                None => {
                    if superscript {
                        superscription.clone()
                    } else {
                        base.clone()
                    }
                }
            };
            if style != Some(SpanStyle::DivineName) {
                chunk.push_str(slice);
            }
            chunks.push((chunk, format));
        }
        chunks
    }

    /// Turns a pointer press into a selected verse.
    ///
    /// The mapping uses the rectangles the last paint recorded, so a press
    /// selects the verse that was under the pointer when it was painted.
    fn resolve_pointer(&mut self, ctx: &egui::Context) {
        let press = ctx.input(|input| {
            input
                .pointer
                .primary_clicked()
                .then(|| input.pointer.interact_pos())
                .flatten()
        });
        let Some(position) = press else {
            return;
        };
        if let Some((reference, _)) = self
            .verse_rects
            .iter()
            .find(|(_, rect)| rect.contains(position))
        {
            self.selected = Some(*reference);
        }
    }
}

impl eframe::App for ReaderApp {
    /// Paints one frame, swapping the style only when the theme changed.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.applied_theme != Some(self.theme) {
            let ctx = ui.ctx().clone();
            self.theme.apply(&ctx);
            self.applied_theme = Some(self.theme);
        }
        self.paint(ui);
    }
}

/// Describes one harness run.
#[derive(Clone, Debug)]
pub struct ShotSpec {
    /// The PNG to write.
    pub path: PathBuf,
    /// The window size in points, which is the PNG size at one point per pixel.
    pub size: [u32; 2],
    /// The theme to paint.
    pub theme: Theme,
    /// The chapter to open.
    pub reference: Reference,
    /// A point to press, for reaching states that only the pointer reaches.
    pub click: Option<Pos2>,
    /// A point to hover, for the same reason.
    pub hover: Option<Pos2>,
    /// How long to wait between the last input and the picture, so content that
    /// arrives asynchronously is on screen before the shot.
    pub pause: Duration,
    /// How many frames to write, one file per frame, when greater than one.
    pub filmstrip: u32,
}

impl Default for ShotSpec {
    fn default() -> Self {
        Self {
            path: PathBuf::from("shot.png"),
            size: [1280, 800],
            theme: Theme::Light,
            reference: default_reference(),
            click: None,
            hover: None,
            pause: Duration::ZERO,
            filmstrip: 1,
        }
    }
}

impl ShotSpec {
    /// Builds a shot spec from the harness command line and the environment.
    ///
    /// The positional arguments are the output path and a `WIDTHxHEIGHT` size.
    /// The environment selects the rest, so a caller can drive the harness from
    /// a script without a long argument list.
    ///
    /// - `SHOT_THEME` is `light`, `sepia`, or `dark`.
    /// - `SHOT_REFERENCE` is any reference the reader understands.
    /// - `SHOT_CLICK` and `SHOT_HOVER` are `X,Y` points in points.
    /// - `SHOT_PAUSE` is a wait in milliseconds before the picture.
    /// - `SHOT_FILMSTRIP` is the number of frames to write.
    pub fn from_args(args: &[String]) -> Result<Self> {
        let mut spec = Self::default();
        let mut args = args.iter();
        if let Some(path) = args.next() {
            spec.path = PathBuf::from(path);
        }
        if let Some(size) = args.next() {
            let (width, height) = size
                .split_once('x')
                .ok_or_else(|| anyhow!("the size must look like 1280x800, not {size:?}"))?;
            spec.size = [
                width.parse().context("the width is not a number")?,
                height.parse().context("the height is not a number")?,
            ];
        }
        if spec.size[0] == 0 || spec.size[1] == 0 {
            bail!("the shot size must be at least one pixel across");
        }
        if let Some(theme) = std::env::var_os("SHOT_THEME") {
            let theme = theme.to_string_lossy();
            spec.theme =
                Theme::from_name(&theme).ok_or_else(|| anyhow!("no theme called {theme:?}"))?;
        }
        if let Some(reference) = std::env::var_os("SHOT_REFERENCE") {
            let reference = reference.to_string_lossy();
            spec.reference = Reference::parse(&reference)
                .with_context(|| format!("cannot read the reference {reference:?}"))?;
        }
        if let Some(point) = std::env::var_os("SHOT_CLICK") {
            spec.click = Some(parse_point(&point.to_string_lossy())?);
        }
        if let Some(point) = std::env::var_os("SHOT_HOVER") {
            spec.hover = Some(parse_point(&point.to_string_lossy())?);
        }
        if let Some(pause) = std::env::var_os("SHOT_PAUSE") {
            let pause: u64 = pause.to_string_lossy().parse().context("SHOT_PAUSE")?;
            spec.pause = Duration::from_millis(pause);
        }
        if let Some(frames) = std::env::var_os("SHOT_FILMSTRIP") {
            let frames: u32 = frames.to_string_lossy().parse().context("SHOT_FILMSTRIP")?;
            spec.filmstrip = frames.max(1);
        }
        Ok(spec)
    }

    /// The file a filmstrip frame is written to.
    ///
    /// Frame zero is the path itself, and later frames take a `-1`, `-2` suffix
    /// before the extension, so a filmstrip sits beside its first frame.
    pub fn frame_path(&self, frame: u32) -> PathBuf {
        if frame == 0 {
            return self.path.clone();
        }
        let stem = self
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "shot".to_owned());
        let extension = self
            .path
            .extension()
            .map(|extension| extension.to_string_lossy().into_owned())
            .unwrap_or_else(|| "png".to_owned());
        self.path
            .with_file_name(format!("{stem}-{frame}.{extension}"))
    }
}

/// Drives the reader offscreen and writes the PNGs a shot spec asks for.
///
/// The harness never opens a window, so this runs on a build machine with no
/// display. Rendering goes through wgpu, which picks Metal on macOS.
pub fn render_shot(spec: &ShotSpec) -> Result<Vec<PathBuf>> {
    let size = vec2(spec.size[0] as f32, spec.size[1] as f32);
    let theme = spec.theme;
    let reference = spec.reference;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open(cc, theme, reference));

    harness.run();

    if let Some(position) = spec.hover {
        harness.hover_at(position);
        harness.run();
    }
    if let Some(position) = spec.click {
        harness.hover_at(position);
        harness.event(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        });
        harness.event(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        });
        harness.run();
    }
    if !spec.pause.is_zero() {
        std::thread::sleep(spec.pause);
        harness.run();
    }

    let frames = spec.filmstrip.max(1);
    let mut written = Vec::with_capacity(frames as usize);
    for frame in 0..frames {
        if frame > 0 {
            harness.step();
            harness.run();
        }
        let image = harness.render().map_err(|error| anyhow!(error))?;
        let path = spec.frame_path(frame);
        save_png(&image, &path)?;
        written.push(path);
    }
    Ok(written)
}

/// Writes an image as a PNG, creating the folder it lives in.
fn save_png(image: &image::RgbaImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    image
        .save(path)
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Parses an `X,Y` point in points.
fn parse_point(input: &str) -> Result<Pos2> {
    let (x, y) = input
        .split_once(',')
        .ok_or_else(|| anyhow!("a point must look like 640,400, not {input:?}"))?;
    Ok(pos2(
        x.trim()
            .parse()
            .context("the x coordinate is not a number")?,
        y.trim()
            .parse()
            .context("the y coordinate is not a number")?,
    ))
}

/// The chapter a shot opens by default, used when no reference is given. John 3
/// is the sample page that carries every kind of mark and span.
pub fn default_reference() -> Reference {
    Reference::new(42, 3, None).expect("John 3 is canonical")
}
