//! Atomic runs in the deterministic reference shaper.
use super::*;

impl MonospaceMeasurer {
    pub(super) fn inline_layout(
        &self,
        request: &TextMeasureRequest<'_>,
    ) -> (TextMetrics, Vec<InlinePosition>) {
        let collapsed =
            exact_textflow::collapse(request.runs, request.paragraph.white_space.model());
        let mut text = String::new();
        let mut spans = Vec::new();
        for (i, run) in request.runs.iter().enumerate() {
            let start = text.len();
            text.push_str(collapsed.as_ref().map_or(&*run.text, |c| &c.runs[i]));
            spans.push((start, text.len(), run));
        }
        let advance = |range: std::ops::Range<usize>| -> f32 {
            spans
                .iter()
                .map(|(start, end, run)| {
                    let lo = range.start.max(*start);
                    let hi = range.end.min(*end);
                    if lo >= hi {
                        return 0.0;
                    }
                    run.inline_box.map_or_else(
                        || text[lo..hi].chars().count() as f32 * self.advance(&run.style),
                        |b| b.width,
                    )
                })
                .sum()
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
                hyphen_advance: self.advance(&request.paragraph.strut),
            },
            &mut |r| advance(r),
        );
        let width = match request.width {
            AxisOffer::Definite(w) => w,
            AxisOffer::MinContent => prepared.min_content_width(),
            AxisOffer::MaxContent => f32::MAX,
        };
        let mut cursor = exact_textflow::Cursor::default();
        let mut y = 0f32;
        let mut max_width = 0f32;
        let mut first_baseline = None;
        let mut positions = Vec::new();
        let mut lines = 0;
        while let Some(line) = prepared.next_line(cursor, width) {
            if request.paragraph.line_clamp > 0 && lines >= request.paragraph.line_clamp {
                break;
            }
            let h = self.line_height(&request.paragraph.strut);
            let (mut above, mut below) = (h * self.baseline_frac, h * (1.0 - self.baseline_frac));
            for (start, end, run) in &spans {
                if *end <= line.start.byte || *start >= line.end.byte {
                    continue;
                }
                let (a, b) = run.inline_box.map_or_else(
                    || {
                        let h = self.line_height(&run.style);
                        (h * self.baseline_frac, h * (1.0 - self.baseline_frac))
                    },
                    |b| (b.baseline, b.height - b.baseline),
                );
                above = above.max(a);
                below = below.max(b);
            }
            first_baseline.get_or_insert(y + above);
            let align = request
                .paragraph
                .text_align
                .physical(request.paragraph.direction);
            let offset = match align {
                TextAlign::Center => (width - line.width) / 2.0,
                TextAlign::Right | TextAlign::End => width - line.width,
                _ => 0.0,
            };
            let offset = if matches!(request.width, AxisOffer::Definite(_)) {
                offset.max(0.0)
            } else {
                0.0
            };
            let visible_start = line.start.byte + text[line.start.byte..line.end.byte].len()
                - text[line.start.byte..line.end.byte]
                    .trim_start_matches(' ')
                    .len();
            for (run, (start, _, r)) in spans.iter().enumerate() {
                if let Some(b) = r.inline_box {
                    if *start >= line.start.byte && *start < line.end.byte {
                        positions.push(InlinePosition {
                            run,
                            x: offset + advance(visible_start..*start),
                            y: y + above - b.baseline,
                        });
                    }
                }
            }
            y += above + below;
            max_width = max_width.max(line.width);
            lines += 1;
            if line.end == cursor {
                break;
            }
            cursor = line.end;
        }
        (
            TextMetrics {
                width: max_width,
                height: y,
                first_baseline,
            },
            positions,
        )
    }
}
