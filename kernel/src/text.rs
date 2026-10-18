//! Text measurement: the one structural text IR and the injected measurer.
//!
//! The kernel never embeds platform text APIs. A host supplies a
//! [`TextMeasurer`] per kernel — there is no process-global callback — and the
//! kernel hands it a paragraph as ordered [`TextRun`]s with the offer it must
//! fit. A `Text` node with its own `text` prop is one run; otherwise its `Text`
//! children are its runs, flattened in order. The same runs feed measurement,
//! export, and (later) selection: one traversal, per-consumer projections.
//!
//! [`MonospaceMeasurer`] is the deterministic reference measurer used by tests
//! and headless hosts.

use crate::generated::{
    Direction, FontStyle, OverflowWrap, StyleId, StyleMask, StyleProps, TextAlign, TextOverflow,
};
use crate::id::AxisOffer;
use crate::id::NodeKey;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub(crate) mod case;

/// A payload-free lifetime namespace. Allocation identity is valid only while
/// retained: it is never a wire id, address handle, or serialized cache key.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextDomain(pub(crate) Arc<()>);

impl PartialEq for TextDomain {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for TextDomain {}
impl Hash for TextDomain {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

/// Kernel-issued proof of the current paragraph inputs, not its geometry.
///
/// Offers, catalog identity, runtime liveness and publication eligibility are
/// separate checks. Host appearance, resolved palettes and raster context are
/// independent paint inputs: equal stamps do not identify resolved pixels.
/// This token retains only a payload-free namespace, never text, nodes, a
/// kernel or prior revisions. Exact equality includes authored paint and source
/// mapping; [`Self::same_metrics`] permits metric reuse after repaint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParagraphStamp {
    pub(crate) domain: TextDomain,
    pub(crate) owner: NodeKey,
    pub(crate) view: crate::ViewId,
    pub(crate) metrics: u64,
    pub(crate) paint_source: u64,
}

impl ParagraphStamp {
    /// The independently measured paragraph allocation.
    pub fn owner(&self) -> NodeKey {
        self.owner
    }

    /// Producer identity for the host paragraph presenter.
    pub fn view(&self) -> crate::ViewId {
        self.view
    }

    /// Current metric revision. This scalar alone is not an identity.
    pub fn metric_revision(&self) -> u64 {
        self.metrics
    }

    /// Current authored paint/source-map revision, not resolved pixel identity.
    /// Host appearance, palette and raster context remain separate paint inputs.
    /// This scalar alone is not an identity.
    pub fn paint_source_revision(&self) -> u64 {
        self.paint_source
    }

    /// Same namespace, owner and metric inputs, ignoring paint-only changes.
    pub fn same_metrics(&self, other: &Self) -> bool {
        self.domain == other.domain && self.owner == other.owner && self.metrics == other.metrics
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TextRevisions {
    pub metrics: u64,
    pub paint_source: u64,
}

/// Run-level style: everything that changes glyph metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Font size in points.
    pub font_size: f32,
    /// CSS weight 100–900.
    pub font_weight: u16,
    /// Normal or italic.
    pub font_style: FontStyle,
    /// Index into the host's font registry (0 = system default).
    pub font_family: u16,
    /// Resolved logical length, or natural metrics. Zero remains explicit.
    pub line_height: Option<f32>,
    /// Additional advance per glyph, in points.
    pub letter_spacing: f32,
    /// OpenType numeric-feature bitmask.
    pub font_variant_numeric: u8,
    /// Bold Text weights this run's font (LLP 1115.000 D12.3): its weight is
    /// stepped already, and a weight a host gives a piece of it itself (a
    /// Markdown piece's bold) takes the same step ([`crate::bolder`]).
    pub bold_text: bool,
}

impl TextStyle {
    /// The rows [`TextStyle::from_style`] reads.
    pub const ROWS: [StyleId; 7] = [
        StyleId::FontSize,
        StyleId::FontWeight,
        StyleId::FontStyle,
        StyleId::FontFamily,
        StyleId::LineHeight,
        StyleId::LetterSpacing,
        StyleId::FontVariantNumeric,
    ];

    /// [`TextStyle::from_style`] of a computed style read row by row: the
    /// node's `own` style, then `inherit(rows, take)` hands `take` each
    /// ancestor that supplies inherited `rows` the node lacks, nearest first.
    pub fn from_rows(
        own: &StyleProps,
        inherit: impl FnOnce(StyleMask, &mut dyn FnMut(&StyleProps, StyleMask)),
    ) -> Self {
        let mut rows = StyleMask::EMPTY;
        for id in Self::ROWS {
            rows.set(id);
        }
        let mut s = TextRows::of(own);
        inherit(rows, &mut |from, mask| s.take(from, mask));
        s.style()
    }

    /// This style drawn at `scale` of its size: its font size, a resolved
    /// line height and its letter spacing (LLP 1116.001 D1.4, a fitted text).
    pub fn scaled(mut self, scale: f32) -> Self {
        self.font_size *= scale;
        self.line_height = self.line_height.map(|h| h * scale);
        self.letter_spacing *= scale;
        self
    }

    /// The run style carried by a node's style rows.
    pub fn from_style(s: &StyleProps) -> Self {
        TextStyle {
            font_size: s.font_size,
            font_weight: s.font_weight,
            font_style: s.font_style,
            font_family: s.font_family,
            line_height: s.line_height.resolve(s.font_size),
            letter_spacing: s.letter_spacing,
            font_variant_numeric: s.font_variant_numeric,
            bold_text: false,
        }
    }
}

/// What the node's string is: plain text, or a markup the host styles.
/// @ref LLP 1045 D3 — the kernel carries the prop and the source as one
/// run; a host that links `exact-markdown` expands it for measure and paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Markup {
    /// The text as written.
    #[default]
    None,
    /// Markdown source.
    Markdown,
}

impl Markup {
    /// The `markup` prop's value, as written in Contract.
    pub fn from_prop(value: Option<&str>) -> Self {
        match value {
            Some("markdown") => Markup::Markdown,
            _ => Markup::None,
        }
    }
}

/// Paragraph-level style: what applies to the whole measured block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paragraph {
    /// What the runs' text is; `Markdown` means one run of source.
    pub markup: Markup,
    /// The paragraph strut: minimum line box even when all inline runs are smaller.
    pub strut: TextStyle,
    /// Base direction.
    pub direction: Direction,
    /// Horizontal alignment, physical: `start` and `end` are resolved
    /// against `direction` ([`TextAlign::physical`]), so a host never sees them.
    pub text_align: TextAlign,
    /// Maximum lines; 0 means unlimited.
    pub line_clamp: u32,
    /// How an over-long last line is truncated.
    pub text_overflow: TextOverflow,
    /// Whether otherwise unbreakable text may wrap.
    pub overflow_wrap: OverflowWrap,
    /// CSS whitespace preservation/collapsing.
    pub white_space: crate::WhiteSpace,
    /// CSS `text-wrap-style`: `balance` breaks the lines before the first
    /// forced break at the narrowest width that keeps their count
    /// ([`exact_textflow::balanced_width`]), up to six lines.
    pub text_wrap_style: crate::TextWrapStyle,
    /// CSS `text-indent` in points: the first line starts this far in from
    /// the start edge (right under `rtl`) and has that much less room.
    pub text_indent: f32,
    /// CSS `hyphens`. `none` already reached the runs' text
    /// ([`crate::Hyphens::shown`]); a host hyphenates `auto` where it can.
    pub hyphens: crate::Hyphens,
    /// CSS `text-box-trim` and `text-box-edge` (Inline Layout 3 §4): the
    /// measurer cuts the first line's over edge and the last line's under
    /// edge to these font metrics, so its height and first baseline are the
    /// trimmed box's, and the host paints the lines where it measured them.
    pub text_box_trim: crate::TextBoxTrim,
    /// The edges `text_box_trim` cuts to.
    pub text_box_edge: crate::TextBoxEdge,
}

impl TextAlign {
    /// CSS's `start` and `end` as `left` or `right` for a paragraph whose
    /// base direction is `direction`; every other value is already physical.
    pub fn physical(self, direction: Direction) -> TextAlign {
        match (self, direction) {
            (TextAlign::Start, Direction::Ltr) | (TextAlign::End, Direction::Rtl) => {
                TextAlign::Left
            }
            (TextAlign::Start, Direction::Rtl) | (TextAlign::End, Direction::Ltr) => {
                TextAlign::Right
            }
            (other, _) => other,
        }
    }
}

impl Paragraph {
    /// [`Paragraph::from_style`] of a style read row by row (see
    /// [`TextStyle::from_rows`]).
    pub fn from_rows(
        own: &StyleProps,
        inherit: impl FnOnce(StyleMask, &mut dyn FnMut(&StyleProps, StyleMask)),
    ) -> Self {
        let mut rows = StyleMask::EMPTY;
        for id in TextStyle::ROWS.into_iter().chain(PARAGRAPH_ROWS) {
            rows.set(id);
        }
        let mut t = TextRows::of(own);
        let mut p = ParagraphRows::of(own);
        inherit(rows, &mut |from, mask| {
            t.take(from, mask);
            p.take(from, mask);
        });
        Paragraph {
            markup: Markup::None,
            strut: t.style(),
            direction: p.direction,
            text_align: p.text_align.physical(p.direction),
            line_clamp: p.line_clamp,
            text_overflow: p.text_overflow,
            overflow_wrap: p.overflow_wrap,
            white_space: p.white_space,
            text_wrap_style: p.text_wrap_style,
            text_indent: p.text_indent,
            hyphens: p.hyphens,
            text_box_trim: p.text_box_trim,
            text_box_edge: p.text_box_edge,
        }
    }

    /// The paragraph style carried by a node's style rows.
    pub fn from_style(s: &StyleProps) -> Self {
        Paragraph {
            markup: Markup::None,
            strut: TextStyle::from_style(s),
            direction: s.direction,
            text_align: s.text_align.physical(s.direction),
            line_clamp: s.line_clamp,
            text_overflow: s.text_overflow,
            overflow_wrap: s.overflow_wrap,
            white_space: s.white_space,
            text_wrap_style: s.text_wrap_style,
            text_indent: s.text_indent,
            hyphens: s.hyphens,
            text_box_trim: s.text_box_trim,
            text_box_edge: s.text_box_edge,
        }
    }
}

impl crate::Hyphens {
    /// `text` as this paragraph's runs carry it: under `hyphens: none` a
    /// soft hyphen is no break, so it becomes U+034F COMBINING GRAPHEME
    /// JOINER, as invisible, no break opportunity (UAX #14 CM), and the
    /// same length in UTF-8 and UTF-16, so no offset moves.
    pub fn shown(self, text: Cow<'_, str>) -> Cow<'_, str> {
        if self != crate::Hyphens::None || !text.contains('\u{ad}') {
            return text;
        }
        Cow::Owned(text.replace('\u{ad}', "\u{34f}"))
    }
}

/// One styled run of text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRun<'a> {
    /// The text as measured and painted: `text-transform` already applied
    /// (LLP 1064 D5), borrowed from the node when it changes nothing.
    pub text: Cow<'a, str>,
    /// Its style.
    pub style: TextStyle,
}

impl AsRef<str> for TextRun<'_> {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

/// What the kernel asks a measurer to size.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMeasureRequest<'a> {
    /// The runs, in order. Never empty.
    pub runs: &'a [TextRun<'a>],
    /// Paragraph style.
    pub paragraph: Paragraph,
    /// Horizontal offer.
    pub width: AxisOffer,
    /// Vertical offer.
    pub height: AxisOffer,
    /// Resolved exclusions in content coordinates; empty during box layout.
    /// @ref LLP 1043.000 §3 D5 — hosts answer from their flowed layout.
    pub exclusions: &'a [exact_textflow::FlowShape],
}

/// A measurement.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextMetrics {
    /// Measured width.
    pub width: f32,
    /// Measured height.
    pub height: f32,
    /// Distance from the top to the first line's alphabetic baseline, if known.
    pub first_baseline: Option<f32>,
}

impl TextMetrics {
    pub(crate) fn is_valid(self) -> bool {
        self.width.is_finite()
            && self.width >= 0.0
            && self.height.is_finite()
            && self.height >= 0.0
            && self
                .first_baseline
                .is_none_or(|baseline| baseline.is_finite() && baseline >= 0.0)
    }
}

/// Where a paragraph's text-box edges stand, in points from its top
/// (Inline Layout 3 §4.2): its first line's over edges and its last line's
/// under edges, read from the fonts that line is set in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextEdges {
    /// The first line's text-over edge: the top of its fonts' ascent.
    pub text_over: f32,
    /// The first line's cap height above its baseline.
    pub cap_over: f32,
    /// The first line's x-height above its baseline.
    pub ex_over: f32,
    /// The last line's alphabetic baseline.
    pub alphabetic_under: f32,
    /// The last line's text-under edge: the bottom of its fonts' descent.
    pub text_under: f32,
}

impl crate::TextBoxTrim {
    /// Whether the first line's over edge is cut.
    pub fn start(self) -> bool {
        matches!(
            self,
            crate::TextBoxTrim::TrimStart | crate::TextBoxTrim::TrimBoth
        )
    }
    /// Whether the last line's under edge is cut.
    pub fn end(self) -> bool {
        matches!(
            self,
            crate::TextBoxTrim::TrimEnd | crate::TextBoxTrim::TrimBoth
        )
    }
}

/// `text-box-edge`'s over edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverEdge {
    /// The font's ascent.
    Text = 0,
    /// The cap height.
    Cap = 1,
    /// The x-height.
    Ex = 2,
}

/// `text-box-edge`'s under edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderEdge {
    /// The font's descent.
    Text = 0,
    /// The alphabetic baseline.
    Alphabetic = 1,
}

impl crate::TextBoxEdge {
    /// The over edge: `auto` is `text`.
    pub fn over(self) -> OverEdge {
        use crate::TextBoxEdge::*;
        match self {
            Cap | CapAlphabetic => OverEdge::Cap,
            Ex | ExAlphabetic => OverEdge::Ex,
            Auto | Text | TextAlphabetic => OverEdge::Text,
        }
    }
    /// The under edge: one keyword leaves it `text`.
    pub fn under(self) -> UnderEdge {
        use crate::TextBoxEdge::*;
        match self {
            TextAlphabetic | CapAlphabetic | ExAlphabetic => UnderEdge::Alphabetic,
            Auto | Text | Cap | Ex => UnderEdge::Text,
        }
    }
}

impl Paragraph {
    /// `text-box-trim` applied to a measurement: how much comes off the top,
    /// and the trimmed metrics (height, and a first baseline that many points
    /// higher). With no trim, nothing changes. A host paints the lines that
    /// many points up, where the measurement put them.
    pub fn trimmed(&self, metrics: TextMetrics, edges: TextEdges) -> (f32, TextMetrics) {
        let trim = self.text_box_trim;
        let top = if trim.start() {
            match self.text_box_edge.over() {
                OverEdge::Cap => edges.cap_over,
                OverEdge::Ex => edges.ex_over,
                OverEdge::Text => edges.text_over,
            }
        } else {
            0.0
        };
        let under = match self.text_box_edge.under() {
            UnderEdge::Alphabetic => edges.alphabetic_under,
            UnderEdge::Text => edges.text_under,
        };
        let bottom = if trim.end() {
            metrics.height - under
        } else {
            0.0
        };
        if !(top.is_finite() && bottom.is_finite()) {
            return (0.0, metrics);
        }
        let height = (metrics.height - top - bottom).max(0.0);
        let first_baseline = metrics.first_baseline.map(|b| (b - top).max(0.0));
        (
            top,
            TextMetrics {
                height,
                first_baseline,
                ..metrics
            },
        )
    }
}

/// A system symbol to measure: an image whose source is `symbol:<role>`,
/// drawn at its computed font size and weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolRequest<'a> {
    /// The role after `symbol:`: `sf/<name>`, or a portable role.
    pub role: &'a str,
    /// The image's computed font size, in points.
    pub font_size: f32,
    /// Its computed font weight, 1–1000.
    pub font_weight: u16,
}

/// A host's text engine, injected per kernel.
pub trait TextMeasurer {
    /// The platform formats an instant on the runner clock (LLP 1126 D6).
    fn format_time(&mut self, request: &crate::time::TimeRequest<'_>) -> Option<String> {
        crate::time::english(request)
    }

    /// Revision of host measuring traits (scale, content size, legibility,
    /// appearance). A change invalidates native button offers even if fonts
    /// and the tree stay the same. Hosts without changing traits return zero.
    fn measure_revision(&self) -> u64 {
        0
    }

    /// Native field chrome (LLP 1104 D5). Existing/painted measurers answer
    /// no chrome until they implement their own control look.
    fn field_chrome(&mut self, _request: &crate::FieldChromeRequest) -> crate::FieldChrome {
        crate::FieldChrome::default()
    }

    /// A text control's own text, where the host's control sets it with a
    /// text engine of its own (LLP 1115.001 D8: Android's `EditText`): its
    /// width, height and first baseline at the offer, asked during layout.
    /// `None`, the default, measures it as any paragraph.
    fn field_text(
        &mut self,
        _kind: crate::FieldKind,
        _request: &TextMeasureRequest<'_>,
    ) -> Option<TextMetrics> {
        None
    }

    /// Native button border-box size, including its platform chrome and
    /// authored content insets (LLP 1069.011.001 D11). None preserves the
    /// default size / set_intrinsic_size path for existing hosts.
    fn button_measure(
        &mut self,
        _request: &crate::ButtonMeasureRequest,
    ) -> Option<crate::ButtonMeasure> {
        None
    }

    /// A system symbol's natural size, as the host draws it, measured before
    /// the layout that places it, so the first frame has its box. `None` when
    /// this host cannot say; the image's size is then the one its host
    /// reports once its view is made.
    fn symbol_size(&mut self, _request: &SymbolRequest<'_>) -> Option<(f32, f32)> {
        None
    }

    /// The resolved document language; an empty language is unknown.
    fn set_language(&mut self, _language: &str) {}

    /// Size a paragraph under an offer.
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics;

    /// Size a kernel-identified paragraph. Existing measurers use the ordinary
    /// synchronous path; callers constructing requests still need no stamp.
    /// The stamp identifies exactly this owner's canonical runs, not a subset
    /// rooted at an inline child. Offers and host catalog remain separate keys.
    fn measure_identified(
        &mut self,
        _stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        self.measure(request)
    }

    /// Whether this measurer's metrics never depend on the height offered
    /// (only width wraps): the kernel then reuses a leaf's measurement
    /// across heights instead of asking again.
    fn height_free(&self) -> bool {
        false
    }

    /// The metrics of a kernel-identified paragraph this measurer already
    /// holds by `stamp`, at an offer, without its runs (no exclusions): the
    /// kernel builds runs and paragraph style only when this is `None`.
    /// Measurers that keep no identities answer `None`.
    fn measure_known(
        &mut self,
        _stamp: &ParagraphStamp,
        _width: AxisOffer,
        _height: AxisOffer,
    ) -> Option<TextMetrics> {
        None
    }

    /// @ref LLP 1093 D6 — each line box's bottom, in content coordinates, of
    /// the paragraph the last measure of `request` built: where a column may
    /// end inside it. The kernel asks only for a paragraph that would cross a
    /// column's end. A measurer that answers nothing keeps every paragraph
    /// whole.
    fn lines(
        &mut self,
        _stamp: &ParagraphStamp,
        _request: &TextMeasureRequest<'_>,
        _bottoms: &mut Vec<f32>,
    ) {
    }
}

/// Deterministic reference measurer: every glyph advances `advance_em` ems
/// plus letter spacing; a line is `line_height_em` ems unless the style sets
/// one. Wraps at whitespace; `overflow-wrap` can also split over-long words.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonospaceMeasurer {
    /// Glyph advance as a fraction of font size.
    pub advance_em: f32,
    /// Natural line height as a fraction of font size.
    pub line_height_em: f32,
    /// Baseline position as a fraction of line height.
    pub baseline_frac: f32,
}

impl Default for MonospaceMeasurer {
    fn default() -> Self {
        MonospaceMeasurer {
            advance_em: 0.6,
            line_height_em: 1.2,
            baseline_frac: 0.8,
        }
    }
}

impl MonospaceMeasurer {
    fn line_height(&self, style: &TextStyle) -> f32 {
        style
            .line_height
            .unwrap_or(style.font_size * self.line_height_em)
    }

    fn advance(&self, style: &TextStyle) -> f32 {
        style.font_size * self.advance_em + style.letter_spacing
    }
}

#[derive(Debug, Clone)]
enum Token {
    Word { glyphs: Vec<f32> },
    Space { width: f32 },
    Break,
}

impl crate::WhiteSpace {
    /// The walker's and hosts' model of this row: collapse × wrap mode.
    /// @ref LLP 1053 §0 G5
    pub fn model(self) -> exact_textflow::WhiteSpace {
        match self {
            crate::WhiteSpace::Normal => exact_textflow::WhiteSpace::Normal,
            crate::WhiteSpace::PreWrap => exact_textflow::WhiteSpace::PreWrap,
            crate::WhiteSpace::Nowrap => exact_textflow::WhiteSpace::Nowrap,
            crate::WhiteSpace::PreLine => exact_textflow::WhiteSpace::PreLine,
            crate::WhiteSpace::Pre => exact_textflow::WhiteSpace::Pre,
        }
    }
}

impl MonospaceMeasurer {
    /// @ref LLP 1043.000 §3 D5–D6 — flow around exclusions through the shared
    /// walker over this measurer's advances, as the hosts do over theirs.
    fn flow(&self, request: &TextMeasureRequest<'_>, width: f32) -> TextMetrics {
        let strut = &request.paragraph.strut;
        let mut line_height = self.line_height(strut);
        let mut text = String::new();
        let mut ends = Vec::with_capacity(request.runs.len());
        for run in request.runs {
            text.push_str(&run.text);
            ends.push((text.len(), self.advance(&run.style)));
            line_height = line_height.max(self.line_height(&run.style));
        }
        let mut measure = |range: std::ops::Range<usize>| {
            text[range.clone()]
                .char_indices()
                .map(|(i, _)| {
                    let at = range.start + i;
                    ends.iter().find(|(end, _)| at < *end).map_or(0.0, |r| r.1)
                })
                .sum::<f32>()
        };
        let prepared = exact_textflow::Prepared::new(
            &text,
            exact_textflow::Options {
                white_space: request.paragraph.white_space.model(),
                overflow_wrap: match request.paragraph.overflow_wrap {
                    OverflowWrap::Normal => exact_textflow::OverflowWrap::Normal,
                    OverflowWrap::BreakWord => exact_textflow::OverflowWrap::BreakWord,
                    OverflowWrap::Anywhere => exact_textflow::OverflowWrap::Anywhere,
                },
                hyphen_advance: self.advance(strut),
            },
            &mut measure,
        );
        let mut fragments = Vec::new();
        let result = exact_textflow::flow(
            &prepared,
            request.exclusions,
            &exact_textflow::FlowOptions {
                direction: match request.paragraph.direction {
                    Direction::Ltr => exact_textflow::Direction::Ltr,
                    Direction::Rtl => exact_textflow::Direction::Rtl,
                },
                width,
                line_height,
                min_fragment: strut.font_size * exact_textflow::MIN_FRAGMENT_EM,
                max_lines: request.paragraph.line_clamp,
            },
            &mut fragments,
        );
        TextMetrics {
            width: fragments.iter().map(|f| f.x + f.width).fold(0.0, f32::max),
            height: result.height,
            // No line, no first baseline
            first_baseline: (!fragments.is_empty()).then_some(line_height * self.baseline_frac),
        }
    }
}

impl TextMeasurer for MonospaceMeasurer {
    /// Lines wrap at the width alone; the height offered is never read.
    fn height_free(&self) -> bool {
        true
    }
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        let metrics = self.measure_lines(request);
        self.trim(request, metrics).1
    }

    fn lines(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
        bottoms: &mut Vec<f32>,
    ) {
        self.line_bottoms(stamp, request, bottoms);
        // A trimmed paragraph's lines move up by the top's cut, and the last
        // ends at the trimmed height, as its measurement does.
        if request.paragraph.text_box_trim != crate::TextBoxTrim::None {
            let measured = self.measure_lines(request);
            let (top, metrics) = self.trim(request, measured);
            for b in bottoms.iter_mut() {
                *b = (*b - top).min(metrics.height);
            }
            if let Some(last) = bottoms.last_mut() {
                *last = metrics.height;
            }
        }
    }
}

impl MonospaceMeasurer {
    /// `text-box-trim` on a measurement: the cut off its top, and the
    /// trimmed metrics.
    fn trim(
        &mut self,
        request: &TextMeasureRequest<'_>,
        metrics: TextMetrics,
    ) -> (f32, TextMetrics) {
        let p = &request.paragraph;
        // Trim nothing without a line: CSS trims a formatted line's edges
        let Some(baseline) = metrics.first_baseline else {
            return (0.0, metrics);
        };
        if p.text_box_trim == crate::TextBoxTrim::None {
            return (0.0, metrics);
        }
        // The reference face: cap height 0.7 em and x-height 0.5 em above the
        // baseline, its ascent and descent the line box's.
        // The edges are the root inline box's, the paragraph's own font
        // (Inline Layout 3 §4.2); the line box is every run's.
        let mut line = self.line_height(&p.strut);
        let size = p.strut.font_size;
        for run in request.runs {
            line = line.max(self.line_height(&run.style));
        }
        let last = metrics.height - line + line * self.baseline_frac;
        let edges = TextEdges {
            text_over: 0.0,
            cap_over: baseline - 0.7 * size,
            ex_over: baseline - 0.5 * size,
            alphabetic_under: last,
            text_under: metrics.height,
        };
        p.trimmed(metrics, edges)
    }
}

impl MonospaceMeasurer {
    fn measure_lines(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if let AxisOffer::Definite(width) = request.width {
            if !request.exclusions.is_empty() {
                return self.flow(request, width);
            }
        }
        let white_space = request.paragraph.white_space.model();
        // CSS collapsing first, as every native engine now does (LLP 1053 G5).
        let collapsed = exact_textflow::collapse(request.runs, white_space);
        // Tokenize across runs. A word may span runs; its width accumulates.
        let mut tokens: Vec<Token> = Vec::new();
        let mut line_height = self.line_height(&request.paragraph.strut);
        let mut pending_word: Vec<f32> = Vec::new();
        let mut any_text = false;
        for (index, run) in request.runs.iter().enumerate() {
            line_height = line_height.max(self.line_height(&run.style));
            let advance = self.advance(&run.style);
            let text = collapsed
                .as_ref()
                .map_or(&*run.text, |c| c.runs[index].as_str());
            for ch in text.chars() {
                any_text = true;
                if ch == '\n' {
                    if !pending_word.is_empty() {
                        tokens.push(Token::Word {
                            glyphs: std::mem::take(&mut pending_word),
                        });
                    }
                    tokens.push(Token::Break);
                } else if ch.is_whitespace() {
                    if !pending_word.is_empty() {
                        tokens.push(Token::Word {
                            glyphs: std::mem::take(&mut pending_word),
                        });
                    }
                    tokens.push(Token::Space { width: advance });
                } else {
                    pending_word.push(advance);
                }
            }
        }
        if !pending_word.is_empty() {
            tokens.push(Token::Word {
                glyphs: pending_word,
            });
        }
        if !any_text {
            return TextMetrics::default();
        }

        // Lay lines out under the offer.
        let limit = match request.width {
            _ if !white_space.wraps() => None,
            AxisOffer::Definite(w) => Some(w.max(0.0)),
            AxisOffer::MaxContent => None,
            AxisOffer::MinContent => Some(0.0),
        };
        let breaks_words = request.paragraph.overflow_wrap != OverflowWrap::Normal
            && (!matches!(request.width, AxisOffer::MinContent)
                || request.paragraph.overflow_wrap == OverflowWrap::Anywhere);
        let mut lines: Vec<f32> = vec![0.0];
        let mut trailing_space = 0f32;
        for token in tokens {
            match token {
                Token::Break => {
                    lines.push(0.0);
                    trailing_space = 0.0;
                }
                Token::Space { width } => {
                    if let Some(current) = lines.last_mut() {
                        if *current > 0.0 {
                            *current += width;
                            trailing_space += width;
                        }
                    }
                }
                Token::Word { glyphs } => {
                    let width: f32 = glyphs.iter().sum();
                    let current = lines.last_mut().expect("one line exists");
                    if limit.is_some_and(|max| *current > 0.0 && *current + width > max) {
                        *current -= trailing_space;
                        lines.push(0.0);
                    }
                    trailing_space = 0.0;
                    if breaks_words && limit.is_some_and(|max| width > max) {
                        for advance in glyphs {
                            let current = lines.last_mut().expect("one line exists");
                            if limit.is_some_and(|max| *current > 0.0 && *current + advance > max) {
                                lines.push(advance);
                            } else {
                                *current += advance;
                            }
                        }
                    } else {
                        *lines.last_mut().expect("one line exists") += width;
                    }
                }
            }
        }
        if let Some(last) = lines.last_mut() {
            *last -= trailing_space;
        }
        let max_lines = request.paragraph.line_clamp;
        if max_lines > 0 && lines.len() > max_lines as usize {
            lines.truncate(max_lines as usize);
        }
        // Lines never exceed the limit except for a single unbreakable word,
        // which legitimately overflows — so the widest line is the width.
        let width = lines.iter().copied().fold(0f32, f32::max);
        let height = line_height * lines.len() as f32;
        TextMetrics {
            width,
            height,
            first_baseline: Some(line_height * self.baseline_frac),
        }
    }
}

impl MonospaceMeasurer {
    /// Every line is one line height, so each bottom is a multiple of it.
    fn line_bottoms(
        &mut self,
        _stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
        bottoms: &mut Vec<f32>,
    ) {
        let height = self.measure_lines(request).height;
        let line_height = request
            .runs
            .iter()
            .map(|run| self.line_height(&run.style))
            .fold(self.line_height(&request.paragraph.strut), f32::max);
        if line_height > 0.0 {
            let count = (height / line_height).round() as usize;
            bottoms.extend((1..=count).map(|i| line_height * i as f32));
        }
    }
}

/// What [`TextStyle::from_style`] reads, gathered row by row.
#[derive(Clone, Copy)]
struct TextRows {
    font_size: f32,
    font_weight: u16,
    font_style: crate::FontStyle,
    font_family: u16,
    line_height: crate::LineHeight,
    letter_spacing: f32,
    font_variant_numeric: u8,
}

impl TextRows {
    fn of(s: &StyleProps) -> Self {
        TextRows {
            font_size: s.font_size,
            font_weight: s.font_weight,
            font_style: s.font_style,
            font_family: s.font_family,
            line_height: s.line_height,
            letter_spacing: s.letter_spacing,
            font_variant_numeric: s.font_variant_numeric,
        }
    }
    fn take(&mut self, s: &StyleProps, mask: StyleMask) {
        if mask.has(StyleId::FontSize) {
            self.font_size = s.font_size;
        }
        if mask.has(StyleId::FontWeight) {
            self.font_weight = s.font_weight;
        }
        if mask.has(StyleId::FontStyle) {
            self.font_style = s.font_style;
        }
        if mask.has(StyleId::FontFamily) {
            self.font_family = s.font_family;
        }
        if mask.has(StyleId::LineHeight) {
            self.line_height = s.line_height;
        }
        if mask.has(StyleId::LetterSpacing) {
            self.letter_spacing = s.letter_spacing;
        }
        if mask.has(StyleId::FontVariantNumeric) {
            self.font_variant_numeric = s.font_variant_numeric;
        }
    }
    fn style(&self) -> TextStyle {
        TextStyle {
            font_size: self.font_size,
            font_weight: self.font_weight,
            font_style: self.font_style,
            font_family: self.font_family,
            line_height: self.line_height.resolve(self.font_size),
            letter_spacing: self.letter_spacing,
            font_variant_numeric: self.font_variant_numeric,
            bold_text: false,
        }
    }
}

/// Computed run rows carried down one inline tree. Keep line-height unresolved
/// until a leaf: an inherited unitless value scales with that leaf's font size.
#[derive(Clone, Copy)]
pub(crate) struct RunRows {
    text: TextRows,
    pub hyphens: crate::Hyphens,
    pub transform: crate::TextTransform,
    mask: StyleMask,
}

impl RunRows {
    pub fn from_rows(
        own: &StyleProps,
        inherit: impl FnOnce(StyleMask, &mut dyn FnMut(&StyleProps, StyleMask)),
    ) -> Self {
        let mut rows = StyleMask::EMPTY;
        for id in TextStyle::ROWS {
            rows.set(id);
        }
        rows.set(StyleId::Hyphens);
        rows.set(StyleId::TextTransform);
        let mut out = Self {
            text: TextRows::of(own),
            hyphens: own.hyphens,
            transform: own.text_transform,
            mask: own.mask,
        };
        inherit(rows, &mut |from, mask| {
            out.take(from, mask);
            out.mask = out.mask.union(mask);
        });
        out
    }

    /// Only Text children participate in an inline traversal, so control
    /// typography boundaries have already been resolved at the traversal root.
    pub fn child(mut self, own: &StyleProps) -> Self {
        // A row set nowhere retains the child's own initial value, unmarked,
        // exactly as computed_style does (including arena-only test styles).
        self.take(own, own.mask.union(StyleMask::INHERITED.minus(self.mask)));
        self.mask = self.mask.union(own.mask);
        self
    }

    /// The rows set at or above this run.
    pub fn said(&self) -> StyleMask {
        self.mask
    }

    /// `from`'s rows in `mask` taken, as a text style gives them (LLP
    /// 1115.000 D12.2), and marked set for the runs inside.
    pub fn fill(mut self, from: &StyleProps, mask: StyleMask) -> Self {
        self.take(from, mask);
        self.mask = self.mask.union(mask);
        self
    }

    fn take(&mut self, from: &StyleProps, mask: StyleMask) {
        self.text.take(from, mask);
        if mask.has(StyleId::Hyphens) {
            self.hyphens = from.hyphens;
        }
        if mask.has(StyleId::TextTransform) {
            self.transform = from.text_transform;
        }
    }

    pub fn style(&self) -> TextStyle {
        self.text.style()
    }
}

/// The paragraph rows [`Paragraph::from_style`] reads besides its strut.
const PARAGRAPH_ROWS: [StyleId; 11] = [
    StyleId::Direction,
    StyleId::TextAlign,
    StyleId::LineClamp,
    StyleId::TextOverflow,
    StyleId::OverflowWrap,
    StyleId::WhiteSpace,
    StyleId::TextWrapStyle,
    StyleId::TextIndent,
    StyleId::Hyphens,
    StyleId::TextBoxTrim,
    StyleId::TextBoxEdge,
];

struct ParagraphRows {
    direction: crate::Direction,
    text_align: crate::TextAlign,
    line_clamp: u32,
    text_overflow: crate::TextOverflow,
    overflow_wrap: crate::OverflowWrap,
    white_space: crate::WhiteSpace,
    text_wrap_style: crate::TextWrapStyle,
    text_indent: f32,
    hyphens: crate::Hyphens,
    text_box_trim: crate::TextBoxTrim,
    text_box_edge: crate::TextBoxEdge,
}

impl ParagraphRows {
    fn of(s: &StyleProps) -> Self {
        ParagraphRows {
            direction: s.direction,
            text_align: s.text_align,
            line_clamp: s.line_clamp,
            text_overflow: s.text_overflow,
            overflow_wrap: s.overflow_wrap,
            white_space: s.white_space,
            text_wrap_style: s.text_wrap_style,
            text_indent: s.text_indent,
            hyphens: s.hyphens,
            text_box_trim: s.text_box_trim,
            text_box_edge: s.text_box_edge,
        }
    }
    fn take(&mut self, s: &StyleProps, mask: StyleMask) {
        if mask.has(StyleId::Direction) {
            self.direction = s.direction;
        }
        if mask.has(StyleId::TextAlign) {
            self.text_align = s.text_align;
        }
        if mask.has(StyleId::LineClamp) {
            self.line_clamp = s.line_clamp;
        }
        if mask.has(StyleId::TextOverflow) {
            self.text_overflow = s.text_overflow;
        }
        if mask.has(StyleId::OverflowWrap) {
            self.overflow_wrap = s.overflow_wrap;
        }
        if mask.has(StyleId::WhiteSpace) {
            self.white_space = s.white_space;
        }
        if mask.has(StyleId::TextWrapStyle) {
            self.text_wrap_style = s.text_wrap_style;
        }
        if mask.has(StyleId::TextIndent) {
            self.text_indent = s.text_indent;
        }
        if mask.has(StyleId::Hyphens) {
            self.hyphens = s.hyphens;
        }
        if mask.has(StyleId::TextBoxTrim) {
            self.text_box_trim = s.text_box_trim;
        }
        if mask.has(StyleId::TextBoxEdge) {
            self.text_box_edge = s.text_box_edge;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(font_size: f32) -> TextStyle {
        TextStyle {
            font_size,
            font_weight: 400,
            font_style: FontStyle::Normal,
            font_family: 0,
            line_height: None,
            letter_spacing: 0.0,
            font_variant_numeric: 0,
            bold_text: false,
        }
    }

    #[test]
    fn start_and_end_follow_the_paragraph_direction() {
        use crate::{StyleId, StyleValue};
        let mut s = StyleProps::default();
        assert_eq!(s.text_align, TextAlign::Start, "CSS's initial value");
        assert_eq!(Paragraph::from_style(&s).text_align, TextAlign::Left);
        s.set_dynamic(StyleId::Direction, &StyleValue::Text("rtl".into()))
            .unwrap();
        assert_eq!(Paragraph::from_style(&s).text_align, TextAlign::Right);
        s.set_dynamic(StyleId::TextAlign, &StyleValue::Text("end".into()))
            .unwrap();
        assert_eq!(Paragraph::from_style(&s).text_align, TextAlign::Left);
        for physical in [
            TextAlign::Left,
            TextAlign::Center,
            TextAlign::Right,
            TextAlign::Justify,
        ] {
            for d in [Direction::Ltr, Direction::Rtl] {
                assert_eq!(physical.physical(d), physical);
            }
        }
    }

    fn paragraph() -> Paragraph {
        Paragraph {
            markup: Markup::None,
            strut: style(10.0),
            direction: Direction::Ltr,
            text_align: TextAlign::Left,
            line_clamp: 0,
            text_overflow: TextOverflow::Clip,
            overflow_wrap: OverflowWrap::Normal,
            white_space: crate::WhiteSpace::Normal,
            text_wrap_style: crate::TextWrapStyle::Auto,
            text_indent: 0.0,
            hyphens: crate::Hyphens::Manual,
            text_box_trim: crate::TextBoxTrim::None,
            text_box_edge: crate::TextBoxEdge::Auto,
        }
    }

    /// `text-box: trim-both cap alphabetic` cuts a line to its cap height:
    /// the reference face's cap is 0.7 em above a baseline 0.8 down a 1.2 em
    /// line, so 10 pt text keeps 7 pt, its baseline at the bottom.
    #[test]
    fn text_box_trim_cuts_to_the_cap_height_and_baseline() {
        let runs = [TextRun {
            text: "Ab".into(),
            style: style(10.0),
        }];
        let mut p = paragraph();
        let mut m = MonospaceMeasurer::default();
        let plain = m.measure(&TextMeasureRequest {
            runs: &runs,
            paragraph: p,
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        });
        assert_eq!((plain.height, plain.first_baseline), (12.0, Some(9.6)));
        p.text_box_trim = crate::TextBoxTrim::TrimBoth;
        p.text_box_edge = crate::TextBoxEdge::CapAlphabetic;
        let cut = m.measure(&TextMeasureRequest {
            runs: &runs,
            paragraph: p,
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        });
        assert!((cut.height - 7.0).abs() < 1e-4, "{cut:?}");
        assert!((cut.first_baseline.unwrap() - 7.0).abs() < 1e-4, "{cut:?}");
        p.text_box_trim = crate::TextBoxTrim::TrimStart;
        let start = m.measure(&TextMeasureRequest {
            runs: &runs,
            paragraph: p,
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        });
        assert!((start.height - 9.4).abs() < 1e-4, "only the top: {start:?}");
    }

    /// A line shorter than its glyphs is cut by negative amounts, so the
    /// paragraph grows; its last line still ends where the measurement does.
    /// Empty text has no line to cut and stays empty.
    #[test]
    fn a_trimmed_paragraphs_last_line_ends_at_its_height() {
        let mut short = style(10.0);
        short.line_height = Some(4.0);
        let runs = [TextRun {
            text: "Ab\nAb".into(),
            style: short,
        }];
        let mut p = paragraph();
        p.strut = short;
        p.white_space = crate::WhiteSpace::PreLine;
        p.text_box_trim = crate::TextBoxTrim::TrimBoth;
        p.text_box_edge = crate::TextBoxEdge::CapAlphabetic;
        let request = TextMeasureRequest {
            runs: &runs,
            paragraph: p,
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
            exclusions: &[],
        };
        let mut m = MonospaceMeasurer::default();
        let measured = m.measure(&request);
        assert!(measured.height > 8.0, "the cuts are negative: {measured:?}");
        let stamp = ParagraphStamp {
            domain: TextDomain::default(),
            owner: crate::NodeKey {
                index: 0,
                generation: 0,
            },
            view: 0,
            metrics: 0,
            paint_source: 0,
        };
        let mut bottoms = Vec::new();
        m.lines(&stamp, &request, &mut bottoms);
        assert_eq!(bottoms.len(), 2);
        assert_eq!(bottoms.last(), Some(&measured.height));

        let empty = [TextRun {
            text: "".into(),
            style: style(20.0),
        }];
        let empty = m.measure(&TextMeasureRequest {
            runs: &empty,
            ..request
        });
        assert_eq!(empty.height, 0.0, "{empty:?}");
    }

    fn measure(text: &str, width: AxisOffer, lines: u32) -> TextMetrics {
        let runs = [TextRun {
            text: text.into(),
            style: style(10.0),
        }];
        let mut p = paragraph();
        p.line_clamp = lines;
        MonospaceMeasurer::default().measure(&TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: p,
            width,
            height: AxisOffer::MaxContent,
        })
    }

    #[test]
    fn single_line_max_content() {
        let m = measure("hello", AxisOffer::MaxContent, 0);
        assert_eq!(m.width, 30.0);
        assert_eq!(m.height, 12.0);
        assert_eq!(m.first_baseline, Some(9.6));
    }

    #[test]
    fn wraps_at_whitespace_under_a_definite_width() {
        // "hello world" = 11 chars = 66pt; at 40pt it wraps into two lines of 30.
        let m = measure("hello world", AxisOffer::Definite(40.0), 0);
        assert_eq!(m.width, 30.0);
        assert_eq!(m.height, 24.0);
    }

    #[test]
    fn number_of_lines_caps_height() {
        let m = measure("a b c d e f", AxisOffer::Definite(6.0), 2);
        assert_eq!(m.height, 24.0);
    }

    #[test]
    fn min_content_is_the_longest_word() {
        let m = measure("ab cdef g", AxisOffer::MinContent, 0);
        assert_eq!(m.width, 24.0);
        assert_eq!(m.height, 36.0);
    }

    fn measure_in(text: &str, width: AxisOffer, white_space: crate::WhiteSpace) -> TextMetrics {
        let runs = [TextRun {
            text: text.into(),
            style: style(10.0),
        }];
        let mut p = paragraph();
        p.white_space = white_space;
        MonospaceMeasurer::default().measure(&TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: p,
            width,
            height: AxisOffer::MaxContent,
        })
    }

    #[test]
    fn explicit_newlines_break_only_preserved_lines() {
        let m = measure_in(
            "a\nbb\nccc",
            AxisOffer::MaxContent,
            crate::WhiteSpace::PreWrap,
        );
        assert_eq!(m.width, 18.0);
        assert_eq!(m.height, 36.0);
        // `normal` collapses segment breaks to spaces, as the browser does.
        let m = measure_in(
            "a\nbb\nccc",
            AxisOffer::MaxContent,
            crate::WhiteSpace::Normal,
        );
        assert_eq!(m.width, 48.0);
        assert_eq!(m.height, 12.0);
        let m = measure_in(
            "  a \t b  ",
            AxisOffer::MaxContent,
            crate::WhiteSpace::Normal,
        );
        assert_eq!(m.width, 18.0);
    }

    #[test]
    fn pre_line_breaks_at_line_feeds_and_collapses_the_rest() {
        // @ref LLP 1053 §0 G5 — max-content is the longest forced line,
        // min-content the longest word.
        let text = "  one   two  \n  three";
        let m = measure_in(text, AxisOffer::MaxContent, crate::WhiteSpace::PreLine);
        assert_eq!((m.width, m.height), (42.0, 24.0));
        let m = measure_in(text, AxisOffer::MinContent, crate::WhiteSpace::PreLine);
        assert_eq!((m.width, m.height), (30.0, 36.0));
        let m = measure_in("a\n\nb", AxisOffer::MaxContent, crate::WhiteSpace::PreLine);
        assert_eq!((m.width, m.height), (6.0, 36.0));
    }

    #[test]
    fn nowrap_min_content_is_the_unwrapped_line() {
        // @ref LLP 1053 §0 G5
        for width in [
            AxisOffer::MinContent,
            AxisOffer::Definite(10.0),
            AxisOffer::MaxContent,
        ] {
            let m = measure_in("ab  cdef\ng", width, crate::WhiteSpace::Nowrap);
            assert_eq!((m.width, m.height), (54.0, 12.0));
        }
    }

    #[test]
    fn pre_keeps_spaces_and_breaks_only_at_line_feeds() {
        // @ref LLP 1053 G5 — preserve × nowrap: min-content is max-content.
        for width in [
            AxisOffer::MinContent,
            AxisOffer::Definite(10.0),
            AxisOffer::MaxContent,
        ] {
            let m = measure_in("ab  cdef\n g", width, crate::WhiteSpace::Pre);
            assert_eq!((m.width, m.height), (48.0, 24.0));
        }
    }

    #[test]
    fn empty_text_measures_zero() {
        let m = measure("", AxisOffer::MaxContent, 0);
        assert_eq!(m, TextMetrics::default());
    }

    #[test]
    fn runs_take_the_tallest_line_height() {
        let runs = [
            TextRun {
                text: "ab".into(),
                style: style(10.0),
            },
            TextRun {
                text: "cd".into(),
                style: style(20.0),
            },
        ];
        let m = MonospaceMeasurer::default().measure(&TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: paragraph(),
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
        });
        assert_eq!(m.width, 12.0 + 24.0);
        assert_eq!(m.height, 24.0);
    }

    #[test]
    fn overflow_wrap_changes_emergency_breaks_and_only_anywhere_changes_min_content() {
        let runs = [TextRun {
            text: "abcdefghijklmnopqrst".into(),
            style: style(10.0),
        }];
        let mut request = TextMeasureRequest {
            exclusions: &[],
            runs: &runs,
            paragraph: paragraph(),
            width: AxisOffer::Definite(30.0),
            height: AxisOffer::MaxContent,
        };
        let mut measurer = MonospaceMeasurer::default();
        assert_eq!(measurer.measure(&request).height, 12.0);
        for mode in [OverflowWrap::BreakWord, OverflowWrap::Anywhere] {
            request.paragraph.overflow_wrap = mode;
            request.width = AxisOffer::Definite(30.0);
            assert_eq!(measurer.measure(&request).height, 48.0);
            assert_eq!(measurer.measure(&request).width, 30.0);
            request.width = AxisOffer::MinContent;
            assert_eq!(
                measurer.measure(&request).width,
                if mode == OverflowWrap::Anywhere {
                    6.0
                } else {
                    120.0
                }
            );
        }
    }
}
