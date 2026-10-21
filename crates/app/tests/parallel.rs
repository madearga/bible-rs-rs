// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The parallel view tests.
//!
//! The first half drives the view model directly, so alignment, scrolling, and
//! column swapping are checked without a window. The second half drives the real
//! app offscreen against a scratch data folder, so the toggle, the shared
//! selection, and the harness shot are checked through the same code a reader
//! reaches with the toolbar button.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bible_app::Library;
use bible_app::app::{ReaderApp, ShotSpec, ShotView, default_reference, render_shot};
use bible_app::parallel::{MAX_COLUMNS, ParallelView};
use bible_app::theme::{Theme, Typography};
use bible_app::typography::MonoMeasure;
use bible_core::Reference;
use eframe::egui;
use egui_kittest::kittest::Queryable;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch folder that no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-parallel-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// The translations the parallel tests read, loaded through the real library.
fn library() -> Library {
    Library::discover_in(scratch("library"))
}

/// Loads one translation by identifier, which is what a column header does.
fn load(library: &Library, id: &str) -> Arc<bible_core::Translation> {
    library.load(Some(id)).expect("the translation is bundled")
}

/// Builds a view over the named translations at a chapter.
fn view(library: &Library, ids: &[&str]) -> ParallelView {
    let translations = ids.iter().map(|id| load(library, id)).collect();
    ParallelView::new(translations, default_reference(), 0.0).expect("two or more translations")
}

/// The typography the layout tests use.
fn typography() -> Typography {
    Typography::default()
}

#[test]
fn every_column_carries_the_same_verse_numbers_row_for_row() {
    let library = library();
    let ty = typography();
    let measure = MonoMeasure::new(ty);

    let mut first = view(&library, &["kjv", "web"]);
    let kjv = first.frame(&ty, 300.0, &measure).verse_numbers();

    let mut second = view(&library, &["web", "bsb"]);
    let web = second.frame(&ty, 300.0, &measure).verse_numbers();

    assert_eq!(
        kjv, web,
        "two layouts of John 3 produced different row numbers"
    );
    assert!(
        kjv.windows(2).all(|pair| pair[0] < pair[1]),
        "the rows are not in canonical order"
    );
    assert_eq!(kjv.first(), Some(&1), "the grid starts at verse 1");
    assert_eq!(
        kjv.len(),
        usize::from(bible_core::books::verses_in_chapter(42, 3).expect("John 3 is canonical"))
    );
}

#[test]
fn every_row_is_painted_in_every_column() {
    let library = library();
    let ty = typography();
    let measure = MonoMeasure::new(ty);

    let mut view = view(&library, &["kjv", "web", "bsb"]);
    let grid = view.frame(&ty, 240.0, &measure);
    assert_eq!(grid.layouts().len(), 3, "three columns were asked for");
    for row in &grid.rows {
        assert_eq!(
            row.cells.len(),
            3,
            "row {} has the wrong cell count",
            row.verse
        );
        for (index, cell) in row.cells.iter().enumerate() {
            assert!(
                !cell.is_empty(),
                "column {index} does not paint verse {}",
                row.verse
            );
        }
        assert!(row.height >= ty.line_spacing() * 0.99, "row is too short");
    }
    assert!(
        grid.height > grid.rows.last().expect("rows").top,
        "the grid height does not cover its last row"
    );
}

#[test]
fn a_scroll_moves_the_one_anchor_and_repaints_every_column() {
    let library = library();
    let ty = typography();
    let measure = MonoMeasure::new(ty);

    let mut view = view(&library, &["kjv", "web"]);
    let before: Vec<f32> = view
        .frame(&ty, 300.0, &measure)
        .rows
        .iter()
        .map(|row| row.top)
        .collect();
    assert_eq!(view.offset(), 0.0);

    view.scroll_by(48.0);
    let grid = view.frame(&ty, 300.0, &measure);
    assert_eq!(grid.offset, 48.0, "the scroll did not move the anchor");

    let after: Vec<f32> = grid.rows.iter().map(|row| row.top).collect();
    assert_eq!(after, before, "the grid moved, the anchor did not");
    for (index, top) in before.iter().enumerate() {
        assert_eq!(
            grid.row_top(index, grid.offset),
            Some(top - 48.0),
            "row {index} did not repaint at the new offset"
        );
    }
    assert_eq!(
        (grid.book, grid.chapter),
        (42, 3),
        "the anchor left the chapter on a small scroll"
    );
}

#[test]
fn the_column_count_is_clamped_to_three_columns() {
    let library = library();
    let translations = ["kjv", "web", "bsb", "asv"]
        .iter()
        .map(|id| load(&library, id))
        .collect();
    let view = ParallelView::new(translations, default_reference(), 0.0).expect("four are enough");
    assert_eq!(view.column_count(), MAX_COLUMNS);
    assert!(
        view.column_count() >= 2,
        "the view must show at least two columns"
    );

    let none = ParallelView::new(vec![load(&library, "kjv")], default_reference(), 0.0);
    assert!(none.is_none(), "one translation cannot fill the view");
}

#[test]
fn swapping_a_column_translation_leaves_the_others_alone() {
    let library = library();
    let ty = typography();
    let measure = MonoMeasure::new(ty);

    let mut view = view(&library, &["kjv", "web", "bsb"]);
    let ids: Vec<String> = view
        .columns()
        .iter()
        .map(|column| column.id().to_owned())
        .collect();
    let before = view.frame(&ty, 240.0, &measure).verse_numbers();

    assert!(view.set_translation(1, load(&library, "ylt")));
    let after: Vec<String> = view
        .columns()
        .iter()
        .map(|column| column.id().to_owned())
        .collect();
    assert_eq!(after[0], ids[0], "the first column changed");
    assert_eq!(after[1], "ylt", "the chosen column did not change");
    assert_eq!(after[2], ids[2], "the third column changed");
    assert_eq!(
        view.frame(&ty, 240.0, &measure).verse_numbers(),
        before,
        "the shared grid moved when one column changed translation"
    );
    assert!(!view.set_translation(3, load(&library, "asv")));
}

#[test]
fn the_verse_text_of_each_column_comes_from_its_own_translation() {
    let library = library();
    let ty = typography();
    let measure = MonoMeasure::new(ty);
    let mut view = view(&library, &["kjv", "web"]);
    let grid = view.frame(&ty, 300.0, &measure);

    let row = grid
        .rows
        .iter()
        .find(|row| row.verse == 16)
        .expect("John 3:16 is on the grid");
    let kjv = &grid.layouts()[0];
    let web = &grid.layouts()[1];
    let kjv_text: String = kjv.lines[row.cells[0].clone()]
        .iter()
        .map(|line| line.text())
        .collect();
    let web_text: String = web.lines[row.cells[1].clone()]
        .iter()
        .map(|line| line.text())
        .collect();
    assert!(
        kjv_text.contains("For God so loved the world"),
        "{kjv_text}"
    );
    assert_ne!(kjv_text, web_text, "both columns painted the same text");
}

/// Presses the primary button at a point, which is how a reader selects a verse.
fn click(harness: &egui_kittest::Harness<'static, ReaderApp>, pos: egui::Pos2) {
    harness.hover_at(pos);
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    });
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    });
}

/// Builds a harness around the real app with its own scratch data folder.
fn harness(name: &str) -> egui_kittest::Harness<'static, ReaderApp> {
    let data = scratch(name);
    egui_kittest::Harness::builder()
        .with_size(egui::vec2(1000.0, 760.0))
        .with_pixels_per_point(1.0)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, Theme::Light, default_reference(), &data))
}

#[test]
fn the_toolbar_button_enters_and_leaves_parallel_mode() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("toggle");
    harness.run();
    assert!(
        !harness.state().parallel_is_open(),
        "parallel starts closed"
    );

    harness.get_by_label("Parallel").click();
    harness.run();
    assert!(
        harness.state().parallel_is_open(),
        "the Parallel button did not open the view"
    );
    assert_eq!(harness.state().parallel_columns().len(), 3);

    harness.get_by_label("Parallel").click();
    harness.run();
    assert!(
        !harness.state().parallel_is_open(),
        "the Parallel button did not close the view"
    );
}

#[test]
fn leaving_parallel_mode_restores_the_reader_exactly() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("restore");
    harness.run();
    let before = (
        harness.state().reference(),
        harness.state().reading_offset(),
        harness.state().selected_verse(),
    );

    harness.get_by_label("Parallel").click();
    harness.run();
    harness.state_mut().scroll_parallel(120.0);
    harness.run();
    assert!(
        harness.state().parallel_offset() > 0.0,
        "no scroll happened"
    );

    harness.get_by_label("Parallel").click();
    harness.run();
    let after = (
        harness.state().reference(),
        harness.state().reading_offset(),
        harness.state().selected_verse(),
    );
    assert_eq!(before, after, "parallel mode disturbed the reader");
}

#[test]
fn escape_leaves_parallel_mode() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("escape");
    harness.run();
    harness.get_by_label("Parallel").click();
    harness.run();
    assert!(harness.state().parallel_is_open());

    harness.event(egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    });
    harness.run();
    assert!(
        !harness.state().parallel_is_open(),
        "Escape did not leave parallel mode"
    );
}

#[test]
fn a_click_in_the_second_column_selects_the_verse_everywhere() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("selection");
    harness.run();
    harness.state_mut().set_view(ShotView::Parallel);
    harness.run();

    let rects = harness.state().verse_rects().to_vec();
    assert!(!rects.is_empty(), "no verse was painted");
    let mid = rects
        .iter()
        .map(|(_, rect)| rect.center().x)
        .fold((f32::MAX, f32::MIN), |(min, max), x| {
            (min.min(x), max.max(x))
        });
    let split = (mid.0 + mid.1) * 0.5;
    let target = rects
        .iter()
        .find(|(_, rect)| rect.center().x > split)
        .copied()
        .expect("the second column paints a verse");

    click(&harness, target.1.center());
    harness.run();
    assert_eq!(
        harness.state().selected_verse(),
        Some(target.0),
        "a press in the second column did not select its verse"
    );

    let columns: Vec<f32> = harness
        .state()
        .verse_rects()
        .iter()
        .filter(|(reference, _)| *reference == target.0)
        .map(|(_, rect)| rect.center().x)
        .collect();
    assert!(
        columns.len() >= 2,
        "the selected verse is highlighted in only one column"
    );
}

#[test]
fn the_parallel_shot_differs_from_the_plain_shot() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("shots");
    let plain = dir.join("plain.png");
    let mut plain_spec = ShotSpec {
        path: plain.clone(),
        size: [900, 640],
        theme: Theme::Light,
        pause: Duration::ZERO,
        ..ShotSpec::default()
    };
    render_shot(&plain_spec).expect("the plain shot renders");

    let parallel = dir.join("parallel.png");
    plain_spec.path = parallel.clone();
    plain_spec.view = Some(ShotView::Parallel);
    render_shot(&plain_spec).expect("the parallel shot renders");

    let plain_bytes = std::fs::read(&plain).expect("the plain PNG was written");
    let parallel_bytes = std::fs::read(&parallel).expect("the parallel PNG was written");
    assert_eq!(&parallel_bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_ne!(
        plain_bytes, parallel_bytes,
        "parallel mode painted the same picture as the reader"
    );
}

#[test]
fn the_parallel_shot_starts_at_the_demo_chapter_with_its_marks() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let mut harness = harness("demo");
    harness.run();
    harness.state_mut().set_view(ShotView::Parallel);
    harness.run();
    assert_eq!(
        harness.state().reference(),
        Reference::new(42, 3, None).expect("John 3 is canonical")
    );
    assert_eq!(
        harness.state().selected_verse(),
        Some(Reference::new(42, 3, Some(3)).expect("John 3:3 is canonical")),
        "the demo highlight is not selected"
    );
}

#[test]
fn the_palette_does_not_offer_parallel_without_a_key() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let harness = harness("palette");
    assert!(
        !bible_app::shortcuts::commands().contains(&bible_app::shortcuts::Action::ToggleParallel),
        "the parallel action gained a key combination"
    );
    drop(harness);
}
