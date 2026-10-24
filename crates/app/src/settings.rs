// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reading preferences window.
//!
//! The window paints every appearance control the reader has and edits the live
//! theme and typography in place. It never writes the store itself, because the
//! shell applies the result through the same save path the keyboard actions use.
//! That keeps one writer for the settings row and one place where a value is
//! mapped onto the core record.

use eframe::egui;
use egui::RichText;

use crate::theme;
use crate::theme::{Palette as Colours, Theme, Typography};

/// The modal settings window and whether it is on screen.
#[derive(Clone, Copy, Debug, Default)]
pub struct SettingsWindow {
    open: bool,
}

impl SettingsWindow {
    /// A closed settings window.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the window on the current appearance.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Closes the window.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the window is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Paints the window and reports whether a control changed the appearance.
    ///
    /// The caller owns the theme and typography, so a change reaches the store
    /// through the shell's normal save path and survives a restart the same way
    /// a keyboard shortcut does. Escape and the close button both close the
    /// window without touching the appearance.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        colours: Colours,
        theme: &mut Theme,
        typography: &mut Typography,
    ) -> bool {
        if !self.open {
            return false;
        }
        let before = (*theme, *typography);
        let mut open = self.open;
        let mut close_requested = false;
        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(
                    RichText::new("Reading preferences")
                        .font(egui::FontId::new(14.0, theme::sans()))
                        .color(colours.ink),
                );
                ui.separator();

                ui.label(
                    RichText::new("Theme")
                        .font(egui::FontId::new(12.0, theme::sans()))
                        .color(colours.muted),
                );
                ui.horizontal(|ui| {
                    for choice in theme::THEMES {
                        if ui
                            .selectable_label(*theme == choice, title_case(choice))
                            .clicked()
                        {
                            *theme = choice;
                        }
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Smaller text").clicked() {
                        typography.size = (typography.size - 1.0).max(12.0);
                    }
                    if ui.button("Larger text").clicked() {
                        typography.size = (typography.size + 1.0).min(32.0);
                    }
                    ui.label(
                        RichText::new(format!("Text size: {:.0} points", typography.size))
                            .font(egui::FontId::new(12.0, theme::sans()))
                            .color(colours.ink),
                    );
                });

                let paragraph_label = if typography.paragraph_mode {
                    "Verses flow inside a paragraph"
                } else {
                    "One verse per line"
                };
                ui.checkbox(&mut typography.paragraph_mode, paragraph_label);

                ui.add(
                    egui::Slider::new(&mut typography.line_height, 1.0..=2.6).text("Line spacing"),
                );
                ui.add(
                    egui::Slider::new(&mut typography.measure_em, 18.0..=60.0).text("Line length"),
                );

                ui.checkbox(&mut typography.justify, "Justify text");
                ui.checkbox(&mut typography.verse_numbers, "Verse numbers");
                ui.checkbox(&mut typography.headings, "Section headings");
                ui.checkbox(&mut typography.red_letters, "Words of Christ in red");

                ui.separator();
                if ui.button("Close").clicked() {
                    close_requested = true;
                }
            });
        if close_requested || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        self.open = open;
        before != (*theme, *typography)
    }
}

/// The label a theme button shows, which is the name with a capital letter.
fn title_case(theme: Theme) -> &'static str {
    match theme {
        Theme::Light => "Light",
        Theme::Sepia => "Sepia",
        Theme::Dark => "Dark",
        Theme::Auto => "Auto",
    }
}
