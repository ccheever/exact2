//! One shape per paragraph; each width breaks it and keeps its own lines.
//!
//! A paragraph is shaped once by Parley, one layout per hard line (text
//! between line feeds: a Markdown list item's lines take that line's own
//! inset, as before). Each width breaks the same layouts again, in place —
//! Parley's re-breaking is cheap and a layout's breaking state is its own —
//! and copies out what painting needs ([`super::lines`]). The line box is
//! the host's: the strut plus each face's ascent, descent and gap.
use super::lines::{self, LayoutGlyph, LayoutLine, Lines};
use super::*;
use parley::{
    Alignment, AlignmentOptions, BaseDirection, IndentOptions, LineBreakContext, RangedBuilder,
    StyleProperty,
};
use std::ops::Range;
use std::sync::Mutex;

#[cfg(test)]
thread_local! { static SHAPE_LINES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
/// Hard lines shaped on this thread (test probe).
#[cfg(test)]
pub(super) fn shape_line_calls() -> usize {
    SHAPE_LINES.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! { static BREAKS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) }; }
/// Widths whose lines were copied out on this thread, and hard lines
/// Parley broke for them (test probe).
#[cfg(test)]
pub(super) fn break_calls() -> (usize, usize) {
    BREAKS.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! { static WIDTH_FONT_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
/// Faces whose metrics were read from font data on this thread (test probe).
#[cfg(test)]
pub(super) fn width_font_lookups() -> usize {
    WIDTH_FONT_LOOKUPS.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) fn count_font_lookup() {
    WIDTH_FONT_LOOKUPS.with(|n| n.set(n.get() + 1));
}

/// A face's vertical metrics in font units.
#[derive(Clone, Copy, Debug)]
pub(super) struct RawFontMetrics {
    pub(super) units_per_em: u16,
    pub(super) ascent: f32,
    pub(super) descent: f32,
    pub(super) leading: f32,
}

/// Chrome's line-break choices for ordinary paragraphs (LLP 1085.000 G6):
/// the table and rules the text-flow walker uses, so the two paths break
/// alike (`exact_textflow::chrome_break`).
fn chrome(cx: LineBreakContext) -> Option<bool> {
    exact_textflow::chrome_break(cx.before_before, cx.before, cx.after)
}
static CHROME: &parley::LineBreakOverrideFn = &(chrome as fn(LineBreakContext) -> Option<bool>);

/// One run's style over `range` of the text being built.
pub(super) fn push_style(
    builder: &mut RangedBuilder<'_, u32>,
    run: &Run,
    range: Range<usize>,
    index: u32,
    family: &FamilyChoice,
) {
    builder.push(StyleProperty::Brush(index), range.clone());
    builder.push(StyleProperty::FontFamily(family.family()), range.clone());
    builder.push(StyleProperty::FontSize(run.size.max(0.5)), range.clone());
    builder.push(
        StyleProperty::FontWeight(parley::FontWeight::new(f32::from(run.weight))),
        range.clone(),
    );
    if run.italic {
        builder.push(
            StyleProperty::FontStyle(parley::FontStyle::Italic),
            range.clone(),
        );
    }
    if run.letter_spacing != 0.0 && run.size > 0.0 {
        builder.push(
            StyleProperty::LetterSpacing(run.letter_spacing),
            range.clone(),
        );
    }
    // CSS `tabular-nums` is the chosen face's own `tnum` feature, never a
    // substitute face; a face without it keeps its figures (LLP 1053 G4).
    if run.font_variant_numeric & 1 != 0 {
        builder.push(
            StyleProperty::FontFeatures(parley::FontFeatures::from("\"tnum\" 1")),
            range,
        );
    }
}

/// A hard line: its text and its one shaped layout.
pub(super) struct HardLine {
    pub(super) text: String,
    layout: Mutex<Broken>,
    /// The resolved base direction (CSS `direction`, or the first strong
    /// character under `ltr`).
    pub(super) rtl: bool,
    /// `(first, rest)` inset, points: a Markdown list item's head indent,
    /// its first line's less the marker hung before it (LLP 1045 D4).
    inset: (f32, f32),
    /// The first visual line's indent as Parley applies it.
    indent: f32,
}

/// A hard line's layout and the width and alignment it was last broken
/// and aligned at (`None` until a width breaks it): breaking it again at
/// the same ones leaves the same lines, so a width painted after it was
/// measured is not broken twice.
struct Broken {
    layout: parley::Layout<u32>,
    at: Option<(Option<u32>, Alignment)>,
}

pub(super) struct ShapedSource {
    pub(super) spec: Arc<Spec>,
    pub(super) catalog: catalog::Lease,
    pub(super) data: Arc<ShapeData>,
    pub(super) accessible_capacity_bytes: usize,
    pub(super) flow: RefCell<Option<Rc<super::flow::FlowSource>>>,
}

/// Only shaped data and scalars cross the thread boundary, never the UI
/// lease. A width locks each layout while it breaks it.
pub(super) struct ShapeData {
    pub(super) lines: Vec<HardLine>,
    strut: (f32, f32),
    pub(super) run_metrics: Vec<FontMetrics>,
}

/// A hard line's runs: (range in the line, run index).
type Pieces = Vec<(Range<usize>, usize)>;

/// The paragraph's text split into hard lines as cosmic-text split it: at
/// each paragraph separator (`\r\n` one), the last empty piece dropped,
/// and one empty line for no text. Each line's pieces: (range in the
/// line, run index).
fn hard_lines(spec: &Spec) -> Vec<(String, Pieces)> {
    let mut out: Vec<(String, Pieces)> = vec![(String::new(), Vec::new())];
    let mut after_cr = false;
    for (index, run) in spec.runs.iter().enumerate() {
        let mut rest = run.text.as_str();
        while !rest.is_empty() {
            let split = rest.find([
                '\n', '\r', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2029}',
            ]);
            let (piece, sep) = match split {
                Some(at) => {
                    let ch = rest[at..].chars().next().expect("found");
                    (&rest[..at], Some((at, ch)))
                }
                None => (rest, None),
            };
            if !piece.is_empty() {
                after_cr = false;
                let (text, pieces) = out.last_mut().expect("one line");
                let start = text.len();
                text.push_str(piece);
                pieces.push((start..text.len(), index));
            }
            match sep {
                Some((at, ch)) => {
                    // `\r\n` is one break.
                    if !(ch == '\n' && after_cr && piece.is_empty()) {
                        out.push((String::new(), Vec::new()));
                    }
                    after_cr = ch == '\r';
                    rest = &rest[at + ch.len_utf8()..];
                }
                None => rest = "",
            }
        }
    }
    if out.len() > 1 && out.last().is_some_and(|l| l.0.is_empty()) {
        out.pop();
    }
    out
}

impl ShapedSource {
    pub(super) fn flow_box<'a>(
        &self,
        glyphs: impl Iterator<Item = &'a LayoutGlyph>,
        faces: &[lines::Face],
    ) -> (f32, f32) {
        let (above, below, _) = line_box(
            glyphs,
            faces,
            &self.spec,
            &mut self.catalog.borrow_mut(),
            self.data.strut,
            &self.data.run_metrics,
        );
        (above, above + below)
    }
    pub(super) fn flow_capacity_bytes(&self) -> usize {
        self.flow
            .borrow()
            .as_ref()
            .map_or(0, |f| f.capacity_bytes())
    }

    pub(super) fn new(lease: catalog::Lease, spec: Arc<Spec>) -> Self {
        let mut catalog = lease.borrow_mut();
        let minimum = catalog.line_height(&spec.strut);
        let (ascent, descent, leading) = catalog.font_metrics(&spec.strut);
        let half = (minimum - ascent - descent - leading) / 2.0;
        let strut = (ascent + half, descent + leading + half);
        let run_metrics: Vec<_> = spec.runs.iter().map(|r| catalog.font_metrics(r)).collect();
        let families: Vec<FamilyChoice> =
            spec.runs.iter().map(|r| catalog.choice(r.family)).collect();
        let wrap = if spec.white_space.model().wraps() {
            parley::TextWrapMode::Wrap
        } else {
            parley::TextWrapMode::NoWrap
        };
        let overflow_wrap = match spec.overflow_wrap {
            exact_kernel::OverflowWrap::Normal => parley::OverflowWrap::Normal,
            exact_kernel::OverflowWrap::BreakWord => parley::OverflowWrap::BreakWord,
            exact_kernel::OverflowWrap::Anywhere => parley::OverflowWrap::Anywhere,
        };
        // `direction: rtl` makes the base direction right-to-left, as CSS
        // does (LLP 1053; LLP 1085.000 G1). Under `ltr` the first strong
        // character still decides (declared in LLP 1001 §1): the text-flow
        // walker cannot yet break an RTL run inside an LTR paragraph.
        let base = if spec.direction == exact_kernel::Direction::Rtl {
            BaseDirection::Rtl
        } else {
            BaseDirection::Auto
        };
        let mut text_bytes = 0;
        let pieces = hard_lines(&spec);
        let mut lines = Vec::with_capacity(pieces.len());
        for (h, (text, runs)) in pieces.into_iter().enumerate() {
            #[cfg(test)]
            SHAPE_LINES.with(|n| n.set(n.get() + 1));
            text_bytes += text.len();
            let catalog = &mut *catalog;
            let mut builder = catalog
                .layout
                .ranged_builder(&mut catalog.fonts, &text, 1.0, false);
            builder.push_default(StyleProperty::TextWrapMode(wrap));
            builder.push_default(StyleProperty::OverflowWrap(overflow_wrap));
            if let Some((_, first)) = runs.first() {
                // An empty hard line still has its run's face for metrics.
                builder.push_default(StyleProperty::FontSize(spec.runs[*first].size.max(0.5)));
            }
            for (range, index) in &runs {
                push_style(
                    &mut builder,
                    &spec.runs[*index],
                    range.clone(),
                    *index as u32,
                    &families[*index],
                );
            }
            builder.set_base_direction(base);
            // The line box keeps the CSS direction even where the first
            // strong character set the bidi base: trailing whitespace hangs
            // at its end edge, and an overflowing line starts at its start.
            builder.set_line_direction(if base == BaseDirection::Rtl {
                BaseDirection::Rtl
            } else {
                BaseDirection::Ltr
            });
            builder.set_line_break_override(Some(CHROME));
            let mut layout = builder.build(&text);
            let inset = line_inset(&spec, &runs, &mut layout);
            // CSS `text-indent`: the paragraph's first line only.
            let indent = if h == 0 { spec.text_indent } else { 0.0 } + inset.0 - inset.1;
            if indent != 0.0 {
                layout.set_text_indent(
                    indent,
                    IndentOptions {
                        each_line: false,
                        hanging: false,
                    },
                );
            }
            lines.push(HardLine {
                text,
                rtl: layout.is_rtl(),
                layout: Mutex::new(Broken { layout, at: None }),
                inset,
                indent,
            });
        }
        catalog.release_scratch(text_bytes);
        drop(catalog);
        let mut source = Self {
            spec,
            catalog: lease,
            data: Arc::new(ShapeData {
                lines,
                strut,
                run_metrics,
            }),
            accessible_capacity_bytes: 0,
            flow: RefCell::new(None),
        };
        source.accessible_capacity_bytes = source.capacities();
        source
    }
    pub(super) fn attach(
        catalog: catalog::Lease,
        spec: Arc<Spec>,
        data: Arc<ShapeData>,
        bytes: usize,
    ) -> Self {
        Self {
            spec,
            catalog,
            data,
            accessible_capacity_bytes: bytes,
            flow: RefCell::new(None),
        }
    }
    /// The shape's own storage: the hard lines' text and Parley's arrays.
    fn capacities(&self) -> usize {
        fn vec<T>(v: &Vec<T>) -> usize {
            v.capacity() * std::mem::size_of::<T>()
        }
        let mut bytes = vec(&self.data.lines) + vec(&self.data.run_metrics);
        for line in &self.data.lines {
            bytes +=
                line.text.capacity() + line.layout.lock().map_or(0, |l| l.layout.capacity_bytes());
        }
        bytes
    }
    pub(super) fn layout(self: &Rc<Self>, width: Option<f32>) -> Paragraph {
        self.layout_as(width, false, false)
    }
    /// [`Self::layout`], holding the lines it broke for this frame's paint
    /// ([`Paragraph::measured`]) where it would keep only scalars.
    pub(super) fn layout_held(self: &Rc<Self>, width: Option<f32>) -> Paragraph {
        self.layout_as(width, false, true)
    }
    /// CSS `text-overflow: ellipsis`: each over-wide line ends in "…" (paint).
    pub(super) fn layout_ellipsized(self: &Rc<Self>, width: f32) -> Paragraph {
        self.layout_as(Some(width), true, false)
    }
    /// The paragraph's widest content, min- or max-content (LLP 1085.000
    /// §4, "Content widths"): min-content is Parley's patched content
    /// widths (soft hyphens, text-indent and RTL hanging space included),
    /// which agree with Chrome where the width-zero layout did not (a URL's
    /// `/`); max-content stays the unbroken layout's widest line, which
    /// keeps a preserved trailing space as Chrome's `pre-wrap` does.
    pub(super) fn min_content(&self) -> f32 {
        if !self.spec.white_space.model().wraps() {
            return self.layout_widths(None);
        }
        self.data
            .lines
            .iter()
            .map(|line| {
                let broken = line.layout.lock().expect("layout lock");
                broken.layout.calculate_content_widths().min + line.inset.1
            })
            .fold(0.0, f32::max)
    }
    fn layout_widths(&self, width: Option<f32>) -> f32 {
        let (lines, _) = self.break_lines(width, false, None);
        lines.lines.iter().map(|l| l.w).fold(0.0, f32::max)
    }
    /// A measured width's lines, made again for its first paint when the
    /// measure's were not held: the same breaking as when it was measured
    /// (an unclamped paragraph's lines need no catalog), so what was
    /// measured is what is painted. A layout still broken at this width is
    /// not broken again.
    pub(super) fn remake(&self, width: Option<f32>) -> Lines {
        self.break_lines(width, false, None).0
    }
    /// Break every hard line at `width` and copy its lines out, clamped and
    /// ellipsized as the paragraph asks. Returns the lines and whether a
    /// clamp cut text.
    fn break_lines(
        &self,
        width: Option<f32>,
        ellipsis: bool,
        mut catalog: Option<&mut catalog::Catalog>,
    ) -> (Lines, bool) {
        let spec = &self.spec;
        let preserves = spec.white_space.model().preserves();
        let rtl = spec.direction == exact_kernel::Direction::Rtl;
        let align = self.alignment();
        let mut out = Lines::default();
        let mut left = (spec.line_clamp > 0).then_some(spec.line_clamp as usize);
        let mut cut = false;
        let count = self.data.lines.len();
        #[cfg(test)]
        BREAKS.with(|n| n.set((n.get().0 + 1, n.get().1)));
        'hard: for (h, hard) in self.data.lines.iter().enumerate() {
            let (_, rest) = hard.inset;
            let mut broken = hard.layout.lock().expect("layout lock");
            let at = Some((width.map(f32::to_bits), align));
            if broken.at != at {
                #[cfg(test)]
                BREAKS.with(|n| n.set((n.get().0, n.get().1 + 1)));
                broken.at = None;
                broken
                    .layout
                    .break_all_lines(width.map(|w| (w - rest).max(0.0)));
                broken.layout.align(align, AlignmentOptions::default());
                broken.at = at;
            }
            let layout = &broken.layout;
            // From the start edge: `rtl` lays out from the right.
            let shift = if rtl { 0.0 } else { rest };
            let n = layout.len();
            let styles = layout.styles();
            for (i, line) in layout.lines().enumerate() {
                let start = out.glyphs.len() as u32;
                let content = lines::extract(&line, styles, shift, &mut out);
                let m = line.metrics();
                // A collapsible space that ends a line hangs; a preserved
                // one counts where the hard line ends (`pre`, `pre-wrap`).
                let ends_hard = i + 1 == n;
                let trailing = if preserves && ends_hard {
                    0.0
                } else {
                    m.trailing_whitespace
                };
                let indent = if i == 0 { hard.indent } else { 0.0 };
                out.lines.push(LayoutLine {
                    hard: h as u32,
                    glyphs: (start, out.glyphs.len() as u32),
                    w: (content - trailing + indent).max(0.0) + rest,
                    ..LayoutLine::default()
                });
                if ellipsis {
                    if let (Some(w), Some(catalog)) = (
                        // Overflow as Blink decides fit: past one 1/64 px unit.
                        width.filter(|w| out.lines.last().is_some_and(|l| l.w > w + 1.0 / 64.0)),
                        catalog.as_deref_mut(),
                    ) {
                        self.ellipsize(&mut out, w, catalog);
                    }
                }
                if let Some(k) = left.as_mut() {
                    *k -= 1;
                    if *k == 0 {
                        cut = i + 1 < n || h + 1 < count;
                        if let Some(catalog) = catalog.as_deref_mut().filter(|_| cut) {
                            self.ellipsize(&mut out, width.unwrap_or(f32::INFINITY), catalog);
                        }
                        break 'hard;
                    }
                }
            }
        }
        out.tighten();
        (out, cut)
    }

    /// The paragraph's alignment as Parley takes it.
    fn alignment(&self) -> Alignment {
        match self.spec.align.physical(self.spec.direction) {
            // Physical: Parley's `Start` would align by each line's own bidi
            // direction, not the paragraph's CSS `direction` (LLP 1053).
            TextAlign::Left | TextAlign::Start => Alignment::Left,
            TextAlign::Center => Alignment::Center,
            TextAlign::Right | TextAlign::End => Alignment::Right,
            TextAlign::Justify => Alignment::Justify,
        }
    }

    /// End the last line in "…" in the run's own face, as Blink's line
    /// truncator does: the line keeps its alignment, whole visual clusters
    /// stay from its start edge (the CSS direction's) while they and the
    /// ellipsis fit `width` measured from where the aligned line starts,
    /// spaces included, and the ellipsis follows the last one kept. A
    /// centred or end-aligned line's ellipsis can so pass the box's end
    /// edge, where Chrome clips it too.
    fn ellipsize(&self, out: &mut Lines, width: f32, catalog: &mut catalog::Catalog) {
        let Some(line) = out.lines.last().copied() else {
            return;
        };
        let rtl = self.spec.direction == exact_kernel::Direction::Rtl;
        // How far alignment moved the line from its start edge: Parley
        // aligns a line that fits by its free space.
        let free = if width.is_finite() {
            (width - line.w).max(0.0)
        } else {
            0.0
        };
        let shift = match (self.alignment(), rtl) {
            (Alignment::Center, _) => free / 2.0,
            (Alignment::Right, false) | (Alignment::Left, true) => free,
            _ => 0.0,
        };
        let (a, b) = (line.glyphs.0 as usize, line.glyphs.1 as usize);
        let glyphs: Vec<LayoutGlyph> = out.glyphs[a..b].to_vec();
        let end = if rtl { glyphs.first() } else { glyphs.last() };
        let run = end.map_or(0, |g| g.run());
        let Some(spec_run) = self.spec.runs.get(run) else {
            return;
        };
        let family = catalog.choice(spec_run.family);
        let symbol = catalog.shape_one(spec_run, "\u{2026}", &family);
        let mut ink = Lines::default();
        let mut advance = 0.0;
        if let Some(l) = symbol.lines().next() {
            advance = lines::extract(&l, symbol.styles(), 0.0, &mut ink);
        }
        // Visual clusters, kept from the line's start edge.
        let extent = |g: &LayoutGlyph| (g.x, g.x + g.w);
        let order: Vec<usize> = if rtl {
            (0..glyphs.len()).rev().collect()
        } else {
            (0..glyphs.len()).collect()
        };
        let mut kept: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < order.len() {
            let range = glyphs[order[i]].range();
            let mut j = i;
            while j < order.len() && glyphs[order[j]].range() == range {
                j += 1;
            }
            let cluster = &order[i..j];
            let fits = cluster.iter().all(|&k| {
                let (lo, hi) = extent(&glyphs[k]);
                if rtl {
                    lo + shift >= advance - 0.01
                } else {
                    hi - shift <= width - advance + 0.01
                }
            });
            if !fits {
                break;
            }
            kept.extend_from_slice(cluster);
            i = j;
        }
        let original = glyphs
            .iter()
            .map(extent)
            .fold((f32::INFINITY, f32::NEG_INFINITY), |a, e| {
                (a.0.min(e.0), a.1.max(e.1))
            });
        let edge = if rtl {
            kept.iter()
                .map(|&k| glyphs[k].x)
                .reduce(f32::min)
                .unwrap_or(if original.1.is_finite() {
                    original.1
                } else {
                    0.0
                })
        } else {
            kept.iter()
                .map(|&k| glyphs[k].x + glyphs[k].w)
                .reduce(f32::max)
                .unwrap_or(if original.0.is_finite() {
                    original.0
                } else {
                    0.0
                })
        };
        let pen = if rtl { edge - advance } else { edge };
        let boundary = end.map_or(0, |g| if rtl { g.start } else { g.end });
        let mut next: Vec<LayoutGlyph> = Vec::with_capacity(kept.len() + ink.glyphs.len());
        let mut kept_sorted = kept.clone();
        kept_sorted.sort_unstable();
        let mut symbol_glyphs: Vec<LayoutGlyph> = ink
            .glyphs
            .iter()
            .map(|g| {
                let face = ink.faces[g.face as usize].clone();
                LayoutGlyph {
                    x: g.x + pen,
                    start: boundary,
                    end: boundary,
                    metadata: run as u32,
                    face: out.face(face),
                    ..*g
                }
            })
            .collect();
        if rtl {
            next.append(&mut symbol_glyphs);
            next.extend(kept_sorted.iter().map(|&k| glyphs[k]));
        } else {
            next.extend(kept_sorted.iter().map(|&k| glyphs[k]));
            next.append(&mut symbol_glyphs);
        }
        let new_extent = next
            .iter()
            .map(extent)
            .fold((f32::INFINITY, f32::NEG_INFINITY), |a, e| {
                (a.0.min(e.0), a.1.max(e.1))
            });
        let w = if next.is_empty() || !original.0.is_finite() {
            advance
        } else {
            (line.w - (original.1 - original.0) + (new_extent.1 - new_extent.0)).max(0.0)
        };
        out.glyphs.truncate(a);
        out.glyphs.extend(next);
        let last = out.lines.last_mut().expect("a line");
        last.glyphs.1 = out.glyphs.len() as u32;
        last.w = w;
    }

    fn layout_as(self: &Rc<Self>, width: Option<f32>, ellipsis: bool, hold: bool) -> Paragraph {
        let spec = &self.spec;
        let mut catalog = self.catalog.borrow_mut();
        let (lines, _) = self.break_lines(width, ellipsis, Some(&mut catalog));
        let strut = self.data.strut;
        let run_metrics = &self.data.run_metrics;
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        let mut baselines = Vec::with_capacity(lines.lines.len());
        let mut bottoms = Vec::with_capacity(lines.lines.len());
        let mut explicit = false;
        for line in &lines.lines {
            w = w.max(line.w);
            let (above, below, is_explicit) = line_box(
                lines.glyphs_of(line).iter(),
                &lines.faces,
                spec,
                &mut catalog,
                strut,
                run_metrics,
            );
            explicit |= is_explicit;
            baselines.push(h + above);
            h += above + below;
            bottoms.push(h);
        }
        drop(catalog);
        // A clamped or ellipsized width keeps its lines (its ellipsis was
        // shaped for it); any other keeps only its scalars until painted,
        // and its lines only until this frame's paint when held.
        let keep = ellipsis || spec.line_clamp > 0;
        let (record, measured) = match (keep, hold) {
            (true, _) => (std::cell::OnceCell::from(Arc::new(lines)), None),
            (false, true) => (std::cell::OnceCell::new(), Some(Arc::new(lines))),
            (false, false) => (std::cell::OnceCell::new(), None),
        };
        let mut paragraph = Paragraph {
            source: self.clone(),
            record,
            measured: RefCell::new(measured),
            remake: (!keep).then_some(width),
            flow: None,
            #[cfg(test)]
            layout_lifetime: Arc::new(()),
            width: w.ceil(),
            height: if explicit { h } else { h.ceil() },
            first_baseline: baselines.first().copied().unwrap_or(0.),
            baselines: Arc::new(baselines),
            bottoms: Arc::new(bottoms),
            ink: RefCell::new(ink::Cache::default()),
            ellipsized: RefCell::new(None),
            resident_capacity_bytes: 0,
            private_text_bytes_estimate: 0,
        };
        paragraph.resident_capacity_bytes = cache::capacities(&paragraph);
        paragraph
    }
}

/// Each hard line's inset: a line takes the indent of the run its first
/// character is in, and when that run is a hung marker, the marker's
/// shaped advance comes off the first line's.
fn line_inset(
    spec: &Spec,
    runs: &[(Range<usize>, usize)],
    layout: &mut parley::Layout<u32>,
) -> (f32, f32) {
    let Some(&(_, index)) = runs.first() else {
        return (0.0, 0.0);
    };
    let run = &spec.runs[index];
    if run.indent == 0.0 {
        return (0.0, 0.0);
    }
    let marker: f32 = if run.hang {
        layout.break_all_lines(None);
        let mut sum = 0.0;
        for line in layout.lines() {
            for r in line.runs() {
                for c in r.visual_clusters() {
                    if c.first_style().brush == index as u32 {
                        sum += c.advance();
                    }
                }
            }
        }
        sum
    } else {
        0.0
    };
    (run.indent - marker, run.indent)
}

/// A line's box (CSS 2 §10.8): the strut, grown by each glyph's own face
/// where its run's `line-height` is normal, or by its run's authored box.
/// Returns the space above and below the baseline, and whether an explicit
/// length decided either.
pub(super) fn line_box<'a>(
    glyphs: impl Iterator<Item = &'a LayoutGlyph>,
    faces: &[lines::Face],
    spec: &Spec,
    catalog: &mut catalog::Catalog,
    strut: (f32, f32),
    run_metrics: &[FontMetrics],
) -> (f32, f32, bool) {
    let (mut above, mut below) = strut;
    let mut above_explicit = spec.strut.line_height.is_some();
    let mut below_explicit = above_explicit;
    let mut last: Option<(u16, u32, u32)> = None;
    for glyph in glyphs {
        let key = (glyph.face, glyph.font_size.to_bits(), glyph.metadata);
        if last == Some(key) {
            continue;
        }
        last = Some(key);
        let Some(face) = faces.get(glyph.face as usize) else {
            continue;
        };
        let Some(m) = catalog.raw_metrics(&face.font) else {
            continue;
        };
        if m.units_per_em == 0 {
            continue;
        }
        let run = glyph.run();
        let scale = glyph.font_size / m.units_per_em as f32;
        // Explicit lengths size the authored inline box; only normal
        // expands to the actual fallback glyph font.
        let (ascent, descent, leading) = if spec.runs[run].line_height.is_some() {
            run_metrics[run]
        } else {
            (m.ascent * scale, m.descent.abs() * scale, m.leading * scale)
        };
        let height = spec.runs[run]
            .line_height
            .unwrap_or(ascent + descent + leading);
        let half = (height - ascent - descent - leading) / 2.0;
        let run_explicit = spec.runs[run].line_height.is_some();
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
    (above, below, above_explicit || below_explicit)
}

/// One visual line as painters and tests read it.
#[derive(Clone, Copy, Debug)]
pub struct LayoutRun<'a> {
    /// Its hard line's index.
    pub line_i: usize,
    /// Its hard line's text (glyph ranges index it).
    pub text: &'a str,
    /// The hard line's base direction.
    pub rtl: bool,
    /// Its glyphs, in visual order.
    pub glyphs: &'a [LayoutGlyph],
    /// Its baseline, points from the paragraph's top.
    pub line_y: f32,
    /// Its box's top.
    pub line_top: f32,
    /// Its box's height.
    pub line_height: f32,
    /// Its width.
    pub line_w: f32,
}

pub(super) struct Runs<'a> {
    paragraph: &'a Paragraph,
    index: usize,
}
impl<'a> Runs<'a> {
    pub(super) fn new(paragraph: &'a Paragraph) -> Self {
        Self {
            paragraph,
            index: 0,
        }
    }
}
impl<'a> Iterator for Runs<'a> {
    type Item = LayoutRun<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let p = self.paragraph;
        let record = p.layouts();
        let line = record.lines.get(self.index)?;
        let i = self.index;
        self.index += 1;
        let hard = &p.source.data.lines[line.hard as usize];
        let (line_top, line_y, line_height) =
            if let Some(flow) = p.flow.as_ref().filter(|f| !f.incomplete) {
                (flow.fragments[i].y, p.baselines[i], flow.line_height)
            } else {
                let top = if i == 0 { 0.0 } else { p.bottoms[i - 1] };
                (top, p.baselines[i], p.bottoms[i] - top)
            };
        Some(LayoutRun {
            line_i: line.hard as usize,
            text: &hard.text,
            rtl: hard.rtl,
            glyphs: record.glyphs_of(line),
            line_y,
            line_top,
            line_height,
            line_w: line.w,
        })
    }
}
