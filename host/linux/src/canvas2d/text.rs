//! Canvas 2D text on Linux (LLP 1056 D8): one Parley font context for the
//! canvases, loaded as the host's catalog is (system fonts, `EXACT_FONTS`,
//! the plan's declared faces), behind a mutex so the runner measures on its
//! thread and the replayer draws with the same engine — what was measured
//! is what is drawn.
//!
//! A run is shaped once and drawn as glyph outlines: a tiny-skia path in
//! run space (origin at the run's left end on its alphabetic baseline, y
//! down), so text takes every paint, shadow, compositing operator and clip a
//! path takes. Colour glyphs draw as their outlines (declared, LLP 1056
//! §8.2). A right-to-left run is laid out with a right-to-left base
//! direction, as `direction = "rtl"` asks.

use exact_canvas::{RawMetrics, TextEngine, TextRun};
use fontique::{Attributes, FontStyle, FontWeight, FontWidth, QueryStatus};
use parley::{BaseDirection, FontContext, FontData, LayoutContext, StyleProperty, TextWrapMode};
use std::sync::Mutex;
use swash::scale::ScaleContext;
use swash::zeno::{Angle, Command, PathData, Transform};
use tiny_skia::{Path, PathBuilder};

use crate::text::FamilyChoice;

/// A run's style, as the list's `Font` record carries it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RunStyle {
    pub size: f32,
    pub weight: u16,
    pub style: u8,
    pub stretch: f32,
    pub kerning: u8,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub families: Vec<String>,
}

impl RunStyle {
    pub(crate) fn from_run(run: &TextRun<'_>) -> RunStyle {
        RunStyle {
            size: run.font.size as f32,
            weight: run.font.weight,
            style: run.font.style,
            stretch: run.font.stretch as f32,
            kerning: run.kerning,
            letter_spacing: run.letter_spacing as f32,
            word_spacing: run.word_spacing as f32,
            families: run.font.families.clone(),
        }
    }
}

/// A shaped run: its outline and advance.
pub(crate) struct Shaped {
    pub path: Option<Path>,
    pub width: f32,
    /// The first glyph's face, for vertical metrics.
    face: Option<FontData>,
}

struct Fonts {
    fonts: FontContext,
    layout: LayoutContext<u32>,
    scale: ScaleContext,
    names: Vec<(String, FamilyChoice)>,
}

/// The canvases' text engine. Its fonts load at the first text a canvas
/// measures or draws, so a canvas app without text pays nothing.
pub(crate) struct CanvasText {
    fonts: Mutex<Option<Fonts>>,
    plan: exact_plan::Plan,
    assets: crate::image::Assets,
}

fn font_style(style: u8) -> FontStyle {
    match style {
        1 => FontStyle::Italic,
        2 => FontStyle::Oblique(None),
        _ => FontStyle::Normal,
    }
}

impl Fonts {
    /// The first family of the list that resolves: a generic or declared
    /// family by the plan's choice, another by an installed family's name;
    /// none, serif (Chrome's default).
    fn family(&mut self, families: &[String]) -> FamilyChoice {
        for f in families {
            if let Some((_, c)) = self.names.iter().find(|(n, _)| n.eq_ignore_ascii_case(f)) {
                return c.clone();
            }
            if self.fonts.collection.family_id(f).is_some() {
                return FamilyChoice::Declared(f.clone());
            }
        }
        FamilyChoice::Serif
    }

    fn shape(&mut self, style: &RunStyle, text: &str, rtl: bool) -> Shaped {
        let size = style.size.max(0.0);
        if size == 0.0 || text.is_empty() {
            return Shaped {
                path: None,
                width: 0.0,
                face: None,
            };
        }
        let family = self.family(&style.families);
        let mut builder = self
            .layout
            .ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(family.family()));
        builder.push_default(StyleProperty::FontSize(size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
            style.weight,
        ))));
        builder.push_default(StyleProperty::FontStyle(font_style(style.style)));
        builder.push_default(StyleProperty::FontWidth(FontWidth::from_percentage(
            style.stretch,
        )));
        builder.push_default(StyleProperty::TextWrapMode(TextWrapMode::NoWrap));
        if style.letter_spacing != 0.0 {
            builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        }
        if style.kerning == 2 {
            builder.push_default(StyleProperty::FontFeatures(parley::FontFeatures::from(
                "\"kern\" 0",
            )));
        }
        builder.set_base_direction(if rtl {
            BaseDirection::Rtl
        } else {
            BaseDirection::Ltr
        });
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        let mut pb = PathBuilder::new();
        let mut width = 0.0f32;
        let mut face = None;
        // Word spacing: after each space, in visual order.
        let mut shift = 0.0f32;
        for line in layout.lines() {
            let mut pen = line.metrics().offset;
            for run in line.runs() {
                let font = run.font().clone();
                face.get_or_insert_with(|| font.clone());
                let coords = run.normalized_coords().to_vec();
                let skew = run.synthesis().skew().is_some();
                let Some(font_ref) =
                    swash::FontRef::from_index(font.data.data(), font.index as usize)
                else {
                    continue;
                };
                let mut scaler = self
                    .scale
                    .builder_with_id(font_ref, crate::text::scaler_id(&font))
                    .size(run.font_size())
                    .hint(false)
                    .normalized_coords(coords.iter().copied())
                    .build();
                for cluster in run.visual_clusters() {
                    let space = text
                        .get(cluster.text_range())
                        .is_some_and(|t| t.contains(' '));
                    for g in cluster.glyphs() {
                        let ox = pen + g.x + shift;
                        let oy = g.y;
                        pen += g.advance;
                        let Some(mut outline) = scaler
                            .scale_outline(g.id as u16)
                            .or_else(|| scaler.scale_color_outline(g.id as u16))
                        else {
                            continue;
                        };
                        if skew {
                            outline.transform(&Transform::skew(
                                Angle::from_degrees(14.0),
                                Angle::from_degrees(0.0),
                            ));
                        }
                        for c in outline.path().commands() {
                            match c {
                                Command::MoveTo(p) => pb.move_to(ox + p.x, oy - p.y),
                                Command::LineTo(p) => pb.line_to(ox + p.x, oy - p.y),
                                Command::QuadTo(a, p) => {
                                    pb.quad_to(ox + a.x, oy - a.y, ox + p.x, oy - p.y)
                                }
                                Command::CurveTo(a, b, p) => pb.cubic_to(
                                    ox + a.x,
                                    oy - a.y,
                                    ox + b.x,
                                    oy - b.y,
                                    ox + p.x,
                                    oy - p.y,
                                ),
                                Command::Close => pb.close(),
                            }
                        }
                    }
                    if style.word_spacing != 0.0 && space {
                        shift += style.word_spacing;
                    }
                }
            }
            width = width.max(line.metrics().advance + shift);
        }
        Shaped {
            path: pb.finish(),
            width,
            face,
        }
    }

    /// The face's vertical metrics at `size`: hhea ascent and descent, and
    /// the OS/2 typographic ones normalised to the em.
    fn vertical(&mut self, face: Option<FontData>, style: &RunStyle) -> [f32; 4] {
        use skrifa::raw::TableProvider;
        let size = style.size;
        let face = face.or_else(|| {
            let family = self.family(&style.families);
            let fonts = &mut self.fonts;
            let mut query = fonts.collection.query(&mut fonts.source_cache);
            query.set_families(std::iter::once(family.family_query()));
            query.set_attributes(Attributes::new(
                FontWidth::NORMAL,
                FontStyle::Normal,
                FontWeight::new(f32::from(style.weight)),
            ));
            let mut found = None;
            query.matches_with(|f| {
                found = Some(FontData::new(f.blob.clone(), f.index));
                QueryStatus::Stop
            });
            found
        });
        let fallback = [size * 0.9, size * 0.25, size * 0.8, size * 0.2];
        let Some(font) = face else {
            return fallback;
        };
        let Ok(f) = skrifa::FontRef::from_index(font.data.data(), font.index) else {
            return fallback;
        };
        let upem = f.head().map_or(1000.0, |h| h.units_per_em() as f32);
        let k = size / upem.max(1.0);
        let (a, d) = f.hhea().map_or((900.0, 250.0), |h| {
            (
                h.ascender().to_i16() as f32,
                -(h.descender().to_i16() as f32),
            )
        });
        let (ta, td) = f.os2().map_or((a, d), |o| {
            (o.s_typo_ascender() as f32, -(o.s_typo_descender() as f32))
        });
        let em = if ta + td > 0.0 {
            (size * ta / (ta + td), size * td / (ta + td))
        } else {
            (size * a / (a + d).max(1.0), size * d / (a + d).max(1.0))
        };
        [a * k, d * k, em.0, em.1]
    }
}

impl CanvasText {
    /// The engine for `plan`'s canvases, its faces read through `assets`.
    pub(crate) fn new(plan: exact_plan::Plan, assets: crate::image::Assets) -> CanvasText {
        CanvasText {
            fonts: Mutex::new(None),
            plan,
            assets,
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Fonts) -> T) -> T {
        let mut g = self.fonts.lock().unwrap_or_else(|e| e.into_inner());
        let fonts = g.get_or_insert_with(|| {
            let (fonts, names) = crate::text::canvas_font_system(&self.plan, &self.assets);
            Fonts {
                fonts,
                layout: LayoutContext::new(),
                scale: ScaleContext::new(),
                names,
            }
        });
        f(fonts)
    }

    /// A run shaped for drawing.
    pub(crate) fn shape(&self, style: &RunStyle, text: &str, rtl: bool) -> Shaped {
        self.with(|f| f.shape(style, text, rtl))
    }
}

impl TextEngine for CanvasText {
    fn measure(&self, run: &TextRun<'_>) -> RawMetrics {
        let style = RunStyle::from_run(run);
        let (shaped, [ascent, descent, em_ascent, em_descent]) = self.with(|fonts| {
            let shaped = fonts.shape(&style, run.text, run.rtl);
            let v = fonts.vertical(shaped.face.clone(), &style);
            (shaped, v)
        });
        let ink = shaped.path.as_ref().and_then(|p| p.compute_tight_bounds());
        let (left, right, top, bottom) = ink.map_or((0.0, 0.0, 0.0, 0.0), |b| {
            (-b.left(), b.right(), -b.top(), b.bottom())
        });
        RawMetrics {
            width: shaped.width as f64,
            left: left as f64,
            right: right as f64,
            ascent: top as f64,
            descent: bottom as f64,
            font_ascent: ascent as f64,
            font_descent: descent as f64,
            em_ascent: em_ascent as f64,
            em_descent: em_descent as f64,
            hanging: 0.8 * ascent as f64,
            ideographic: -(descent as f64),
        }
    }
}
