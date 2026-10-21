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
use bible_core::Reference;
use bible_core::Translation;
use bible_core::books;
use bible_core::search::Corpus;
use bible_core::study::{Settings, Store, Theme as StoredTheme};
use bible_core::{Bookmark, HighlightColour};
use eframe::egui;
use egui::{Align, FontId, Pos2, Rect, RichText, Stroke, pos2, vec2};

use crate::Library;
use crate::book_picker::BookPicker;
use crate::history::History;
use crate::palette::{Choice, Palette as JumpPalette};
use crate::parallel::{Column, MAX_COLUMNS, ParallelView};
use crate::reader::Reader;
use crate::search_panel::SearchPanel;
use crate::selection::{self, Selection};
use crate::shortcuts::{self, Action, Combo, Key as ShortcutKey};
use crate::sidebar::{self, MarginMarkers, Row};
use crate::theme::{self, Palette, Theme, Typography};
use crate::typography;
use crate::verse_menu::{CompareRow, MenuAction, VerseMenu};

/// The explicit theme, place, and restoration choices a window starts with.
///
/// Grouping them keeps [`ReaderApp::build`] from threading four related
/// arguments separately through every constructor.
struct Startup {
    theme: Theme,
    reference: Reference,
    restore_position: bool,
    restore_appearance: bool,
}

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
    palette: JumpPalette,
    picker: BookPicker,
    history: History,
    search: SearchPanel,
    menu: VerseMenu,
    study: Store,
    study_sidebar_open: bool,
    study_rows: Vec<Row>,
    corpus: Option<(String, Corpus)>,
    flash: Option<(Reference, f64)>,
    overlay: bool,
    saved_position: Reference,
    parallel: Option<ParallelView>,
    translation_choices: Vec<(String, String)>,
}

impl ReaderApp {
    /// Opens the reader at a reference with a theme already chosen.
    ///
    /// This is the constructor the window and the harness share. The default
    /// bundled translation is used, so the first frame paints scripture even on
    /// a machine that has never installed anything.
    pub fn open(cc: &eframe::CreationContext<'_>, theme: Theme, reference: Reference) -> Self {
        // An environment override is an explicit choice, so it keeps the theme
        // or place the environment asked for instead of the saved one. With no
        // override the window reopens exactly as the reader left it.
        let restore_position = std::env::var_os("BIBLE_REFERENCE").is_none();
        let restore_appearance = std::env::var_os("BIBLE_THEME").is_none();
        Self::open_from(
            cc,
            theme,
            reference,
            None,
            restore_position,
            restore_appearance,
        )
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
        // The harness states the theme, place, and translation it wants, so a
        // shot never depends on what an earlier run happened to save.
        Self::open_from(cc, theme, reference, translation, false, false)
    }

    /// Opens the reader with the default data folder, restoring the saved
    /// place and appearance only when the caller asked for that.
    fn open_from(
        cc: &eframe::CreationContext<'_>,
        theme: Theme,
        reference: Reference,
        translation: Option<&str>,
        restore_position: bool,
        restore_appearance: bool,
    ) -> Self {
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
        Self::build(
            cc,
            Startup {
                theme,
                reference,
                restore_position,
                restore_appearance,
            },
            library,
            translation,
            status,
            &crate::data_dir(),
        )
    }

    /// Opens the reader with the data folder given, which is how a test keeps
    /// its study file out of the reader's own.
    pub fn open_in(
        cc: &eframe::CreationContext<'_>,
        theme: Theme,
        reference: Reference,
        data: &std::path::Path,
    ) -> Self {
        let library = Library::discover_in(data.to_path_buf());
        let translation = library.load(None).unwrap_or_else(|_| {
            Arc::new(crate::sample_translation().expect("the sample is valid"))
        });
        Self::build(
            cc,
            Startup {
                theme,
                reference,
                restore_position: true,
                restore_appearance: true,
            },
            library,
            translation,
            String::new(),
            data,
        )
    }

    /// Opens the reader on a given place and ignores the remembered one.
    ///
    /// The render harness and the tests use this, so a picture always shows the
    /// same page and a test never depends on an earlier run.
    pub fn open_at(
        cc: &eframe::CreationContext<'_>,
        theme: Theme,
        reference: Reference,
        data: &std::path::Path,
    ) -> Self {
        let library = Library::discover_in(data.to_path_buf());
        let translation = library.load(None).unwrap_or_else(|_| {
            Arc::new(crate::sample_translation().expect("the sample is valid"))
        });
        Self::build(
            cc,
            Startup {
                theme,
                reference,
                restore_position: false,
                restore_appearance: false,
            },
            library,
            translation,
            String::new(),
            data,
        )
    }

    /// Builds the window on a place with a study store opened on the side.
    ///
    /// The restoration flags in [`Startup`] are false for the harness, which
    /// must be deterministic, and for an explicit `BIBLE_THEME` override, which
    /// must win over what was saved. A fresh store has no saved row, so the
    /// current GUI defaults stay in force instead of the core defaults.
    fn build(
        cc: &eframe::CreationContext<'_>,
        startup: Startup,
        library: Library,
        translation: Arc<Translation>,
        mut status: String,
        data: &std::path::Path,
    ) -> Self {
        let Startup {
            theme,
            reference,
            restore_position,
            restore_appearance,
        } = startup;
        // Checked before the store creates the file, so a fresh store is
        // distinguishable from one that already holds a saved settings row.
        let had_store = data.join("study.db").exists();
        let study = match Store::open(data.join("study.db")) {
            Ok(study) => study,
            Err(error) => {
                // Keep the window usable, but say plainly that nothing will be
                // saved rather than presenting the in-memory fallback as the
                // reader's persisted study data.
                let notice =
                    format!("study data is not being saved and will be lost on exit: {error:#}");
                status = if status.is_empty() {
                    notice
                } else {
                    format!("{status}; {notice}")
                };
                Store::open_in_memory().expect("an in-memory study store always opens")
            }
        };
        // The store remembers where the reader was, so a window opens where the
        // last one stopped rather than at the first page. An explicit place
        // from the environment or the harness wins over the saved one.
        let remembered = study.settings().position;
        let start = if restore_position && remembered.chapter != 0 {
            remembered
        } else {
            reference
        };
        // The saved row is the reader's appearance. On a fresh store there is
        // none, so the current GUI typography and the caller's theme remain.
        let (theme, typography) = if restore_appearance && had_store {
            preferences(study.settings())
        } else {
            (theme, Typography::default())
        };
        theme::install(&cc.egui_ctx, theme);
        let translation_choices = translation_choices(&library);
        Self {
            library,
            translation,
            reference: start.chapter_only(),
            theme,
            typography,
            applied_theme: Some(theme),
            reader: Reader::new(start.chapter_only()),
            selection: Selection::new(),
            hovered: None,
            verse_rects: Vec::new(),
            status,
            palette: JumpPalette::new(),
            picker: BookPicker::new(),
            history: History::new(start.chapter_only()),
            search: SearchPanel::new(),
            menu: VerseMenu::new(),
            study,
            study_sidebar_open: false,
            study_rows: Vec::new(),
            corpus: None,
            flash: None,
            overlay: false,
            saved_position: start.chapter_only(),
            parallel: None,
            translation_choices,
        }
    }

    /// Puts the window into one of the states the render harness photographs.
    ///
    /// This is how a shot reaches a panel without a pointer, and it is the same
    /// state a reader reaches with the shortcut.
    pub fn set_view(&mut self, view: ShotView) {
        match view {
            ShotView::Palette => {
                self.palette.open();
                self.palette.set_input("jn 3:16");
            }
            ShotView::Search => {
                self.search.open();
                self.search.set_input("light");
            }
            ShotView::Picker => self.picker.open_at(self.reference),
            ShotView::VerseMenu => {
                // The first verse of the chapter, which is a real verse in the
                // canonical grid.
                let target = Reference::new(self.reference.book, self.reference.chapter, Some(1))
                    .unwrap_or(self.reference.chapter_only());
                self.selection.select(target);
                self.menu.open_at(target);
            }
            ShotView::Shortcuts => self.overlay = true,
            ShotView::StudySidebar => self.show_demo_study_sidebar(),
            ShotView::Parallel => self.show_demo_parallel(),
        }
    }

    /// Opens the study sidebar with a synthetic mark of each kind.
    ///
    /// This is the state the render harness photographs. It writes only to the
    /// store the harness opened in a scratch folder, never to a reader's data.
    fn show_demo_study_sidebar(&mut self) {
        self.seed_demo_marks(16, 17, 18);
        // The harness may open on a remembered place, so move the demo page to
        // the chapter its synthetic marks belong to.
        let highlight = Reference::new(42, 3, Some(16)).expect("John 3:16 is canonical");
        self.navigate(highlight, 0.0);
        self.study_sidebar_open = true;
        self.refresh_study_rows();
    }

    /// Writes one highlight, one note, and one bookmark into the store the
    /// harness opened.
    ///
    /// The marks are synthetic and live only in the scratch store, never in a
    /// reader's own data.
    fn seed_demo_marks(&mut self, highlight: u8, note: u8, bookmark: u8) {
        let highlight = Reference::new(42, 3, Some(highlight)).expect("John 3 is canonical");
        let note = Reference::new(42, 3, Some(note)).expect("John 3 is canonical");
        let bookmark = Reference::new(42, 3, Some(bookmark)).expect("John 3 is canonical");
        if let Err(error) = self
            .study
            .set_highlight(highlight, Some(HighlightColour::Yellow))
        {
            self.status = format!("cannot seed the highlight: {error:#}");
        }
        if let Err(error) = self.study.set_note(note, "A synthetic note for the shot") {
            self.status = format!("cannot seed the note: {error:#}");
        }
        if let Err(error) = self
            .study
            .set_bookmark(bookmark, Some(Bookmark::with_label("synthetic bookmark")))
        {
            self.status = format!("cannot seed the bookmark: {error:#}");
        }
    }

    /// Opens the study sidebar, or closes it when it is already open.
    ///
    /// Opening rebuilds the row cache from one store snapshot. While the
    /// sidebar is open its rows are rebuilt after a successful study edit, not
    /// on every frame.
    pub fn toggle_study_sidebar(&mut self) {
        if self.study_sidebar_open {
            self.study_sidebar_open = false;
        } else {
            self.study_sidebar_open = true;
            self.refresh_study_rows();
        }
    }

    /// Rebuilds the sidebar row cache from the store's mirror.
    fn refresh_study_rows(&mut self) {
        self.study_rows = sidebar::rows_from_snapshot(&self.study.snapshot());
    }

    /// Opens the parallel view with one synthetic mark of each kind on screen.
    ///
    /// This is the state the render harness photographs. Like the sidebar demo
    /// it writes only to the store the harness opened in a scratch folder, never
    /// to a reader's data.
    fn show_demo_parallel(&mut self) {
        // The verses sit near the top of the chapter, so a shot at the page's
        // first screen shows the shared highlight and the margin note in every
        // column rather than photographing an empty page.
        self.seed_demo_marks(3, 4, 5);
        let highlight = Reference::new(42, 3, Some(3)).expect("John 3:3 is a canonical verse");
        self.navigate(highlight, 0.0);
        self.open_parallel();
    }

    /// Shows or hides the parallel translations view.
    ///
    /// The single column reader is not moved while the view is open, so hiding
    /// the view returns the reader to the exact page and offset it had.
    pub fn toggle_parallel(&mut self) {
        if self.parallel.is_some() {
            self.parallel = None;
        } else {
            self.open_parallel();
        }
    }

    /// Opens the parallel view at the reader's place, in the current translation
    /// and the next two bundled ones.
    fn open_parallel(&mut self) {
        let mut translations = Vec::new();
        for id in self.parallel_ids() {
            match self.library.load(Some(&id)) {
                Ok(translation) => translations.push(translation),
                Err(error) => {
                    self.status = format!("cannot open the {id} column: {error:#}");
                    return;
                }
            }
        }
        match ParallelView::new(translations, self.reader.anchor(), self.reader.offset()) {
            Some(view) => self.parallel = Some(view),
            None => self.status = "parallel view needs at least two translations".to_owned(),
        }
    }

    /// The translations the view opens with, the current one first and then the
    /// next bundled ones in table order.
    fn parallel_ids(&self) -> Vec<String> {
        let current = self.translation.meta().id.clone();
        let bundled: Vec<&str> = crate::BUNDLED_TRANSLATIONS
            .iter()
            .map(|entry| entry.id())
            .collect();
        let mut ids = vec![current.clone()];
        let start = bundled
            .iter()
            .position(|id| **id == *current)
            .map_or(0, |index| index + 1);
        for step in 0..bundled.len() {
            let id = bundled[(start + step) % bundled.len()];
            if !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_owned());
            }
            if ids.len() == MAX_COLUMNS {
                break;
            }
        }
        // The current translation may be one the reader installed rather than
        // one compiled in, so the list can still be short after the bundled pass.
        for installed in self.library.installed() {
            if ids.len() == MAX_COLUMNS {
                break;
            }
            if !ids.contains(&installed.meta.id) {
                ids.push(installed.meta.id.clone());
            }
        }
        ids
    }

    /// Reads one column in another translation, which is what the column's
    /// combobox asks for. The other columns and the shared anchor stay put.
    pub fn set_parallel_translation(&mut self, column: usize, id: &str) -> Result<()> {
        let translation = self.library.load(Some(id))?;
        let Some(view) = &mut self.parallel else {
            bail!("the parallel view is not open");
        };
        if !view.set_translation(column, translation) {
            bail!("there is no parallel column {column}");
        }
        self.status.clear();
        Ok(())
    }

    /// Whether the parallel translations view is on screen.
    pub fn parallel_is_open(&self) -> bool {
        self.parallel.is_some()
    }

    /// The columns the parallel view is showing, empty when it is closed.
    pub fn parallel_columns(&self) -> &[Column] {
        self.parallel.as_ref().map_or(&[], |view| view.columns())
    }

    /// The chapter the parallel view's shared anchor sits in, absent when the
    /// view is closed.
    pub fn parallel_anchor(&self) -> Option<Reference> {
        self.parallel.as_ref().map(ParallelView::anchor)
    }

    /// The shared offset of the parallel view, zero when it is closed.
    pub fn parallel_offset(&self) -> f32 {
        self.parallel.as_ref().map_or(0.0, ParallelView::offset)
    }

    /// Scrolls the parallel view by a number of points, positive meaning down.
    ///
    /// The scrolled view paints on the next frame, which is what the scroll test
    /// asserts and what the window does from the scroll wheel.
    pub fn scroll_parallel(&mut self, delta: f32) {
        if let Some(view) = &mut self.parallel {
            view.scroll_by(delta);
        }
    }

    /// The offset of the single column reader, which a test reads to prove that
    /// parallel mode left it alone.
    pub fn reading_offset(&self) -> f32 {
        self.reader.offset()
    }

    /// The verse rectangles the last frame recorded, in paint order.
    pub fn verse_rects(&self) -> &[(Reference, Rect)] {
        &self.verse_rects
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

    /// The typography (size, spacing, measure, and text treatments) currently
    /// painted, which a test reads to prove a saved preference was restored.
    pub fn typography(&self) -> Typography {
        self.typography
    }

    /// The status line's message, empty when there is nothing to report.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Whether the jump palette is on screen.
    pub fn palette_is_open(&self) -> bool {
        self.palette.is_open()
    }

    /// The text in the jump palette's input.
    pub fn palette_input(&self) -> &str {
        self.palette.input()
    }

    /// Whether the search panel is on screen.
    pub fn search_is_open(&self) -> bool {
        self.search.is_open()
    }

    /// Whether the book and chapter picker is on screen.
    pub fn picker_is_open(&self) -> bool {
        self.picker.is_open()
    }

    /// Whether the verse menu is on screen.
    pub fn menu_is_open(&self) -> bool {
        self.menu.is_open()
    }

    /// Whether the study sidebar is on screen.
    pub fn study_sidebar_is_open(&self) -> bool {
        self.study_sidebar_open
    }

    /// The cached rows the study sidebar is showing.
    pub fn study_rows(&self) -> &[Row] {
        &self.study_rows
    }

    /// The study mark state of one verse, which the margin indicators paint.
    pub fn margin_markers(&self, reference: Reference) -> MarginMarkers {
        sidebar::margin_markers(&self.study, reference)
    }

    /// The verse the reader last selected, which a study jump sets.
    pub fn selected_verse(&self) -> Option<Reference> {
        self.selection.focus()
    }

    /// The place the reader was, which the history also holds.
    pub fn history_current(&self) -> Option<Reference> {
        self.history.current()
    }

    /// Paints the toolbar, the reading surface, and the status bar.
    fn paint(&mut self, ui: &mut egui::Ui) {
        let palette = self.theme.palette();
        self.paint_toolbar(ui, palette);

        let mut study_jump = None;
        if self.study_sidebar_open {
            let rows = &self.study_rows;
            egui::Panel::right("study-sidebar")
                .resizable(false)
                .default_size(240.0)
                .frame(
                    egui::Frame::NONE
                        .fill(palette.paper)
                        .inner_margin(12)
                        .stroke(Stroke::new(1.0, palette.rule)),
                )
                .show(ui, |ui| {
                    study_jump = sidebar::show(ui, rows, palette);
                });
        }
        if let Some(reference) = study_jump {
            let now = ui.ctx().input(|input| input.time);
            self.navigate(reference, now);
        }

        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE.fill(palette.paper).inner_margin(8))
            .show(ui, |ui| self.paint_status(ui, palette));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(palette.paper))
            .show(ui, |ui| {
                if self.parallel.is_some() {
                    self.paint_parallel(ui, palette);
                } else {
                    self.paint_chapter(ui, palette);
                }
            });
        let ctx = ui.ctx().clone();
        self.resolve_pointer(&ctx);
        self.paint_overlays(&ctx, palette);
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
                            self.save_appearance();
                        }
                        ui.label(
                            RichText::new(self.translation.meta().abbreviation.clone())
                                .font(FontId::new(13.0, theme::sans()))
                                .color(palette.muted),
                        );
                        if ui.button("Book").clicked() {
                            self.picker.open_at(self.reference);
                        }
                        if ui.button("Search").clicked() {
                            self.search.open();
                        }
                        if ui.button("Jump").clicked() {
                            self.palette.open();
                        }
                        if ui.button("Study").clicked() {
                            self.toggle_study_sidebar();
                        }
                        if ui.button("Parallel").clicked() {
                            self.toggle_parallel();
                        }
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
                    let markers = sidebar::margin_markers(&self.study, reference);
                    if markers.note {
                        ui.painter().circle_filled(
                            pos2(left - 10.0, line_top + line.height * 0.5),
                            2.5,
                            palette.accent,
                        );
                    }
                    if markers.bookmark {
                        ui.painter().rect_filled(
                            Rect::from_min_size(
                                pos2(left - 6.0, line_top + line.height * 0.25),
                                vec2(2.0, line.height * 0.5),
                            ),
                            1.0,
                            palette.muted,
                        );
                    }
                    if let Some((flash, until)) = self.flash
                        && flash == reference
                        && ui.ctx().input(|input| input.time) < until
                    {
                        ui.painter().rect_filled(
                            line_rect,
                            2.0,
                            palette.accent.gamma_multiply(0.28),
                        );
                    }
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
        if self.reference.chapter_only() != self.saved_position {
            self.remember(self.reference);
        }
    }

    /// Paints the parallel columns and records the verse rectangles so a press
    /// in any column selects that verse for the whole view.
    ///
    /// Each column is laid out at the same width and painted from the one shared
    /// offset, so a scroll moves every column together and a row's fill covers
    /// the same verse everywhere.
    fn paint_parallel(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let Some(mut view) = self.parallel.take() else {
            return;
        };
        self.verse_rects.clear();
        self.hovered = None;

        let ty = self.typography;
        let columns = view.column_count();
        let gap = 22.0;
        let margin = 24.0;
        let available = (ui.available_width() - margin * 2.0).max(240.0);
        let gaps = gap * (columns.saturating_sub(1)) as f32;
        let width = ((available - gaps) / columns as f32).max(80.0);
        let block = width * columns as f32 + gaps;

        let mut chosen: Option<(usize, String)> = None;
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.add_space(((ui.available_width() - block) * 0.5).max(0.0));
            for index in 0..columns {
                let Some(column) = view.columns().get(index) else {
                    continue;
                };
                let current = column.id().to_owned();
                let selected = column.abbreviation().to_owned();
                ui.allocate_ui_with_layout(
                    vec2(width, 22.0),
                    egui::Layout::left_to_right(Align::Center),
                    |ui| {
                        egui::ComboBox::from_id_salt(("parallel-column", index))
                            .selected_text(
                                RichText::new(selected).font(FontId::new(12.0, theme::sans())),
                            )
                            .show_ui(ui, |ui| {
                                for (id, label) in &self.translation_choices {
                                    if ui.selectable_label(*id == current, label).clicked() {
                                        chosen = Some((index, id.clone()));
                                    }
                                }
                            });
                    },
                );
            }
        });

        ui.add_space(8.0);
        let viewport = ui.available_height().max(120.0);
        let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
        if scroll != 0.0 {
            view.scroll_by(-scroll);
        }
        let ctx = ui.ctx().clone();
        let measure = typography::EguiMeasure::new(&ctx, ty);
        let grid = view.frame(&ty, width, &measure);
        let offset = grid.offset;
        let selection = self.selection;
        let flash = self.flash;
        let now = ctx.input(|input| input.time);

        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), viewport), egui::Sense::hover());
        let left = rect.min.x + ((rect.width() - block) * 0.5).max(0.0);
        let top = rect.min.y;
        let hover = ctx.pointer_hover_pos();
        let mut rects: Vec<(Reference, Rect)> = Vec::new();
        let mut hovered = None;
        for column in 0..columns {
            let x = left + column as f32 * (width + gap);
            let Some(layout) = grid.layouts().get(column) else {
                continue;
            };
            if column > 0 {
                let rule = x - gap * 0.5;
                ui.painter().line_segment(
                    [pos2(rule, top), pos2(rule, top + viewport)],
                    Stroke::new(1.0, palette.rule),
                );
            }
            for (row_index, row) in grid.rows.iter().enumerate() {
                let Some(row_top) = grid.row_top(row_index, offset) else {
                    continue;
                };
                if row_top > viewport || row_top + row.height < 0.0 {
                    continue;
                }
                let cell_rect =
                    Rect::from_min_size(pos2(x, top + row_top), vec2(width, row.height));
                if selection.contains(row.reference) {
                    ui.painter().rect_filled(cell_rect, 2.0, palette.selection);
                }
                if let Some((flash, until)) = flash
                    && flash == row.reference
                    && now < until
                {
                    ui.painter()
                        .rect_filled(cell_rect, 2.0, palette.accent.gamma_multiply(0.28));
                }
                let markers = sidebar::margin_markers(&self.study, row.reference);
                if markers.note {
                    ui.painter().circle_filled(
                        pos2(x - 8.0, top + row_top + row.height * 0.5),
                        2.5,
                        palette.accent,
                    );
                }
                if markers.bookmark {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            pos2(x - 5.0, top + row_top + row.height * 0.25),
                            vec2(2.0, row.height * 0.5),
                        ),
                        1.0,
                        palette.muted,
                    );
                }
                rects.push((row.reference, cell_rect));
                if hover.is_some_and(|position| cell_rect.contains(position)) {
                    hovered = Some(row.reference);
                }

                let Some(cell) = row.cells.get(column) else {
                    continue;
                };
                let Some(first) = layout.lines.get(cell.start) else {
                    continue;
                };
                let block_top = first.top;
                for line_index in cell.clone() {
                    let line = &layout.lines[line_index];
                    let line_top = top + row_top + (line.top - block_top);
                    if line_top > top + viewport || line_top + line.height < top {
                        continue;
                    }
                    for run in &line.runs {
                        ui.painter().text(
                            pos2(x + run.x, line_top + (run.top - line.top)),
                            egui::Align2::LEFT_TOP,
                            &run.text,
                            typography::font_id(run.role, &ty),
                            run.role.color(&ty, &palette),
                        );
                    }
                }
                if hovered == Some(row.reference) {
                    ui.painter().rect_stroke(
                        cell_rect,
                        2.0,
                        Stroke::new(1.0, palette.rule),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }

        self.verse_rects = rects;
        self.hovered = hovered;
        self.reference = view.anchor();
        self.parallel = Some(view);
        if self.reference.chapter_only() != self.saved_position {
            self.remember(self.reference);
        }
        if let Some((column, id)) = chosen
            && let Err(error) = self.set_parallel_translation(column, &id)
        {
            self.status = format!("{error:#}");
        }
    }

    /// Turns a pointer press into a selected verse and the J and K keys into a
    /// one-verse move.
    ///
    /// The mapping uses the rectangles the last paint recorded, so a press
    /// selects the verse that was under the pointer when it was painted. A press
    /// with shift held extends the selection instead of replacing it.
    fn resolve_pointer(&mut self, ctx: &egui::Context) {
        if self.palette.is_open() || self.search.is_open() || self.picker.is_open() {
            return;
        }
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

        let secondary = ctx.input(|input| {
            if input.pointer.secondary_clicked() {
                input.pointer.interact_pos()
            } else {
                None
            }
        });
        if let Some(position) = secondary
            && let Some((reference, _)) = self
                .verse_rects
                .iter()
                .find(|(_, rect)| rect.contains(position))
        {
            self.selection.select(*reference);
            self.menu.open_at(*reference);
        }
    }

    /// Moves the reader to a place and records it in the history.
    ///
    /// The verse is selected as well, so the study actions that follow a jump
    /// have a target, and the chapter flashes so the reader can see the landing.
    fn navigate(&mut self, reference: Reference, now: f64) {
        self.selection.select(reference);
        self.reference = reference.chapter_only();
        self.reader.set_position(reference.chapter_only());
        if let Some(view) = &mut self.parallel {
            view.set_reference(reference.chapter_only());
        }
        self.history.visit(reference.chapter_only());
        self.flash = Some((reference, now + 0.9));
        self.remember(reference);
    }

    /// Shows the place the history moved to without recording a new visit.
    fn show_place(&mut self, reference: Reference, now: f64) {
        self.selection.select(reference);
        self.reference = reference.chapter_only();
        self.reader.set_position(reference.chapter_only());
        if let Some(view) = &mut self.parallel {
            view.set_reference(reference.chapter_only());
        }
        self.flash = Some((reference, now + 0.9));
        self.remember(reference);
    }

    /// Writes the reading position into the study settings, so the next window
    /// opens where this one stopped.
    ///
    /// The store writes the settings at most every 1.5 seconds, so this is cheap
    /// enough to call on every move.
    fn remember(&mut self, reference: Reference) {
        self.saved_position = reference.chapter_only();
        let mut settings = self.study.settings().clone();
        settings.position = reference;
        if let Err(error) = self.study.set_settings(settings) {
            self.status = format!("cannot save the reading position: {error:#}");
        }
    }

    /// Writes the theme and typography into the study settings.
    ///
    /// The position is left alone, so changing the appearance never moves the
    /// reader. The store coalesces these writes on its 1.5 second throttle, and
    /// [`ReaderApp::flush_settings`] writes a pending change on the way out.
    fn save_appearance(&mut self) {
        let mut settings = self.study.settings().clone();
        settings.theme = match self.theme {
            Theme::Light => StoredTheme::Light,
            Theme::Sepia => StoredTheme::Sepia,
            Theme::Dark => StoredTheme::Dark,
        };
        settings.text_size = self.typography.size;
        settings.line_spacing = self.typography.line_height;
        settings.line_length = self.typography.measure_em;
        settings.justify = self.typography.justify;
        settings.show_verse_numbers = self.typography.verse_numbers;
        settings.show_headings = self.typography.headings;
        settings.words_of_christ_red = self.typography.red_letters;
        if let Err(error) = self.study.set_settings(settings) {
            self.status = format!("cannot save the reader preferences: {error:#}");
        }
    }

    /// Writes a settings change that is still waiting on the throttle.
    ///
    /// Called from the shutdown hook and from [`Drop`], so a change made right
    /// before the window closes survives an immediate restart. A disk error is
    /// reported rather than silently swallowed.
    fn flush_settings(&mut self) {
        if let Err(error) = self.study.flush() {
            let message = format!("cannot save the reader preferences: {error:#}");
            eprintln!("{message}");
            self.status = message;
        }
    }

    /// The chapter before or after a place, staying inside the canon.
    fn adjacent_chapter(reference: Reference, forward: bool) -> Option<Reference> {
        let book = books::book(reference.book)?;
        if forward {
            if reference.chapter < book.chapter_count {
                return Reference::new(book.index, reference.chapter + 1, None).ok();
            }
            let next = books::book(book.index + 1)?;
            Reference::new(next.index, 1, None).ok()
        } else {
            if reference.chapter > 1 {
                return Reference::new(book.index, reference.chapter - 1, None).ok();
            }
            let previous = books::book(book.index.checked_sub(1)?)?;
            Reference::new(previous.index, previous.chapter_count, None).ok()
        }
    }

    /// Runs the keyboard shortcuts that are not owned by an open panel.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Escape leaves the parallel view. The key is checked before the table
        // because it is a way out of a mode, not an action with a key of its own.
        if self.parallel.is_some() && ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.parallel = None;
        }
        if self.palette.is_open() || self.search.is_open() || self.picker.is_open() {
            return;
        }
        let combos: Vec<Combo> = ctx.input(|input| {
            let mut combos = Vec::new();
            for event in &input.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                    && let Some(key) = shortcut_key(*key)
                {
                    combos.push(Combo::new(
                        key,
                        modifiers.command || modifiers.ctrl,
                        modifiers.shift,
                        modifiers.alt,
                    ));
                }
            }
            combos
        });
        for combo in combos {
            if let Some(action) = shortcuts::action_for(combo) {
                self.perform(action, ctx);
            }
        }
    }

    /// Performs one action from the shortcut table.
    fn perform(&mut self, action: Action, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        let appearance = matches!(
            action,
            Action::ToggleRedLetter
                | Action::CycleTheme
                | Action::TextLarger
                | Action::TextSmaller
                | Action::TextDefault
        );
        match action {
            Action::JumpPalette => self.palette.open(),
            Action::Search => self.search.open(),
            Action::BookPicker => self.picker.open_at(self.reference),
            Action::Translation(index) => {
                // The installed translations come first, so the first installed
                // one is number one. With none installed the bundled list still
                // gives the shortcut something to ask for.
                let id = self
                    .library
                    .installed()
                    .get(index as usize)
                    .map(|installed| installed.meta.id.clone())
                    .or_else(|| {
                        crate::BUNDLED_TRANSLATIONS
                            .get(index as usize)
                            .map(|translation| translation.id().to_owned())
                    });
                match id {
                    Some(id) => {
                        if let Err(error) = self.switch_translation(&id) {
                            self.status = format!("{error:#}");
                        }
                    }
                    None => {
                        self.status = format!("there is no translation number {}", index + 1);
                    }
                }
            }
            Action::Back => {
                if let Some(reference) = self.history.back() {
                    self.show_place(reference, now);
                }
            }
            Action::Forward => {
                if let Some(reference) = self.history.forward() {
                    self.show_place(reference, now);
                }
            }
            Action::PreviousChapter => {
                if let Some(reference) = Self::adjacent_chapter(self.reference, false) {
                    self.navigate(reference, now);
                }
            }
            Action::NextChapter => {
                if let Some(reference) = Self::adjacent_chapter(self.reference, true) {
                    self.navigate(reference, now);
                }
            }
            Action::Highlight(colour) => self.apply_menu(MenuAction::Highlight(Some(colour)), ctx),
            Action::ClearHighlight => self.apply_menu(MenuAction::Highlight(None), ctx),
            Action::Note => {
                if let Some(target) = self.selection.focus() {
                    self.menu.open_at(target);
                    let existing = self.study.note(target).unwrap_or_default().to_owned();
                    self.menu.set_note_draft(existing);
                }
            }
            Action::Bookmark => self.apply_menu(MenuAction::Bookmark, ctx),
            Action::Copy => self.apply_menu(MenuAction::Copy, ctx),
            Action::CopyWithReference => self.apply_menu(MenuAction::CopyWithReference, ctx),
            Action::Compare => self.compare_selected(),
            Action::ToggleParallel => self.toggle_parallel(),
            Action::ExportMarkdown => {
                let path = crate::data_dir().join("study.md");
                match std::fs::write(&path, self.study.export_markdown()) {
                    Ok(()) => self.status = format!("exported {}", path.display()),
                    Err(error) => self.status = format!("cannot write {}: {error}", path.display()),
                }
            }
            Action::ExportJson => {
                let path = crate::data_dir().join("study.json");
                let written = self
                    .study
                    .export_json()
                    .and_then(|json| std::fs::write(&path, json).map_err(anyhow::Error::from));
                match written {
                    Ok(()) => self.status = format!("exported {}", path.display()),
                    Err(error) => self.status = format!("cannot write {}: {error}", path.display()),
                }
            }
            Action::ToggleRedLetter => {
                self.typography.red_letters = !self.typography.red_letters;
            }
            Action::CycleTheme => self.theme = self.theme.next(),
            Action::TextLarger => self.typography.size = (self.typography.size + 1.0).min(32.0),
            Action::TextSmaller => self.typography.size = (self.typography.size - 1.0).max(12.0),
            Action::TextDefault => self.typography.size = Typography::default().size,
        }
        if appearance {
            self.save_appearance();
        }
    }

    /// Runs a verse menu action against the study store and reports the result.
    fn apply_menu(&mut self, action: MenuAction, ctx: &egui::Context) {
        // The menu is opened on the selection when it is not already open, so a
        // note the reader is writing survives the action.
        if self.menu.target().is_none()
            && let Some(target) = self.selection.focus()
        {
            self.menu.open_at(target);
        }
        match self.menu.apply(action, &mut self.study, &self.translation) {
            Ok(outcome) => {
                self.status = outcome.message;
                if self.study_sidebar_open {
                    self.refresh_study_rows();
                }
                if let Some(text) = outcome.clipboard {
                    ctx.copy_text(text);
                }
            }
            Err(error) => self.status = format!("{error:#}"),
        }
    }

    /// Compares the selected verse across the installed translations.
    fn compare_selected(&mut self) {
        let Some(target) = self.selection.focus() else {
            self.status = "nothing is selected".to_owned();
            return;
        };
        let mut rows = Vec::new();
        for installed in self.library.installed().iter().take(4) {
            if let Ok(translation) = self.library.load(Some(installed.meta.id.as_str()))
                && let Some(text) = translation.reference_text(&target)
            {
                rows.push(CompareRow {
                    abbreviation: installed.meta.abbreviation.clone(),
                    text: text.to_owned(),
                });
            }
        }
        self.menu.set_comparison(rows.clone());
        self.status = if rows.is_empty() {
            format!("no installed translation has {target}")
        } else {
            rows.iter()
                .map(|row| format!("{}: {}", row.abbreviation, row.text))
                .collect::<Vec<String>>()
                .join("  |  ")
        };
    }

    /// Rebuilds the normalized corpus when the translation changed.
    fn rebuild_corpus(&mut self) {
        let id = self.translation.meta().id.clone();
        let stale = self.corpus.as_ref().is_none_or(|(cached, _)| *cached != id);
        if stale {
            self.corpus = Some((id, Corpus::from_translation(&self.translation)));
        }
    }

    /// Paints the palette, the search panel, the picker, and the verse menu.
    fn paint_overlays(&mut self, ctx: &egui::Context, colours: Palette) {
        let recent: Vec<Reference> = self.history.entries().rev().collect();
        let translations: Vec<(String, String)> = self
            .library
            .installed()
            .iter()
            .map(|installed| {
                (
                    installed.meta.id.clone(),
                    format!("{} ({})", installed.meta.abbreviation, installed.meta.name),
                )
            })
            .collect();
        if let Some(choice) = self.palette.show(ctx, colours, &recent, &translations) {
            let now = ctx.input(|input| input.time);
            match choice {
                Choice::Jump(reference) => self.navigate(reference, now),
                Choice::Translation(id) => {
                    if let Err(error) = self.switch_translation(&id) {
                        self.status = format!("{error:#}");
                    }
                }
                Choice::Command(action) => self.perform(action, ctx),
            }
        }

        if self.search.is_open() {
            self.rebuild_corpus();
            let corpus = &self
                .corpus
                .as_ref()
                .expect("the corpus was rebuilt above")
                .1;
            let chosen = self.search.show(ctx, colours, corpus, &self.translation);
            if let Some(reference) = chosen {
                let now = ctx.input(|input| input.time);
                self.navigate(reference, now);
            }
        }

        if let Some(reference) = self.picker.show(ctx, colours) {
            let now = ctx.input(|input| input.time);
            self.navigate(reference, now);
        }

        self.paint_verse_menu(ctx, colours);
        self.paint_shortcut_overlay(ctx, colours);
    }

    /// Paints the verse menu window and applies the row the reader picks.
    fn paint_verse_menu(&mut self, ctx: &egui::Context, colours: Palette) {
        if !self.menu.is_open() {
            return;
        }
        let mut open = true;
        let mut chosen = None;
        let mut draft = self.menu.note_draft().to_owned();
        egui::Window::new("Verse")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(target) = self.menu.target() {
                    ui.label(
                        RichText::new(target.to_string())
                            .font(FontId::new(13.0, theme::sans()))
                            .color(colours.ink),
                    );
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        for colour in bible_core::HighlightColour::ALL {
                            if ui.button(colour.name()).clicked() {
                                chosen = Some(MenuAction::Highlight(Some(colour)));
                            }
                        }
                        if ui.button("No colour").clicked() {
                            chosen = Some(MenuAction::Highlight(None));
                        }
                    });
                    ui.separator();
                    ui.add(
                        egui::TextEdit::multiline(&mut draft)
                            .hint_text("Note")
                            .desired_rows(3),
                    );
                    if ui.button("Save note").clicked() {
                        chosen = Some(MenuAction::Note);
                    }
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Bookmark").clicked() {
                            chosen = Some(MenuAction::Bookmark);
                        }
                        if ui.button("Copy").clicked() {
                            chosen = Some(MenuAction::Copy);
                        }
                        if ui.button("Copy with reference").clicked() {
                            chosen = Some(MenuAction::CopyWithReference);
                        }
                        if ui.button("Compare").clicked() {
                            chosen = Some(MenuAction::Compare);
                        }
                    });
                    let comparison = self.menu.comparison().to_vec();
                    if !comparison.is_empty() {
                        ui.separator();
                        for row in comparison {
                            ui.label(
                                RichText::new(row.abbreviation)
                                    .font(FontId::new(11.0, theme::sans()))
                                    .color(colours.accent),
                            );
                            ui.label(
                                RichText::new(row.text)
                                    .font(FontId::new(12.0, theme::body_family()))
                                    .color(colours.muted),
                            );
                        }
                    }
                }
            });
        self.menu.set_note_draft(draft);
        if let Some(action) = chosen {
            self.apply_menu(action, ctx);
        }
        if !open {
            self.menu.close();
        }
    }

    /// Paints the keyboard shortcut overlay, which reads the same table the
    /// key handler reads, so a shortcut cannot be listed and missing.
    fn paint_shortcut_overlay(&mut self, ctx: &egui::Context, colours: Palette) {
        if !self.overlay {
            return;
        }
        let mut open = true;
        egui::Window::new("Keyboard shortcuts")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(520.0)
                    .show(ui, |ui| {
                        for (description, label) in shortcuts::overlay_rows() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(description)
                                        .font(FontId::new(12.0, theme::sans()))
                                        .color(colours.accent),
                                );
                                ui.label(
                                    RichText::new(label)
                                        .font(FontId::new(12.0, theme::sans()))
                                        .color(colours.muted),
                                );
                            });
                        }
                    });
            });
        self.overlay = open;
    }
}

/// Maps a toolkit key to the key the shortcut table records.
fn shortcut_key(key: egui::Key) -> Option<ShortcutKey> {
    Some(match key {
        egui::Key::A => ShortcutKey::Char('a'),
        egui::Key::B => ShortcutKey::Char('b'),
        egui::Key::C => ShortcutKey::Char('c'),
        egui::Key::D => ShortcutKey::Char('d'),
        egui::Key::E => ShortcutKey::Char('e'),
        egui::Key::F => ShortcutKey::Char('f'),
        egui::Key::G => ShortcutKey::Char('g'),
        egui::Key::H => ShortcutKey::Char('h'),
        egui::Key::I => ShortcutKey::Char('i'),
        egui::Key::J => ShortcutKey::Char('j'),
        egui::Key::K => ShortcutKey::Char('k'),
        egui::Key::L => ShortcutKey::Char('l'),
        egui::Key::M => ShortcutKey::Char('m'),
        egui::Key::N => ShortcutKey::Char('n'),
        egui::Key::O => ShortcutKey::Char('o'),
        egui::Key::P => ShortcutKey::Char('p'),
        egui::Key::Q => ShortcutKey::Char('q'),
        egui::Key::R => ShortcutKey::Char('r'),
        egui::Key::S => ShortcutKey::Char('s'),
        egui::Key::T => ShortcutKey::Char('t'),
        egui::Key::U => ShortcutKey::Char('u'),
        egui::Key::V => ShortcutKey::Char('v'),
        egui::Key::W => ShortcutKey::Char('w'),
        egui::Key::X => ShortcutKey::Char('x'),
        egui::Key::Y => ShortcutKey::Char('y'),
        egui::Key::Z => ShortcutKey::Char('z'),
        egui::Key::Num0 => ShortcutKey::Char('0'),
        egui::Key::Num1 => ShortcutKey::Char('1'),
        egui::Key::Num2 => ShortcutKey::Char('2'),
        egui::Key::Num3 => ShortcutKey::Char('3'),
        egui::Key::Num4 => ShortcutKey::Char('4'),
        egui::Key::Num5 => ShortcutKey::Char('5'),
        egui::Key::Num6 => ShortcutKey::Char('6'),
        egui::Key::Num7 => ShortcutKey::Char('7'),
        egui::Key::Num8 => ShortcutKey::Char('8'),
        egui::Key::Num9 => ShortcutKey::Char('9'),
        egui::Key::OpenBracket => ShortcutKey::Char('['),
        egui::Key::CloseBracket => ShortcutKey::Char(']'),
        egui::Key::Minus => ShortcutKey::Char('-'),
        egui::Key::Plus | egui::Key::Equals => ShortcutKey::Char('+'),
        egui::Key::ArrowLeft => ShortcutKey::Left,
        egui::Key::ArrowRight => ShortcutKey::Right,
        _ => return None,
    })
}

impl Drop for ReaderApp {
    /// A harness drops the app without an `eframe` shutdown hook, so the pending
    /// settings write is flushed here as well as in the window's exit hook.
    fn drop(&mut self) {
        self.flush_settings();
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
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.paint(ui);
    }

    /// Writes any pending settings change before the window closes.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_settings();
    }
}

/// A panel state the render harness can photograph.
///
/// The harness reaches these states the same way the shortcut does, so the
/// picture proves the wiring rather than a separate paint path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotView {
    /// The jump palette, open with a reference typed in.
    Palette,
    /// The search panel, open with a query typed in.
    Search,
    /// The book and chapter picker.
    Picker,
    /// The verse menu on the current selection.
    VerseMenu,
    /// The keyboard shortcut overlay.
    Shortcuts,
    /// The study sidebar, with one synthetic mark of each kind.
    StudySidebar,
    /// The parallel translations view, with one synthetic mark of each kind.
    Parallel,
}

impl ShotView {
    /// Recovers a view from the name the harness environment uses.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "palette" => Some(Self::Palette),
            "search" => Some(Self::Search),
            "picker" => Some(Self::Picker),
            "menu" | "verse-menu" => Some(Self::VerseMenu),
            "shortcuts" | "overlay" => Some(Self::Shortcuts),
            "sidebar" | "study-sidebar" => Some(Self::StudySidebar),
            "parallel" => Some(Self::Parallel),
            _ => None,
        }
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
    /// A panel to open before the picture is taken.
    pub view: Option<ShotView>,
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
            view: None,
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
    /// - `SHOT_DEMO` opens `palette`, `search`, `picker`, `menu`, `shortcuts`,
    ///   `sidebar`, or `parallel`.
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
        if let Some(view) = std::env::var_os("SHOT_DEMO") {
            let view = view.to_string_lossy();
            spec.view = Some(
                ShotView::from_name(&view)
                    .ok_or_else(|| anyhow!("no shot view called {view:?}"))?,
            );
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
    let view = spec.view;
    // The sidebar and parallel demos write study marks, so they must open a
    // store in a scratch folder rather than the one a reader has on this machine.
    let scratch_data = match view {
        Some(ShotView::StudySidebar | ShotView::Parallel) => Some(shot_scratch_dir()?),
        _ => None,
    };
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| {
            let mut app = match &scratch_data {
                Some(data) => ReaderApp::open_at(cc, theme, reference, data),
                None => {
                    ReaderApp::open_with_translation(cc, theme, reference, translation.as_deref())
                }
            };
            if let Some(view) = view {
                app.set_view(view);
            }
            app
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

/// A unique scratch data folder for a shot that writes study marks.
fn shot_scratch_dir() -> Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("bible-sidebar-shot-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("cannot create the shot folder {}", dir.display()))?;
    Ok(dir)
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

/// The translations a column header can switch to, in menu order.
///
/// The bundled translations come first, because they are always available, and
/// an installed translation of the same identifier replaces its bundled entry
/// in the library rather than appearing twice.
fn translation_choices(library: &Library) -> Vec<(String, String)> {
    let mut choices: Vec<(String, String)> = crate::BUNDLED_TRANSLATIONS
        .iter()
        .filter_map(|bundled| bundled.meta().ok())
        .map(|meta| {
            (
                meta.id.clone(),
                format!("{} ({})", meta.abbreviation, meta.name),
            )
        })
        .collect();
    for installed in library.installed() {
        if !choices.iter().any(|(id, _)| *id == installed.meta.id) {
            choices.push((
                installed.meta.id.clone(),
                format!("{} ({})", installed.meta.abbreviation, installed.meta.name),
            ));
        }
    }
    choices
}

/// Maps a saved settings row onto the reader's theme and typography.
///
/// `paragraph_mode` has no counterpart in the core settings row, so it keeps
/// the current GUI default rather than being invented here.
fn preferences(settings: &Settings) -> (Theme, Typography) {
    let theme = match settings.theme {
        StoredTheme::Light => Theme::Light,
        StoredTheme::Sepia => Theme::Sepia,
        StoredTheme::Dark => Theme::Dark,
    };
    let typography = Typography {
        size: settings.text_size,
        line_height: settings.line_spacing,
        measure_em: settings.line_length,
        justify: settings.justify,
        verse_numbers: settings.show_verse_numbers,
        headings: settings.show_headings,
        red_letters: settings.words_of_christ_red,
        ..Typography::default()
    };
    (theme, typography)
}

/// The chapter a shot opens by default, used when no reference is given. John 3
/// is the sample page that carries every kind of mark and span.
pub fn default_reference() -> Reference {
    Reference::new(42, 3, None).expect("John 3 is canonical")
}
