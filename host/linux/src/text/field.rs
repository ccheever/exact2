//! Editing geometry from the immutable glyphs and CSS line boxes used to paint a field.
use super::Paragraph;
use exact_kernel::{Direction, TextAlign};
use unicode_segmentation::UnicodeSegmentation;

type Rect = (f32, f32, f32, f32);

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Cursor {
    pub offset: u32,
    /// The preceding visual line at a shared wrap boundary. Commands default downstream.
    pub upstream: bool,
}

#[derive(Debug)]
struct Span {
    start: u32,
    end: u32,
    // Logical start and end positions; RTL spans run from right to left.
    left: f32,
    right: f32,
    // Its font's content area (ascent over descent at the baseline), the
    // box a DOM range gives its text.
    top: f32,
    height: f32,
}

#[derive(Debug)]
struct Line {
    start: u32,
    end: u32,
    top: f32,
    height: f32,
    baseline: f32,
    empty_x: f32,
    /// The strut font's content area, for a line with no text.
    text: (f32, f32),
    spans: Vec<Span>,
}

#[derive(Debug)]
pub(crate) struct Geometry {
    lines: Vec<Line>,
    len: u32,
}

pub(crate) fn line_left(align: TextAlign, direction: Direction, width: f32, w: f32) -> f32 {
    if w > width {
        return if direction == Direction::Rtl {
            width - w
        } else {
            0.
        };
    }
    let align = match align {
        TextAlign::Justify => TextAlign::Start,
        other => other,
    };
    let share = match align.physical(direction) {
        TextAlign::Center => 0.5,
        TextAlign::Right => 1.,
        _ => 0.,
    };
    share * (width - w)
}

impl Geometry {
    pub(crate) fn new(
        paragraph: &Paragraph,
        value: &str,
        masked: bool,
        empty_x: f32,
        first_x: f32,
    ) -> Self {
        // Painted bytes and DOM offsets differ for passwords and non-BMP characters.
        let mut offsets = vec![(0, 0)];
        let (mut byte, mut units) = (0, 0);
        for c in value.chars() {
            byte += if masked {
                '•'.len_utf8()
            } else {
                c.len_utf8()
            };
            units += c.len_utf16() as u32;
            offsets.push((byte, units));
        }
        let offset = |byte| {
            let i = offsets
                .partition_point(|(b, _)| *b <= byte)
                .saturating_sub(1);
            offsets[i].1
        };
        let mut hard_starts = vec![0];
        if !masked {
            let mut after_cr = false;
            for (byte, c) in value.char_indices() {
                if matches!(
                    c,
                    '\n' | '\r' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2029}'
                ) {
                    let next = byte + c.len_utf8();
                    if c == '\n' && after_cr {
                        *hard_starts.last_mut().unwrap() = next;
                    } else {
                        hard_starts.push(next);
                    }
                }
                after_cr = c == '\r';
            }
        }
        let data = &paragraph.source.data;
        let (ascent, descent) = data.strut_font;
        // A content area at baseline `y`: the font's ascent over it.
        let area = |y: f32, (ascent, descent): (f32, f32)| (y - ascent, ascent + descent);
        let mut lines = Vec::new();
        for run in paragraph.layout_runs() {
            let base = hard_starts[run.line_i];
            let mut spans = Vec::new();
            if !value.is_empty() {
                let mut glyphs = run.glyphs.iter().peekable();
                while let Some(g) = glyphs.next() {
                    let (mut left, mut right) = (g.x, g.x + g.w);
                    while let Some(next) = glyphs.peek().filter(|n| n.range() == g.range()) {
                        left = left.min(next.x);
                        right = right.max(next.x + next.w);
                        glyphs.next();
                    }
                    let (top, height) = data
                        .run_metrics
                        .get(g.run())
                        .map_or(area(run.line_y, (ascent, descent)), |m| {
                            area(run.line_y, (m.0, m.1))
                        });
                    let cluster = &run.text[g.range()];
                    let mut boundaries: Vec<_> =
                        cluster.grapheme_indices(true).map(|(i, _)| i).collect();
                    boundaries.push(cluster.len());
                    let count = boundaries.len() - 1;
                    for (i, pair) in boundaries.windows(2).enumerate() {
                        let x = |part: usize| {
                            let t = part as f32 / count as f32;
                            if g.level & 1 == 0 {
                                left + (right - left) * t
                            } else {
                                right - (right - left) * t
                            }
                        };
                        spans.push(Span {
                            start: offset(base + g.start as usize + pair[0]),
                            end: offset(base + g.start as usize + pair[1]),
                            left: x(i),
                            right: x(i + 1),
                            top,
                            height,
                        });
                    }
                }
            }
            spans.sort_by_key(|s| s.start);
            let start = spans.first().map_or(offset(base), |s| s.start);
            let end = spans.last().map_or(start, |s| s.end);
            lines.push(Line {
                start,
                end,
                top: run.line_top,
                height: run.line_height,
                baseline: run.line_y,
                empty_x: if lines.is_empty() { first_x } else { empty_x },
                text: area(run.line_y, (ascent, descent)),
                spans,
            });
            if value.is_empty() {
                break;
            }
        }
        // Ordinary text omits its empty final hard line. An editor can put
        // a caret there; its box is the paragraph's own empty-line strut.
        if !value.is_empty() && hard_starts.last() == Some(&value.len()) {
            let top = lines.last().map_or(0., |l| l.top + l.height);
            let height = paragraph.source.flow_box(std::iter::empty(), &[]).1;
            lines.push(Line {
                start: units,
                end: units,
                top,
                height,
                baseline: top + data.strut.0,
                empty_x,
                text: area(top + data.strut.0, (ascent, descent)),
                spans: Vec::new(),
            });
        }
        Self { lines, len: units }
    }

    pub(crate) fn wrap_boundary(&self, offset: u32) -> bool {
        self.lines
            .windows(2)
            .any(|pair| pair[0].end == offset && pair[1].start == offset)
    }

    fn line_index(&self, cursor: Cursor) -> usize {
        if cursor.upstream {
            if let Some(i) = self.lines.iter().position(|l| l.end == cursor.offset) {
                return i;
            }
        }
        self.lines
            .iter()
            .rposition(|l| l.start <= cursor.offset)
            .unwrap_or(0)
    }

    pub(crate) fn caret(&self, cursor: Cursor) -> Rect {
        let Some(line) = self.lines.get(self.line_index(cursor)) else {
            return (0., 0., 1., 0.);
        };
        let x = line
            .spans
            .iter()
            .find(|s| cursor.offset < s.end)
            .map(|s| s.left)
            .or_else(|| line.spans.last().map(|s| s.right))
            .unwrap_or(line.empty_x);
        (x, line.top, 1., line.height)
    }

    fn on_line(&self, index: usize, x: f32) -> Cursor {
        let line = &self.lines[index];
        let offset = line
            .spans
            .iter()
            .flat_map(|s| [(s.start, s.left), (s.end, s.right)])
            .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
            .map_or(line.start, |s| s.0);
        Cursor {
            offset,
            upstream: offset == line.end
                && self.lines.get(index + 1).is_some_and(|l| l.start == offset),
        }
    }

    pub(crate) fn hit(&self, x: f32, y: f32) -> Cursor {
        if self.lines.is_empty() {
            return Cursor::default();
        }
        let index = self
            .lines
            .iter()
            .position(|l| y < l.top + l.height)
            .unwrap_or(self.lines.len() - 1);
        self.on_line(index, x)
    }

    /// The character under the pointer, rather than the nearest insertion edge.
    pub(crate) fn hit_character(&self, x: f32, y: f32) -> u32 {
        let Some(line) = self
            .lines
            .iter()
            .find(|l| y < l.top + l.height)
            .or_else(|| self.lines.last())
        else {
            return 0;
        };
        line.spans
            .iter()
            .min_by(|a, b| {
                let distance = |s: &Span| {
                    let left = s.left.min(s.right);
                    let right = s.left.max(s.right);
                    (left - x).max(0.).max(x - right)
                };
                distance(a).total_cmp(&distance(b))
            })
            .map_or(line.start, |span| span.start)
    }

    pub(crate) fn vertical(&self, cursor: Cursor, x: f32, down: bool) -> Cursor {
        let index = self.line_index(cursor);
        if down && index + 1 < self.lines.len() {
            self.on_line(index + 1, x)
        } else if !down && index > 0 {
            self.on_line(index - 1, x)
        } else {
            Cursor {
                offset: if down { self.len } else { 0 },
                upstream: false,
            }
        }
    }

    pub(crate) fn edge(&self, cursor: Cursor, end: bool) -> Cursor {
        let Some(line) = self.lines.get(self.line_index(cursor)) else {
            return Cursor::default();
        };
        Cursor {
            offset: if end { line.end } else { line.start },
            upstream: end,
        }
    }

    /// `rangeFrame`'s boxes (LLP 1114.007), field-local, as Chrome's
    /// `Range.getClientRects()` gives them for a textarea: each a text box,
    /// its glyphs across and its font's content area down. Collapsed, the
    /// caret's at width 0, upstream at a wrap (the line it ends); wider,
    /// each span's the range overlaps, then a zero-width one at the end of a
    /// line whose break it holds, which a bound skips unless it is alone.
    /// `None` past the text.
    pub(crate) fn range(&self, start: u32, end: u32) -> Option<Vec<Rect>> {
        if start > end || end > self.len {
            return None;
        }
        let at = |line: &Line, span: Option<&Span>| span.map_or(line.text, |s| (s.top, s.height));
        if start == end {
            let cursor = Cursor {
                offset: start,
                upstream: self.wrap_boundary(start),
            };
            let line = self.lines.get(self.line_index(cursor))?;
            // The run the offset closes and the one it opens meet there,
            // or (where two directions meet) stand apart: Chrome takes the
            // first in visual order, left to right, whatever the line's
            // direction (measured: `abc אבג` at 4 is the space's end, and
            // under `direction: rtl` `אבג abc` at 4 is `abc`'s left edge).
            let before = line.spans.iter().find(|s| s.end == start);
            let after = line.spans.iter().find(|s| start < s.end);
            let leftmost = |s: &Span| s.left.min(s.right);
            let (x, span) = match (before, after) {
                (Some(b), Some(a)) if leftmost(a) < leftmost(b) => (a.left, Some(a)),
                (Some(s), _) => (s.right, Some(s)),
                (None, Some(s)) => (s.left, Some(s)),
                (None, None) => line
                    .spans
                    .last()
                    .map_or((line.empty_x, None), |s| (s.right, Some(s))),
            };
            let (top, height) = at(line, span);
            return Some(vec![(x, top, 0., height)]);
        }
        let mut rects = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            for s in line.spans.iter().filter(|s| s.start < end && s.end > start) {
                let (left, right) = (s.left.min(s.right), s.left.max(s.right));
                rects.push((left, s.top, right - left, s.height));
            }
            // The break's characters (a CR LF is two) lie between the lines.
            let broken = self
                .lines
                .get(index + 1)
                .is_some_and(|next| next.start > line.end && start < next.start);
            if broken && end > line.end {
                let last = line.spans.last();
                let x = last.map_or(line.empty_x, |s| s.right);
                let (top, height) = at(line, last);
                rects.push((x, top, 0., height));
            }
        }
        Some(rects)
    }

    pub(crate) fn highlight(&self, start: u32, end: u32) -> Vec<Rect> {
        self.marks(start, end, |line| (line.top, line.height))
    }

    pub(crate) fn underlines(&self, start: u32, end: u32, thickness: f32) -> Vec<Rect> {
        self.marks(start, end, |line| (line.baseline + thickness, thickness))
    }

    fn marks(&self, start: u32, end: u32, vertical: impl Fn(&Line) -> (f32, f32)) -> Vec<Rect> {
        let mut rects: Vec<Rect> = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let (top, height) = vertical(line);
            let mut pieces: Vec<_> = line
                .spans
                .iter()
                .filter(|s| s.start < end && s.end > start)
                .map(|s| (s.left.min(s.right), s.left.max(s.right)))
                .collect();
            // A selected hard break also marks an empty line.
            if start <= line.end
                && end > line.end
                && self
                    .lines
                    .get(index + 1)
                    .is_some_and(|l| l.start > line.end)
            {
                let x = line.spans.last().map_or(line.empty_x, |s| s.right);
                pieces.push((x, x + line.height / 4.));
            }
            pieces.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (left, right) in pieces {
                if let Some(last) = rects
                    .last_mut()
                    .filter(|r| r.1 == top && left <= r.0 + r.2 + 0.01)
                {
                    last.2 = right.max(last.0 + last.2) - last.0;
                } else {
                    rects.push((left, top, right - left, height));
                }
            }
        }
        rects
    }
}
