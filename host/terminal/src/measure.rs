//! Text in cells: one wrap, shared by the measurer and the painter.
//!
//! @ref LLP 1101 D3 — whole columns × rows from a pinned grapheme and width
//! policy (UAX #29 clusters, East Asian wide = 2, ambiguous = 1). The kernel
//! sizes a paragraph by [`CellMeasurer`]; the painter re-runs [`wrap`] over
//! the same runs at the same width, so the lines it draws are the lines that
//! were measured.

use exact_kernel::style::cells::{COLUMN, ROW};
use exact_kernel::text::Paragraph;
use exact_kernel::{
    AxisOffer, ButtonFaceStyle, ButtonMeasure, ButtonMeasureRequest, OverflowWrap, PressFace,
    TextMeasureRequest, TextMeasurer, TextMetrics, TextRun,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// One grapheme cluster on a line: its text, its columns, and the run it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The cluster.
    pub text: String,
    /// Columns it takes: 1, or 2 for a wide one.
    pub cols: usize,
    /// Index into the paragraph's runs.
    pub run: usize,
}

/// One laid-out line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    /// Its clusters, in order.
    pub glyphs: Vec<Glyph>,
}

impl Line {
    /// Columns the line takes.
    pub fn cols(&self) -> usize {
        self.glyphs.iter().map(|g| g.cols).sum()
    }
}

enum Token {
    Word(Vec<Glyph>),
    Space(Glyph),
    Break,
}

/// The columns a width in layout pixels holds, forgiving float error.
pub fn columns(px: f32) -> usize {
    ((px / COLUMN) + 0.01).floor().max(0.0) as usize
}

/// Break the runs into lines under `limit` columns (`None`: no wrapping).
pub fn wrap(
    runs: &[TextRun<'_>],
    paragraph: &Paragraph,
    limit: Option<usize>,
    min_content: bool,
) -> Vec<Line> {
    let white_space = paragraph.white_space.model();
    let collapsed = exact_textflow::collapse(runs, white_space);
    let mut tokens = Vec::new();
    let mut word: Vec<Glyph> = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        let text = collapsed
            .as_ref()
            .map_or(&*run.text, |c| c.runs[index].as_str());
        for cluster in text.graphemes(true) {
            if cluster == "\n" || cluster == "\r\n" {
                if !word.is_empty() {
                    tokens.push(Token::Word(std::mem::take(&mut word)));
                }
                tokens.push(Token::Break);
            } else if cluster.chars().all(char::is_whitespace)
                && !matches!(cluster, "\u{a0}" | "\u{202f}" | "\u{2007}" | "\u{feff}")
            {
                if !word.is_empty() {
                    tokens.push(Token::Word(std::mem::take(&mut word)));
                }
                tokens.push(Token::Space(Glyph {
                    text: " ".into(),
                    cols: 1,
                    run: index,
                }));
            } else {
                // A control character is never drawn as itself (LLP 1101 D7).
                let (text, cols) = if cluster.chars().any(char::is_control) {
                    ("\u{fffd}".to_string(), 1)
                } else {
                    (cluster.to_string(), cluster.width().max(1))
                };
                word.push(Glyph {
                    text,
                    cols,
                    run: index,
                });
            }
        }
    }
    if !word.is_empty() {
        tokens.push(Token::Word(word));
    }
    let limit = if white_space.wraps() { limit } else { None };
    // `pre` and `pre-wrap` keep every space, leading and trailing.
    let keeps = white_space.preserves();
    let breaks_words = paragraph.overflow_wrap != OverflowWrap::Normal
        && (!min_content || paragraph.overflow_wrap == OverflowWrap::Anywhere);
    let mut lines = vec![Line::default()];
    let mut trailing = 0usize;
    for token in tokens {
        match token {
            Token::Break => {
                if !keeps {
                    trim(lines.last_mut().expect("a line"), trailing);
                }
                lines.push(Line::default());
                trailing = 0;
            }
            Token::Space(glyph) => {
                let line = lines.last_mut().expect("a line");
                if keeps || !line.glyphs.is_empty() {
                    line.glyphs.push(glyph);
                    trailing += 1;
                }
            }
            Token::Word(glyphs) => {
                let width: usize = glyphs.iter().map(|g| g.cols).sum();
                let current = lines.last().expect("a line").cols();
                if limit.is_some_and(|max| current > 0 && current + width > max) {
                    if !keeps {
                        trim(lines.last_mut().expect("a line"), trailing);
                    }
                    lines.push(Line::default());
                }
                trailing = 0;
                if breaks_words && limit.is_some_and(|max| width > max) {
                    for glyph in glyphs {
                        let current = lines.last().expect("a line").cols();
                        if limit.is_some_and(|max| current > 0 && current + glyph.cols > max) {
                            lines.push(Line::default());
                        }
                        lines.last_mut().expect("a line").glyphs.push(glyph);
                    }
                } else {
                    lines.last_mut().expect("a line").glyphs.extend(glyphs);
                }
            }
        }
    }
    if !keeps {
        trim(lines.last_mut().expect("a line"), trailing);
    }
    let clamp = paragraph.line_clamp as usize;
    if clamp > 0 && lines.len() > clamp {
        lines.truncate(clamp);
    }
    lines
}

fn trim(line: &mut Line, trailing: usize) {
    let keep = line.glyphs.len().saturating_sub(trailing);
    line.glyphs.truncate(keep);
}

/// LLP 1104 D7: the terminal drops the symbol and paints the title, or
/// the accessible label of a symbol-only face. Measurement and paint share
/// this wrap, with one cell of chrome on each side.
pub(crate) fn button_lines(
    face: &PressFace,
    style: &ButtonFaceStyle,
    width: AxisOffer,
) -> Vec<Line> {
    let title = face
        .title
        .as_deref()
        .or(face.label.as_deref())
        .unwrap_or("");
    let runs = [TextRun {
        text: title.into(),
        style: exact_kernel::text::TextStyle::from_style(&style.title),
    }];
    let (limit, min) = match width {
        AxisOffer::Definite(w) => (Some(columns(w).saturating_sub(2)), false),
        AxisOffer::MinContent => (Some(0), true),
        AxisOffer::MaxContent => (None, false),
    };
    wrap(&runs, &Paragraph::from_style(&style.title), limit, min)
}

/// The kernel's measurer: every cluster is a column (two if wide), every
/// line a row, whatever the font size — the terminal's face is the user's.
#[derive(Debug, Default)]
pub struct CellMeasurer;

impl TextMeasurer for CellMeasurer {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if request.runs.iter().all(|r| r.text.is_empty()) {
            return TextMetrics::default();
        }
        let (limit, min) = match request.width {
            AxisOffer::Definite(w) => (Some(columns(w)), false),
            AxisOffer::MaxContent => (None, false),
            AxisOffer::MinContent => (Some(0), true),
        };
        let lines = wrap(request.runs, &request.paragraph, limit, min);
        let cols = lines.iter().map(Line::cols).max().unwrap_or(0);
        TextMetrics {
            width: cols as f32 * COLUMN,
            height: lines.len() as f32 * ROW,
            first_baseline: Some(ROW * 0.75),
        }
    }

    fn button_measure(&mut self, request: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
        let lines = button_lines(&request.face, &request.style, request.width);
        Some(ButtonMeasure {
            width: (lines.iter().map(Line::cols).max().unwrap_or(0) + 2) as f32 * COLUMN,
            height: lines.len() as f32 * ROW,
            provisional: false,
        })
    }

    fn height_free(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_kernel::{StyleProps, WhiteSpace};

    #[test]
    fn nonbreaking_spaces_stay_in_the_word() {
        let mut paragraph = Paragraph::from_style(&StyleProps::default());
        for space in ['\u{a0}', '\u{202f}', '\u{2007}', '\u{feff}'] {
            let text = format!("XX{space}YY");
            let runs = [TextRun {
                text: text.as_str().into(),
                style: paragraph.strut,
            }];
            for white_space in [
                WhiteSpace::Normal,
                WhiteSpace::Nowrap,
                WhiteSpace::Pre,
                WhiteSpace::PreWrap,
                WhiteSpace::PreLine,
            ] {
                paragraph.white_space = white_space;
                paragraph.overflow_wrap = OverflowWrap::Normal;
                let lines = wrap(&runs, &paragraph, Some(4), false);
                assert_eq!(lines.len(), 1, "{space:?}, {white_space:?}");
                assert_eq!(lines[0].cols(), 5);
                assert_eq!(lines[0].glyphs[2].text, space.to_string());
                paragraph.overflow_wrap = OverflowWrap::Anywhere;
                let lines = wrap(&runs, &paragraph, Some(4), false);
                if matches!(white_space, WhiteSpace::Nowrap | WhiteSpace::Pre) {
                    assert_eq!(lines.len(), 1);
                    assert_eq!(lines[0].cols(), 5);
                } else {
                    assert_eq!(lines.len(), 2);
                    assert_eq!(lines[0].cols(), 4);
                    assert_eq!(lines[1].cols(), 1);
                }
            }
        }
    }

    #[test]
    fn ordinary_spaces_tabs_and_newlines_keep_their_breaks() {
        let mut paragraph = Paragraph::from_style(&StyleProps::default());
        for (text, white_space) in [
            ("XX YY", WhiteSpace::Normal),
            ("XX\tYY", WhiteSpace::PreWrap),
            ("XX\nYY", WhiteSpace::PreWrap),
        ] {
            paragraph.white_space = white_space;
            let runs = [TextRun {
                text: text.into(),
                style: paragraph.strut,
            }];
            let lines = wrap(&runs, &paragraph, Some(4), false);
            assert_eq!(lines.len(), 2, "{text:?}");
            assert!(lines[0].cols() >= 2, "{text:?}");
            assert_eq!(
                lines[1]
                    .glyphs
                    .iter()
                    .map(|g| g.text.as_str())
                    .collect::<String>(),
                "YY"
            );
        }
    }

    #[test]
    fn unicode_spaces_and_preserved_carriage_returns_break() {
        let mut paragraph = Paragraph::from_style(&StyleProps::default());
        for (text, white_space, expected) in [
            ("XX\u{2009}YY", WhiteSpace::Normal, "YY"),
            ("XX\u{3000}YY", WhiteSpace::Normal, "YY"),
            ("XX\rYY", WhiteSpace::PreWrap, "YY"),
        ] {
            paragraph.white_space = white_space;
            let runs = [TextRun {
                text: text.into(),
                style: paragraph.strut,
            }];
            let lines = wrap(&runs, &paragraph, Some(4), false);
            assert_eq!(lines.len(), 2, "{text:?}");
            assert_eq!(
                lines[1]
                    .glyphs
                    .iter()
                    .map(|g| g.text.as_str())
                    .collect::<String>(),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn original_tokenizer_keeps_unicode_space_and_crlf_behavior() {
        let mut paragraph = Paragraph::from_style(&StyleProps::default());
        for space in ['\u{2009}', '\u{3000}'] {
            let source = format!("XX{space}YY");
            let runs = [TextRun {
                text: source.as_str().into(),
                style: paragraph.strut,
            }];
            let lines = wrap(&runs, &paragraph, Some(4), false);
            assert_eq!(
                lines.iter().map(|line| line.cols()).collect::<Vec<_>>(),
                [2, 2],
                "{space:?}"
            );
            assert_eq!(
                lines[0]
                    .glyphs
                    .iter()
                    .map(|g| g.text.as_str())
                    .collect::<String>(),
                "XX"
            );
            assert_eq!(
                lines[1]
                    .glyphs
                    .iter()
                    .map(|g| g.text.as_str())
                    .collect::<String>(),
                "YY"
            );
        }
        paragraph.white_space = WhiteSpace::PreWrap;
        let runs = [TextRun {
            text: "A\r\nB".into(),
            style: paragraph.strut,
        }];
        let lines = wrap(&runs, &paragraph, Some(80), false);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].glyphs[0].text, "A");
        assert_eq!(lines[1].glyphs[0].text, "B");
    }
}
