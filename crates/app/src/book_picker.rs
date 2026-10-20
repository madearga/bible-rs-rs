// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The book and chapter picker.
//!
//! The picker is a two column browser. The left column lists the books of the
//! chosen testament and the right column lists the chapters of the chosen book,
//! so reaching any chapter takes two clicks. It is a plain model with a small
//! paint routine, which is what lets the toolbar and the sidebar share it.

use bible_core::{Book, Reference, Testament, books};
use eframe::egui;

use crate::theme::Palette as Colours;

/// A book and chapter browser.
#[derive(Clone, Debug)]
pub struct BookPicker {
    open: bool,
    testament: Testament,
    book: u8,
    chapter: u8,
}

impl Default for BookPicker {
    fn default() -> Self {
        Self {
            open: false,
            testament: Testament::Old,
            book: 0,
            chapter: 1,
        }
    }
}

impl BookPicker {
    /// A closed picker pointing at Genesis 1.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the picker on the testament and book of a place.
    pub fn open_at(&mut self, reference: Reference) {
        self.open = true;
        self.book = reference.book;
        self.chapter = reference.chapter.clamp(1, self.chapter_count());
        if let Some(book) = books::book(reference.book) {
            self.testament = book.testament;
        }
    }

    /// Closes the picker.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the picker is on screen.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The testament whose books the left column shows.
    pub fn testament(&self) -> Testament {
        self.testament
    }

    /// Switches the testament and moves the selected book into it.
    pub fn set_testament(&mut self, testament: Testament) {
        if self.testament == testament {
            return;
        }
        self.testament = testament;
        if let Some(first) = self.books().first().map(|book| book.index) {
            self.book = first;
            self.chapter = 1;
        }
    }

    /// The books listed in the left column.
    pub fn books(&self) -> Vec<&'static Book> {
        books::books()
            .iter()
            .filter(|book| book.testament == self.testament)
            .collect()
    }

    /// The selected book index.
    pub fn selected_book(&self) -> u8 {
        self.book
    }

    /// Selects a book and moves the chapter to its first.
    pub fn select_book(&mut self, book: u8) {
        if books::book(book).is_some() {
            self.book = book;
            self.chapter = 1;
        }
    }

    /// How many chapters the selected book has.
    pub fn chapter_count(&self) -> u8 {
        books::book(self.book).map_or(1, |book| book.chapter_count)
    }

    /// The selected chapter number.
    pub fn selected_chapter(&self) -> u8 {
        self.chapter
    }

    /// Selects a chapter inside the selected book.
    pub fn select_chapter(&mut self, chapter: u8) {
        if chapter >= 1 && chapter <= self.chapter_count() {
            self.chapter = chapter;
        }
    }

    /// The place the current selection names.
    pub fn reference(&self) -> Option<Reference> {
        Reference::new(self.book, self.chapter, None).ok()
    }

    /// Paints the picker and returns a place when the reader picks a chapter.
    pub fn show(&mut self, ctx: &egui::Context, colours: Colours) -> Option<Reference> {
        if !self.open {
            return None;
        }
        let mut chosen = None;
        let mut open = self.open;
        egui::Window::new("Book and chapter")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for testament in [Testament::Old, Testament::New] {
                        let label = match testament {
                            Testament::Old => "Old Testament",
                            Testament::New => "New Testament",
                        };
                        if ui
                            .selectable_label(self.testament == testament, label)
                            .clicked()
                        {
                            self.set_testament(testament);
                        }
                    }
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    let books = self.books();
                    ui.vertical(|ui| {
                        ui.set_width(160.0);
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("picker-books")
                            .show(ui, |ui| {
                                for book in books {
                                    let selected = book.index == self.book;
                                    if ui
                                        .selectable_label(
                                            selected,
                                            egui::RichText::new(book.name).color(if selected {
                                                colours.ink
                                            } else {
                                                colours.muted
                                            }),
                                        )
                                        .clicked()
                                    {
                                        self.select_book(book.index);
                                    }
                                }
                            });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(70.0);
                        let count = self.chapter_count();
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .id_salt("picker-chapters")
                            .show(ui, |ui| {
                                for chapter in 1..=count {
                                    let selected = chapter == self.chapter;
                                    if ui
                                        .selectable_label(
                                            selected,
                                            egui::RichText::new(chapter.to_string()).color(
                                                if selected {
                                                    colours.accent
                                                } else {
                                                    colours.muted
                                                },
                                            ),
                                        )
                                        .clicked()
                                    {
                                        self.select_chapter(chapter);
                                        chosen = self.reference();
                                    }
                                }
                            });
                    });
                });
            });
        self.open = open && chosen.is_none();
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_opens_on_the_current_place() {
        let mut picker = BookPicker::new();
        let reference = Reference::new(45, 13, None).expect("canonical");
        picker.open_at(reference);
        assert!(picker.is_open());
        assert_eq!(picker.testament(), Testament::New);
        assert_eq!(picker.selected_book(), 45);
        assert_eq!(picker.selected_chapter(), 13);
    }

    #[test]
    fn switching_testament_moves_to_its_first_book() {
        let mut picker = BookPicker::new();
        picker.open_at(Reference::new(0, 1, None).expect("canonical"));
        picker.set_testament(Testament::New);
        assert_eq!(picker.selected_book(), 39);
        assert_eq!(picker.selected_chapter(), 1);
    }

    #[test]
    fn a_chapter_outside_the_book_is_ignored() {
        let mut picker = BookPicker::new();
        picker.select_book(64);
        picker.select_chapter(2);
        assert_eq!(picker.selected_chapter(), 1);
    }
}
