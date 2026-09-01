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

use crate::generated::{Direction, FontStyle, StyleProps, TextAlign, TextOverflow};
use crate::id::AxisOffer;

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
    /// Line height in points; 0 means the font's natural line height.
    pub line_height: f32,
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
            line_height: s.line_height,
            letter_spacing: s.letter_spacing,
            font_variant_numeric: s.font_variant_numeric,
        }
    }
}

/// Paragraph-level style: what applies to the whole measured block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paragraph {
    /// Base direction.
    pub direction: Direction,
    /// Horizontal alignment.
    pub text_align: TextAlign,
    /// Maximum lines; 0 means unlimited.
    pub line_clamp: u32,
    /// How an over-long last line is truncated.
    pub text_overflow: TextOverflow,
}

impl Paragraph {
    /// The paragraph style carried by a node's style rows.
    pub fn from_style(s: &StyleProps) -> Self {
        Paragraph {
            direction: s.direction,
            text_align: s.text_align,
            line_clamp: s.line_clamp,
            text_overflow: s.text_overflow,
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
}

/// Deterministic reference measurer: every glyph advances `advance_em` ems
/// plus letter spacing; a line is `line_height_em` ems unless the style sets
/// one. Wraps greedily at whitespace and breaks over-long words by character.
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
        if style.line_height > 0.0 {
            style.line_height
        } else {
            style.font_size * self.line_height_em
        }
    }

    fn advance(&self, style: &TextStyle) -> f32 {
        style.font_size * self.advance_em + style.letter_spacing
    }
}

#[derive(Debug, Clone, Copy)]
enum Token {
    Word { width: f32 },
    Space { width: f32 },
    Break,
}

impl TextMeasurer for MonospaceMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        // Tokenize across runs. A word may span runs; its width accumulates.
        let mut tokens: Vec<Token> = Vec::new();
        let mut line_height = 0f32;
        let mut pending_word: Option<f32> = None;
        let mut any_text = false;
        for run in request.runs {
            line_height = line_height.max(self.line_height(&run.style));
            let advance = self.advance(&run.style);
            for ch in run.text.chars() {
                any_text = true;
                if ch == '\n' {
                    if let Some(w) = pending_word.take() {
                        tokens.push(Token::Word { width: w });
                    }
                    tokens.push(Token::Break);
                } else if ch.is_whitespace() {
                    if let Some(w) = pending_word.take() {
                        tokens.push(Token::Word { width: w });
                    }
                    tokens.push(Token::Space { width: advance });
                } else {
                    *pending_word.get_or_insert(0.0) += advance;
                }
            }
        }
        if let Some(w) = pending_word.take() {
            tokens.push(Token::Word { width: w });
        }
        if !any_text {
            return TextMetrics::default();
        }

        // Lay lines out under the offer.
        let limit = match request.width {
            AxisOffer::Definite(w) => Some(w.max(0.0)),
            AxisOffer::MaxContent => None,
            AxisOffer::MinContent => Some(0.0),
        };
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
                Token::Word { width } => {
                    let Some(current) = lines.last_mut() else {
                        continue;
                    };
                    match limit {
                        Some(max) if *current > 0.0 && *current + width > max => {
                            // Drop trailing spaces from the wrapped line, start a new one.
                            *current -= trailing_space;
                            trailing_space = 0.0;
                            lines.push(width);
                        }
                        _ => {
                            *current += width;
                            trailing_space = 0.0;
                        }
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
            line_height: 0.0,
            letter_spacing: 0.0,
            font_variant_numeric: 0,
        }
    }

    fn paragraph() -> Paragraph {
        Paragraph {
            direction: Direction::Ltr,
            text_align: TextAlign::Left,
            line_clamp: 0,
            text_overflow: TextOverflow::Clip,
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

    #[test]
    fn explicit_newlines_break_lines() {
        let m = measure("a\nbb\nccc", AxisOffer::MaxContent, 0);
        assert_eq!(m.width, 18.0);
        assert_eq!(m.height, 36.0);
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
            runs: &runs,
            paragraph: paragraph(),
            width: AxisOffer::MaxContent,
            height: AxisOffer::MaxContent,
        });
        assert_eq!(m.width, 12.0 + 24.0);
        assert_eq!(m.height, 24.0);
    }
}
