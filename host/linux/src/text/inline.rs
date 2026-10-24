//! Character ownership shared by inline backgrounds and client rectangles.
use super::{LayoutGlyph, LayoutRun, Paragraph};
use crate::paint::Rect4;
use std::ops::Range;

pub(super) struct Ownership {
    pub text: String,
    pub runs: Vec<(Range<usize>, usize)>,
    pub hard: Vec<Range<usize>>,
}

pub(super) struct Piece {
    pub lo: f32,
    pub hi: f32,
    pub run: usize,
}

impl Ownership {
    pub fn new(p: &Paragraph) -> Self {
        let text: String = p.runs().iter().map(|r| r.text.as_str()).collect();
        let mut offset = 0;
        let runs = p
            .runs()
            .iter()
            .enumerate()
            .map(|(i, run)| {
                let start = offset;
                offset += run.text.len();
                (start..offset, i)
            })
            .collect();
        let mut offset = 0;
        let hard = p
            .source
            .data
            .lines
            .iter()
            .map(|line| {
                let start = offset + text[offset..].find(&line.text).expect("source hard line");
                offset = start + line.text.len();
                start..offset
            })
            .collect();
        Self { text, runs, hard }
    }

    fn overlaps(&self, range: Range<usize>) -> impl Iterator<Item = (Range<usize>, usize)> + '_ {
        let from = self.runs.partition_point(|(r, _)| r.end <= range.start);
        self.runs[from..]
            .iter()
            .take_while(move |(r, _)| r.start < range.end)
            .filter_map(move |(r, run)| {
                let part = r.start.max(range.start)..r.end.min(range.end);
                (!part.is_empty()).then_some((part, *run))
            })
    }

    /// Background paint divides a cross-run ligature by characters in its
    /// bidi direction, without reshaping the paragraph.
    pub fn pieces(&self, g: &LayoutGlyph, range: Range<usize>) -> Vec<Piece> {
        let chars = self.text[range.clone()].chars().count() as f32;
        let mut near = 0.;
        self.overlaps(range)
            .map(|(part, run)| {
                let far = near + self.text[part].chars().count() as f32 / chars * g.w;
                let (lo, hi) = if g.level % 2 == 1 {
                    (g.x + g.w - far, g.x + g.w - near)
                } else {
                    (g.x + near, g.x + far)
                };
                near = far;
                Piece { lo, hi, run }
            })
            .collect()
    }

    /// Chrome's inline client rectangles give a cluster's advance to its
    /// first character's run. Later runs have insertion boxes at its end.
    pub fn cluster_pieces(&self, g: &LayoutGlyph, range: Range<usize>) -> Vec<Piece> {
        let start = range.start;
        let end_x = if g.level % 2 == 1 { g.x } else { g.x + g.w };
        self.overlaps(range)
            .map(|(part, run)| {
                let (lo, hi) = if part.start == start {
                    (g.x, g.x + g.w)
                } else {
                    (end_x, end_x)
                };
                Piece { lo, hi, run }
            })
            .collect()
    }

    pub fn base(&self, p: &Paragraph, line: &LayoutRun<'_>) -> usize {
        if p.flow.as_ref().is_some_and(|f| !f.incomplete) {
            0
        } else {
            self.hard[line.line_i].start
        }
    }
}

impl Paragraph {
    pub(crate) fn inline_metrics(&self, style: exact_kernel::TextStyle) -> (f32, f32) {
        let (ascent, descent, _) = self
            .source
            .catalog
            .borrow_mut()
            .font_metrics(&super::Run::from_style("", style));
        (ascent, descent)
    }

    /// Each contiguous fragment has the addressed inline's own ascent and
    /// descent. Line height places its baseline; it does not size the box.
    pub(crate) fn inline_rects(
        &self,
        runs: Range<usize>,
        metrics: (f32, f32),
    ) -> Vec<(usize, Rect4)> {
        let ownership = Ownership::new(self);
        let start = ownership
            .runs
            .get(runs.start)
            .map_or(ownership.text.len(), |r| r.0.start);
        let end = runs
            .clone()
            .last()
            .and_then(|i| ownership.runs.get(i))
            .map_or(start, |r| r.0.end);
        let (ascent, descent) = metrics;
        let rect = |x, y, width| (x, y - ascent, width, ascent + descent);
        let mut out = Vec::new();
        let mut insertion = None;
        let mut previous = None;
        for (i, line) in self.layout_runs().enumerate() {
            let base = ownership.base(self, &line);
            let logical_end = line
                .glyphs
                .iter()
                .map(|g| g.end as usize)
                .max()
                .unwrap_or(0);
            let trim = if self.source.spec.white_space.model().preserves() {
                logical_end
            } else {
                line.text[..logical_end].trim_end_matches([' ', '\t']).len()
            };
            let mut spans = Vec::new();
            for g in line.glyphs {
                let local = g.range();
                if local.is_empty() {
                    continue;
                }
                let range = base + local.start..base + local.end;
                if start == end {
                    let caret = |at: usize| {
                        let at_start = at == range.start;
                        let x = if at_start == (g.level % 2 == 0) {
                            g.x
                        } else {
                            g.x + g.w
                        };
                        (i, rect(x, line.line_y, 0.))
                    };
                    if range.start <= start && start <= range.end && insertion.is_none() {
                        insertion = Some(caret(start));
                    }
                    if range.end <= start {
                        previous = Some(caret(range.end));
                    }
                }
                if g.w <= 0. || local.start >= trim {
                    continue;
                }
                for piece in ownership.cluster_pieces(g, range) {
                    if runs.contains(&piece.run) {
                        spans.push((piece.lo, piece.hi));
                    }
                }
            }
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut merged: Vec<(f32, f32)> = Vec::new();
            for (lo, hi) in spans {
                match merged.last_mut() {
                    Some(last) if lo <= last.1 + 0.01 => last.1 = last.1.max(hi),
                    _ => merged.push((lo, hi)),
                }
            }
            out.extend(
                merged
                    .into_iter()
                    .map(|(lo, hi)| (i, rect(lo, line.line_y, hi - lo))),
            );
            if start == end && insertion.is_none() && line.glyphs.is_empty() && base == start {
                insertion = Some((i, rect(0., line.line_y, 0.)));
            }
        }
        if start == end {
            return insertion.or(previous).into_iter().collect();
        }
        out
    }
}
