// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The render harness tests. Every one of them drives the real app offscreen and
//! asserts on a PNG that came out of it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use bible_app::app::{ReaderApp, ShotSpec, ShotView, default_reference, render_shot};
use bible_app::theme::Theme;
use bible_core::Reference;
use eframe::egui;

/// wgpu device creation is serialised so parallel tests do not fight over the
/// Metal device, which keeps the harness deterministic on a build machine.
static GPU: Mutex<()> = Mutex::new(());

/// A scratch folder that no other test shares.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bible-shot-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// A shot spec with a small canvas, so the tests stay quick.
fn spec(path: PathBuf, size: [u32; 2], theme: Theme) -> ShotSpec {
    ShotSpec {
        path,
        size,
        theme,
        pause: Duration::ZERO,
        ..ShotSpec::default()
    }
}

/// Reads the width and height out of a PNG's IHDR chunk.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).expect("the PNG was written");
    assert!(bytes.len() > 24, "a PNG header is longer than 24 bytes");
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("four bytes"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("four bytes"));
    (width, height)
}

#[test]
fn the_harness_writes_a_png() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("writes");
    let path = dir.join("one.png");
    let written = render_shot(&spec(path.clone(), [400, 300], Theme::Light)).expect("render");
    assert_eq!(written, vec![path.clone()]);

    let bytes = std::fs::read(&path).expect("the PNG was written");
    assert!(bytes.len() > 1000, "the PNG is {} bytes", bytes.len());
    assert_eq!(
        &bytes[..8],
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
        "the file does not start with the PNG signature"
    );
}

#[test]
fn the_harness_honours_the_size_argument() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("size");
    let path = dir.join("sized.png");
    render_shot(&spec(path.clone(), [640, 360], Theme::Light)).expect("render");
    assert_eq!(png_size(&path), (640, 360));
}

#[test]
fn the_three_themes_render_pngs_that_differ() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("themes");
    let mut images = Vec::new();
    for (index, theme) in [Theme::Light, Theme::Sepia, Theme::Dark]
        .into_iter()
        .enumerate()
    {
        let path = dir.join(format!("theme-{index}.png"));
        render_shot(&spec(path.clone(), [480, 320], theme)).expect("render");
        assert_eq!(png_size(&path), (480, 320));
        images.push(std::fs::read(&path).expect("the PNG was written"));
    }
    assert_ne!(images[0], images[1], "light and sepia are identical");
    assert_ne!(images[1], images[2], "sepia and dark are identical");
    assert_ne!(images[0], images[2], "light and dark are identical");
}

#[test]
fn a_click_changes_the_picture() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("click");
    let plain = dir.join("plain.png");
    render_shot(&spec(plain.clone(), [480, 320], Theme::Light)).expect("render");
    let clicked = dir.join("clicked.png");
    let mut clicked_spec = spec(clicked.clone(), [480, 320], Theme::Light);
    clicked_spec.click = Some(eframe::egui::pos2(240.0, 150.0));
    render_shot(&clicked_spec).expect("render");
    assert_ne!(
        std::fs::read(&plain).expect("the PNG was written"),
        std::fs::read(&clicked).expect("the PNG was written"),
        "the click did not reach the reader"
    );
}

#[test]
fn a_filmstrip_writes_one_png_per_frame() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let dir = scratch("filmstrip");
    let mut filmstrip = spec(dir.join("frame.png"), [320, 240], Theme::Sepia);
    filmstrip.filmstrip = 3;
    let written = render_shot(&filmstrip).expect("render");
    assert_eq!(written.len(), 3);
    for path in &written {
        assert!(path.exists(), "{} is missing", path.display());
    }
    assert_eq!(written[1], dir.join("frame-1.png"));
    assert_eq!(written[2], dir.join("frame-2.png"));
}

/// The widest rectangle the last frame recorded for one verse.
fn largest_rect(rects: &[(Reference, egui::Rect)], reference: Reference) -> egui::Rect {
    rects
        .iter()
        .filter(|(found, _)| *found == reference)
        .map(|(_, rect)| *rect)
        .max_by_key(|rect| rect.width() as u32 * rect.height() as u32)
        .expect("the verse is on screen")
}

/// Counts the pixels a yellow highlight wash covers inside a rectangle.
///
/// The wash is the fixed yellow swatch blended over the paper, which reads as
/// warm and bright; the ink, the verse numbers, the selection, and the brown
/// flash all fall outside the test, so the count answers one question only.
fn yellow_pixels(image: &image::RgbaImage, rect: egui::Rect) -> u32 {
    let width = image.width() as f32;
    let height = image.height() as f32;
    let left = rect.min.x.max(0.0).min(width - 1.0).round() as u32;
    let right = rect.max.x.max(1.0).min(width).round() as u32;
    let top = rect.min.y.max(0.0).min(height - 1.0).round() as u32;
    let bottom = rect.max.y.max(1.0).min(height).round() as u32;
    let mut count = 0;
    for y in top..bottom {
        for x in left..right {
            let pixel = image.get_pixel(x, y);
            let (red, green, blue) = (pixel[0] as u16, pixel[1] as u16, pixel[2] as u16);
            if red > 200 && green > blue + 35 {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn a_highlighted_verse_is_painted_behind_its_words() {
    let _guard = GPU.lock().unwrap_or_else(|error| error.into_inner());
    let data = scratch("highlight");
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(900.0, 700.0))
        .with_pixels_per_point(1.0)
        .with_max_steps(64)
        .wgpu()
        .build_eframe(move |cc| ReaderApp::open_at(cc, Theme::Light, default_reference(), &data));
    harness.run();
    harness.state_mut().set_view(ShotView::StudySidebar);
    harness.run();

    // The demo selects the verse it highlights, and the selection fill is
    // opaque, so the selection moves to the bookmark verse first and the
    // highlighted verse keeps only its wash and its flash.
    let rects = harness.state().verse_rects().to_vec();
    let target = largest_rect(&rects, Reference::new(42, 3, Some(18)).expect("canonical")).center();
    harness.hover_at(target);
    harness.event(egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    });
    harness.event(egui::Event::PointerButton {
        pos: target,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    });
    harness.run();

    let rects = harness.state().verse_rects().to_vec();
    let highlighted = largest_rect(&rects, Reference::new(42, 3, Some(16)).expect("canonical"));
    let bare = largest_rect(&rects, Reference::new(42, 3, Some(18)).expect("canonical"));
    let image = harness.render().expect("the frame renders");

    let filled = yellow_pixels(&image, highlighted);
    let untouched = yellow_pixels(&image, bare);
    assert!(
        filled > 500,
        "the highlight wash covers only {filled} pixels of its verse"
    );
    assert!(
        filled > untouched * 10,
        "the unhighlighted verse shows {untouched} wash pixels against {filled}"
    );
}
