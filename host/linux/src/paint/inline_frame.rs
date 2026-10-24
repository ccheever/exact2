//! Inline geometry retained with the paragraph's accepted picture.
use super::*;
use std::ops::Range;

struct InlineRun {
    runs: Range<usize>,
    metrics: (f32, f32),
}

pub(super) struct Captured {
    paragraph: Rc<Paragraph>,
    runs: HashMap<ViewId, InlineRun>,
}

impl Captured {
    pub(super) fn new(
        kernel: &Kernel,
        owner: &NodeRef<'_>,
        paragraph: &Rc<Paragraph>,
        palette: &[RunPaint],
    ) -> Option<Rc<Self>> {
        let mut runs = HashMap::<ViewId, InlineRun>::new();
        // The palette already maps canonical runs to their source nodes.
        // Ancestors keep their own font, even when every leaf is larger.
        for (i, paint) in palette.iter().enumerate() {
            let mut at = paint.source;
            while at != owner.id {
                let Some(node) = kernel.node(at) else { break };
                runs.entry(at)
                    .and_modify(|run| run.runs.end = i + 1)
                    .or_insert_with(|| {
                        let style = node.computed_style(StyleMask::INHERITED);
                        let style =
                            exact_kernel::TextStyle::from_style(&style).scaled(owner.fit_scale());
                        InlineRun {
                            runs: i..i + 1,
                            metrics: paragraph.inline_metrics(style),
                        }
                    });
                let Some(parent) = node.parent else { break };
                at = parent;
            }
        }
        (!runs.is_empty()).then(|| {
            Rc::new(Self {
                paragraph: paragraph.clone(),
                runs,
            })
        })
    }

    pub(super) fn place(
        self: &Rc<Self>,
        origin: (f32, f32),
        fragments: &[exact_kernel::fragment::Fragment],
        transform: Transform,
    ) -> Rc<PaintedInline> {
        let placements = if fragments.is_empty() {
            vec![(0..usize::MAX, origin)]
        } else {
            fragments
                .iter()
                .map(|g| {
                    (
                        g.lines.0 as usize..g.lines.1 as usize,
                        (origin.0 + g.dx, origin.1 + g.dy),
                    )
                })
                .collect()
        };
        Rc::new(PaintedInline {
            captured: self.clone(),
            placements: placements.into(),
            transform,
        })
    }
}

type LinePlacement = (Range<usize>, (f32, f32));

#[derive(Clone)]
pub(crate) struct PaintedInline {
    captured: Rc<Captured>,
    placements: Rc<[LinePlacement]>,
    transform: Transform,
}

impl std::fmt::Debug for PaintedInline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PaintedInline")
    }
}

impl PaintedInline {
    pub(crate) fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.captured.runs.keys().copied()
    }

    pub(super) fn translated(&self, dx: f32, dy: f32) -> Rc<Self> {
        Rc::new(Self {
            transform: self.transform.post_translate(dx, dy),
            ..self.clone()
        })
    }

    pub(crate) fn frame(&self, view: ViewId) -> Option<Rect4> {
        let run = self.captured.runs.get(&view)?;
        let fragments = self
            .captured
            .paragraph
            .inline_rects(run.runs.clone(), run.metrics);
        let mut first = None;
        let mut bound: Option<Rect4> = None;
        for (line, rect) in fragments {
            let (_, origin) = self
                .placements
                .iter()
                .find(|(lines, _)| lines.contains(&line))?;
            let r = bbox(
                self.transform,
                (rect.0 + origin.0, rect.1 + origin.1, rect.2, rect.3),
            );
            first.get_or_insert(r);
            if r.2 <= 0. || r.3 <= 0. {
                continue;
            }
            bound = Some(match bound {
                None => r,
                Some(a) => {
                    let (x, y) = (a.0.min(r.0), a.1.min(r.1));
                    (
                        x,
                        y,
                        (a.0 + a.2).max(r.0 + r.2) - x,
                        (a.1 + a.3).max(r.1 + r.3) - y,
                    )
                }
            });
        }
        bound.or(first)
    }
}
