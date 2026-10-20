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
use bible_core::study::Store;
use eframe::egui;
use egui::{Align, FontId, Pos2, Rect, RichText, Stroke, pos2, vec2};

use crate::Library;
use crate::book_picker::BookPicker;
use crate::history::History;
use crate::palette::{Choice, Palette as JumpPalette};
use crate::reader::Reader;
use crate::search_panel::SearchPanel;
use crate::selection::{self, Selection};
use crate::shortcuts::{self, Action, Combo, Key as ShortcutKey};
use crate::theme::{self, Palette, Theme, Typography};
use crate::typography;
use crate::verse_menu::{CompareRow, MenuAction, VerseMenu};

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
    corpus: Option<(String, Corpus)>,
    flash: Option<(Reference, f64)>,
    overlay: bool,
    saved_position: Reference,
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
        Self::build(
            cc,
            theme,
            reference,
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
            theme,
            reference,
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
            theme,
            reference,
            library,
            translation,
            String::new(),
            data,
        )
    }

    /// Builds the window on a place with a study store opened on the side.
    fn build(
        cc: &eframe::CreationContext<'_>,
        theme: Theme,
        reference: Reference,
        library: Library,
        translation: Arc<Translation>,
        mut status: String,
        data: &std::path::Path,
    ) -> Self {
        theme::install(&cc.egui_ctx, theme);
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
        // last one stopped rather than at the first page.
        let remembered = study.settings().position;
        let start = if remembered.chapter == 0 {
            reference
        } else {
            remembered
        };
        Self {
            library,
            translation,
            reference: start.chapter_only(),
            theme,
            typography: Typography::default(),
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
            corpus: None,
            flash: None,
            overlay: false,
            saved_position: start.chapter_only(),
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

    /// The place the reader was, which the history also holds.
    pub fn history_current(&self) -> Option<Reference> {
        self.history.current()
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
        self.history.visit(reference.chapter_only());
        self.flash = Some((reference, now + 0.9));
        self.remember(reference);
    }

    /// Shows the place the history moved to without recording a new visit.
    fn show_place(&mut self, reference: Reference, now: f64) {
        self.selection.select(reference);
        self.reference = reference.chapter_only();
        self.reader.set_position(reference.chapter_only());
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
    /// - `SHOT_DEMO` opens `palette`, `search`, `picker`, `menu`, or `shortcuts`.
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
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| {
            let mut app =
                ReaderApp::open_with_translation(cc, theme, reference, translation.as_deref());
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
