// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reader window and the offscreen render harness.
//!
//! [`ReaderApp`] owns the reading position, the theme, and the loaded
//! translation, and paints one frame at a time. [`render_shot`] drives the same
//! app through the toolkit's test harness and writes a PNG, which is how every
//! change to this file is verified on a machine with no display.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use bible_core::{Reference, Translation};
use eframe::egui;
use egui::{Align, FontId, Pos2, Rect, RichText, Stroke, pos2, vec2};

use crate::Library;
use crate::reader::Reader;
use crate::selection::{self, Selection};
use crate::theme::{self, Palette, Theme, Typography};
use crate::typography;

/// The reading application.
///
/// The state is flat on purpose. One translation is loaded at a time, the
/// position is a canonical reference rather than a scroll offset, and the verse
/// rectangles of the last frame are kept so a pointer press resolves to a verse
/// without laying the chapter out twice.
pub struct ReaderApp {
    library: Library,
    translation: Arc<Translation>,
    reference: Reference,
    theme: Theme,
    typography: Typography,
    applied_theme: Option<Theme>,
    reader: Reader,
    selection: Selection,
    hovered: Option<Reference>,
    verse_rects: Vec<(Reference, Rect)>,
    status: String,
}

impl ReaderApp {
    /// Opens the reader at a reference with a theme already chosen.
    ///
    /// This is the constructor the window and the harness share. The default
    /// bundled translation is used, so the first frame paints scripture even on
    /// a machine that has never installed anything.
    pub fn open(cc: &eframe::CreationContext<'_>, theme: Theme, reference: Reference) -> Self {
        Self::open_with_translation(cc, theme, reference, None)
    }

    /// Opens the reader with a named translation rather than the default.
    ///
    /// The identifier may be one of the bundled translations, one installed in
    /// the data folder, or [`crate::SAMPLE_TRANSLATION_ID`] for the synthetic
    /// page. The harness uses the named form so it can shoot the same reference
    /// in real text and in the sample and compare them.
    pub fn open_with_translation(
        cc: &eframe::CreationContext<'_>,
        theme: Theme,
        reference: Reference,
        translation: Option<&str>,
    ) -> Self {
        theme::install(&cc.egui_ctx, theme);
        let library = Library::discover();
        let mut status = String::new();
        let translation = match library.load(translation) {
            Ok(translation) => translation,
            Err(error) => {
                status = format!("{error:#}");
                Arc::new(
                    crate::sample_translation().expect("the embedded sample translation is valid"),
                )
            }
        };
        Self {
            library,
            translation,
            reference: reference.chapter_only(),
            theme,
            typography: Typography::default(),
            applied_theme: Some(theme),
            reader: Reader::new(reference.chapter_only()),
            selection: Selection::new(),
            hovered: None,
            verse_rects: Vec::new(),
            status,
        }
    }

    /// Switches the page to another translation, keeping the reading position.
    ///
    /// The first switch to a translation decodes it, and a later switch back to
    /// one already read is served from the library's cache.
    pub fn switch_translation(&mut self, id: &str) -> Result<()> {
        self.translation = self.library.load(Some(id))?;
        self.status.clear();
        Ok(())
    }

    /// The library this reader reads installed translations from, which a
    /// translation menu switches between.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// The translation currently on screen.
    pub fn translation(&self) -> &Translation {
        self.translation.as_ref()
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

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(palette.paper))
            .show(ui, |ui| self.paint_chapter(ui, palette));
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
            if let Some((start, end)) = self.selection.range() {
                let line = if start == end {
                    format!("selected {start}")
                } else {
                    format!("selected {start} to {end}")
                };
                ui.label(
                    RichText::new(line)
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

    /// Paints the visible chapters of the reading surface and records the verse
    /// rectangles so a press can resolve to a verse.
    fn paint_chapter(&mut self, ui: &mut egui::Ui, palette: Palette) {
        self.verse_rects.clear();
        self.hovered = None;

        let ty = self.typography;
        let margin = 24.0;
        let width = ty
            .measure()
            .min((ui.available_width() - margin * 2.0).max(200.0));
        let viewport = ui.available_height().max(120.0);
        let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            self.reader.scroll_by(-scroll);
        }

        let ctx = ui.ctx().clone();
        let measure = typography::EguiMeasure::new(&ctx, ty);
        let frame = self
            .reader
            .frame(self.translation.as_ref(), &ty, width, viewport, &measure);
        let selection = self.selection;

        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), viewport), egui::Sense::hover());
        let left = rect.min.x + ((rect.width() - width) * 0.5).max(0.0);
        let top = rect.min.y;
        let hover = ui.ctx().pointer_hover_pos();
        let mut rects: Vec<(Reference, Rect)> = Vec::new();
        let mut hovered = None;

        for placement in &frame.placements {
            let Some(layout) = self.reader.layout(placement.book, placement.chapter) else {
                continue;
            };
            let chapter_top = top + placement.top;
            for line in &layout.lines {
                let line_top = chapter_top + line.top;
                if line_top > top + viewport || line_top + line.height < top {
                    continue;
                }
                let right = line
                    .runs
                    .iter()
                    .map(|run| run.x + run.width)
                    .fold(line.indent, f32::max);
                let line_rect = Rect::from_min_size(
                    pos2(left + line.indent, line_top),
                    vec2((right - line.indent).max(2.0), line.height),
                );

                if let Some(reference) = line.verse {
                    if selection.contains(reference) {
                        ui.painter().rect_filled(line_rect, 2.0, palette.selection);
                    }
                    if hover.is_some_and(|position| line_rect.contains(position)) {
                        hovered = Some(reference);
                    }
                    rects.push((reference, line_rect));
                }

                for run in &line.runs {
                    ui.painter().text(
                        pos2(left + run.x, chapter_top + run.top),
                        egui::Align2::LEFT_TOP,
                        &run.text,
                        typography::font_id(run.role, &ty),
                        run.role.color(&ty, &palette),
                    );
                }

                if hovered.is_some_and(|reference| Some(reference) == line.verse) {
                    ui.painter().rect_stroke(
                        line_rect,
                        2.0,
                        Stroke::new(1.0, palette.rule),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }

        self.verse_rects = rects;
        self.hovered = hovered;
        self.reference = self.reader.anchor();
    }

    /// Turns a pointer press into a selected verse and the J and K keys into a
    /// one-verse move.
    ///
    /// The mapping uses the rectangles the last paint recorded, so a press
    /// selects the verse that was under the pointer when it was painted. A press
    /// with shift held extends the selection instead of replacing it.
    fn resolve_pointer(&mut self, ctx: &egui::Context) {
        let press = ctx.input(|input| {
            if input.pointer.primary_clicked() {
                input
                    .pointer
                    .interact_pos()
                    .map(|position| (position, input.modifiers.shift))
            } else {
                None
            }
        });
        if let Some((position, shift)) = press
            && let Some((reference, _)) = self
                .verse_rects
                .iter()
                .find(|(_, rect)| rect.contains(position))
        {
            if shift {
                self.selection.extend_to(*reference);
            } else {
                self.selection.select(*reference);
            }
        }

        let step = ctx.input(|input| {
            if input.key_pressed(egui::Key::J) {
                Some(selection::Step::Next)
            } else if input.key_pressed(egui::Key::K) {
                Some(selection::Step::Previous)
            } else {
                None
            }
        });
        if let Some(direction) = step
            && let Some(focus) = self.selection.focus()
        {
            let chapter = self
                .translation
                .chapter(focus.book, focus.chapter)
                .unwrap_or(&[]);
            self.selection.step(direction, chapter);
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
    /// The translation to open, or `None` for the default bundled translation.
    pub translation: Option<String>,
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
            translation: None,
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
    /// - `SHOT_TRANSLATION` is a translation id, or `sample` for the synthetic
    ///   page the harness uses as a baseline.
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
        if let Some(translation) = std::env::var_os("SHOT_TRANSLATION") {
            spec.translation = Some(translation.to_string_lossy().into_owned());
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
    let translation = spec.translation.clone();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| {
            ReaderApp::open_with_translation(cc, theme, reference, translation.as_deref())
        });

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
