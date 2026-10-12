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
    AxisOffer, ButtonFaceStyle, ButtonMeasure, ButtonMeasureRequest, OverflowWrap, ParagraphStamp,
    PressFace, TextMeasureRequest, TextMeasurer, TextMetrics, TextRun,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// One grapheme cluster on a line: its text, its columns, and the run it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The cluster.
    pub text: smol_str::SmolStr,
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
    layout::<true>(runs, paragraph, limit, min_content).lines
}

// Measurement and paint take the same breaks. Only paint owns glyphs;
// measurement keeps the current width and the largest completed line.
struct Wrapped<const PAINT: bool> {
    lines: Vec<Line>,
    cols: usize,
    width: usize,
    rows: usize,
    trailing: usize,
    clamp: usize,
    keeps: bool,
    breaks_words: bool,
    limit: Option<usize>,
}

#[derive(Clone, Copy)]
struct Cluster<'a> {
    text: &'a str,
    cols: usize,
    run: usize,
}

impl<const PAINT: bool> Wrapped<PAINT> {
    fn included(&self) -> bool {
        self.clamp == 0 || self.rows <= self.clamp
    }

    fn push(&mut self, glyph: Cluster<'_>) {
        self.cols += glyph.cols;
        if PAINT && self.included() {
            self.lines.last_mut().unwrap().glyphs.push(Glyph {
                text: glyph.text.into(),
                cols: glyph.cols,
                run: glyph.run,
            });
        }
    }

    fn finish_line(&mut self) {
        if !self.keeps {
            self.cols -= self.trailing;
            if PAINT && self.included() {
                let glyphs = &mut self.lines.last_mut().unwrap().glyphs;
                glyphs.truncate(glyphs.len() - self.trailing);
            }
        }
        if self.included() {
            self.width = self.width.max(self.cols);
        }
        self.trailing = 0;
    }

    fn newline(&mut self) {
        self.finish_line();
        self.rows += 1;
        self.cols = 0;
        if PAINT && self.included() {
            self.lines.push(Line::default());
        }
    }

    fn word(&mut self, word: &mut Vec<Cluster<'_>>) {
        if word.is_empty() {
            return;
        }
        let width: usize = word.iter().map(|g| g.cols).sum();
        if self
            .limit
            .is_some_and(|max| self.cols > 0 && self.cols + width > max)
        {
            self.newline();
        }
        self.trailing = 0;
        let split = self.breaks_words && self.limit.is_some_and(|max| width > max);
        for glyph in word.drain(..) {
            if split
                && self
                    .limit
                    .is_some_and(|max| self.cols > 0 && self.cols + glyph.cols > max)
            {
                self.newline();
            }
            self.push(glyph);
        }
    }
}

fn layout<const PAINT: bool>(
    runs: &[TextRun<'_>],
    paragraph: &Paragraph,
    limit: Option<usize>,
    min_content: bool,
) -> Wrapped<PAINT> {
    let white_space = paragraph.white_space.model();
    let collapsed = exact_textflow::collapse(runs, white_space);
    let mut out = Wrapped {
        lines: if PAINT {
            vec![Line::default()]
        } else {
            Vec::new()
        },
        cols: 0,
        width: 0,
        rows: 1,
        trailing: 0,
        clamp: paragraph.line_clamp as usize,
        keeps: white_space.preserves(),
        breaks_words: paragraph.overflow_wrap != OverflowWrap::Normal
            && (!min_content || paragraph.overflow_wrap == OverflowWrap::Anywhere),
        limit: if white_space.wraps() { limit } else { None },
    };
    // Reuse one word buffer. A word may span styled runs; whitespace
    // flushes it directly into the current line, without a token list.
    let mut word = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        let text = collapsed
            .as_ref()
            .map_or(&*run.text, |c| c.runs[index].as_str());
        clusters(text, |cluster| {
            if cluster == "\n" || cluster == "\r\n" {
                out.word(&mut word);
                out.newline();
            } else if breakable_space(cluster) {
                out.word(&mut word);
                if out.keeps || out.cols > 0 {
                    out.push(Cluster {
                        text: " ",
                        cols: 1,
                        run: index,
                    });
                    out.trailing += 1;
                }
            } else {
                // A control character is never drawn as itself (LLP 1101 D7).
                let (text, cols) = if cluster.chars().any(char::is_control) {
                    ("\u{fffd}", 1)
                } else {
                    (cluster, cluster.width().max(1))
                };
                let glyph = Cluster {
                    text,
                    cols,
                    run: index,
                };
                if out.limit.is_none() {
                    // Without wrapping, a word needs no lookahead or scratch storage.
                    out.trailing = 0;
                    out.push(glyph);
                } else {
                    word.push(glyph);
                }
            }
        });
    }
    out.word(&mut word);
    out.finish_line();
    if out.clamp > 0 {
        out.rows = out.rows.min(out.clamp);
    }
    out
}

// Styled terminal runs often contain just one scalar (coloured animation
// cells), which is already one grapheme. ASCII has only one multi-byte cluster:
// CR LF. All other text stays with the pinned Unicode segmenter.
fn clusters<'a>(text: &'a str, mut each: impl FnMut(&'a str)) {
    if text.chars().nth(1).is_none() {
        if !text.is_empty() {
            each(text);
        }
    } else if text.is_ascii() {
        let mut rest = text;
        while !rest.is_empty() {
            let len = if rest.starts_with("\r\n") { 2 } else { 1 };
            let (cluster, tail) = rest.split_at(len);
            each(cluster);
            rest = tail;
        }
    } else {
        for cluster in text.graphemes(true) {
            each(cluster);
        }
    }
}

// The non-wrapping, space-preserving case can paint directly into cells.
// Keep the same cluster, control and width policy as layout; no glyph list
// or owned copy of a cluster is needed when alignment needs no lookahead.
pub(crate) fn visit_preserved(
    runs: &[TextRun<'_>],
    mut each: impl FnMut(usize, usize, &str, usize, usize),
) {
    let (mut row, mut col) = (0, 0);
    for (run, source) in runs.iter().enumerate() {
        clusters(&source.text, |cluster| {
            if cluster == "\n" || cluster == "\r\n" {
                row += 1;
                col = 0;
                return;
            }
            let (text, cols) = if breakable_space(cluster) {
                (" ", 1)
            } else if cluster.chars().any(char::is_control) {
                ("\u{fffd}", 1)
            } else {
                (cluster, cluster.width().max(1))
            };
            each(row, col, text, cols, run);
            col += cols;
        });
    }
}

fn breakable_space(cluster: &str) -> bool {
    cluster.chars().all(char::is_whitespace)
        && !matches!(cluster, "\u{a0}" | "\u{202f}" | "\u{2007}" | "\u{feff}")
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
#[derive(Debug)]
pub struct CellMeasurer {
    // Non-wrapping sizes are independent of either offered axis. Bound the
    // storage regardless of document length; collisions only cause remeasurement.
    // Stamps retain no text and include the kernel, allocation and metric revision.
    unwrapped: [Option<(ParagraphStamp, TextMetrics)>; 128],
}

impl Default for CellMeasurer {
    fn default() -> Self {
        Self {
            unwrapped: std::array::from_fn(|_| None),
        }
    }
}

impl TextMeasurer for CellMeasurer {
    fn measure_identified(
        &mut self,
        stamp: &ParagraphStamp,
        request: &TextMeasureRequest<'_>,
    ) -> TextMetrics {
        let metrics = self.measure(request);
        let at = stamp.owner().index as usize % self.unwrapped.len();
        self.unwrapped[at] = (!request.paragraph.white_space.model().wraps()
            && request.exclusions.is_empty())
        .then(|| (stamp.clone(), metrics));
        metrics
    }

    fn measure_known(
        &mut self,
        stamp: &ParagraphStamp,
        _width: AxisOffer,
        _height: AxisOffer,
    ) -> Option<TextMetrics> {
        self.unwrapped[stamp.owner().index as usize % self.unwrapped.len()]
            .as_ref()
            .filter(|(known, _)| known.same_metrics(stamp))
            .map(|(_, metrics)| *metrics)
    }

    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if request.runs.iter().all(|r| r.text.is_empty()) {
            return TextMetrics::default();
        }
        let (limit, min) = match request.width {
            AxisOffer::Definite(w) => (Some(columns(w)), false),
            AxisOffer::MaxContent => (None, false),
            AxisOffer::MinContent => (Some(0), true),
        };
        let measured = layout::<false>(request.runs, &request.paragraph, limit, min);
        TextMetrics {
            width: measured.width as f32 * COLUMN,
            height: measured.rows as f32 * ROW,
            first_baseline: Some(ROW * 0.75),
        }
    }

    fn button_measure(&mut self, request: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
        let inset = if request.grouped_row { 16.0 } else { 0.0 };
        let width = match request.width {
            AxisOffer::Definite(w) => AxisOffer::Definite((w - inset).max(0.0)),
            other => other,
        };
        let lines = button_lines(&request.face, &request.style, width);
        Some(ButtonMeasure {
            width: (lines.iter().map(Line::cols).max().unwrap_or(0) + 2) as f32 * COLUMN + inset,
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
    use exact_kernel::{text::TextStyle, StyleProps, WhiteSpace};

    #[test]
    fn unbounded_text_matches_a_wide_wrapping_layout_at_every_run_boundary() {
        let style = StyleProps::default();
        for text in [
            "",
            "  words with   spaces  ",
            "\nfirst\r\nsecond\nthird\n",
            "a\tb\u{1b}c\u{7}d ",
            "界é 👩🏽‍💻 🇺🇸 ｱ\nend  ",
            "a\u{a0}b\u{202f}c\u{2007}d\u{feff}e",
        ] {
            for split in (0..=text.len()).filter(|&i| text.is_char_boundary(i)) {
                let runs = [&text[..split], &text[split..]].map(|text| TextRun {
                    text: text.into(),
                    style: TextStyle::from_style(&style),
                });
                for (space, wrapping) in [
                    (WhiteSpace::Normal, WhiteSpace::Normal),
                    (WhiteSpace::PreWrap, WhiteSpace::PreWrap),
                    (WhiteSpace::Pre, WhiteSpace::PreWrap),
                    (WhiteSpace::Nowrap, WhiteSpace::Normal),
                ] {
                    for clamp in [0, 1, 2] {
                        let mut paragraph = Paragraph::from_style(&style);
                        paragraph.white_space = space;
                        paragraph.line_clamp = clamp;
                        let mut reference = Paragraph::from_style(&style);
                        reference.white_space = wrapping;
                        reference.line_clamp = clamp;
                        let drawn = layout::<true>(&runs, &paragraph, None, false);
                        let measured = layout::<false>(&runs, &paragraph, None, false);
                        let wide = layout::<true>(&runs, &reference, Some(1000), false);
                        assert_eq!(
                            drawn.lines, wide.lines,
                            "{text:?} {split} {space:?} {clamp}"
                        );
                        assert_eq!((measured.width, measured.rows), (wide.width, wide.rows));
                        if space == WhiteSpace::Pre {
                            let mut expected = Vec::new();
                            for (row, line) in wide.lines.iter().enumerate() {
                                let mut col = 0;
                                for glyph in &line.glyphs {
                                    expected.push((
                                        row,
                                        col,
                                        glyph.text.to_string(),
                                        glyph.cols,
                                        glyph.run,
                                    ));
                                    col += glyph.cols;
                                }
                            }
                            let mut streamed = Vec::new();
                            visit_preserved(&runs, |row, col, text, cols, run| {
                                if clamp == 0 || row < clamp as usize {
                                    streamed.push((row, col, text.to_string(), cols, run));
                                }
                            });
                            assert_eq!(streamed, expected, "streamed {text:?} {split} {clamp}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn identified_sizes_match_fresh_measurement_across_offers_edits_and_kernels() {
        use exact_kernel::{Kernel, NodeType, Op, PropId, StyleId, TextTransform};
        let mut cached = CellMeasurer::default();
        for _ in 0..2 {
            let mut kernel = Kernel::with_monospace();
            kernel
                .apply(
                    0,
                    0,
                    &[
                        Op::CreateView {
                            id: 1,
                            node_type: NodeType::Text,
                        },
                        Op::CreateView {
                            id: 2,
                            node_type: NodeType::Text,
                        },
                        Op::CreateView {
                            id: 3,
                            node_type: NodeType::Text,
                        },
                        Op::SetChildren {
                            id: 1,
                            children: vec![2, 3],
                        },
                        Op::SetProp {
                            id: 3,
                            prop: PropId::Text,
                            value: "d 界 é\n👩🏽‍💻 end  ".into(),
                        },
                        Op::AttachRoot { id: 1 },
                    ],
                )
                .unwrap();
            for space in [
                WhiteSpace::Pre,
                WhiteSpace::Nowrap,
                WhiteSpace::PreWrap,
                WhiteSpace::Normal,
            ] {
                for words in ["  wor", "a longer wor", ""] {
                    let mut style = StyleProps {
                        white_space: space,
                        text_transform: TextTransform::Capitalize,
                        line_clamp: 2,
                        ..StyleProps::default()
                    };
                    for row in [
                        StyleId::WhiteSpace,
                        StyleId::TextTransform,
                        StyleId::LineClamp,
                    ] {
                        style.mask.set(row);
                    }
                    kernel
                        .apply(
                            0,
                            0,
                            &[
                                Op::SetStyle {
                                    id: 1,
                                    patch: Box::new(style),
                                },
                                Op::SetProp {
                                    id: 2,
                                    prop: PropId::Text,
                                    value: words.into(),
                                },
                            ],
                        )
                        .unwrap();
                    let node = kernel.node(1).unwrap();
                    let stamp = node.paragraph_stamp().unwrap();
                    let runs = node.text_runs();
                    for (i, width) in [
                        AxisOffer::MinContent,
                        AxisOffer::MaxContent,
                        AxisOffer::Definite(8.0),
                        AxisOffer::Definite(800.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let height = AxisOffer::Definite((i + 1) as f32 * ROW);
                        let request = TextMeasureRequest {
                            runs: &runs,
                            paragraph: node.paragraph(),
                            width,
                            height,
                            exclusions: &[],
                        };
                        let expected = CellMeasurer::default().measure(&request);
                        let known = cached.measure_known(&stamp, width, height);
                        assert_eq!(known.is_some(), i > 0 && !space.model().wraps());
                        let measured =
                            known.unwrap_or_else(|| cached.measure_identified(&stamp, &request));
                        assert_eq!(measured, expected, "{space:?} {words:?} {width:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn identified_sizes_survive_other_paragraphs_and_refuse_evicted_or_reused_slots() {
        use exact_kernel::{Kernel, NodeType, Op, PropId, StyleId};
        let mut cached = CellMeasurer::default();
        let mut kernel = Kernel::with_monospace();
        let mut style = StyleProps {
            white_space: WhiteSpace::Pre,
            ..StyleProps::default()
        };
        style.mask.set(StyleId::WhiteSpace);
        let mut ops = Vec::new();
        for id in 1..=130 {
            ops.extend([
                Op::CreateView {
                    id,
                    node_type: NodeType::Text,
                },
                Op::SetStyle {
                    id,
                    patch: Box::new(style.clone()),
                },
                Op::SetProp {
                    id,
                    prop: PropId::Text,
                    value: "界".repeat(id as usize).into(),
                },
            ]);
        }
        kernel.apply(0, 0, &ops).unwrap();
        for id in 1..=130 {
            let node = kernel.node(id).unwrap();
            let request = TextMeasureRequest {
                runs: &node.text_runs(),
                paragraph: node.paragraph(),
                width: AxisOffer::MaxContent,
                height: AxisOffer::MaxContent,
                exclusions: &[],
            };
            cached.measure_identified(&node.paragraph_stamp().unwrap(), &request);
        }
        let mut hits = 0;
        for id in 1..=130 {
            let node = kernel.node(id).unwrap();
            let request = TextMeasureRequest {
                runs: &node.text_runs(),
                paragraph: node.paragraph(),
                width: AxisOffer::Definite(COLUMN),
                height: AxisOffer::Definite(ROW),
                exclusions: &[],
            };
            if let Some(known) = cached.measure_known(
                &node.paragraph_stamp().unwrap(),
                request.width,
                request.height,
            ) {
                assert_eq!(known, CellMeasurer::default().measure(&request));
                hits += 1;
            }
        }
        assert_eq!(hits, 128);
        let old = kernel.node(130).unwrap().key;
        kernel
            .apply(
                0,
                0,
                &[
                    Op::DestroyView { id: 130 },
                    Op::CreateView {
                        id: 131,
                        node_type: NodeType::Text,
                    },
                ],
            )
            .unwrap();
        let node = kernel.node(131).unwrap();
        assert_eq!(old.index, node.key.index);
        assert_ne!(old.generation, node.key.generation);
        assert!(cached
            .measure_known(
                &node.paragraph_stamp().unwrap(),
                AxisOffer::MaxContent,
                AxisOffer::MaxContent,
            )
            .is_none());
    }

    #[test]
    fn terminal_clusters_match_unicode_for_ascii_controls_and_composed_text() {
        let check = |text: &str| {
            let mut observed = Vec::new();
            clusters(text, |s| observed.push(s.to_string()));
            assert_eq!(
                observed,
                text.graphemes(true).collect::<Vec<_>>(),
                "{text:?}"
            );
        };
        for a in 0..128u8 {
            check(std::str::from_utf8(&[a]).unwrap());
            for b in 0..128u8 {
                check(std::str::from_utf8(&[a, b]).unwrap());
            }
        }
        for text in [
            "",
            "abc\r\ndef\r\n\n",
            "界",
            "ｱ",
            "🧪",
            "\u{301}",
            "e\u{301}",
            "a\u{200d}b",
            "👩🏽‍💻",
            "🇺🇸🇨🇦",
            "क्‍ष",
            "a界é\r\nx",
        ] {
            check(text);
        }
    }

    #[test]
    fn measuring_and_painting_keep_wrapping_whitespace_and_cluster_boundaries() {
        let style = StyleProps::default();
        let runs = ["  ab", "c 界 é\nxy  "].map(|text| TextRun {
            text: text.into(),
            style: TextStyle::from_style(&style),
        });
        for (white_space, limit, clamp, expected) in [
            (WhiteSpace::Normal, Some(4), 0, vec!["abc", "界 é", "xy"]),
            (WhiteSpace::Pre, Some(4), 0, vec!["  abc 界 é", "xy  "]),
            (
                WhiteSpace::PreWrap,
                Some(4),
                0,
                vec!["  ", "abc ", "界 é", "xy  "],
            ),
            (WhiteSpace::Normal, Some(4), 2, vec!["abc", "界 é"]),
        ] {
            let mut paragraph = Paragraph::from_style(&style);
            paragraph.white_space = white_space;
            paragraph.line_clamp = clamp;
            let lines = wrap(&runs, &paragraph, limit, false);
            let text: Vec<String> = lines
                .iter()
                .map(|l| l.glyphs.iter().map(|g| g.text.as_str()).collect())
                .collect();
            assert_eq!(text, expected);
            let measured = CellMeasurer::default().measure(&TextMeasureRequest {
                runs: &runs,
                paragraph,
                width: limit.map_or(AxisOffer::MaxContent, |n| {
                    AxisOffer::Definite(n as f32 * COLUMN)
                }),
                height: AxisOffer::MaxContent,
                exclusions: &[],
            });
            assert_eq!(
                measured.width,
                lines.iter().map(Line::cols).max().unwrap() as f32 * COLUMN
            );
            assert_eq!(measured.height, lines.len() as f32 * ROW);
            assert!(
                lines
                    .iter()
                    .flat_map(|l| &l.glyphs)
                    .any(|g| g.run == 1 && g.text == "c"),
                "one word may span runs"
            );
        }
    }
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
