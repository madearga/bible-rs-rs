// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The theme tests. They check that the font files are in the repository and
//! that the theme module can build a font set from them, and that the three
//! palettes are actually different pages.

use std::path::PathBuf;

use bible_app::theme::{FONT_FILES, THEMES, Theme, font_definitions};

/// The assets folder, resolved from this crate so the test works from anywhere.
fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

#[test]
fn the_font_files_are_present_and_not_empty() {
    for file in FONT_FILES {
        let path = assets().join("fonts").join(file);
        let meta = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("{} is missing: {error}", path.display()));
        assert!(meta.len() > 10_000, "{} looks truncated", path.display());
        let bytes = std::fs::read(&path).expect("the font file reads");
        assert_eq!(
            &bytes[..4],
            &[0x00, 0x01, 0x00, 0x00],
            "not a TrueType font"
        );
    }
}

#[test]
fn the_font_set_builds_from_the_files() {
    let definitions = font_definitions();
    for key in ["literata-roman", "literata-italic", "inter"] {
        assert!(
            definitions.font_data.contains_key(key),
            "{key} is not registered"
        );
    }
    assert!(definitions.font_data.len() >= 3);
    for family in [
        bible_app::theme::body_family(),
        bible_app::theme::body_italic_family(),
        bible_app::theme::sans(),
    ] {
        let faces = definitions
            .families
            .get(&family)
            .unwrap_or_else(|| panic!("{family:?} has no faces"));
        assert!(!faces.is_empty(), "{family:?} has no faces");
    }
}

#[test]
fn the_three_themes_have_three_different_pages() {
    let palettes: Vec<_> = THEMES.iter().map(|theme| theme.palette()).collect();
    assert_eq!(THEMES.len(), 3);
    assert_eq!(THEMES[0], Theme::Light);
    assert_eq!(THEMES[0].next(), Theme::Sepia);
    assert_eq!(THEMES[1].next(), Theme::Dark);
    assert_eq!(THEMES[2].next(), Theme::Light);
    for (index, palette) in palettes.iter().enumerate() {
        for other in &palettes[index + 1..] {
            assert_ne!(palette.paper, other.paper, "two themes share a page colour");
            assert_ne!(palette.ink, other.ink, "two themes share an ink colour");
        }
        assert_ne!(palette.paper, palette.ink);
    }
    assert_eq!(Theme::from_name("sepia"), Some(Theme::Sepia));
    assert_eq!(Theme::from_name("purple"), None);
}

#[test]
fn the_sample_page_is_present_and_synthetic() {
    let path = assets().join("sample/passage.bbl");
    let bytes = std::fs::read(&path).expect("the sample translation is committed");
    assert_eq!(&bytes[..4], b"BIBL");
    let translation = bible_core::format::decode(&bytes).expect("the sample decodes");
    assert_eq!(translation.meta().id, "sample");
    assert!(translation.meta().licence.contains("not scripture"));
}
