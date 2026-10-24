// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The reading palettes, the typography metrics, and the font faces.
//!
//! Every colour the reader paints comes from a [`Palette`], so switching the
//! theme never touches a widget. Every font file the reader uses is registered
//! from [`FONT_FACES`], so replacing a face is one edit in one table.

use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke};

/// The looks the reader offers: three chosen for long reading, plus the
/// automatic one that follows the system appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Warm paper and dark ink for daylight.
    Light,
    /// Cream paper and brown ink for low contrast reading.
    Sepia,
    /// Charcoal paper and bone ink for a dark room.
    Dark,
    /// Whichever of light and dark the system is showing.
    Auto,
}

/// The palettes in the order the theme shortcut cycles them.
pub const THEMES: [Theme; 4] = [Theme::Light, Theme::Sepia, Theme::Dark, Theme::Auto];

impl Theme {
    /// The lowercase name used by the render harness and by saved preferences.
    pub fn name(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Sepia => "sepia",
            Self::Dark => "dark",
            Self::Auto => "auto",
        }
    }

    /// Recovers a theme from its lowercase name, rejecting anything else.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "light" => Some(Self::Light),
            "sepia" => Some(Self::Sepia),
            "dark" => Some(Self::Dark),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    /// The concrete theme a choice paints with, given the system appearance.
    ///
    /// Only the automatic choice depends on it; the rest are themselves.
    pub fn resolve(self, dark: bool) -> Self {
        match self {
            Self::Auto if dark => Self::Dark,
            Self::Auto => Self::Light,
            other => other,
        }
    }

    /// The next theme in the cycle, which is what the theme shortcut selects.
    pub fn next(self) -> Self {
        match self {
            Self::Light => Self::Sepia,
            Self::Sepia => Self::Dark,
            Self::Dark => Self::Auto,
            Self::Auto => Self::Light,
        }
    }

    /// The colours the reader paints with under this theme.
    ///
    /// The automatic choice resolves before anything paints, so this arm only
    /// serves a caller that skipped the resolution and takes the daylight look.
    pub fn palette(self) -> Palette {
        match self.resolve(false) {
            Self::Light => Palette {
                name: Self::Light.name(),
                paper: Color32::from_rgb(0xF6, 0xF1, 0xE7),
                ink: Color32::from_rgb(0x22, 0x1D, 0x17),
                muted: Color32::from_rgb(0x6E, 0x65, 0x59),
                rule: Color32::from_rgb(0xDE, 0xD4, 0xC5),
                accent: Color32::from_rgb(0x8A, 0x55, 0x25),
                verse_number: Color32::from_rgb(0x99, 0x7A, 0x4E),
                selection: Color32::from_rgb(0xE9, 0xDD, 0xC6),
                words_of_christ: Color32::from_rgb(0x9B, 0x2C, 0x20),
            },
            Self::Sepia => Palette {
                name: Self::Sepia.name(),
                paper: Color32::from_rgb(0xF1, 0xE2, 0xC6),
                ink: Color32::from_rgb(0x3A, 0x2C, 0x1B),
                muted: Color32::from_rgb(0x7B, 0x66, 0x45),
                rule: Color32::from_rgb(0xDB, 0xC6, 0x9E),
                accent: Color32::from_rgb(0x8A, 0x51, 0x1E),
                verse_number: Color32::from_rgb(0xA1, 0x7C, 0x44),
                selection: Color32::from_rgb(0xE5, 0xD1, 0xA8),
                words_of_christ: Color32::from_rgb(0x8E, 0x2A, 0x1C),
            },
            Self::Dark => Palette {
                name: Self::Dark.name(),
                paper: Color32::from_rgb(0x1C, 0x1A, 0x17),
                ink: Color32::from_rgb(0xE9, 0xE2, 0xD5),
                muted: Color32::from_rgb(0x9C, 0x92, 0x84),
                rule: Color32::from_rgb(0x35, 0x31, 0x2B),
                accent: Color32::from_rgb(0xC9, 0x93, 0x5A),
                verse_number: Color32::from_rgb(0xB1, 0x8B, 0x5C),
                selection: Color32::from_rgb(0x30, 0x2C, 0x26),
                words_of_christ: Color32::from_rgb(0xD9, 0x7E, 0x6C),
            },
            // Resolution never lands here, but the match must name it.
            Self::Auto => Self::Light.palette(),
        }
    }

    /// Paints the whole widget set in the theme's colours.
    ///
    /// This replaces the toolkit's default style, so calling it once when the
    /// theme changes is enough.
    pub fn apply(self, ctx: &egui::Context) {
        let palette = self.palette();
        let dark = self == Self::Dark;
        let mut style = egui::Style::default();
        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = palette.paper;
        visuals.window_fill = palette.paper;
        visuals.faint_bg_color = palette.selection;
        visuals.extreme_bg_color = palette.selection;
        visuals.override_text_color = Some(palette.ink);
        visuals.hyperlink_color = palette.accent;
        visuals.selection.bg_fill = palette.selection;
        visuals.selection.stroke = Stroke::new(1.0, palette.ink);
        visuals.widgets.noninteractive.bg_fill = palette.paper;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, palette.rule);
        visuals.widgets.inactive.weak_bg_fill = palette.selection;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, palette.rule);
        visuals.widgets.hovered.weak_bg_fill = palette.rule;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette.accent);
        visuals.widgets.active.weak_bg_fill = palette.accent;
        style.visuals = visuals;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.text_styles = [
            (egui::TextStyle::Body, FontId::new(14.0, sans())),
            (egui::TextStyle::Button, FontId::new(13.0, sans())),
            (egui::TextStyle::Small, FontId::new(11.0, sans())),
            (egui::TextStyle::Heading, body(19.0)),
            (
                egui::TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
        ]
        .into();
        ctx.all_styles_mut(|existing| *existing = style.clone());
    }
}

/// Every colour the reading surface paints with.
///
/// The fields are public so widgets can borrow the palette instead of the
/// theme, which keeps a repaint from re-resolving the palette per widget.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// The lowercase theme name this palette belongs to.
    pub name: &'static str,
    /// The page background.
    pub paper: Color32,
    /// Body text.
    pub ink: Color32,
    /// Secondary text such as the toolbar and the reference.
    pub muted: Color32,
    /// Hairlines and dividers.
    pub rule: Color32,
    /// The active accent, used for the pointer and for headings.
    pub accent: Color32,
    /// Verse numbers in the margin.
    pub verse_number: Color32,
    /// The selection fill.
    pub selection: Color32,
    /// The words of Christ when red letters are on.
    pub words_of_christ: Color32,
}

/// The font family that renders the scripture text.
pub fn body_family() -> FontFamily {
    FontFamily::Name(Arc::from("reader-body"))
}

/// The font family that renders the same face in italic.
pub fn body_italic_family() -> FontFamily {
    FontFamily::Name(Arc::from("reader-body-italic"))
}

/// The font family that renders the interface and the verse numbers.
pub fn sans() -> FontFamily {
    FontFamily::Name(Arc::from("reader-sans"))
}

/// A body font id at the given size, used by the heading styles.
pub fn body(size: f32) -> FontId {
    FontId::new(size, body_family())
}

/// The font files the app embeds, all under the SIL Open Font License.
///
/// Literata carries the body text in roman and italic and Inter carries the
/// interface and the verse numbers.
pub const FONT_FACES: [(&str, &[u8]); 3] = [
    (
        "literata-roman",
        include_bytes!("../../../assets/fonts/Literata.ttf"),
    ),
    (
        "literata-italic",
        include_bytes!("../../../assets/fonts/Literata-Italic.ttf"),
    ),
    ("inter", include_bytes!("../../../assets/fonts/Inter.ttf")),
];

/// The font files as registered faces, for tests and for the font panel.
pub const FONT_FILES: [&str; 3] = ["Literata.ttf", "Literata-Italic.ttf", "Inter.ttf"];

/// Builds the font set the reader uses.
///
/// The toolkit's own faces stay in the list behind the reader's faces, so a
/// glyph the reading faces lack still renders instead of showing tofu.
pub fn font_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    for (key, bytes) in FONT_FACES {
        definitions
            .font_data
            .insert(key.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let fallback: Vec<String> = definitions
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let with_fallback = |first: &[&str]| {
        let mut list: Vec<String> = first.iter().map(|name| (*name).to_owned()).collect();
        list.extend(fallback.iter().cloned());
        list
    };

    definitions
        .families
        .insert(body_family(), with_fallback(&["literata-roman"]));
    definitions
        .families
        .insert(body_italic_family(), with_fallback(&["literata-italic"]));
    definitions
        .families
        .insert(sans(), with_fallback(&["inter"]));
    definitions
        .families
        .insert(FontFamily::Proportional, with_fallback(&["inter"]));
    definitions
}

/// Installs the reader's faces and the theme into a context.
pub fn install(ctx: &egui::Context, theme: Theme) {
    ctx.set_fonts(font_definitions());
    theme.apply(ctx);
}

/// The type sizes and spacing the reading surface uses.
///
/// They are all in points and all derived from the body size, so changing the
/// size keeps the page in proportion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    /// The body text size in points.
    pub size: f32,
    /// Line height as a multiple of the body size.
    pub line_height: f32,
    /// The measure, the maximum line length, in ems.
    pub measure_em: f32,
    /// Whether verses flow inside paragraphs or each gets its own line.
    pub paragraph_mode: bool,
    /// Whether the verse numbers are drawn at all.
    pub verse_numbers: bool,
    /// Whether section headings are drawn.
    pub headings: bool,
    /// Whether the words of Christ are set in red.
    pub red_letters: bool,
    /// Whether wrapped lines are justified.
    pub justify: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            size: 19.0,
            line_height: 1.62,
            measure_em: 34.0,
            paragraph_mode: true,
            verse_numbers: true,
            headings: true,
            red_letters: true,
            justify: false,
        }
    }
}

impl Typography {
    /// The line height in points, which every text format on the page uses so
    /// mixed font sizes share one baseline grid.
    pub fn line_spacing(self) -> f32 {
        self.size * self.line_height
    }

    /// The maximum width of a text column in points.
    pub fn measure(self) -> f32 {
        self.size * self.measure_em
    }

    /// The size of a verse number, set as a superscript in the margin.
    pub fn verse_number_size(self) -> f32 {
        self.size * 0.62
    }

    /// The size a chapter title is set at.
    pub fn chapter_title_size(self) -> f32 {
        self.size * 1.32
    }

    /// The size a section heading is set at.
    pub fn heading_size(self) -> f32 {
        self.size * 0.86
    }

    /// The small capitals size for the divine name, which stands in for a small
    /// caps toggle the toolkit does not have.
    pub fn divine_name_size(self) -> f32 {
        self.size * 0.78
    }
}
