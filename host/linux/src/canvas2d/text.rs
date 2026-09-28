//! Canvas 2D text on Linux (LLP 1056 D8): one cosmic-text font system for
//! the canvases, loaded as the host's catalog is (system fonts,
//! `EXACT_FONTS`, the plan's declared faces), behind a mutex so the runner
//! measures on its thread and the replayer draws with the same engine —
//! what was measured is what is drawn.
//!
//! A run is shaped once and drawn as glyph outlines: a tiny-skia path in
//! run space (origin at the run's left end on its alphabetic baseline, y
//! down), so text takes every paint, shadow, compositing operator and clip a
//! path takes. Colour glyphs draw as their outlines (declared, LLP 1056
//! §8.2). A right-to-left run is shaped with a leading RLM so the paragraph
//! direction is RTL, as `direction = "rtl"` asks.

use cosmic_text::{
    fontdb, Attrs, Buffer, CacheKeyFlags, Command, FeatureTag, FontFeatures, FontSystem, Metrics,
    Shaping, Stretch, Style, SwashCache, Weight,
};
use exact_canvas::{RawMetrics, TextEngine, TextRun};
use std::sync::Mutex;
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
    face: Option<(fontdb::ID, u16)>,
}

struct Fonts {
    system: FontSystem,
    swash: SwashCache,
    names: Vec<(String, FamilyChoice)>,
}

/// The canvases' text engine. Its font system loads at the first text a
/// canvas measures or draws, so a canvas app without text pays nothing.
pub(crate) struct CanvasText {
    fonts: Mutex<Option<Fonts>>,
    plan: exact_plan::Plan,
    assets: crate::image::Assets,
}

fn stretch(pct: f32) -> Stretch {
    match pct {
        p if p <= 56.0 => Stretch::UltraCondensed,
        p if p <= 68.0 => Stretch::ExtraCondensed,
        p if p <= 81.0 => Stretch::Condensed,
        p if p <= 93.0 => Stretch::SemiCondensed,
        p if p <= 106.0 => Stretch::Normal,
        p if p <= 118.0 => Stretch::SemiExpanded,
        p if p <= 137.0 => Stretch::Expanded,
        p if p <= 175.0 => Stretch::ExtraExpanded,
        _ => Stretch::UltraExpanded,
    }
}

impl Fonts {
    /// The first family of the list that resolves: a generic or declared
    /// family by the plan's choice, another by an installed family's name;
    /// none, serif (Chrome's default).
    fn family(&self, families: &[String]) -> FamilyChoice {
        for f in families {
            if let Some((_, c)) = self.names.iter().find(|(n, _)| n.eq_ignore_ascii_case(f)) {
                return c.clone();
            }
            let installed = self
                .system
                .db()
                .faces()
                .any(|face| face.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(f)));
            if installed {
                return FamilyChoice::Declared(f.clone());
            }
        }
        FamilyChoice::Serif
    }

    fn shape(&mut self, style: &RunStyle, text: &str, rtl: bool) -> Shaped {
        let size = style.size.max(0.0);
        let family = self.family(&style.families);
        let mut attrs = Attrs::new()
            .family(family.cosmic())
            .weight(Weight(style.weight))
            .style(match style.style {
                1 => Style::Italic,
                2 => Style::Oblique,
                _ => Style::Normal,
            })
            .stretch(stretch(style.stretch))
            .cache_key_flags(CacheKeyFlags::DISABLE_HINTING);
        if style.letter_spacing != 0.0 && size > 0.0 {
            attrs = attrs.letter_spacing(style.letter_spacing / size);
        }
        if style.kerning == 2 {
            let mut features = FontFeatures::new();
            features.disable(FeatureTag::new(b"kern"));
            attrs = attrs.font_features(features);
        }
        if size == 0.0 || text.is_empty() {
            return Shaped {
                path: None,
                width: 0.0,
                face: None,
            };
        }
        let shaped_text;
        let text = if rtl {
            shaped_text = format!("\u{200f}{text}");
            &shaped_text
        } else {
            text
        };
        let mut buffer = Buffer::new(&mut self.system, Metrics::new(size, size * 1.2));
        buffer.set_size(None, None);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.system, false);
        let mut pb = PathBuilder::new();
        let mut width = 0.0f32;
        let mut face = None;
        let lead = if rtl { '\u{200f}'.len_utf8() } else { 0 };
        for run in buffer.layout_runs() {
            let mut glyphs: Vec<_> = run.glyphs.iter().collect();
            glyphs.sort_by(|a, b| a.x.total_cmp(&b.x));
            // Word spacing: after each space, in visual order.
            let mut shift = 0.0f32;
            for g in &glyphs {
                if g.start < lead {
                    continue;
                }
                face.get_or_insert((g.font_id, g.font_weight.0));
                let key = g.physical((0.0, 0.0), 1.0).cache_key;
                let ox = g.x + g.font_size * g.x_offset + shift;
                let oy = -g.font_size * g.y_offset;
                if let Some(commands) = self.swash.get_outline_commands(&mut self.system, key) {
                    for c in commands {
                        match *c {
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
                if style.word_spacing != 0.0 && text[g.start..g.end].contains(' ') {
                    shift += style.word_spacing;
                }
            }
            width = width.max(run.line_w + shift);
        }
        Shaped {
            path: pb.finish(),
            width,
            face,
        }
    }

    /// The face's vertical metrics at `size`: hhea ascent and descent, and
    /// the OS/2 typographic ones normalised to the em.
    fn vertical(&mut self, face: Option<(fontdb::ID, u16)>, style: &RunStyle) -> [f32; 4] {
        use cosmic_text::skrifa::raw::TableProvider;
        let size = style.size;
        let face = face.or_else(|| {
            let family = self.family(&style.families);
            let id = self.system.db().query(&fontdb::Query {
                families: &[family.cosmic()],
                weight: fontdb::Weight(style.weight),
                stretch: fontdb::Stretch::Normal,
                style: fontdb::Style::Normal,
            })?;
            Some((id, style.weight))
        });
        let fallback = [size * 0.9, size * 0.25, size * 0.8, size * 0.2];
        let Some((id, weight)) = face else {
            return fallback;
        };
        let index = self.system.db().face(id).map_or(0, |f| f.index);
        let Some(font) = self.system.get_font(id, Weight(weight)) else {
            return fallback;
        };
        let Ok(f) = cosmic_text::skrifa::FontRef::from_index(font.data(), index) else {
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
            let (system, names) = crate::text::canvas_font_system(&self.plan, &self.assets);
            Fonts {
                system,
                swash: SwashCache::new(),
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
            let v = fonts.vertical(shaped.face, &style);
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
