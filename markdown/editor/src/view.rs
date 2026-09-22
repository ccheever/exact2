//! The editor drawn as source lines: each line's paragraph style and its text
//! cut into segments of one style, hidden syntax included. A host draws a
//! line per `\n`-separated source line, so its text is always the source.

use exact_markdown::{Paragraph, ParagraphKind as K};

use crate::projection::Deco;
use crate::Editor;

/// A segment's code units are hidden syntax (the rest are `exact_markdown`'s
/// span flags: bold, italic, code, strike, link, image, marker).
pub const HIDDEN: u8 = 128;

/// Line flags.
pub mod flags {
    /// A code block or table line: monospace, with a block background.
    pub const CODE: u8 = 1;
    /// Hidden entirely: a code fence.
    pub const COLLAPSED: u8 = 2;
    /// The first line of a code background.
    pub const CODE_FIRST: u8 = 4;
    /// The last line of a code background.
    pub const CODE_LAST: u8 = 8;
    /// A list item's line, indented by its depth.
    pub const LIST: u8 = 16;
}

/// One source line as drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The line's first code unit.
    pub start: u32,
    /// One past its last, before the `\n`.
    pub end: u32,
    /// Heading level, or zero.
    pub heading: u8,
    /// Block quote depth.
    pub quote: u8,
    /// List depth.
    pub depth: u8,
    /// [`flags`].
    pub flags: u8,
    /// What its paragraph draws before it, on its first line.
    pub deco: Option<Deco>,
    /// `(start, end, style)`: covering `start..end` in order, no gaps.
    pub segments: Vec<(u32, u32, u8)>,
}

impl Editor {
    /// The source as lines to draw.
    pub fn lines(&self) -> Vec<Line> {
        let src = self.source();
        let (st, proj) = (&self.styled, &self.proj);
        let len = src.len() as u32;
        let mut starts = vec![0u32];
        starts.extend(
            src.iter()
                .enumerate()
                .filter(|(_, &c)| c == 10)
                .map(|(i, _)| i as u32 + 1),
        );
        let n = starts.len();
        let end_of = |i: usize| if i + 1 < n { starts[i + 1] - 1 } else { len };
        let mut para: Vec<Option<&Paragraph>> = vec![None; n];
        let mut li = 0;
        for p in &st.paragraphs {
            while li < n && end_of(li) < p.range.start {
                li += 1;
            }
            let mut k = li;
            while k < n && starts[k] <= p.range.end {
                para[k] = Some(p);
                k += 1;
            }
        }
        let (spans, runs) = (&st.spans, &proj.runs);
        let (mut si, mut ri) = (0, 0);
        let mut lines = Vec::with_capacity(n);
        for (i, &a) in starts.iter().enumerate() {
            let b = end_of(i);
            while si < spans.len() && spans[si].range.end <= a && spans[si].range.start < a {
                si += 1;
            }
            while ri < runs.len() && runs[ri].end <= a {
                ri += 1;
            }
            let here: Vec<_> = spans[si..]
                .iter()
                .take_while(|s| s.range.start < b)
                .filter(|s| s.range.end > a)
                .collect();
            let hidden: Vec<_> = runs[ri..]
                .iter()
                .take_while(|r| r.start <= b)
                .filter(|r| r.end > a)
                .collect();
            let mut cuts = vec![a];
            let edges = here
                .iter()
                .flat_map(|s| [s.range.start, s.range.end])
                .chain(hidden.iter().flat_map(|r| [r.start, r.end]));
            for x in edges.chain([b]).map(|x| x.clamp(a, b)) {
                if let Err(k) = cuts.binary_search(&x) {
                    cuts.insert(k, x);
                }
            }
            let segments = cuts
                .windows(2)
                .map(|w| (w[0], w[1]))
                .map(|(s, e)| {
                    let style = if hidden.iter().any(|r| r.start <= s && e <= r.end) {
                        HIDDEN
                    } else {
                        here.iter()
                            .filter(|sp| sp.range.start <= s && e <= sp.range.end)
                            .fold(0, |f, sp| f | sp.style)
                    };
                    (s, e, style)
                })
                .collect();
            let mut line = Line {
                start: a,
                end: b,
                heading: 0,
                quote: 0,
                depth: 0,
                flags: 0,
                deco: None,
                segments,
            };
            if let Some(p) = para[i] {
                match p.kind {
                    K::Heading(level) => line.heading = level,
                    K::Code(_) | K::Table => line.flags |= flags::CODE,
                    K::Fence => line.flags |= flags::COLLAPSED,
                    K::Bullet | K::Ordered | K::Task(_) => {
                        line.flags |= flags::LIST;
                        line.depth = p.depth;
                    }
                    _ => {}
                }
                line.quote = p.quote;
                if a <= p.range.start {
                    line.deco = proj
                        .decorations
                        .binary_search_by_key(&p.range.start, |d| d.0)
                        .ok()
                        .map(|k| proj.decorations[k].1);
                }
            }
            if proj.collapsed.iter().any(|&(s, e)| s <= a && b < e) {
                line.flags |= flags::COLLAPSED;
            }
            lines.push(line);
        }
        let code = |l: Option<&Line>| l.is_some_and(|l| l.flags & flags::CODE != 0);
        for i in 0..lines.len() {
            if code(lines.get(i)) {
                if i == 0 || !code(lines.get(i - 1)) {
                    lines[i].flags |= flags::CODE_FIRST;
                }
                if !code(lines.get(i + 1)) {
                    lines[i].flags |= flags::CODE_LAST;
                }
            }
        }
        lines
    }
}
