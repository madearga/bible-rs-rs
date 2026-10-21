// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The binary entry point: a verb runs the command line, and no argument opens
//! the reader window.

use anyhow::{Context, Result, bail};
use bible_app::app::{ReaderApp, default_reference};
use bible_app::theme::Theme;
use eframe::egui;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(bible_app::cli::run(&args));
    }
    launch()
}

/// Opens the reader window with the renderer the environment selects.
///
/// wgpu is the renderer everywhere and lands on Metal on macOS. `BIBLE_RENDERER`
/// overrides the choice with `glow` for the comparison the spec asks for.
fn launch() -> Result<()> {
    let renderer = match std::env::var("BIBLE_RENDERER").as_deref() {
        Ok("glow") | Ok("opengl") => eframe::Renderer::Glow,
        Ok("wgpu") | Ok("metal") | Err(_) | Ok("") => eframe::Renderer::Wgpu,
        Ok(other) => bail!("no renderer called {other:?}"),
    };
    let theme = match std::env::var("BIBLE_THEME") {
        Ok(name) => Theme::from_name(&name).with_context(|| format!("no theme called {name:?}"))?,
        Err(_) => Theme::Light,
    };
    let reference = match std::env::var("BIBLE_REFERENCE") {
        Ok(reference) => bible_core::Reference::parse(&reference)
            .with_context(|| format!("cannot read the reference {reference:?}"))?,
        Err(_) => default_reference(),
    };

    let mut options = eframe::NativeOptions {
        renderer,
        viewport: egui::ViewportBuilder::default()
            .with_title("Bible reader")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([520.0, 420.0]),
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    {
        // A reader has no use for a discrete GPU, and waking one costs startup
        // time and battery.
        let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
        options.wgpu_options.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup);
    }

    eframe::run_native(
        "Bible reader",
        options,
        Box::new(move |cc| {
            // The menu is built before the first frame, so the menu bar is in
            // place as soon as the window is. A plain binary run from a
            // terminal gets the same menu as a bundled application.
            #[cfg(target_os = "macos")]
            bible_app::macos_menu::install(cc.egui_ctx.clone())
                .context("the menu bar did not install")?;
            Ok(Box::new(ReaderApp::open(cc, theme, reference)))
        }),
    )
    .map_err(|error| anyhow::anyhow!("the window did not open: {error}"))
}
