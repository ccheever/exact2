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

use crate::generated::{Direction, FontStyle, OverflowWrap, StyleProps, TextAlign, TextOverflow};
use crate::id::AxisOffer;
use crate::id::NodeKey;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

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
}

impl TextStyle {
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
        }
    }
}

/// One styled run of text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRun<'a> {
    /// The text.
    pub text: &'a str,
    /// Its style.
    pub style: TextStyle,
}

impl AsRef<str> for TextRun<'_> {
    fn as_ref(&self) -> &str {
        self.text
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

/// A host's text engine, injected per kernel.
pub trait TextMeasurer {
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
            text.push_str(run.text);
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
            first_baseline: Some(line_height * self.baseline_frac),
        }
    }
}

impl TextMeasurer for MonospaceMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
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
                .map_or(run.text, |c| c.runs[index].as_str());
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
        }
    }

    fn measure(text: &str, width: AxisOffer, lines: u32) -> TextMetrics {
        let runs = [TextRun {
            text,
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
            text,
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
    fn empty_text_measures_zero() {
        let m = measure("", AxisOffer::MaxContent, 0);
        assert_eq!(m, TextMetrics::default());
    }

    #[test]
    fn runs_take_the_tallest_line_height() {
        let runs = [
            TextRun {
                text: "ab",
                style: style(10.0),
            },
            TextRun {
                text: "cd",
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
            text: "abcdefghijklmnopqrst",
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
