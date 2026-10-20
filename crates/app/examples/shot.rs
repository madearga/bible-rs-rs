// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The offscreen render harness.
//!
//! It drives the real reader through the toolkit's test harness and writes a
//! PNG, so it works on a build machine with no display. Every GUI change is
//! verified with a picture that came out of this binary.
//!
//! Run it with an output path and a size, and select what to show from the
//! environment:
//!
//! ```text
//! cargo run --release --example shot -- /tmp/shot.png 1280x800
//! SHOT_THEME=sepia SHOT_REFERENCE="Ps 23" cargo run --example shot -- /tmp/ps.png 1280x800
//! SHOT_CLICK=640,300 cargo run --example shot -- /tmp/click.png 1280x800
//! SHOT_FILMSTRIP=4 cargo run --example shot -- /tmp/frame.png 1280x800
//! ```

use bible_app::app::{ShotSpec, render_shot};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let spec = ShotSpec::from_args(&args)?;
    for path in render_shot(&spec)? {
        let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        println!("wrote {} ({size} bytes)", path.display());
    }
    Ok(())
}
