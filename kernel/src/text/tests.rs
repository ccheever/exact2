use super::*;

fn style(font_size: f32) -> TextStyle {
    TextStyle {
        font_size,
        font_weight: 400,
        font_style: FontStyle::Normal,
        font_stretch: 100.0,
        font_variations: Default::default(),
        font_optical_sizing: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
        atoms: Default::default(),
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
