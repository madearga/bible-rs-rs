// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The text shaping rules and the character treatments the reader paints with.
//!
//! Every run of text on the page has a [`TextRole`], and the role decides the
//! face, size, weight, and colour. That keeps the typesetting rules in one table
//! instead of spread through the painting code, and it lets the layout measure
//! text without knowing anything about the toolkit.
//!
//! Two treatments stand in for features the toolkit has no toggle for. The
//! divine name is set in capitals at 78 percent size with the baseline held to
//! the body baseline, which reads as small capitals. The verse number is a
//! superscript followed by a non-breaking space, so it can never be separated
//! from the verse it numbers.

use std::borrow::Cow;

use bible_core::{Span, SpanStyle, Translation, Verse};
use eframe::egui;
use egui::epaint::text::VariationCoords;
use egui::{Align, Color32, FontFamily, FontId, TextFormat};

use crate::theme::{self, Palette, Typography};

/// What a run of text is on the page, which decides how it is set.
///
/// The role is deliberately coarse: it names a typographic job, not a source
/// span, so the reader and the layout agree on the few styles that exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Running verse text.
    Body,
    /// The chapter title at the top of a chapter.
    Title,
    /// A section heading above the verse it introduces.
    Heading,
    /// A Psalm superscription, muted and italic.
    Superscription,
    /// The divine name, set as small capitals.
    DivineName,
    /// Words spoken by Christ, red when red letters are on.
    WordsOfChrist,
    /// A translator's supplied words, italic.
    TranslatorAddition,
    /// The word Selah, italic and set to the right.
    Selah,
    /// A verse number, a superscript in the sans face.
    VerseNumber,
}

impl TextRole {
    /// The size in points this role is set at.
    pub fn size(self, ty: &Typography) -> f32 {
        match self {
            Self::Body | Self::WordsOfChrist => ty.size,
            Self::Title => ty.chapter_title_size(),
            Self::Heading => ty.heading_size(),
            Self::Superscription | Self::TranslatorAddition | Self::Selah => ty.size,
            Self::DivineName => ty.divine_name_size(),
            Self::VerseNumber => ty.verse_number_size(),
        }
    }

    /// The face this role is set in.
    pub fn family(self) -> FontFamily {
        match self {
            Self::Title | Self::Heading | Self::VerseNumber => theme::sans(),
            Self::Superscription | Self::TranslatorAddition | Self::Selah => {
                theme::body_italic_family()
            }
            Self::Body | Self::DivineName | Self::WordsOfChrist => theme::body_family(),
        }
    }

    /// The font weight, with section headings semibold and everything else regular.
    pub fn weight(self) -> f32 {
        match self {
            Self::Title | Self::Heading => 600.0,
            _ => 400.0,
        }
    }

    /// Whether the role is set in capitals, which is how the divine name reads
    /// as small capitals.
    pub fn uppercase(self) -> bool {
        self == Self::DivineName
    }

    /// The ink this role is painted with under a palette.
    pub fn color(self, ty: &Typography, palette: &Palette) -> Color32 {
        match self {
            Self::Body | Self::Title | Self::DivineName => palette.ink,
            Self::Heading => palette.accent,
            Self::Superscription | Self::Selah => palette.muted,
            Self::WordsOfChrist => {
                if ty.red_letters {
                    palette.words_of_christ
                } else {
                    palette.ink
                }
            }
            Self::TranslatorAddition => palette.ink,
            Self::VerseNumber => palette.verse_number,
        }
    }

    /// The distance from the top of the role's em box down to its baseline.
    ///
    /// Every role shares one body baseline, so a role set at a smaller size
    /// starts lower on the line instead of sitting on a different baseline.
    pub fn ascent(self, ty: &Typography) -> f32 {
        self.size(ty) * 0.8
    }

    /// How far above the body baseline a superscript role is raised, zero for
    /// every role that sits on the baseline.
    pub fn superscript_lift(self, ty: &Typography) -> f32 {
        if self == Self::VerseNumber {
            ty.size * 0.33
        } else {
            0.0
        }
    }
}

/// The toolkit font id for a role.
pub fn font_id(role: TextRole, ty: &Typography) -> FontId {
    FontId::new(role.size(ty), role.family())
}

/// The paint format for a role, including the weight axis and the shared line
/// height so mixed sizes sit on one baseline grid.
pub fn format(role: TextRole, ty: &Typography, palette: &Palette) -> TextFormat {
    TextFormat {
        font_id: font_id(role, ty),
        color: role.color(ty, palette),
        coords: VariationCoords::new([(b"wght", role.weight())]),
        line_height: Some(ty.line_spacing()),
        valign: if role == TextRole::VerseNumber {
            Align::TOP
        } else {
            Align::BOTTOM
        },
        ..Default::default()
    }
}

/// The text of one role, with the capital treatment applied.
pub fn shaped<'a>(text: &'a str, role: TextRole) -> Cow<'a, str> {
    if role.uppercase() {
        Cow::Owned(text.to_uppercase())
    } else {
        Cow::Borrowed(text)
    }
}

/// One piece of verse text with the role it is set in.
///
/// The runs come from the span ranges the core crate stores, so the layout never
/// has to know which characters are red, italic, or small capitals.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The characters, already capitalised when the role asks for it.
    pub text: String,
    /// How the characters are set.
    pub role: TextRole,
}

/// Splits one verse into styled runs.
///
/// Overlapping spans resolve to the innermost one, and adjacent runs of the same
/// role merge, so a verse with no spans is one run. A superscription sets the
/// whole verse in the muted italic role.
pub fn verse_runs(translation: &Translation, verse: &Verse) -> Vec<Run> {
    let text = translation.verse_text(verse);
    runs_between(translation, verse, 0, text.len())
}

/// Splits one byte range of a verse into styled runs.
///
/// The layout uses this when a poetry break divides a verse, so a span that
/// crosses the break still gets its face on both sides of it.
pub fn verse_runs_in(translation: &Translation, verse: &Verse, from: usize, to: usize) -> Vec<Run> {
    let len = translation.verse_text(verse).len();
    runs_between(translation, verse, from.min(len), to.min(len))
}

/// The shared splitter behind both public entry points.
fn runs_between(translation: &Translation, verse: &Verse, from: usize, to: usize) -> Vec<Run> {
    let text = translation.verse_text(verse);
    if text.is_empty() || from >= to {
        return Vec::new();
    }

    let base = if translation
        .verse_marks(verse)
        .iter()
        .any(|mark| mark.kind == bible_core::MarkKind::Superscription)
    {
        TextRole::Superscription
    } else {
        TextRole::Body
    };

    let spans: &[Span] = translation.verse_spans(verse);
    let mut cuts: Vec<usize> = vec![from, to];
    for span in spans {
        cuts.push((span.start as usize).clamp(from, to));
        cuts.push((span.end as usize).clamp(from, to));
    }
    cuts.retain(|cut| *cut >= from && *cut <= to && text.is_char_boundary(*cut));
    cuts.sort_unstable();
    cuts.dedup();

    let mut runs: Vec<Run> = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        if start == end {
            continue;
        }
        let style = spans
            .iter()
            .filter(|span| start >= span.start as usize && start < span.end as usize)
            .min_by_key(|span| span.end - span.start)
            .map(|span| span.style);
        let role = match style {
            Some(SpanStyle::WordsOfChrist) => TextRole::WordsOfChrist,
            Some(SpanStyle::TranslatorAddition) => TextRole::TranslatorAddition,
            Some(SpanStyle::DivineName) => TextRole::DivineName,
            Some(SpanStyle::Selah) => TextRole::Selah,
            None => base,
        };
        let shaped = shaped(&text[start..end], role);
        if let Some(last) = runs.last_mut()
            && last.role == role
        {
            last.text.push_str(&shaped);
            continue;
        }
        runs.push(Run {
            text: shaped.into_owned(),
            role,
        });
    }
    runs
}

/// The printed label of a verse number, a superscript followed by a
/// non-breaking space so it never ends a line away from its verse.
pub fn verse_label(verse: &Verse) -> String {
    if verse.number_end > verse.number {
        format!("{}\u{2013}{}\u{00A0}", verse.number, verse.number_end)
    } else {
        format!("{}\u{00A0}", verse.number)
    }
}

/// Measures text the way the page will paint it.
///
/// The layout asks for a width per word, and the reader answers with the real
/// toolkit measurement. Tests answer with a fixed advance, which keeps the
/// layout assertions independent of any installed font.
pub trait Measure {
    /// The width in points of a run of text set in a role.
    fn width(&self, text: &str, role: TextRole) -> f32;
}

/// Real shaping through the toolkit's font atlas.
pub struct EguiMeasure<'a> {
    /// The context whose fonts shape the text.
    ctx: &'a egui::Context,
    /// The metrics the size comes from.
    typography: Typography,
}

impl<'a> EguiMeasure<'a> {
    /// Measures with the context's loaded faces at a given text size.
    pub fn new(ctx: &'a egui::Context, typography: Typography) -> Self {
        Self { ctx, typography }
    }
}

impl Measure for EguiMeasure<'_> {
    fn width(&self, text: &str, role: TextRole) -> f32 {
        let font = font_id(role, &self.typography);
        self.ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(text.to_owned(), font, Color32::BLACK)
                .size()
                .x
        })
    }
}

/// A deterministic advance of half the em per character, for layout tests.
pub struct MonoMeasure {
    /// The metrics the size comes from.
    typography: Typography,
}

impl MonoMeasure {
    /// Measures every character at half the role's em.
    pub fn new(typography: Typography) -> Self {
        Self { typography }
    }
}

impl Measure for MonoMeasure {
    fn width(&self, text: &str, role: TextRole) -> f32 {
        text.chars().count() as f32 * role.size(&self.typography) * 0.5
    }
}
