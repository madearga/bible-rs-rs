// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Settings window tests that drive the real window.
//!
//! Each test owns a scratch data folder and reaches the window through the same
//! harness view or menu action a reader uses. The appearance is asserted in the
//! store after the window closes, so the test proves the save path rather than a
//! paint path.

use std::path::PathBuf;
use std::sync::Mutex;

use bible_app::app::{ReaderApp, ShotView, default_reference};
use bible_app::theme::Theme;
use bible_core::study::{Store, Theme as StoredTheme};
use eframe::egui;
use egui_kittest::kittest::Queryable;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch data folder no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-settings-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// Opens a normal window, which restores the saved appearance.
fn open_normal(data: &std::path::Path) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = data.to_path_buf();
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_in(cc, Theme::Light, default_reference(), &data))
}

/// Opens a shot-style window with a known appearance and a scratch store.
fn open_explicit(data: &std::path::Path) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = data.to_path_buf();
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, Theme::Light, default_reference(), &data))
}

/// Writes a saved appearance with a theme, size, and paragraph mode to check in
/// the window.
fn seed_appearance(dir: &std::path::Path, theme: StoredTheme, size: f32, paragraph: bool) {
    let mut store = Store::open(dir.join("study.db")).expect("study store");
    let mut settings = store.settings().clone();
    settings.theme = theme;
    settings.text_size = size;
    settings.paragraph_mode = paragraph;
    store.set_settings(settings).expect("settings are accepted");
    store.close().expect("the settings row is written");
}

#[test]
fn the_window_opens_from_the_shot_view_and_saves_its_changes() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("save");
    let mut harness = open_explicit(&dir);
    harness.run();
    assert!(
        !harness.state().settings_window_is_open(),
        "the window starts closed"
    );
    assert_eq!(
        harness.state().status(),
        "",
        "a scratch data folder should leave the status line clean"
    );

    harness.state_mut().set_view(ShotView::Settings);
    harness.run();
    assert!(
        harness.state().settings_window_is_open(),
        "the Settings view did not open the window"
    );

    harness.get_by_label("Larger text").click();
    harness.run();
    harness
        .get_by_label("Verses flow inside a paragraph")
        .click();
    harness.run();
    assert!(
        !harness.state().typography().paragraph_mode,
        "the checkbox did not switch the reader to one verse per line"
    );
    assert_eq!(harness.state().typography().size, 20.0);

    harness.get_by_label("Close").click();
    harness.run();
    assert!(
        !harness.state().settings_window_is_open(),
        "the Close button did not close the window"
    );
    drop(harness);

    let store = Store::open(dir.join("study.db")).expect("the app released the store");
    let saved = store.settings().clone();
    assert_eq!(
        saved.text_size, 20.0,
        "the text size did not reach the store"
    );
    assert!(
        !saved.paragraph_mode,
        "the paragraph mode did not reach the store"
    );
    store
        .close()
        .expect("a read-only reopen still closes cleanly");
}

#[test]
fn escape_closes_the_window() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("escape");
    let mut harness = open_explicit(&dir);
    harness.run();
    harness.state_mut().set_view(ShotView::Settings);
    harness.run();
    assert!(harness.state().settings_window_is_open());

    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        !harness.state().settings_window_is_open(),
        "Escape did not close the settings window"
    );
}

#[test]
fn the_window_shows_the_saved_theme_size_and_paragraph_mode() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("shows");
    seed_appearance(&dir, StoredTheme::Dark, 23.0, false);

    let mut harness = open_normal(&dir);
    harness.run();
    harness.state_mut().set_view(ShotView::Settings);
    harness.run();

    assert_eq!(harness.state().theme(), Theme::Dark);
    assert!(
        harness
            .query_by_label_contains("Text size: 23 points")
            .is_some(),
        "the window does not show the saved text size"
    );
    assert!(
        harness
            .query_by_label_contains("One verse per line")
            .is_some(),
        "the window does not show the saved paragraph mode"
    );
    assert!(
        harness.query_by_label_contains("Dark").is_some(),
        "the window does not show the saved theme"
    );
}
