//! Inline background fragments use character ownership, independent of shaping.
use super::Paragraph;
use crate::paint::{border::resolve_radii, Shape};
use exact_kernel::{Dimension, Direction, Env, ViewId};
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineBackground {
    pub color: [u8; 4],
    pub id: ViewId,
    pub radii: [Dimension; 4],
}

#[derive(Default)]
struct Edges {
    start: bool,
    end: bool,
    left: Option<usize>,
    right: usize,
}

fn endpoints_in(
    endpoints: &[(usize, ViewId, bool)],
    range: Range<usize>,
    edges: &mut BTreeMap<ViewId, Edges>,
) {
    let from = endpoints.partition_point(|(at, _, _)| *at < range.start);
    for &(at, id, end) in &endpoints[from..] {
        if at >= range.end {
            break;
        }
        let edge = edges.entry(id).or_default();
        if end {
            edge.end = true;
        } else {
            edge.start = true;
        }
    }
}

struct Piece {
    lo: f32,
    hi: f32,
    run: usize,
}

impl Paragraph {
    /// CSS 2 §8.6 and `box-decoration-break: slice`: a box's first and last
    /// characters select lines; the paragraph direction selects each line's
    /// outermost piece for the start and end corners. Breaks stay open.
    pub(crate) fn inline_backgrounds(
        &self,
        backgrounds: &[Option<InlineBackground>],
        env: &Env,
    ) -> Vec<(Shape, [u8; 4])> {
        if !backgrounds.iter().flatten().any(|b| {
            b.radii.iter().any(|d| match d {
                Dimension::Auto => false,
                Dimension::Points(n) | Dimension::Percent(n) => *n > 0.0,
                Dimension::Calc(p, n) => *p > 0.0 || *n > 0.0,
                _ => true,
            })
        }) {
            return self
                .run_backgrounds(
                    &backgrounds
                        .iter()
                        .map(|b| b.map(|b| b.color))
                        .collect::<Vec<_>>(),
                )
                .into_iter()
                .map(|(rect, color)| (Shape::rect(rect), color))
                .collect();
        }
        // Built only for rounded paint, never part of the paragraph's shape key.
        let flow = self.flow.as_ref().filter(|f| !f.incomplete);
        let text: String = self
            .source
            .spec
            .runs
            .iter()
            .map(|r| r.text.as_str())
            .collect();
        let mut offset = 0;
        let runs: Vec<_> = self
            .source
            .spec
            .runs
            .iter()
            .enumerate()
            .filter_map(|(i, run)| {
                let start = offset;
                offset += run.text.len();
                (start != offset).then_some((start..offset, i))
            })
            .collect();
        let mut offset = 0;
        let hard_ranges: Vec<_> = self
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
        let mut boxes: BTreeMap<ViewId, Range<usize>> = BTreeMap::new();
        let mut add = |range: Range<usize>, run: usize| {
            if let Some(background) = backgrounds.get(run).copied().flatten() {
                boxes
                    .entry(background.id)
                    .and_modify(|b| {
                        b.start = b.start.min(range.start);
                        b.end = b.end.max(range.end);
                    })
                    .or_insert(range);
            }
        };
        if flow.is_some() {
            for (range, run) in &runs {
                add(range.clone(), *run);
            }
        } else {
            // Ordinary layout omits hard separators from its glyph ranges.
            for hard in hard_ranges.iter().filter(|r| !r.is_empty()) {
                let from = runs.partition_point(|(r, _)| r.end <= hard.start);
                for (range, run) in &runs[from..] {
                    if range.start >= hard.end {
                        break;
                    }
                    add(range.start.max(hard.start)..range.end.min(hard.end), *run);
                }
            }
        }
        let mut endpoints: Vec<_> = boxes
            .iter()
            .flat_map(|(&id, range)| [(range.start, id, false), (range.end - 1, id, true)])
            .collect();
        endpoints.sort_unstable();
        let metrics = &self.source.data.run_metrics;
        let rtl = self.source.spec.direction == Direction::Rtl;
        let mut out = Vec::new();
        for (index, line) in self.layout_runs().enumerate() {
            let mut edges = BTreeMap::<ViewId, Edges>::new();
            let mut spans = Vec::new();
            let base = if flow.is_some() {
                0
            } else {
                hard_ranges[line.line_i].start
            };
            if let Some(flow) = flow {
                endpoints_in(
                    &endpoints,
                    flow.fragments[index].start..flow.paint_ends[index],
                    &mut edges,
                );
            }
            // Flow has already removed collapsible trailing whitespace. Ordinary
            // layout keeps its endpoints, but the spaces carry no background ink.
            let logical_end = line
                .glyphs
                .iter()
                .map(|g| g.end as usize)
                .max()
                .unwrap_or(0);
            let trim = if flow.is_some() || self.source.spec.white_space.model().preserves() {
                logical_end
            } else {
                line.text[..logical_end].trim_end_matches([' ', '\t']).len()
            };
            for g in line.glyphs {
                let local = g.range();
                if local.is_empty() {
                    // An ellipsis carries its run's background, but no endpoint.
                    if backgrounds.get(g.run()).copied().flatten().is_some() && g.w > 0.0 {
                        spans.push(Piece {
                            lo: g.x,
                            hi: g.x + g.w,
                            run: g.run(),
                        });
                    }
                    continue;
                }
                let range = base + local.start..base + local.end;
                if flow.is_none() {
                    endpoints_in(&endpoints, range.clone(), &mut edges);
                }
                if g.w <= 0.0 || local.start >= trim {
                    continue;
                }
                let chars = text[range.clone()].chars().count() as f32;
                // A ligature can cross canonical runs. Divide its advance by
                // characters in its bidi direction, without reshaping it.
                let from = runs.partition_point(|(owner, _)| owner.end <= range.start);
                let mut near = 0.0;
                for (owner, run) in &runs[from..] {
                    if owner.start >= range.end {
                        break;
                    }
                    let a = owner.start.max(range.start);
                    let z = owner.end.min(range.end);
                    let far = near + text[a..z].chars().count() as f32 / chars * g.w;
                    if backgrounds.get(*run).copied().flatten().is_some() {
                        let (lo, hi) = if g.level % 2 == 1 {
                            (g.x + g.w - far, g.x + g.w - near)
                        } else {
                            (g.x + near, g.x + far)
                        };
                        spans.push(Piece { lo, hi, run: *run });
                    }
                    near = far;
                }
            }
            spans.sort_by(|a, b| a.lo.total_cmp(&b.lo));
            let mut merged: Vec<Piece> = Vec::new();
            for span in spans {
                match merged.last_mut() {
                    Some(last)
                        if backgrounds[last.run].unwrap().id
                            == backgrounds[span.run].unwrap().id
                            && metrics[last.run] == metrics[span.run]
                            && span.lo <= last.hi + 0.01 =>
                    {
                        last.hi = last.hi.max(span.hi);
                    }
                    _ => merged.push(span),
                }
            }
            for (i, piece) in merged.iter().enumerate() {
                let edge = edges.entry(backgrounds[piece.run].unwrap().id).or_default();
                edge.left.get_or_insert(i);
                edge.right = i;
            }
            for (i, piece) in merged.into_iter().enumerate() {
                let background = backgrounds[piece.run].expect("background span");
                let edge = &edges[&background.id];
                let left = edge.left == Some(i) && if rtl { edge.end } else { edge.start };
                let right = edge.right == i && if rtl { edge.start } else { edge.end };
                let kept = [left, right, right, left];
                let (ascent, descent, _) = metrics[piece.run];
                let rect = (
                    piece.lo,
                    line.line_y - ascent,
                    piece.hi - piece.lo,
                    ascent + descent,
                );
                let radii = std::array::from_fn(|i| {
                    if kept[i] {
                        background.radii[i]
                    } else {
                        Dimension::Points(0.0)
                    }
                });
                out.push((
                    Shape::elliptical(rect, resolve_radii(radii, env, rect.2, rect.3)),
                    background.color,
                ));
            }
        }
        out
    }
}
