// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The startup probe.
//!
//! It opens the real window and prints the milliseconds from the top of the
//! process to the first painted frame, which is the metric the performance
//! budget puts under fifty milliseconds. The window closes itself immediately
//! after that frame.

use std::time::Instant;

use bible_app::app::{ReaderApp, default_reference};
use bible_app::theme::Theme;
use eframe::egui;

fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Bible reader startup probe")
            .with_inner_size([1280.0, 800.0])
            .with_visible(false),
        ..Default::default()
    };
    eframe::run_native(
        "Bible reader startup probe",
        options,
        Box::new(move |cc| {
            let window_up = Instant::now();
            Ok(Box::new(Probe {
                app: ReaderApp::open(cc, Theme::Light, default_reference()),
                start,
                window_up,
                reported: false,
            }))
        }),
    )
    .map_err(|error| anyhow::anyhow!("the window did not open: {error}"))
}

/// Wraps the reader and reports the first frame before closing the window.
struct Probe {
    app: ReaderApp,
    start: Instant,
    window_up: Instant,
    reported: bool,
}

impl eframe::App for Probe {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if self.reported {
            return;
        }
        self.reported = true;
        let frame_millis = self.window_up.elapsed().as_secs_f64() * 1000.0;
        let total_millis = self.start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "first frame: {frame_millis:.1} ms after the window existed, \
             {total_millis:.1} ms after the process started (1280x800, wgpu)"
        );
        let ctx = ui.ctx().clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
