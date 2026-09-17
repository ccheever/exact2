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

mod cache;
mod ink;
pub use cache::{HandoffResidency, Residency, RetiringResidency};

use crate::image::Assets;
use cosmic_text::{
    fontdb, Align, Attrs, Buffer, CacheKey, CacheKeyFlags, Ellipsize, EllipsizeHeightLimit, Family,
    FontSystem, LayoutGlyph, Metrics, PenikoFont, Shaping, Style, SwashCache, SwashContent, Weight,
    Wrap,
};
use exact_kernel::{
    AxisOffer, ParagraphStamp, TextAlign, TextMeasureRequest, TextMeasurer, TextMetrics,
};
use exact_plan::{Plan, StackMemberKind, StacksId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_skia::{IntSize, Mask, Pixmap, PixmapPaint, Transform};

// Ascent, descent, and leading in logical points.
type FontMetrics = (f32, f32, f32);

/// One styled run: what changes glyph metrics.
#[derive(Debug, PartialEq)]
#[cfg_attr(not(test), derive(Clone))]
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
    /// Resolved logical length, or normal font metrics.
    pub line_height: Option<f32>,
    /// Points per glyph.
    pub letter_spacing: f32,
}

#[cfg(test)]
impl Clone for Run {
    fn clone(&self) -> Self {
        identified_tests::copied(self.text.len());
        Self {
            text: self.text.clone(),
            size: self.size,
            weight: self.weight,
            family: self.family,
            italic: self.italic,
            line_height: self.line_height,
            letter_spacing: self.letter_spacing,
        }
    }
}

impl Run {
    /// Project the kernel's resolved text style for measuring and painting.
    pub fn from_style(text: &str, style: exact_kernel::TextStyle) -> Self {
        #[cfg(test)]
        identified_tests::copied(text.len());
        Self {
            text: text.into(),
            size: style.font_size,
            weight: style.font_weight,
            family: style.font_family,
            italic: style.font_style != exact_kernel::FontStyle::Normal,
            line_height: style.line_height,
            letter_spacing: style.letter_spacing,
        }
    }
}

/// A paragraph's specification: runs plus paragraph style.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// Paragraph minimum line box font and resolved length.
    pub strut: Run,
    /// The runs, in order.
    pub runs: Vec<Run>,
    /// Alignment.
    pub align: TextAlign,
    /// Maximum lines; 0 is unlimited.
    pub line_clamp: u32,
    /// CSS emergency line-breaking policy.
    pub overflow_wrap: exact_kernel::OverflowWrap,
}

impl Spec {
    /// The spec a kernel measure request describes.
    pub fn from_request(request: &TextMeasureRequest<'_>) -> Spec {
        Spec {
            strut: Run::from_style("", request.paragraph.strut),
            runs: request
                .runs
                .iter()
                .map(|r| Run::from_style(r.text, r.style))
                .collect(),
            align: request.paragraph.text_align,
            line_clamp: request.paragraph.line_clamp,
            overflow_wrap: request.paragraph.overflow_wrap,
        }
    }

    /// Whether there is nothing to shape.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }
}

/// A shaped, wrapped paragraph at one width: what is measured is what is
/// painted.
pub struct Paragraph {
    /// The laid-out buffer.
    buffer: Buffer,
    /// Points, rounded up.
    pub width: f32,
    /// Points, rounded up.
    pub height: f32,
    /// Top to the first alphabetic baseline, points.
    pub first_baseline: f32,
    /// CSS shared-baseline placement for each wrapped line, used by both painters.
    baselines: Vec<f32>,
    ink: RefCell<ink::Cache>,
    // Immutable Buffer/baseline capacities; lazy ink is read separately below.
    resident_capacity_bytes: usize,
    private_text_bytes_estimate: usize,
}

/// Paint-only data in canonical text-run order. Colors and source identity
/// do not enter the shaping cache: identical text can be shared by nodes
/// and repainted in another appearance without changing its metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunPaint {
    /// The inherited color resolved for the current appearance.
    pub color: [u8; 4],
    /// The text leaf; its logical ancestors retain href/press metadata.
    pub source: exact_kernel::ViewId,
}

impl Paragraph {
    /// Immutable full shaped source; CPU visibility never changes this buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// CSS baselines in original wrapped-line order.
    pub fn baselines(&self) -> &[f32] {
        &self.baselines
    }

    /// Current CPU ink arrays, counted by allocated capacity. This grows lazily
    /// at paint and replaces its previous scale/catalog rather than retaining it.
    pub fn ink_capacity_bytes(&self) -> usize {
        self.ink.borrow().bytes()
    }

    /// Current accessible capacity in O(1), without visiting shaped glyphs.
    /// Accounting/maintenance runs outside paint's exclusive ink borrow.
    fn owned_capacity_bytes(&self) -> usize {
        self.resident_capacity_bytes + self.ink_capacity_bytes()
    }

    /// The shared CPU/GPU stream: every glyph keeps its canonical run index
    /// and CSS baseline, and selects its paint data by that index.
    pub fn paint_glyphs<'a>(
        &'a self,
        palette: &'a [RunPaint],
    ) -> impl Iterator<Item = (&'a LayoutGlyph, f32, RunPaint)> + 'a {
        self.buffer
            .layout_runs()
            .zip(&self.baselines)
            .flat_map(move |(line, baseline)| {
                line.glyphs
                    .iter()
                    .map(move |glyph| (glyph, *baseline, palette[glyph.metadata]))
            })
    }
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
    /// Canonical run index, retained across font fallback and wrapping.
    pub run_index: usize,
    /// Resolved ink and logical source identity.
    pub paint: RunPaint,
    /// Match cosmic-text's synthesized oblique outline when no italic face exists.
    pub synthetic_italic: bool,
    /// (glyph id, x, y) — y is the baseline.
    pub glyphs: Vec<(u32, f32, f32)>,
}

/// The engine: fonts, the paragraph cache, the glyph cache, the counters.
pub struct TextEngine {
    fonts: FontSystem,
    swash: SwashCache,
    ink_catalog: Rc<()>,
    #[cfg(test)]
    ink_visits: usize,
    #[cfg(test)]
    ink_nodes: usize,
    #[cfg(test)]
    ink_builds: usize,
    paragraphs: cache::Cache,
    /// Buffer builds, including temporary intrinsic measurements.
    pub shape_calls: usize,
    #[cfg(test)]
    before_layout: Option<Box<dyn FnMut()>>,
    glyphs: HashMap<(CacheKey, u32), Option<Rc<Glyph>>>,
    normal: HashMap<(u16, u32, u16, bool), FontMetrics>,
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

/// Keep a configured generic when installed. Otherwise choose a real
/// monospace family before cosmic-text's per-glyph fallback: an absent
/// `Courier New` on a minimal Linux image can select an oblique face even
/// for normal code. Font matching within the family then owns style/weight.
fn monospace_family(db: &fontdb::Database) -> Option<String> {
    let installed = |name: &str| {
        db.faces()
            .any(|f| f.monospaced && f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)))
    };
    let current = db.family_name(&fontdb::Family::Monospace);
    if installed(current) {
        return Some(current.to_owned());
    }
    [
        "Noto Sans Mono",
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Menlo",
        "Monaco",
        "Courier New",
    ]
    .into_iter()
    .find(|name| installed(name))
    .map(str::to_owned)
    .or_else(|| {
        db.faces()
            .filter(|f| f.monospaced && f.style == fontdb::Style::Normal)
            .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
            .min()
    })
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
        if let Some(name) = monospace_family(fonts.db()) {
            fonts.db_mut().set_monospace_family(name);
        }
        TextEngine {
            sans: sans.unwrap_or_default(),
            fonts,
            swash: SwashCache::new(),
            ink_catalog: Rc::new(()),
            #[cfg(test)]
            ink_visits: 0,
            #[cfg(test)]
            ink_nodes: 0,
            #[cfg(test)]
            ink_builds: 0,
            paragraphs: cache::Cache::default(),
            shape_calls: 0,
            #[cfg(test)]
            before_layout: None,
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
        let (ascent, descent, leading) = self.font_metrics(run);
        ascent + descent + leading
    }

    fn font_metrics(&mut self, run: &Run) -> FontMetrics {
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
        let mut height = (run.size * 0.9, run.size * 0.3, 0.0);
        if let Some(g) = probe.layout_runs().flat_map(|r| r.glyphs.iter()).next() {
            if let Some(font) = self.fonts.get_font(g.font_id, g.font_weight) {
                let m = font.metrics();
                if m.units_per_em > 0 {
                    let scale = run.size / m.units_per_em as f32;
                    height = (m.ascent * scale, m.descent.abs() * scale, m.leading * scale);
                }
            }
        }
        self.normal.insert(key, height);
        height
    }

    fn line_height(&mut self, run: &Run) -> f32 {
        run.line_height
            .unwrap_or_else(|| self.normal_line_height(run))
    }

    /// Accessible paragraph storage for this catalog. This excludes fonts,
    /// shared shaper scratch, glyphs and allocator overhead; it is not RSS.
    /// Accepted paragraphs from a replaced catalog are reported separately by
    /// `Painter::retiring_text_residency`, not included here.
    pub fn residency(&self) -> Residency {
        self.paragraphs.residency()
    }

    /// Bounded pending measurements, reported separately from overlapping frame
    /// owners and the cold target. This is not a total resident-memory estimate.
    pub fn handoff_residency(&self) -> HandoffResidency {
        self.paragraphs.handoff_residency()
    }

    pub(crate) fn finish_text_frame(&mut self) {
        self.paragraphs.finish_handoff();
        self.trim_paragraphs();
    }

    pub(crate) fn retiring_accepted<'a>(
        &self,
        accepted: impl Iterator<Item = &'a Rc<Paragraph>>,
    ) -> RetiringResidency {
        self.paragraphs.retiring(accepted)
    }

    pub(crate) fn trim_paragraphs(&mut self) {
        self.paragraphs.trim(None);
    }

    /// The paragraph for `spec` wrapped at `width` (`None` is max-content).
    /// Accepted frames pin snapshots; width lookup does not own their lifetime.
    pub fn paragraph(&mut self, spec: &Spec, width: Option<f32>) -> Rc<Paragraph> {
        let key = self.paragraphs.identity(spec);
        self.paragraph_for(spec, width, key)
    }

    fn identified_spec(
        &mut self,
        stamp: &ParagraphStamp,
        build: impl FnOnce() -> Spec,
    ) -> ((u64, u64), Rc<Spec>) {
        if let Some(key) = self.paragraphs.identified(stamp) {
            return (key, self.paragraphs.spec(key).expect("checked identity"));
        }
        let spec = build();
        let key = self.paragraphs.identity(&spec);
        self.paragraphs.bind(stamp, key);
        (key, self.paragraphs.spec(key).expect("new identity"))
    }

    /// Only the kernel's complete independent paragraph may use this proof.
    /// The closure is never called on a warm metric identity (even a new width).
    pub(crate) fn paragraph_identified(
        &mut self,
        stamp: &ParagraphStamp,
        width: Option<f32>,
        build: impl FnOnce() -> Spec,
    ) -> Option<Rc<Paragraph>> {
        let (key, spec) = self.identified_spec(stamp, build);
        if spec.is_empty() {
            return None;
        }
        Some(self.paragraph_for(&spec, width, key))
    }

    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        let (key, spec) = self.identified_spec(stamp, || Spec::from_request(request));
        self.measure_for(&spec, request.width, key)
    }

    fn paragraph_for(&mut self, spec: &Spec, width: Option<f32>, key: (u64, u64)) -> Rc<Paragraph> {
        if let Some(p) = self.paragraphs.get(key, width.into()) {
            return p;
        }
        self.paragraphs.before_shape(key);
        let p = Rc::new(self.layout(spec, width));
        self.paragraphs.insert(key, width.into(), &p);
        p
    }

    fn layout(&mut self, spec: &Spec, width: Option<f32>) -> Paragraph {
        #[cfg(test)]
        if let Some(callback) = &mut self.before_layout {
            callback();
        }
        #[cfg(test)]
        identified_tests::shaped(spec);
        self.shape_calls += 1;
        let minimum = self.line_height(&spec.strut);
        let (ascent, descent, leading) = self.font_metrics(&spec.strut);
        let half = (minimum - ascent - descent - leading) / 2.0;
        let strut = (ascent + half, descent + leading + half);
        let line_heights: Vec<f32> = spec.runs.iter().map(|r| self.line_height(r)).collect();
        let run_metrics: Vec<_> = spec.runs.iter().map(|r| self.font_metrics(r)).collect();
        let base = spec.runs.first().map_or(16.0, |r| r.size.max(0.5));
        let tallest = line_heights.iter().copied().fold(minimum, f32::max);
        let mut buffer = Buffer::new(
            &mut self.fonts,
            Metrics::new(base, tallest.max(f32::EPSILON)),
        );
        // CSS `overflow-wrap: normal`: lines break between words; a word
        // longer than the line overflows it, never breaks.
        buffer.set_wrap(
            if spec.overflow_wrap == exact_kernel::OverflowWrap::Normal {
                Wrap::Word
            } else {
                Wrap::WordOrGlyph
            },
        );
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
            .enumerate()
            .map(|(index, (((r, lh), w), family))| {
                (
                    r.text.as_str(),
                    // cosmic-text's scrolling loop requires positive pitch.
                    // This shaping-only pitch never escapes: CSS placement below
                    // uses the original lengths, including zero, by run metadata.
                    Self::attrs(r, *w, family.cosmic())
                        .metadata(index)
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
        let mut baselines = Vec::new();
        let mut explicit = false;
        for run in buffer.layout_runs() {
            w = w.max(run.line_w);
            let (mut above, mut below) = strut;
            let mut above_explicit = spec.strut.line_height.is_some();
            let mut below_explicit = above_explicit;
            for glyph in run.glyphs {
                if let Some(font) = self.fonts.get_font(glyph.font_id, glyph.font_weight) {
                    let m = font.metrics();
                    let scale = glyph.font_size / m.units_per_em as f32;
                    // Explicit lengths size the authored inline box; only
                    // normal expands to the actual fallback glyph font.
                    let (ascent, descent, leading) =
                        if spec.runs[glyph.metadata].line_height.is_some() {
                            run_metrics[glyph.metadata]
                        } else {
                            (m.ascent * scale, m.descent.abs() * scale, m.leading * scale)
                        };
                    let height = spec.runs[glyph.metadata]
                        .line_height
                        .unwrap_or(ascent + descent + leading);
                    let half = (height - ascent - descent - leading) / 2.0;
                    let run_explicit = spec.runs[glyph.metadata].line_height.is_some();
                    let (a, b) = (ascent + half, descent + leading + half);
                    if a > above {
                        above = a;
                        above_explicit = run_explicit;
                    } else if a == above {
                        above_explicit &= run_explicit;
                    }
                    if b > below {
                        below = b;
                        below_explicit = run_explicit;
                    } else if b == below {
                        below_explicit &= run_explicit;
                    }
                }
            }
            explicit |= above_explicit || below_explicit;
            baselines.push(h + above);
            h += above + below;
        }
        let mut paragraph = Paragraph {
            buffer,
            width: w.ceil(),
            height: if explicit { h } else { h.ceil() },
            first_baseline: baselines.first().copied().unwrap_or(0.0),
            baselines,
            ink: RefCell::new(ink::Cache::default()),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        (
            paragraph.resident_capacity_bytes,
            paragraph.private_text_bytes_estimate,
        ) = cache::capacities(&paragraph);
        paragraph
    }

    // Intrinsic questions keep scalar answers only. The zero-width scratch
    // Buffer is dropped before allocating the final min-content measurement.
    fn intrinsic(&mut self, spec: &Spec, minimum: bool, key: (u64, u64)) -> TextMetrics {
        if let Some(metrics) = self.paragraphs.intrinsic(key, minimum) {
            return metrics;
        }
        // No intrinsic Buffer enters the handoff set. A miss trades this
        // identity's pending definite result for scratch; accepted owners remain.
        self.paragraphs.release_handoff(key.1);
        self.paragraphs.before_shape(key);
        let width = if minimum {
            if spec.overflow_wrap == exact_kernel::OverflowWrap::BreakWord {
                let mut intrinsic = spec.clone();
                intrinsic.overflow_wrap = exact_kernel::OverflowWrap::Normal;
                Some(self.layout(&intrinsic, Some(0.0)).width)
            } else {
                Some(self.layout(spec, Some(0.0)).width)
            }
        } else {
            None
        };
        let metrics = paragraph_metrics(&self.layout(spec, width));
        self.paragraphs.set_intrinsic(key, minimum, metrics);
        metrics
    }

    /// The kernel's question: a paragraph under an offer.
    pub fn measure(&mut self, spec: &Spec, width: AxisOffer) -> TextMetrics {
        if spec.is_empty() {
            self.measures += 1;
            return TextMetrics::default();
        }
        let key = self.paragraphs.identity(spec);
        self.measure_for(spec, width, key)
    }

    fn measure_for(&mut self, spec: &Spec, width: AxisOffer, key: (u64, u64)) -> TextMetrics {
        self.measures += 1;
        if spec.is_empty() {
            return TextMetrics::default();
        }
        let started = Instant::now();
        let before = self.shape_calls;
        let metrics = match width {
            AxisOffer::Definite(w) => {
                let width = Some(w.max(0.0));
                self.paragraphs.prepare_handoff(key.1, width.into());
                let p = self.paragraph_for(spec, width, key);
                self.paragraphs.hold_measured(key.1, width.into(), &p);
                paragraph_metrics(&p)
            }
            AxisOffer::MaxContent => self.intrinsic(spec, false, key),
            AxisOffer::MinContent => self.intrinsic(spec, true, key),
        };
        if self.shape_calls == before {
            self.hits += 1;
        } else {
            self.shaping += started.elapsed();
        }
        metrics
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
        palette: &[RunPaint],
        origin: (f32, f32),
        scale: f32,
        transform: Transform,
        mask: Option<&Mask>,
    ) {
        let clip = (0.0, 0.0, target.width() as f32, target.height() as f32);
        self.paint_clipped(
            target, paragraph, palette, origin, scale, transform, mask, clip,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint_clipped(
        &mut self,
        target: &mut Pixmap,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        scale: f32,
        transform: Transform,
        mask: Option<&Mask>,
        clip: (f32, f32, f32, f32),
    ) {
        // Glyph positions already carry device scale. The GPU stream is separate
        // and remains full; only this CPU loop selects conservative ink spans.
        let glyph_ts = transform.pre_scale(1.0 / scale, 1.0 / scale);
        let mut cache = paragraph.ink.borrow_mut();
        if !cache.matches(&self.ink_catalog, scale) {
            cache.reset(&self.ink_catalog, scale);
            #[cfg(test)]
            {
                self.ink_builds += 1;
            }
            cache.index = ink::Index::build(self, paragraph, scale);
        }
        let paint = PixmapPaint::default();
        let mut draw = |g: &LayoutGlyph, baseline: f32, ink: RunPaint| {
            #[cfg(test)]
            {
                self.ink_visits += 1;
            }
            if ink.color[3] == 0 {
                return;
            }
            let phys = g.physical((origin.0 * scale, (origin.1 + baseline) * scale), scale);
            let Some(glyph) = self.glyph(phys.cache_key, ink.color) else {
                return;
            };
            target.draw_pixmap(
                phys.x + glyph.left,
                phys.y - glyph.top,
                glyph.pixmap.as_ref(),
                &paint,
                glyph_ts,
                mask,
            );
        };
        if let Some((index, query)) = cache.index.as_ref().and_then(|index| {
            index
                .viewport(origin, scale, glyph_ts, clip)
                .map(|query| (index, query))
        }) {
            let _nodes = index.visit(query, |line| {
                let (glyphs, baseline) = index.glyphs(paragraph, line);
                for g in glyphs {
                    draw(g, baseline, palette[g.metadata]);
                }
            });
            #[cfg(test)]
            {
                self.ink_nodes += _nodes;
            }
        } else {
            for (g, baseline, ink) in paragraph.paint_glyphs(palette) {
                draw(g, baseline, ink);
            }
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

    /// A paragraph as glyph runs grouped by font, size and source run, positions in
    /// points from the paragraph's top-left, the same numbers the raster
    /// path snaps to pixels.
    pub fn glyph_runs(&mut self, paragraph: &Paragraph, palette: &[RunPaint]) -> Vec<GlyphRun> {
        type Key = (fontdb::ID, u16, u32, usize, bool);
        type Runs = Vec<(Key, Vec<(u32, f32, f32)>)>;
        let mut runs: Runs = Vec::new();
        for (g, baseline, _) in paragraph.paint_glyphs(palette) {
            let key = (
                g.font_id,
                g.font_weight.0,
                g.font_size.to_bits(),
                g.metadata,
                g.cache_key_flags.contains(CacheKeyFlags::FAKE_ITALIC),
            );
            let x = g.x + g.x_offset * g.font_size;
            let y = baseline + g.y - g.y_offset * g.font_size;
            match runs.last_mut() {
                Some((k, glyphs)) if *k == key => glyphs.push((g.glyph_id as u32, x, y)),
                _ => runs.push((key, vec![(g.glyph_id as u32, x, y)])),
            }
        }
        runs.into_iter()
            .filter_map(
                |((id, weight, size, run_index, synthetic_italic), glyphs)| {
                    let font = self.font_data(id, Weight(weight))?;
                    Some(GlyphRun {
                        font,
                        size: f32::from_bits(size),
                        run_index,
                        paint: palette[run_index],
                        synthetic_italic,
                        glyphs,
                    })
                },
            )
            .collect()
    }
}

fn paragraph_metrics(p: &Paragraph) -> TextMetrics {
    TextMetrics {
        width: p.width,
        height: p.height,
        first_baseline: Some(p.first_baseline),
    }
}

fn premultiply(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let p = |c: u8| (c as u32 * a as u32 / 255) as u8;
    [p(r), p(g), p(b), a]
}

/// The kernel's measurer over the shared engine.
pub struct Measurer(pub Shared);

impl TextMeasurer for Measurer {
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        self.0.borrow_mut().measure_identified(stamp, request)
    }
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let spec = Spec::from_request(request);
        self.0.borrow_mut().measure(&spec, request.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_normal_monospace_is_used_for_normal_code() {
        let mut engine = TextEngine::new();
        if !engine
            .fonts
            .db()
            .faces()
            .any(|f| f.monospaced && f.style == fontdb::Style::Normal && f.weight == Weight::NORMAL)
        {
            eprintln!("normal monospace face unavailable; installed-font regression not checked");
            return;
        }
        let style = exact_kernel::StyleProps {
            font_family: 5,
            font_size: 16.0,
            ..exact_kernel::StyleProps::default()
        };
        let spec = crate::paint::text_spec(&style, "let section = 0;");
        let paragraph = engine.paragraph(&spec, Some(300.0));
        let glyph = &paragraph.buffer.layout_runs().next().unwrap().glyphs[0];
        let face = engine.fonts.db().face(glyph.font_id).unwrap();
        assert!(face.monospaced, "code selected {:?}", face.families);
        assert_eq!(
            face.style,
            fontdb::Style::Normal,
            "code selected {:?}",
            face.families
        );
    }
}

#[cfg(test)]
#[path = "text/residency_tests.rs"]
mod residency_tests;

#[cfg(test)]
#[path = "text/ink_tests.rs"]
mod ink_tests;

#[cfg(test)]
#[path = "text/identified_tests.rs"]
mod identified_tests;
