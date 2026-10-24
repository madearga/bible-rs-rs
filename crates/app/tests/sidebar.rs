// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The study sidebar tests that drive the real window.
//!
//! They open the app offscreen against a scratch data folder, then reach the
//! sidebar through the same toolbar button or harness view a reader uses. The
//! seeded rows live only in the scratch store, never in the reader's own data.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use bible_app::app::{ReaderApp, ShotSpec, ShotView, default_reference, render_shot};
use bible_app::sidebar::RowKind;
use bible_app::theme::Theme;
use bible_core::Reference;
use eframe::egui;
use egui_kittest::kittest::Queryable;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch folder that no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-sidebar-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// Builds a harness around the real app with its own scratch data folder.
fn harness(name: &str) -> egui_kittest::Harness<'static, ReaderApp> {
    harness_sized(name, egui::vec2(900.0, 700.0))
}

/// Builds a harness with a given window size, for tests that need the whole
/// sidebar column on screen at once.
fn harness_sized(name: &str, size: egui::Vec2) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = scratch(name);
    egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, Theme::Light, default_reference(), &data))
}

/// A canonical verse for the sidebar tests.
fn verse(book: u8, chapter: u8, verse: u8) -> Reference {
    Reference::new(book, chapter, Some(verse)).expect("the verse is canonical")
}

#[test]
fn the_toolbar_button_opens_and_closes_the_sidebar() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("open-close");
    harness.run();
    assert!(
        !harness.state().sidebar_is_open(),
        "the sidebar starts closed"
    );
    harness.get_by_label("Sidebar").click();
    harness.run();
    assert!(
        harness.state().sidebar_is_open(),
        "the Sidebar button did not open the sidebar"
    );
    harness.get_by_label("Sidebar").click();
    harness.run();
    assert!(
        !harness.state().sidebar_is_open(),
        "the Sidebar button did not close the sidebar"
    );
}

#[test]
fn opening_the_sidebar_reveals_the_books_around_the_reader() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("reveal");
    harness.run();
    harness.get_by_label("Sidebar").click();
    harness.run();
    assert!(
        harness.query_by_label_contains("Genesis").is_none(),
        "the Old Testament should stay folded when the reader is in John"
    );
    assert!(
        harness.query_by_label("3").is_some(),
        "the current chapter's chip is not on screen"
    );
}

#[test]
fn a_chapter_chip_jumps_to_its_chapter_and_records_history() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("chip");
    harness.run();
    harness.get_by_label("Sidebar").click();
    harness.run();
    harness.get_by_label("5").click();
    harness.run();
    let expected = Reference::new(42, 5, None).expect("John 5 is canonical");
    assert_eq!(harness.state().reference(), expected);
    assert_eq!(harness.state().history_current(), Some(expected));
}

#[test]
fn the_seeded_sidebar_lists_all_three_kinds() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("three-kinds");
    harness.run();
    harness.state_mut().set_view(ShotView::StudySidebar);
    harness.run();

    let rows = harness.state().study_rows();
    assert_eq!(rows.len(), 3, "the demo seed should produce three rows");
    assert_eq!(rows[0].kind, RowKind::Highlight);
    assert_eq!(rows[1].kind, RowKind::Note);
    assert_eq!(rows[2].kind, RowKind::Bookmark);
    assert_eq!(rows[0].reference, verse(42, 3, 16));
    assert_eq!(rows[1].reference, verse(42, 3, 17));
    assert_eq!(rows[2].reference, verse(42, 3, 18));
}

#[test]
fn activating_a_row_jumps_to_its_reference_with_history() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    // The study rows sit below the Books section, so the window is tall enough
    // to show the whole column and the row is clickable.
    let mut harness = harness_sized("activate", egui::vec2(900.0, 1400.0));
    harness.run();
    harness.state_mut().set_view(ShotView::StudySidebar);
    harness.run();

    let bookmark = harness
        .state()
        .study_rows()
        .iter()
        .find(|row| row.kind == RowKind::Bookmark)
        .expect("the demo seed has a bookmark row")
        .clone();
    harness.get_by_label(&bookmark.title()).click();
    harness.run();

    assert_eq!(
        harness.state().selected_verse(),
        Some(bookmark.reference),
        "the row did not select the bookmark's verse"
    );
    assert_eq!(
        harness.state().history_current(),
        Some(bookmark.reference.chapter_only()),
        "the row jump did not reach the history"
    );
}

#[test]
fn an_empty_store_shows_the_empty_state() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness_sized("empty", egui::vec2(900.0, 1400.0));
    harness.run();
    harness.get_by_label("Sidebar").click();
    harness.run();
    assert!(harness.state().study_rows().is_empty());
    assert!(
        harness
            .query_by_label_contains("No study marks yet")
            .is_some(),
        "the empty state text is not on screen"
    );
}

#[test]
fn the_margin_helper_reports_note_and_bookmark_marks() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("margins");
    harness.run();
    harness.state_mut().set_view(ShotView::StudySidebar);
    harness.run();

    let note = harness.state().margin_markers(verse(42, 3, 17));
    assert!(note.note, "the note indicator is missing");
    assert!(!note.bookmark, "the note verse is not bookmarked");

    let bookmark = harness.state().margin_markers(verse(42, 3, 18));
    assert!(bookmark.bookmark, "the bookmark indicator is missing");
    assert!(!bookmark.note, "the bookmark verse has no note");

    let plain = harness.state().margin_markers(verse(42, 3, 19));
    assert!(plain.is_empty(), "an unmarked verse reports a mark");
}

#[test]
fn the_sidebar_shot_is_written_in_a_scratch_folder() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let path = scratch("shot").join("sidebar.png");
    let spec = ShotSpec {
        path: path.clone(),
        size: [640, 480],
        theme: Theme::Light,
        view: Some(ShotView::StudySidebar),
        pause: Duration::ZERO,
        ..ShotSpec::default()
    };
    render_shot(&spec).expect("the sidebar shot renders");
    assert!(path.exists(), "the sidebar PNG was not written");
    let bytes = std::fs::read(&path).expect("the PNG was written");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn a_reopened_window_restores_the_sidebar_state() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let folder = scratch("restore");
    {
        let data = folder.clone();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 700.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_eframe(move |cc| {
                ReaderApp::open_in(cc, Theme::Light, default_reference(), &data)
            });
        harness.run();
        harness.get_by_label("Sidebar").click();
        harness.run();
        assert!(harness.state().sidebar_is_open());
        // Dropping the harness drops the app, which flushes the settings the
        // throttle is still holding.
    }
    let data = folder;
    let mut reopened = egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_in(cc, Theme::Light, default_reference(), &data));
    reopened.run();
    assert!(
        reopened.state().sidebar_is_open(),
        "the reopened window did not restore the open sidebar"
    );
    assert!(
        reopened.query_by_label("3").is_some(),
        "the reopened sidebar did not reveal the chapter the reader is in"
    );
}
