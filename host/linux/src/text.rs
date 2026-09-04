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

use crate::image::Assets;
use cosmic_text::{
    fontdb, Align, Attrs, Buffer, CacheKey, Ellipsize, EllipsizeHeightLimit, Family, FontSystem,
    Metrics, PenikoFont, Shaping, Style, SwashCache, SwashContent, Weight, Wrap,
};
use exact_kernel::{AxisOffer, TextAlign, TextMeasureRequest, TextMeasurer, TextMetrics};
use exact_plan::{Plan, StackMemberKind, StacksId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
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
    /// Plan font stack id.
    pub family: u16,
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
                    family: r.style.font_family,
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
                "{}|{}|{}|{}|{}|{}|{}\u{1}",
                r.text,
                r.size.to_bits(),
                r.weight,
                r.family,
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

#[derive(Debug, Clone)]
enum FamilyChoice {
    SansSerif,
    Serif,
    Monospace,
    Declared(String),
}

impl FamilyChoice {
    fn cosmic(&self) -> Family<'_> {
        match self {
            FamilyChoice::SansSerif => Family::SansSerif,
            FamilyChoice::Serif => Family::Serif,
            FamilyChoice::Monospace => Family::Monospace,
            FamilyChoice::Declared(name) => Family::Name(name),
        }
    }
}

/// A run of glyphs from one font at one size, for a backend that draws
/// outlines itself (the GPU): glyph ids with their positions in points from
/// the paragraph's top-left.
pub struct GlyphRun {
    /// The font's data and collection index.
    pub font: PenikoFont,
    /// Points.
    pub size: f32,
    /// (glyph id, x, y) — y is the baseline.
    pub glyphs: Vec<(u32, f32, f32)>,
}

/// The engine: fonts, the paragraph cache, the glyph cache, the counters.
pub struct TextEngine {
    fonts: FontSystem,
    swash: SwashCache,
    paragraphs: HashMap<String, Rc<Paragraph>>,
    glyphs: HashMap<(CacheKey, u32), Option<Rc<Glyph>>>,
    normal: HashMap<(u16, u32, u16, bool), f32>,
    font_data: HashMap<(fontdb::ID, u16), Option<PenikoFont>>,
    /// A requested (weight, italic) → the weight of the face the family
    /// actually has for it (see `snap_weight`).
    weights: HashMap<(u16, u16, bool), u16>,
    /// Plan stack id → the fontdb family this plan-scoped catalog owns.
    families: Vec<FamilyChoice>,
    /// Declared face identity: the ids loaded from the declared bytes.
    declared_faces: HashMap<(u16, u16, bool), fontdb::ID>,
    /// How many times the kernel asked, since launch.
    pub measures: usize,
    /// How many were answered from cache.
    pub hits: usize,
    /// Time spent shaping on misses.
    pub shaping: Duration,
    /// The family `sans-serif` resolves to.
    pub sans: String,
}

/// The installed family `sans-serif` should mean: `EXACT_FONT`, else the
/// database's default when it is installed, else the first present of
/// fontconfig's preference list for `sans-serif` (60-latin.conf) with the
/// platform's own families after it.
fn sans_family(db: &fontdb::Database) -> Option<String> {
    let installed = |name: &str| {
        db.faces()
            .any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
    };
    if let Ok(name) = std::env::var("EXACT_FONT") {
        if installed(&name) {
            return Some(name);
        }
        eprintln!("exact: EXACT_FONT {name} is not an installed family");
    }
    let current = db.family_name(&fontdb::Family::SansSerif).to_string();
    if installed(&current) {
        return Some(current);
    }
    // fontconfig's 60-latin.conf order on Linux; the browser's on macOS.
    let preferred: &[&str] = if cfg!(target_os = "macos") {
        &["Helvetica Neue", "Helvetica", "Arial", "Verdana"]
    } else {
        &[
            "Noto Sans",
            "DejaVu Sans",
            "Verdana",
            "Arial",
            "Liberation Sans",
            "Nimbus Sans",
            "Cantarell",
            "Ubuntu",
            "Roboto",
            "Segoe UI",
        ]
    };
    preferred
        .iter()
        .find(|n| installed(n))
        .map(|n| n.to_string())
}

/// The engine shared between the measurer and the painter.
pub type Shared = Rc<RefCell<TextEngine>>;

impl Default for TextEngine {
    fn default() -> Self {
        TextEngine::new()
    }
}

impl TextEngine {
    /// Load the system's fonts (`EXACT_FONTS` adds a directory) and settle
    /// what `sans-serif` means: `EXACT_FONT` when set, else the first
    /// installed family in fontconfig's own preference order — the answer
    /// a browser gets from `fc-match sans-serif`. Left to its default,
    /// cosmic-text names a family that may not exist ("Open Sans" on Linux),
    /// and its per-glyph fallback then scores every font on the machine by
    /// weight: at 600 the Caltrain app's button came out in URW Bookman with
    /// a space from Noto Color Emoji, 16 pt wide.
    pub fn new() -> TextEngine {
        let mut fonts = FontSystem::new();
        if let Ok(dir) = std::env::var("EXACT_FONTS") {
            fonts.db_mut().load_fonts_dir(dir);
        }
        if fonts.db().faces().next().is_none() {
            eprintln!("exact: no fonts found; text will not shape (set EXACT_FONTS to a directory of .ttf files)");
        }
        let sans = sans_family(fonts.db());
        if let Some(name) = &sans {
            fonts.db_mut().set_sans_serif_family(name.clone());
        }
        TextEngine {
            sans: sans.unwrap_or_default(),
            fonts,
            swash: SwashCache::new(),
            paragraphs: HashMap::new(),
            glyphs: HashMap::new(),
            normal: HashMap::new(),
            font_data: HashMap::new(),
            weights: HashMap::new(),
            families: vec![
                FamilyChoice::SansSerif,
                FamilyChoice::SansSerif,
                FamilyChoice::SansSerif,
                FamilyChoice::Serif,
                FamilyChoice::Serif,
                FamilyChoice::Monospace,
                FamilyChoice::Monospace,
                FamilyChoice::SansSerif,
            ],
            declared_faces: HashMap::new(),
            measures: 0,
            hits: 0,
            shaping: Duration::ZERO,
        }
    }

    /// Shared, for a measurer and a painter.
    pub fn shared() -> Shared {
        Rc::new(RefCell::new(TextEngine::new()))
    }

    /// A fresh, plan-scoped catalog. Registration failure leaves that stack
    /// on the system last resort and names the refused identity on stderr;
    /// boot still presents (LLP 1019 D5).
    pub fn shared_for_plan(plan: &Plan, assets: &Path) -> Shared {
        Self::shared_for_assets(plan, &Assets::embedded(assets.to_path_buf()))
    }

    /// A fresh catalog whose declared faces resolve through one immutable
    /// bundle generation.
    pub(crate) fn shared_for_assets(plan: &Plan, assets: &Assets) -> Shared {
        let mut engine = TextEngine::new();
        engine.install_plan_assets(plan, assets);
        Rc::new(RefCell::new(engine))
    }

    /// Replace the complete catalog and every family-bearing cache. This is
    /// the plan-identity boundary on a dev reload (LLP 1019 D4).
    pub fn install_plan(&mut self, plan: &Plan, assets: &Path) {
        self.install_plan_assets(plan, &Assets::embedded(assets.to_path_buf()));
    }

    fn install_plan_assets(&mut self, plan: &Plan, assets: &Assets) {
        let mut next = TextEngine::new();
        next.families = Vec::with_capacity(plan.stacks.len());
        next.families
            .extend(plan.stacks.iter().enumerate().map(|(i, _)| {
                let stack = plan.stack(StacksId(i as u32));
                let member =
                    plan.stack_member(stack.members.iter().next().expect("validated stack"));
                match member.kind {
                    StackMemberKind::UiSerif | StackMemberKind::Serif => FamilyChoice::Serif,
                    StackMemberKind::UiMonospace | StackMemberKind::Monospace => {
                        FamilyChoice::Monospace
                    }
                    _ => FamilyChoice::SansSerif,
                }
            }));

        for (stack_index, stack) in plan.stacks.iter().enumerate() {
            let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
            if member.kind != StackMemberKind::Family {
                continue;
            }
            let family_id = member.family.expect("validated family member");
            let family = plan.familie(family_id);
            let alias = format!("ExactPlanStack{stack_index}");
            let mut staged = Vec::new();
            let mut failed = false;
            for face_id in family.faces.iter() {
                let face = plan.face(face_id);
                let source = plan.str(face.source);
                let Some(bytes) = assets.read(source) else {
                    failed = true;
                    break;
                };
                let mut parsed = fontdb::Database::new();
                let ids = parsed.load_font_source(fontdb::Source::Binary(Arc::new(bytes.to_vec())));
                if ids.len() != 1 {
                    failed = true;
                    break;
                }
                let mut info = parsed.face(ids[0]).expect("returned face id").clone();
                let Some(language) = info.families.first().map(|(_, language)| *language) else {
                    failed = true;
                    break;
                };
                info.id = fontdb::ID::dummy();
                info.families = vec![(alias.clone(), language)];
                info.weight = fontdb::Weight(face.weight);
                info.style = if face.italic {
                    fontdb::Style::Italic
                } else {
                    fontdb::Style::Normal
                };
                info.stretch = fontdb::Stretch::Normal;
                staged.push((face.weight, face.italic, info));
            }
            if failed || staged.len() != family.faces.len as usize {
                eprintln!(
                    "[Fonts] font.registration.failed: stack={stack_index} family={}",
                    family_id.0
                );
                continue;
            }
            next.families[stack_index] = FamilyChoice::Declared(alias);
            for (weight, italic, info) in staged {
                let id = next.fonts.db_mut().push_face_info(info);
                next.declared_faces
                    .insert((stack_index as u16, weight, italic), id);
            }
        }
        *self = next;
    }

    /// The exact loaded face id chosen for a run, for identity assertions.
    pub fn resolved_face_id(
        &mut self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<fontdb::ID> {
        let choice = self
            .families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        self.fonts.db().query(&fontdb::Query {
            families: &[choice.cosmic()],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        })
    }

    /// The id loaded for one declared face before matching.
    pub fn declared_face_id(&self, family: u16, weight: u16, italic: bool) -> Option<fontdb::ID> {
        self.declared_faces.get(&(family, weight, italic)).copied()
    }

    /// How many font faces are loaded.
    pub fn face_count(&self) -> usize {
        self.fonts.db().faces().count()
    }

    /// The weight to shape with for a requested one: the weight of the face
    /// CSS font matching picks from the `sans-serif` family (`fontdb::Query`
    /// — for 600 with Book and Bold on hand, Bold). cosmic-text's fallback
    /// takes the requested weight literally and ranks any face whose
    /// variable `wght` axis covers it above the family's nearest static
    /// face: on a Mac, weight 500 and 600 came out in San Francisco while
    /// 400 and 700 were the pinned DejaVu, and the app's weight-600 button
    /// measured 104 wide here against 128 on a builder with the same font
    /// bytes. Asking for the family's own weight keeps the family first,
    /// the browser's rule (family, then weight).
    fn snap_weight(&mut self, family: u16, weight: u16, italic: bool) -> u16 {
        let key = (family, weight, italic);
        if let Some(w) = self.weights.get(&key) {
            return *w;
        }
        let family_choice = self
            .families
            .get(family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        let query = fontdb::Query {
            families: &[family_choice.cosmic()],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
        };
        let db = self.fonts.db();
        let snapped = db
            .query(&query)
            .and_then(|id| db.face(id))
            .map(|face| face.weight.0)
            .unwrap_or(weight);
        self.weights.insert(key, snapped);
        snapped
    }

    fn attrs<'a>(run: &Run, weight: u16, family: Family<'a>) -> Attrs<'a> {
        let mut a = Attrs::new()
            .family(family)
            .weight(Weight(weight))
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
        let key = (run.family, run.size.to_bits(), run.weight, run.italic);
        if let Some(h) = self.normal.get(&key) {
            return *h;
        }
        let weight = self.snap_weight(run.family, run.weight, run.italic);
        let family = self
            .families
            .get(run.family as usize)
            .cloned()
            .unwrap_or(FamilyChoice::SansSerif);
        let mut probe = Buffer::new(
            &mut self.fonts,
            Metrics::new(run.size.max(1.0), run.size.max(1.0)),
        );
        probe.set_text(
            "x",
            &Self::attrs(run, weight, family.cosmic()),
            Shaping::Advanced,
            None,
        );
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
        let weights: Vec<u16> = spec
            .runs
            .iter()
            .map(|r| self.snap_weight(r.family, r.weight, r.italic))
            .collect();
        let families: Vec<FamilyChoice> = spec
            .runs
            .iter()
            .map(|r| {
                self.families
                    .get(r.family as usize)
                    .cloned()
                    .unwrap_or(FamilyChoice::SansSerif)
            })
            .collect();
        let spans: Vec<(&str, Attrs<'_>)> = spec
            .runs
            .iter()
            .zip(line_heights.iter())
            .zip(weights.iter())
            .zip(families.iter())
            .map(|(((r, lh), w), family)| {
                (
                    r.text.as_str(),
                    Self::attrs(r, *w, family.cosmic())
                        .metrics(Metrics::new(r.size.max(0.5), lh.max(1.0))),
                )
            })
            .collect();
        let default = spec
            .runs
            .first()
            .zip(weights.first())
            .zip(families.first())
            .map(|((r, w), family)| Self::attrs(r, *w, family.cosmic()))
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

impl TextEngine {
    /// A font's data handle (shared, cheap to clone), cached.
    pub fn font_data(&mut self, id: fontdb::ID, weight: Weight) -> Option<PenikoFont> {
        let key = (id, weight.0);
        if let Some(f) = self.font_data.get(&key) {
            return f.clone();
        }
        let f = self.fonts.get_font(id, weight).map(|f| f.as_peniko());
        self.font_data.insert(key, f.clone());
        f
    }

    /// A paragraph as glyph runs grouped by font and size, positions in
    /// points from the paragraph's top-left, the same numbers the raster
    /// path snaps to pixels.
    pub fn glyph_runs(&mut self, paragraph: &Paragraph) -> Vec<GlyphRun> {
        type Key = (fontdb::ID, u16, u32);
        type Runs = Vec<(Key, Vec<(u32, f32, f32)>)>;
        let mut runs: Runs = Vec::new();
        for run in paragraph.buffer.layout_runs() {
            for g in run.glyphs {
                let key = (g.font_id, g.font_weight.0, g.font_size.to_bits());
                let x = g.x + g.x_offset * g.font_size;
                let y = run.line_y + g.y - g.y_offset * g.font_size;
                match runs.last_mut() {
                    Some((k, glyphs)) if *k == key => glyphs.push((g.glyph_id as u32, x, y)),
                    _ => runs.push((key, vec![(g.glyph_id as u32, x, y)])),
                }
            }
        }
        runs.into_iter()
            .filter_map(|((id, weight, size), glyphs)| {
                let font = self.font_data(id, Weight(weight))?;
                Some(GlyphRun {
                    font,
                    size: f32::from_bits(size),
                    glyphs,
                })
            })
            .collect()
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
