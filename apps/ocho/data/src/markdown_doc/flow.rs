//! Inline flow (markdown/flow.rs): how upstream lays out a paragraph that
//! holds inline formulas. Text stays one `StyledText` wherever a paragraph
//! has no formula; with one, the paragraph is cut into word tokens (each
//! word with the whitespace after it, a hard break as its own token) and
//! formula tokens, measured, and wrapped greedily by [`wrap`]. A line is as
//! tall as its tallest ascent plus its deepest descent, never less than the
//! line height; text on a line shares one baseline with the formulas.
//! Adjacent text tokens on a line merge into one run, so a host draws a
//! line of words as one text element, not one per word.
//!
//! Measuring is the host's: text tokens take the shaped widths of their
//! byte ranges and the font's ascent and descent plus half the leading
//! (`(line height − ascent − descent) / 2`) each; formula tokens take the
//! formula's own width, height and baseline. A text word wider than the
//! column is split into graphemes before wrapping; a formula never splits
//! and is clamped to the column (it scrolls inside its box).

use std::ops::Range;

/// One measured piece of a paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    /// The piece's byte range in the paragraph's text (a formula's object
    /// replacement character, `U+FFFC`, upstream).
    pub range: Range<usize>,
    /// Its width, px.
    pub width: f32,
    /// Height above the baseline, px.
    pub ascent: f32,
    /// Depth below the baseline, px.
    pub descent: f32,
    /// The formula's index, for a formula token.
    pub math: Option<usize>,
    /// Whitespace only: dropped at the start of a line.
    pub whitespace: bool,
    /// A hard break.
    pub newline: bool,
}

/// One placed piece: a run of text, or a formula.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// The text's byte range.
    pub range: Range<usize>,
    /// Left edge, px from the paragraph's.
    pub x: f32,
    /// Top edge, px from the paragraph's.
    pub y: f32,
    /// Width, px.
    pub width: f32,
    /// Height (ascent + descent), px.
    pub height: f32,
    /// The formula's index, for a formula.
    pub math: Option<usize>,
}

/// A wrapped paragraph.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// The column's width, px.
    pub width: f32,
    /// The paragraph's height, px.
    pub height: f32,
    /// What to draw where, in order.
    pub placements: Vec<Placement>,
}

/// The word tokens of `text[range]` before measuring: (range, whitespace,
/// newline). Each word keeps the whitespace after it; a `\n` ends its word
/// and becomes a newline token of its own (flow.rs `add_text_tokens`).
pub fn words(text: &str, range: Range<usize>) -> Vec<(Range<usize>, bool, bool)> {
    let mut out = Vec::new();
    let mut start = range.start;
    for word in text[range].split_inclusive(char::is_whitespace) {
        let end = start + word.trim_end_matches('\n').len();
        if start < end {
            out.push((start..end, word.trim().is_empty(), false));
        }
        if word.ends_with('\n') {
            out.push((end..end + 1, true, true));
        }
        start += word.len();
    }
    out
}

/// Wrap `tokens` into lines `width` wide (flow.rs `wrap`).
pub fn wrap(tokens: &[Token], width: f32, line_height: f32) -> Layout {
    let mut layout = Layout {
        width,
        ..Default::default()
    };
    let mut line: Vec<(Token, f32)> = Vec::new();
    let mut x = 0.0;
    for token in tokens {
        if token.newline {
            if line.is_empty() {
                layout.height += line_height;
            }
            flush(&mut line, &mut layout, line_height);
            x = 0.0;
            continue;
        }
        if x > 0.0 && x + token.width > width {
            flush(&mut line, &mut layout, line_height);
            x = 0.0;
        }
        if x == 0.0 && token.whitespace {
            continue;
        }
        let mut token = token.clone();
        token.width = token.width.min(width);
        let next_x = x + token.width;
        line.push((token, x));
        x = next_x;
    }
    flush(&mut line, &mut layout, line_height);
    layout
}

fn flush(line: &mut Vec<(Token, f32)>, layout: &mut Layout, line_height: f32) {
    if line.is_empty() {
        return;
    }
    let ascent = line.iter().map(|(t, _)| t.ascent).fold(0.0, f32::max);
    let descent = line.iter().map(|(t, _)| t.descent).fold(0.0, f32::max);
    for (token, x) in line.drain(..) {
        let y = layout.height + ascent - token.ascent;
        let merge = layout.placements.last().is_some_and(|last| {
            token.math.is_none()
                && last.math.is_none()
                && last.range.end == token.range.start
                && last.y == y
        });
        if merge {
            if let Some(last) = layout.placements.last_mut() {
                last.range.end = token.range.end;
                last.width += token.width;
            }
        } else {
            layout.placements.push(Placement {
                range: token.range,
                x,
                y,
                width: token.width,
                height: token.ascent + token.descent,
                math: token.math,
            });
        }
    }
    layout.height += (ascent + descent).max(line_height);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(range: Range<usize>, width: f32) -> Token {
        Token {
            range,
            width,
            ascent: 17.0,
            descent: 6.0,
            math: None,
            whitespace: false,
            newline: false,
        }
    }

    #[test]
    fn narrow_list_items_wrap_whole_words_and_account_for_tall_math() {
        let tokens = vec![
            word(0..3, 28.0),
            Token {
                math: Some(0),
                ascent: 25.0,
                descent: 10.0,
                ..word(3..6, 32.0)
            },
            word(6..12, 50.0),
        ];
        let layout = wrap(&tokens, 80.0, 23.0);
        assert_eq!(layout.height, 58.0);
        assert_eq!(layout.placements.len(), 3);
        let text = &layout.placements[0];
        let math = &layout.placements[1];
        let next_line = &layout.placements[2];
        assert_eq!(text.width, 28.0);
        assert_eq!(text.y + 17.0, math.y + 25.0);
        assert!(next_line.y >= math.y + math.height);
        for placed in &layout.placements {
            assert!(placed.x + placed.width <= layout.width);
            assert!(placed.y + placed.height <= layout.height);
        }
    }

    #[test]
    fn text_is_batched_into_line_runs_instead_of_per_word_elements() {
        let tokens: Vec<_> = (0..100).map(|i| word(i * 5..(i + 1) * 5, 5.0)).collect();
        let layout = wrap(&tokens, 100.0, 23.0);
        assert_eq!(layout.placements.len(), 5);
        assert_eq!(layout.height, 115.0);
        assert_eq!(
            layout
                .placements
                .iter()
                .map(|p| p.range.len())
                .sum::<usize>(),
            500
        );
    }

    #[test]
    fn explicit_breaks_and_wide_formulas_reserve_their_height() {
        let tokens = vec![
            word(0..3, 28.0),
            Token {
                newline: true,
                ..word(3..4, 0.0)
            },
            Token {
                math: Some(0),
                ascent: 35.0,
                descent: 10.0,
                ..word(4..7, 400.0)
            },
            word(7..10, 20.0),
        ];
        let layout = wrap(&tokens, 80.0, 23.0);
        assert_eq!(layout.height, 91.0);
        assert_eq!(layout.placements[1].width, 80.0);
        assert_eq!(layout.placements[2].y, 68.0);
    }

    #[test]
    fn words_keep_trailing_space_and_split_hard_breaks() {
        let text = "ab cd\nef";
        assert_eq!(
            words(text, 0..text.len()),
            vec![
                (0..3, false, false),
                (3..5, false, false),
                (5..6, true, true),
                (6..8, false, false),
            ]
        );
        assert_eq!(
            words("  x", 0..3),
            vec![
                (0..1, true, false),
                (1..2, true, false),
                (2..3, false, false)
            ]
        );
    }
}
