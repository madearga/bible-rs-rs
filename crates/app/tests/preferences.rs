// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! Preference persistence tests that drive the real window.
//!
//! Each test owns a scratch data folder, seeds the core store directly when it
//! needs an existing row, and then opens the app through the same constructors
//! the window and the harness use. The GPU lock matches the other integration
//! tests so the Metal device is never created twice at once.

use std::path::PathBuf;
use std::sync::Mutex;

use bible_app::app::{ReaderApp, default_reference};
use bible_app::theme::Theme;
use bible_core::Reference;
use bible_core::study::{Store, Theme as StoredTheme};
use eframe::egui;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch data folder no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-preferences-{}-{name}", std::process::id()));
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

/// Writes a non-default settings row and closes the store so the app can lock it.
fn seed_nondefault(dir: &std::path::Path, position: Reference) {
    let mut store = Store::open(dir.join("study.db")).expect("study store");
    let mut settings = store.settings().clone();
    settings.position = position;
    settings.theme = StoredTheme::Dark;
    settings.text_size = 23.0;
    settings.line_spacing = 1.8;
    settings.line_length = 40.0;
    settings.justify = true;
    settings.show_verse_numbers = false;
    settings.show_headings = false;
    settings.words_of_christ_red = false;
    store.set_settings(settings).expect("settings are accepted");
    store.close().expect("the settings row is written");
}

/// Opens a normal window, which restores the saved place and appearance.
fn open_normal(data: &std::path::Path) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = data.to_path_buf();
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_in(cc, Theme::Light, default_reference(), &data))
}

/// Opens a shot-style window, which honours the explicit theme and place.
fn open_explicit(
    data: &std::path::Path,
    theme: Theme,
    reference: Reference,
) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = data.to_path_buf();
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, theme, reference, &data))
}

#[test]
fn a_normal_window_restores_the_saved_appearance_and_place() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("restore");
    let saved = Reference::parse("jn 3:16").expect("a reference");
    seed_nondefault(&dir, saved);

    let mut harness = open_normal(&dir);
    harness.run();

    assert_eq!(harness.state().theme(), Theme::Dark);
    let typography = harness.state().typography();
    assert_eq!(typography.size, 23.0);
    assert_eq!(typography.line_height, 1.8);
    assert_eq!(typography.measure_em, 40.0);
    assert!(typography.justify);
    assert!(!typography.verse_numbers);
    assert!(!typography.headings);
    assert!(!typography.red_letters);
    let place = harness.state().reference();
    assert_eq!((place.book, place.chapter), (42, 3));
    drop(harness);
}

#[test]
fn appearance_shortcuts_survive_an_immediate_restart() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("roundtrip");

    let mut harness = open_normal(&dir);
    harness.run();
    // A fresh store keeps the current GUI typography, not the core 17 point
    // default, and uses the caller's theme until something is saved.
    assert_eq!(harness.state().theme(), Theme::Light);
    assert_eq!(harness.state().typography().size, 19.0);
    assert!(harness.state().typography().red_letters);

    harness.key_press_modifiers(
        egui::Modifiers {
            command: true,
            shift: true,
            ..egui::Modifiers::default()
        },
        egui::Key::L,
    );
    harness.run();
    assert_eq!(harness.state().theme(), Theme::Sepia);

    harness.key_press_modifiers(command(), egui::Key::Plus);
    harness.run();
    assert_eq!(harness.state().typography().size, 20.0);

    harness.key_press_modifiers(command(), egui::Key::R);
    harness.run();
    assert!(!harness.state().typography().red_letters);

    // Drop the app immediately, well inside the 1.5 second write throttle. The
    // close flush is what makes the shortcut changes durable.
    drop(harness);

    let store = Store::open(dir.join("study.db")).expect("the app released the store");
    let saved = store.settings().clone();
    assert_eq!(saved.theme, StoredTheme::Sepia);
    assert_eq!(saved.text_size, 20.0);
    assert!(!saved.words_of_christ_red);
    store
        .close()
        .expect("a read-only reopen still closes cleanly");

    let mut reopened = open_normal(&dir);
    reopened.run();
    assert_eq!(reopened.state().theme(), Theme::Sepia);
    assert_eq!(reopened.state().typography().size, 20.0);
    assert!(!reopened.state().typography().red_letters);
    drop(reopened);
}

#[test]
fn an_explicit_shot_theme_and_place_ignore_the_saved_preferences() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("explicit");
    let saved = Reference::parse("jn 3:16").expect("a reference");
    seed_nondefault(&dir, saved);

    let shot = Reference::parse("ps 23").expect("a reference");
    let mut harness = open_explicit(&dir, Theme::Sepia, shot);
    harness.run();

    assert_eq!(harness.state().theme(), Theme::Sepia);
    let place = harness.state().reference();
    assert_eq!((place.book, place.chapter), (18, 23));
    // A shot must not inherit the saved font size or red-letter choice either.
    assert_eq!(
        harness.state().typography(),
        bible_app::theme::Typography::default()
    );
    drop(harness);
}
