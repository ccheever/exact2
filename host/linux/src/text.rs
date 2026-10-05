//! Text: cosmic-text, one engine for measuring and painting — the lesson of
//! LLP 1008 §3 (measure with one engine, paint with another, and every
//! difference is a bug), in Rust this time.
//!
//! @ref LLP 1015 §3; LLP 1001 §6 (a per-kernel injected measurer)
//!
//! A [`Paragraph`] is a width-specific snapshot sharing an immutable full
//! cosmic-text shape and its font catalog. Layout vectors and CSS baselines
//! are immutable and may be shared by distinct exact-request wrappers. It is
//! cached by (spec, width); the measurer answers
//! from it and the painter paints from it, so what was measured is what is
//! painted, by construction. `line-height: normal` is the font's ascent +
//! descent + line gap (the browser's); a set line height centers the glyphs
//! in the box, which cosmic-text does itself. Glyphs are rasterized by swash
//! once per (glyph, color) into small premultiplied pixmaps.

pub(crate) mod cache;
mod catalog;
mod catalog_recipe;
mod flow;
#[cfg(test)]
mod flow_tests;
mod font_cache;
mod shaping;
#[allow(dead_code)] // Private transfer proof; controller integration is a separate increment.
pub(crate) mod transfer;
#[cfg(test)]
mod transfer_tests;
use shaping::ShapedSource;
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
    /// CSS `font-variant-numeric` bits (LLP 1053 G4): 1 is `tabular-nums`.
    pub font_variant_numeric: u8,
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
            font_variant_numeric: self.font_variant_numeric,
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
            font_variant_numeric: style.font_variant_numeric,
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
    /// CSS whitespace processing mode.
    pub white_space: exact_kernel::WhiteSpace,
    /// CSS paragraph base direction.
    pub direction: exact_kernel::Direction,
    /// CSS `text-indent`, points: the first line's inset from its start edge.
    pub text_indent: f32,
}

impl Spec {
    /// CSS white space collapsing, before shaping (LLP 1053 G5): cosmic-text
    /// shapes what it is given, the browser collapses first. Collapsed runs are
    /// the spec's identity, so equal renderings share one shape.
    pub fn collapse_white_space(mut self) -> Self {
        let texts: Vec<&str> = self.runs.iter().map(|r| r.text.as_str()).collect();
        if let Some(collapsed) = exact_textflow::collapse(&texts, self.white_space.model()) {
            for (run, text) in self.runs.iter_mut().zip(collapsed.runs) {
                run.text = text;
            }
        }
        self
    }

    /// The spec a kernel measure request describes.
    pub fn from_request(request: &TextMeasureRequest<'_>) -> Spec {
        // A soft hyphen breaks and shows here as on every host; `auto`'s own
        // hyphenation points need a language's patterns, which this pure-Rust
        // host does not carry (LLP 1001).
        static AUTO: std::sync::Once = std::sync::Once::new();
        if request.paragraph.hyphens == exact_kernel::Hyphens::Auto {
            AUTO.call_once(|| eprintln!("[Text] hyphens-auto: Linux has no hyphenation dictionary; `hyphens: auto` breaks only at soft hyphens, as `manual` (LLP 1001)"));
        }
        Spec {
            strut: Run::from_style("", request.paragraph.strut),
            runs: request
                .runs
                .iter()
                .map(|r| Run::from_style(&r.text, r.style))
                .collect(),
            align: request.paragraph.text_align,
            line_clamp: request.paragraph.line_clamp,
            overflow_wrap: request.paragraph.overflow_wrap,
            white_space: request.paragraph.white_space,
            direction: request.paragraph.direction,
            text_indent: request.paragraph.text_indent,
        }
        .collapse_white_space()
    }

    /// Whether there is nothing to shape.
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }
}

/// A shaped, wrapped paragraph at one width: what is measured is what is
/// painted.
pub struct Paragraph {
    /// Width-independent canonical text, shape and catalog.
    source: Rc<ShapedSource>,
    layouts: Arc<Vec<Vec<cosmic_text::LayoutLine>>>,
    flow: Option<flow::FlowLayout>,
    #[cfg(test)]
    layout_lifetime: Arc<()>,
    /// Points, rounded up.
    pub width: f32,
    /// Points, rounded up.
    pub height: f32,
    /// Top to the first alphabetic baseline, points.
    pub first_baseline: f32,
    /// CSS shared-baseline placement for each wrapped line, used by both painters.
    baselines: Arc<Vec<f32>>,
    ink: RefCell<ink::Cache>,
    ellipsized: RefCell<Option<(f32, Rc<Paragraph>)>>,
    // S + L, excluding canonical key K. Shared S must be deduplicated across
    // snapshots; lazy ink is read separately below.
    resident_capacity_bytes: usize,
    // Moved source String capacities are included above; no length estimate.
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
    /// Full immutable width layout and canonical source in cosmic line order.
    pub fn layout_runs(&self) -> impl Iterator<Item = cosmic_text::LayoutRun<'_>> {
        shaping::Runs::new(self)
    }

    fn layout_capacity_bytes(&self) -> usize {
        self.owned_capacity_bytes() - self.source.accessible_capacity_bytes
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

    /// Whether CPU paint can use the existing index for these exact inputs.
    /// Pass paint's device transform after pre_scale(1/scale, 1/scale), and
    /// a device-pixel clip. No lazy build, glyph access, allocation or mutation.
    #[allow(dead_code)] // Consumed by the separately integrated region replay.
    pub(crate) fn prepared_ink_supports(
        &self,
        origin: (f32, f32),
        scale: f32,
        glyph_transform: Transform,
        device_clip: (f32, f32, f32, f32),
    ) -> bool {
        let Ok(catalog) = self.source.catalog.try_borrow() else {
            return false;
        };
        let Ok(cache) = self.ink.try_borrow() else {
            return false;
        };
        cache.matches(&catalog.ink_catalog, scale)
            && cache.index.as_ref().is_some_and(|index| {
                index
                    .viewport(origin, scale, glyph_transform, device_clip)
                    .is_some()
            })
    }

    /// Current accessible capacity in O(1), without visiting shaped glyphs.
    /// Accounting/maintenance runs outside paint's exclusive ink borrow.
    fn owned_capacity_bytes(&self) -> usize {
        self.resident_capacity_bytes + self.ink_capacity_bytes()
    }

    /// CSS inline backgrounds (LLP 1053 §0): each run's `background-color`
    /// covers each of its line fragments, its glyphs' advance across and its
    /// own font's content area down; touching spans of one colour join.
    /// Rectangles are in points from the paragraph's top-left, in paint order.
    pub fn run_backgrounds(
        &self,
        backgrounds: &[Option<[u8; 4]>],
    ) -> Vec<(crate::paint::Rect4, [u8; 4])> {
        let mut out = Vec::new();
        if backgrounds.iter().all(Option::is_none) {
            return out;
        }
        let metrics = &self.source.data.run_metrics;
        for (line, baseline) in self.layout_runs().zip(self.baselines.iter()) {
            let mut spans: Vec<(f32, f32, usize)> = Vec::new();
            for g in line.glyphs {
                if backgrounds.get(g.metadata).copied().flatten().is_none() {
                    continue;
                }
                let (lo, hi) = (g.x.min(g.x + g.w), g.x.max(g.x + g.w));
                match spans.last_mut() {
                    Some(last)
                        if last.2 == g.metadata && lo <= last.1 + 0.01 && hi >= last.0 - 0.01 =>
                    {
                        last.0 = last.0.min(lo);
                        last.1 = last.1.max(hi);
                    }
                    _ => spans.push((lo, hi, g.metadata)),
                }
            }
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f32, f32, usize)> = Vec::new();
            for span in spans {
                let color = backgrounds[span.2];
                match merged.last_mut() {
                    Some(last) if backgrounds[last.2] == color && span.0 <= last.1 + 0.01 => {
                        last.1 = last.1.max(span.1)
                    }
                    _ => merged.push(span),
                }
            }
            for (lo, hi, run) in merged {
                let (ascent, descent, _) = metrics[run];
                out.push((
                    (lo, baseline - ascent, hi - lo, ascent + descent),
                    backgrounds[run].expect("filtered"),
                ));
            }
        }
        out
    }

    /// The same source laid out for CSS `text-overflow: ellipsis` at `width`:
    /// each line wider than the box ends in an ellipsis (paint only; the
    /// measured paragraph is unchanged). Kept for the last width asked.
    pub fn ellipsized(&self, width: f32) -> Option<Rc<Paragraph>> {
        if self.width <= width + 0.01 || self.source.spec.line_clamp > 0 {
            return None;
        }
        let mut cached = self.ellipsized.borrow_mut();
        if let Some((w, p)) = cached.as_ref() {
            if w.to_bits() == width.to_bits() {
                return Some(p.clone());
            }
        }
        let p = Rc::new(self.source.layout_ellipsized(width));
        *cached = Some((width, p.clone()));
        Some(p)
    }

    /// The shared CPU/GPU stream: every glyph keeps its canonical run index
    /// and CSS baseline, and selects its paint data by that index.
    pub fn paint_glyphs<'a>(
        &'a self,
        palette: &'a [RunPaint],
    ) -> impl Iterator<Item = (&'a LayoutGlyph, f32, RunPaint)> + 'a {
        self.layout_runs()
            .zip(self.baselines.iter())
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
pub(crate) enum FamilyChoice {
    SansSerif,
    Serif,
    Monospace,
    Declared(String),
}

/// Canvas 2D's own font system (LLP 1056 D8): the catalog's fonts and
/// family choices for `plan`, detached from its `Rc` caches so the canvas
/// text engine can live behind a `Mutex`, with each family name a canvas
/// `font` may give (the generics and the plan's declared families).
pub(crate) fn canvas_font_system(
    plan: &Plan,
    assets: &Assets,
) -> (FontSystem, Vec<(String, FamilyChoice)>) {
    let c = catalog::Catalog::for_assets(plan, assets);
    let mut names: Vec<(String, FamilyChoice)> = [
        ("sans-serif", FamilyChoice::SansSerif),
        ("system-ui", FamilyChoice::SansSerif),
        ("ui-sans-serif", FamilyChoice::SansSerif),
        ("ui-rounded", FamilyChoice::SansSerif),
        ("serif", FamilyChoice::Serif),
        ("ui-serif", FamilyChoice::Serif),
        ("monospace", FamilyChoice::Monospace),
        ("ui-monospace", FamilyChoice::Monospace),
    ]
    .into_iter()
    .map(|(n, f)| (n.to_string(), f))
    .collect();
    for (i, stack) in plan.stacks.iter().enumerate() {
        let member = plan.stack_member(stack.members.iter().next().expect("validated stack"));
        if let (StackMemberKind::Family, Some(family)) = (member.kind, member.family) {
            if let Some(choice) = c.families.get(i) {
                let name = plan.str(plan.familie(family).name).to_string();
                names.push((name, choice.clone()));
            }
        }
    }
    (c.fonts, names)
}

impl FamilyChoice {
    pub(crate) fn cosmic(&self) -> Family<'_> {
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
    /// Normalized variation coordinates (a variable face's `wght`).
    pub coords: std::sync::Arc<[i16]>,
    /// The face's file and collection index, when it was loaded from one.
    pub file: Option<(std::sync::Arc<str>, u32)>,
    /// The weight shaped with (the `wght` a variable face is set to).
    pub weight: u16,
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
    catalog: catalog::Lease,
    paragraphs: cache::Cache,
    /// Canonical shaped-source builds, independent of width layout.
    pub shape_calls: usize,
    layout_calls: usize,
    #[cfg(test)]
    before_layout: Option<Box<dyn FnMut()>>,
    #[cfg(test)]
    ink_visits: usize,
    #[cfg(test)]
    ink_nodes: usize,
    #[cfg(test)]
    ink_builds: usize,
    /// How many times the kernel asked, since launch.
    pub measures: usize,
    /// Questions answered without a layout miss.
    pub hits: usize,
    /// Total synchronous shape/layout miss work, not isolated shaper CPU time.
    pub shaping: Duration,
    /// Cumulative flowed layout work, including fragment glyph placement.
    pub flowing: Duration,
    /// Subset of flowing spent in the shared band/walker itself.
    pub flow_walk: Duration,
    /// The family sans-serif resolves to.
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
        Self::with_catalog(catalog::Catalog::new())
    }

    fn with_catalog(catalog: catalog::Catalog) -> Self {
        Self {
            sans: catalog.sans.clone(),
            catalog: Rc::new(RefCell::new(catalog)),
            paragraphs: cache::Cache::default(),
            shape_calls: 0,
            layout_calls: 0,
            measures: 0,
            hits: 0,
            shaping: Duration::ZERO,
            flowing: Duration::ZERO,
            flow_walk: Duration::ZERO,
            #[cfg(test)]
            before_layout: None,
            #[cfg(test)]
            ink_visits: 0,
            #[cfg(test)]
            ink_nodes: 0,
            #[cfg(test)]
            ink_builds: 0,
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
        Rc::new(RefCell::new(Self::with_catalog(
            catalog::Catalog::for_assets(plan, assets),
        )))
    }

    /// Replace the complete catalog and every family-bearing cache. This is
    /// the plan-identity boundary on a dev reload (LLP 1019 D4).
    pub fn install_plan(&mut self, plan: &Plan, assets: &Path) {
        self.install_plan_assets(plan, &Assets::embedded(assets.to_path_buf()));
    }

    fn install_plan_assets(&mut self, plan: &Plan, assets: &Assets) {
        *self = Self::with_catalog(catalog::Catalog::for_assets(plan, assets));
    }

    /// The exact loaded face id chosen for a run, for identity assertions.
    pub fn resolved_face_id(
        &mut self,
        family: u16,
        weight: u16,
        italic: bool,
    ) -> Option<fontdb::ID> {
        self.catalog
            .borrow_mut()
            .resolved_face_id(family, weight, italic)
    }

    /// The id loaded for one declared face before matching.
    pub fn declared_face_id(&self, family: u16, weight: u16, italic: bool) -> Option<fontdb::ID> {
        self.catalog
            .borrow()
            .declared_face_id(family, weight, italic)
    }

    /// How many font faces are loaded.
    pub fn face_count(&self) -> usize {
        self.catalog.borrow().face_count()
    }

    /// CSS `line-height: normal` for a run: the font's ascent + descent +
    /// line gap at the run's size, from the font the shaper picks.
    pub fn normal_line_height(&mut self, run: &Run) -> f32 {
        self.catalog.borrow_mut().normal_line_height(run)
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
    ) -> ((u64, u64), Arc<Spec>) {
        if let Some(identity) = self.paragraphs.identified(stamp) {
            return identity;
        }
        let spec = build();
        let key = self.paragraphs.identity_owned(spec);
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
        if let AxisOffer::Definite(width) = request.width {
            if !request.exclusions.is_empty() {
                self.measures += 1;
                return paragraph_metrics(&self.flow_for(key, width, request.exclusions, None));
            }
        }
        self.measure_for(&spec, request.width, key)
    }

    fn paragraph_for(
        &mut self,
        _spec: &Spec,
        width: Option<f32>,
        key: (u64, u64),
    ) -> Rc<Paragraph> {
        if let Some(p) = self.paragraphs.get(key, width.into()) {
            return p;
        }
        self.paragraphs.before_shape(key);
        let source = self.source(key);
        let p = Rc::new(self.layout_source(&source, width, None));
        self.paragraphs.insert(key, width.into(), &p);
        p
    }

    fn source(&mut self, key: (u64, u64)) -> Rc<ShapedSource> {
        if let Some(source) = self.paragraphs.source(key) {
            return source;
        }
        let spec = self.paragraphs.spec(key).expect("current identity");
        let source = self.build_source(spec);
        self.paragraphs.set_source(key, source.clone());
        source
    }

    fn build_source(&mut self, spec: Arc<Spec>) -> Rc<ShapedSource> {
        self.shape_calls += 1;
        #[cfg(test)]
        identified_tests::shaped(&spec);
        Rc::new(ShapedSource::new(self.catalog.clone(), spec))
    }

    fn layout_source(
        &mut self,
        source: &Rc<ShapedSource>,
        width: Option<f32>,
        wrap: Option<Wrap>,
    ) -> Paragraph {
        #[cfg(test)]
        if let Some(callback) = &mut self.before_layout {
            callback();
        }
        self.layout_calls += 1;
        source.layout(width, wrap)
    }

    #[cfg(test)]
    fn layout(&mut self, spec: &Spec, width: Option<f32>) -> Paragraph {
        let source = self.build_source(Arc::new(spec.clone()));
        self.layout_source(&source, width, None)
    }

    // Intrinsic questions retain the shared source and scalar answers only.
    // Zero-width layout scratch drops before the final min-content layout.
    fn intrinsic(&mut self, spec: &Spec, minimum: bool, key: (u64, u64)) -> TextMetrics {
        if let Some(metrics) = self.paragraphs.intrinsic(key, minimum) {
            return metrics;
        }
        self.paragraphs.release_handoff(key.1);
        self.paragraphs.before_shape(key);
        let source = self.source(key);
        let width = if minimum {
            let wrap =
                (spec.overflow_wrap == exact_kernel::OverflowWrap::BreakWord).then_some(Wrap::Word);
            Some(self.layout_source(&source, Some(0.0), wrap).width)
        } else {
            None
        };
        let metrics = paragraph_metrics(&self.layout_source(&source, width, None));
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
        let before = self.layout_calls;
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
        if self.layout_calls == before {
            self.hits += 1;
        } else {
            self.shaping += started.elapsed();
        }
        metrics
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
        let mut catalog = paragraph.source.catalog.borrow_mut();
        let mut cache = paragraph.ink.borrow_mut();
        if !cache.matches(&catalog.ink_catalog, scale) {
            cache.reset(&catalog.ink_catalog, scale);
            #[cfg(test)]
            {
                self.ink_builds += 1;
            }
            cache.index = ink::Index::build(&mut catalog, paragraph, scale).map(Into::into);
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
            let Some(glyph) = catalog.glyph(phys.cache_key, ink.color) else {
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
        let mut catalog = paragraph.source.catalog.borrow_mut();
        runs.into_iter()
            .filter_map(
                |((id, weight, size, run_index, synthetic_italic), glyphs)| {
                    let face = catalog.font_data(id, Weight(weight))?;
                    Some(GlyphRun {
                        font: face.font,
                        coords: face.coords,
                        file: face.file,
                        weight,
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
    fn set_language(&mut self, language: &str) {
        let mut engine = self.0.borrow_mut();
        let catalog = engine.catalog.borrow();
        if catalog.fonts.locale() == language {
            return;
        }
        // Retained paragraphs keep their old catalog for an accepted frame.
        let mut next = catalog::Catalog::with_fonts(FontSystem::new_with_locale_and_db(
            language.into(),
            catalog.fonts.db().clone(),
        ));
        next.families = catalog.families.clone();
        next.declared_faces = catalog.declared_faces.clone();
        next.sans = catalog.sans.clone();
        drop(catalog);
        engine.catalog = Rc::new(RefCell::new(next));
        engine.paragraphs = cache::Cache::default();
    }

    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        self.0.borrow_mut().measure_identified(stamp, request)
    }
    fn height_free(&self) -> bool {
        true
    }
    fn measure_known(
        &mut self,
        stamp: &ParagraphStamp,
        width: AxisOffer,
        _height: AxisOffer,
    ) -> Option<TextMetrics> {
        // Offers here are width-only: height never changes a paragraph.
        let mut engine = self.0.borrow_mut();
        let (key, spec) = engine.paragraphs.identified(stamp)?;
        Some(engine.measure_for(&spec, width, key))
    }
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let spec = Spec::from_request(request);
        let mut engine = self.0.borrow_mut();
        if let AxisOffer::Definite(width) = request.width {
            if !request.exclusions.is_empty() {
                engine.measures += 1;
                return paragraph_metrics(&engine.paragraph_flow(
                    &spec,
                    width,
                    request.exclusions,
                    None,
                ));
            }
        }
        engine.measure(&spec, request.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_normal_monospace_is_used_for_normal_code() {
        let mut engine = TextEngine::new();
        if !engine
            .catalog
            .borrow()
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
        let glyph = &paragraph.layout_runs().next().unwrap().glyphs[0];
        let catalog = paragraph.source.catalog.borrow();
        let face = catalog.fonts.db().face(glyph.font_id).unwrap();
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

#[cfg(test)]
#[path = "text/sharing_tests.rs"]
mod sharing_tests;

#[cfg(test)]
#[path = "text/span_capacity_tests.rs"]
mod span_capacity_tests;

#[cfg(test)]
#[path = "text/css_tests.rs"]
mod css_tests;
