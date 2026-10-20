// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The navigation tests that drive the real window.
//!
//! They open the app offscreen, press the keys the spec lists, and assert on
//! what the app did. The data folder is a scratch folder, so the test never
//! touches the reader's own study file and never reads scripture: the sample
//! translation carries generated words only.

use std::path::PathBuf;
use std::sync::Mutex;

use bible_app::app::{ReaderApp, default_reference};
use bible_app::theme::Theme;
use eframe::egui;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// The scratch data folder the tests point the app at.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-nav-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The command modifier as the toolkit reports it.
fn command() -> egui::Modifiers {
    egui::Modifiers {
        command: true,
        ..egui::Modifiers::default()
    }
}

/// Builds a harness around the real app, with the scratch data folder in place.
fn harness() -> egui_kittest::Harness<'static, ReaderApp> {
    let data = scratch();
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, Theme::Light, default_reference(), &data))
}

#[test]
fn command_k_opens_the_jump_palette() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    assert!(!harness.state().palette_is_open());
    harness.key_press_modifiers(command(), egui::Key::K);
    harness.run();
    assert!(
        harness.state().palette_is_open(),
        "command-K did not open the palette"
    );
}

#[test]
fn command_f_opens_the_search_panel() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    assert!(!harness.state().search_is_open());
    harness.key_press_modifiers(command(), egui::Key::F);
    harness.run();
    assert!(
        harness.state().search_is_open(),
        "command-F did not open the search panel"
    );
}

#[test]
fn command_b_opens_the_book_picker() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    assert!(!harness.state().picker_is_open());
    harness.key_press_modifiers(command(), egui::Key::B);
    harness.run();
    assert!(
        harness.state().picker_is_open(),
        "command-B did not open the picker"
    );
}

#[test]
fn the_palette_jumps_to_the_typed_reference() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    harness.key_press_modifiers(command(), egui::Key::K);
    harness.run();
    harness.root().type_text("jn 3:16");
    harness.run();
    assert_eq!(harness.state().palette_input(), "jn 3:16");
    harness.key_press(egui::Key::Enter);
    harness.run();
    let reference = harness.state().reference();
    assert_eq!(reference.book, 42, "the palette did not jump to John");
    assert_eq!(reference.chapter, 3);
}

#[test]
fn the_back_shortcut_returns_to_the_previous_chapter() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    let start = harness.state().reference();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert_eq!(harness.state().reference().chapter, start.chapter + 1);
    harness.key_press_modifiers(command(), egui::Key::OpenBracket);
    harness.run();
    assert_eq!(
        harness.state().reference().chapter,
        start.chapter,
        "the back shortcut did not return to the previous chapter"
    );
}

#[test]
fn the_note_key_opens_the_verse_menu_on_the_selected_verse() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    harness.key_press_modifiers(command(), egui::Key::K);
    harness.run();
    harness.root().type_text("jn 3:16");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(!harness.state().menu_is_open());
    harness.key_press(egui::Key::N);
    harness.run();
    assert!(
        harness.state().menu_is_open(),
        "the N key did not open the verse menu"
    );
}

#[test]
fn the_history_holds_the_place_the_reader_jumped_to() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    harness.key_press_modifiers(command(), egui::Key::K);
    harness.run();
    harness.root().type_text("ps 23");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    let place = harness.state().history_current().expect("a place is held");
    assert_eq!(place.book, 18);
    assert_eq!(place.chapter, 23);
}

#[test]
fn the_right_arrow_moves_to_the_next_chapter() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness();
    harness.run();
    let start = harness.state().reference();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    let after = harness.state().reference();
    assert_eq!(after.chapter, start.chapter + 1);
}
