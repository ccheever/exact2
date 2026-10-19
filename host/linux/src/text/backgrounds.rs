//! Inline background fragments use character ownership, independent of shaping.
use super::{shaping, Paragraph};
use crate::paint::{border::resolve_radii, Shape};
use exact_kernel::{Dimension, Direction, Env, ViewId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineBackground {
    pub color: [u8; 4],
    pub id: ViewId,
    pub radii: [Dimension; 4],
}

struct BoxRange {
    id: ViewId,
    first: (usize, usize),
    last: (usize, usize),
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
        let hard_lines = shaping::hard_lines(&self.source.spec);
        let mut boxes: Vec<BoxRange> = Vec::new();
        for (hard, (_, runs)) in hard_lines.iter().enumerate() {
            for (range, run) in runs {
                let Some(background) = backgrounds.get(*run).copied().flatten() else {
                    continue;
                };
                let first = (hard, range.start);
                let last = (hard, range.end - 1);
                if let Some(b) = boxes.iter_mut().find(|b| b.id == background.id) {
                    b.first = b.first.min(first);
                    b.last = b.last.max(last);
                } else {
                    boxes.push(BoxRange {
                        id: background.id,
                        first,
                        last,
                    });
                }
            }
        }
        let run_boxes: Vec<_> = backgrounds
            .iter()
            .map(|b| b.and_then(|b| boxes.iter().position(|r| r.id == b.id)))
            .collect();
        let metrics = &self.source.data.run_metrics;
        let rtl = self.source.spec.direction == Direction::Rtl;
        let mut out = Vec::new();
        for line in self.layout_runs() {
            let mut starts = vec![false; boxes.len()];
            let mut ends = vec![false; boxes.len()];
            let mut spans = Vec::new();
            let (_, runs) = &hard_lines[line.line_i];
            // Collapsible trailing spaces carry endpoints but no background ink.
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
            for g in line.glyphs {
                let range = g.range();
                if range.is_empty() {
                    // An ellipsis carries its run's background, but cannot
                    // supply a box's endpoints.
                    if backgrounds.get(g.run()).copied().flatten().is_some() && g.w > 0.0 {
                        spans.push(Piece {
                            lo: g.x,
                            hi: g.x + g.w,
                            run: g.run(),
                        });
                    }
                    continue;
                }
                for (i, b) in boxes.iter().enumerate() {
                    let first = (line.line_i, range.start);
                    let last = (line.line_i, range.end - 1);
                    starts[i] |= first <= b.first && b.first <= last;
                    ends[i] |= first <= b.last && b.last <= last;
                }
                if g.w <= 0.0 || range.start >= trim {
                    continue;
                }
                let chars = line.text[range.clone()].chars().count() as f32;
                // A ligature can cross canonical runs. Divide its advance by
                // characters in its bidi direction, without reshaping it.
                for (owner, run) in runs {
                    if owner.end <= range.start {
                        continue;
                    }
                    if owner.start >= range.end {
                        break;
                    }
                    if backgrounds.get(*run).copied().flatten().is_none() {
                        continue;
                    }
                    let a = owner.start.max(range.start);
                    let z = owner.end.min(range.end);
                    let near = line.text[range.start..a].chars().count() as f32 / chars * g.w;
                    let far = near + line.text[a..z].chars().count() as f32 / chars * g.w;
                    let (lo, hi) = if g.level % 2 == 1 {
                        (g.x + g.w - far, g.x + g.w - near)
                    } else {
                        (g.x + near, g.x + far)
                    };
                    spans.push(Piece { lo, hi, run: *run });
                }
            }
            spans.sort_by(|a, b| a.lo.total_cmp(&b.lo));
            let mut merged: Vec<Piece> = Vec::new();
            for span in spans {
                match merged.last_mut() {
                    Some(last)
                        if run_boxes[last.run] == run_boxes[span.run]
                            && metrics[last.run] == metrics[span.run]
                            && span.lo <= last.hi + 0.01 =>
                    {
                        last.hi = last.hi.max(span.hi);
                    }
                    _ => merged.push(span),
                }
            }
            let mut kept = vec![[false; 4]; merged.len()];
            for b in 0..boxes.len() {
                let mut mine = merged
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| run_boxes[p.run] == Some(b));
                let Some((left, _)) = mine.next() else {
                    continue;
                };
                let right = mine.next_back().map_or(left, |(i, _)| i);
                if if rtl { ends[b] } else { starts[b] } {
                    kept[left][0] = true;
                    kept[left][3] = true;
                }
                if if rtl { starts[b] } else { ends[b] } {
                    kept[right][1] = true;
                    kept[right][2] = true;
                }
            }
            for (piece, kept) in merged.into_iter().zip(kept) {
                let background = backgrounds[piece.run].expect("background span");
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
