//! Text: cosmic-text, one engine for measuring and painting — the lesson of
//! LLP 1008 §3 (measure with one engine, paint with another, and every
//! difference is a bug), in Rust this time.
//!
//! @ref LLP 1015 §3; LLP 1001 §6 (a per-kernel injected measurer)
//!
//! A [`Paragraph`] is a width-specific snapshot: the shaped, wrapped
//! cosmic-text `Buffer`, its size with the same `ceil` the kernel receives,
//! its first baseline. It is cached by (spec, width); the measurer answers
//! from it and the painter paints from it, so what was measured is what is
//! painted, by construction. `line-height: normal` is the font's ascent +
//! descent + line gap (the browser's); a set line height centers the glyphs
//! in the box, which cosmic-text does itself. Glyphs are rasterized by swash
//! once per (glyph, color) into small premultiplied pixmaps.

use cosmic_text::{
    Align, Attrs, Buffer, CacheKey, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics,
    Shaping, Style, SwashCache, SwashContent, Weight, Wrap,
};
use exact_kernel::{AxisOffer, TextAlign, TextMeasureRequest, TextMeasurer, TextMetrics};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tiny_skia::{IntSize, Mask, Pixmap, PixmapPaint, Transform};

/// One styled run: what changes glyph metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The text.
    pub text: String,
    /// Points.
    pub size: f32,
    /// CSS 100–900.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Points; 0 is `normal`.
    pub line_height: f32,
    /// Points per glyph.
    pub letter_spacing: f32,
}

/// A paragraph's specification: runs plus paragraph style.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// The runs, in order.
    pub runs: Vec<Run>,
    /// Alignment.
    pub align: TextAlign,
    /// Maximum lines; 0 is unlimited.
    pub line_clamp: u32,
}

impl Spec {
    /// The spec a kernel measure request describes.
    pub fn from_request(request: &TextMeasureRequest<'_>) -> Spec {
        Spec {
            runs: request
                .runs
                .iter()
                .map(|r| Run {
                    text: r.text.to_string(),
                    size: r.style.font_size,
                    weight: r.style.font_weight,
                    italic: r.style.font_style != exact_kernel::FontStyle::Normal,
                    line_height: r.style.line_height,
                    letter_spacing: r.style.letter_spacing,
                })
                .collect(),
            align: request.paragraph.text_align,
            line_clamp: request.paragraph.line_clamp,
        }
    }

    /// Whether there is nothing to shape.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }

    fn key(&self, width: Option<f32>) -> String {
        // A hash key with the floats as bits, so 0.1 + 0.2 is not 0.3.
        let mut s = String::new();
        for r in &self.runs {
            s.push_str(&format!(
                "{}|{}|{}|{}|{}|{}\u{1}",
                r.text,
                r.size.to_bits(),
                r.weight,
                r.italic,
                r.line_height.to_bits(),
                r.letter_spacing.to_bits()
            ));
        }
        s.push_str(&format!(
            "{:?}|{}|{}",
            self.align,
            self.line_clamp,
            width.map_or(u32::MAX, f32::to_bits)
        ));
        s
    }
}

/// A shaped, wrapped paragraph at one width: what is measured is what is
/// painted.
pub struct Paragraph {
    /// The laid-out buffer.
    pub buffer: Buffer,
    /// Points, rounded up.
    pub width: f32,
    /// Points, rounded up.
    pub height: f32,
    /// Top to the first alphabetic baseline, points.
    pub first_baseline: f32,
}

struct Glyph {
    pixmap: Pixmap,
    left: i32,
    top: i32,
}

/// The engine: fonts, the paragraph cache, the glyph cache, the counters.
pub struct TextEngine {
    fonts: FontSystem,
    swash: SwashCache,
    paragraphs: HashMap<String, Rc<Paragraph>>,
    glyphs: HashMap<(CacheKey, u32), Option<Rc<Glyph>>>,
    normal: HashMap<(u32, u16, bool), f32>,
    /// How many times the kernel asked, since launch.
    pub measures: usize,
    /// How many were answered from cache.
    pub hits: usize,
    /// Time spent shaping on misses.
    pub shaping: Duration,
}

/// The engine shared between the measurer and the painter.
pub type Shared = Rc<RefCell<TextEngine>>;

impl Default for TextEngine {
    fn default() -> Self {
        TextEngine::new()
    }
}

impl TextEngine {
    /// Load the system's fonts (`EXACT_FONTS` adds a directory).
    pub fn new() -> TextEngine {
        let mut fonts = FontSystem::new();
        if let Ok(dir) = std::env::var("EXACT_FONTS") {
            fonts.db_mut().load_fonts_dir(dir);
        }
        if fonts.db().faces().next().is_none() {
            eprintln!("exact: no fonts found; text will not shape (set EXACT_FONTS to a directory of .ttf files)");
        }
        TextEngine {
            fonts,
            swash: SwashCache::new(),
            paragraphs: HashMap::new(),
            glyphs: HashMap::new(),
            normal: HashMap::new(),
            measures: 0,
            hits: 0,
            shaping: Duration::ZERO,
        }
    }

    /// Shared, for a measurer and a painter.
    pub fn shared() -> Shared {
        Rc::new(RefCell::new(TextEngine::new()))
    }

    /// How many font faces are loaded.
    pub fn face_count(&self) -> usize {
        self.fonts.db().faces().count()
    }

    fn attrs(run: &Run) -> Attrs<'static> {
        let mut a = Attrs::new()
            .family(Family::SansSerif)
            .weight(Weight(run.weight))
            .style(if run.italic {
                Style::Italic
            } else {
                Style::Normal
            });
        if run.letter_spacing != 0.0 && run.size > 0.0 {
            a = a.letter_spacing(run.letter_spacing / run.size);
        }
        a
    }

    /// CSS `line-height: normal` for a run: the font's ascent + descent +
    /// line gap at the run's size, from the font the shaper picks.
    pub fn normal_line_height(&mut self, run: &Run) -> f32 {
        let key = (run.size.to_bits(), run.weight, run.italic);
        if let Some(h) = self.normal.get(&key) {
            return *h;
        }
        let mut probe = Buffer::new(
            &mut self.fonts,
            Metrics::new(run.size.max(1.0), run.size.max(1.0)),
        );
        probe.set_text("x", &Self::attrs(run), Shaping::Advanced, None);
        probe.shape_until_scroll(&mut self.fonts, false);
        let mut height = run.size * 1.2;
        if let Some(g) = probe.layout_runs().flat_map(|r| r.glyphs.iter()).next() {
            if let Some(font) = self.fonts.get_font(g.font_id, g.font_weight) {
                let m = font.metrics();
                if m.units_per_em > 0 {
                    height =
                        (m.ascent + m.descent.abs() + m.leading) * run.size / m.units_per_em as f32;
                }
            }
        }
        self.normal.insert(key, height);
        height
    }

    fn line_height(&mut self, run: &Run) -> f32 {
        if run.line_height > 0.0 {
            run.line_height
        } else {
            self.normal_line_height(run)
        }
    }

    /// The paragraph for `spec` wrapped at `width` (`None` is max-content).
    /// Cached.
    pub fn paragraph(&mut self, spec: &Spec, width: Option<f32>) -> Rc<Paragraph> {
        let key = spec.key(width);
        if let Some(p) = self.paragraphs.get(&key) {
            return p.clone();
        }
        if self.paragraphs.len() > 4096 {
            self.paragraphs.clear();
        }
        let p = Rc::new(self.layout(spec, width));
        self.paragraphs.insert(key, p.clone());
        p
    }

    fn layout(&mut self, spec: &Spec, width: Option<f32>) -> Paragraph {
        let line_heights: Vec<f32> = spec.runs.iter().map(|r| self.line_height(r)).collect();
        let base = spec.runs.first().map_or(16.0, |r| r.size.max(0.5));
        let tallest = line_heights.iter().copied().fold(0.0f32, f32::max).max(1.0);
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(base, tallest));
        // CSS `overflow-wrap: normal`: lines break between words; a word
        // longer than the line overflows it, never breaks.
        buffer.set_wrap(Wrap::Word);
        buffer.set_size(width.map(|w| w.max(0.0)), None);
        if spec.line_clamp > 0 {
            buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(
                spec.line_clamp as usize,
            )));
        }
        let align = match spec.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(Align::Center),
            TextAlign::Right => Some(Align::Right),
            TextAlign::Justify => Some(Align::Justified),
        };
        let spans: Vec<(&str, Attrs<'static>)> = spec
            .runs
            .iter()
            .zip(line_heights.iter())
            .map(|(r, lh)| {
                (
                    r.text.as_str(),
                    Self::attrs(r).metrics(Metrics::new(r.size.max(0.5), lh.max(1.0))),
                )
            })
            .collect();
        let default = spec
            .runs
            .first()
            .map(Self::attrs)
            .unwrap_or_else(Attrs::new);
        buffer.set_rich_text(spans, &default, Shaping::Advanced, align);
        buffer.shape_until_scroll(&mut self.fonts, false);
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let mut first = None;
        for run in buffer.layout_runs() {
            w = w.max(run.line_w);
            h = h.max(run.line_top + run.line_height);
            first.get_or_insert(run.line_y);
        }
        Paragraph {
            buffer,
            width: w.ceil(),
            height: h.ceil(),
            first_baseline: first.unwrap_or(0.0),
        }
    }

    /// As narrow as the content can be: the longest unbreakable piece —
    /// wrapped at every word boundary (width zero), the widest line is the
    /// widest word.
    fn min_content_width(&mut self, spec: &Spec) -> f32 {
        let p = self.paragraph(spec, Some(0.0));
        p.width
    }

    /// The kernel's question: a paragraph under an offer.
    pub fn measure(&mut self, spec: &Spec, width: AxisOffer) -> TextMetrics {
        self.measures += 1;
        if spec.is_empty() {
            return TextMetrics::default();
        }
        let started = Instant::now();
        let before = self.paragraphs.len();
        let w = match width {
            AxisOffer::Definite(w) => Some(w.max(0.0)),
            AxisOffer::MaxContent => None,
            AxisOffer::MinContent => Some(self.min_content_width(spec)),
        };
        let p = self.paragraph(spec, w);
        if self.paragraphs.len() == before {
            self.hits += 1;
        } else {
            self.shaping += started.elapsed();
        }
        TextMetrics {
            width: p.width,
            height: p.height,
            first_baseline: Some(p.first_baseline),
        }
    }

    fn glyph(&mut self, key: CacheKey, color: [u8; 4]) -> Option<Rc<Glyph>> {
        let color_bits = u32::from_be_bytes(color);
        if let Some(g) = self.glyphs.get(&(key, color_bits)) {
            return g.clone();
        }
        if self.glyphs.len() > 8192 {
            self.glyphs.clear();
        }
        let image = self.swash.get_image(&mut self.fonts, key).clone();
        let glyph = image.and_then(|img| {
            let (w, h) = (img.placement.width, img.placement.height);
            if w == 0 || h == 0 {
                return None;
            }
            let [r, g, b, a] = color;
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            match img.content {
                SwashContent::Mask => {
                    for &m in &img.data {
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                SwashContent::SubpixelMask => {
                    for px in img.data.chunks_exact(4) {
                        let m = px[0].max(px[1]).max(px[2]);
                        let alpha = (m as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(r, g, b, alpha));
                    }
                }
                SwashContent::Color => {
                    for px in img.data.chunks_exact(4) {
                        let alpha = (px[3] as u32 * a as u32 / 255) as u8;
                        data.extend_from_slice(&premultiply(px[0], px[1], px[2], alpha));
                    }
                }
            }
            let pixmap = Pixmap::from_vec(data, IntSize::from_wh(w, h)?)?;
            Some(Rc::new(Glyph {
                pixmap,
                left: img.placement.left,
                top: img.placement.top,
            }))
        });
        self.glyphs.insert((key, color_bits), glyph.clone());
        glyph
    }

    /// Paint a paragraph. `origin` is the paragraph's top-left in points;
    /// `transform` maps points to device pixels (the scale included);
    /// glyphs are rasterized at the device scale and positioned on the
    /// pixel grid by cosmic-text.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        target: &mut Pixmap,
        paragraph: &Paragraph,
        color: [u8; 4],
        origin: (f32, f32),
        scale: f32,
        transform: Transform,
        mask: Option<&Mask>,
    ) {
        if color[3] == 0 {
            return;
        }
        // Glyph positions carry the device scale; the transform's own scale
        // must not apply twice.
        let glyph_ts = transform.pre_scale(1.0 / scale, 1.0 / scale);
        let paint = PixmapPaint::default();
        let mut placed = Vec::new();
        for run in paragraph.buffer.layout_runs() {
            for g in run.glyphs {
                let phys = g.physical(((origin.0) * scale, (origin.1 + run.line_y) * scale), scale);
                placed.push((phys.cache_key, phys.x, phys.y));
            }
        }
        for (key, x, y) in placed {
            let Some(glyph) = self.glyph(key, color) else {
                continue;
            };
            target.draw_pixmap(
                x + glyph.left,
                y - glyph.top,
                glyph.pixmap.as_ref(),
                &paint,
                glyph_ts,
                mask,
            );
        }
    }
}

fn premultiply(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let p = |c: u8| (c as u32 * a as u32 / 255) as u8;
    [p(r), p(g), p(b), a]
}

/// The kernel's measurer over the shared engine.
pub struct Measurer(pub Shared);

impl TextMeasurer for Measurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let spec = Spec::from_request(request);
        self.0.borrow_mut().measure(&spec, request.width)
    }
}
