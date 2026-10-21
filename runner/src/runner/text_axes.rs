//! `font-weight` and `font-stretch` transitions (LLP 1019 §13 D10). A font
//! axis changes glyph advances, so no compositor can play it: the runner
//! does, with a motion engine of its own for the two rows. A commit that
//! changes one under a matching `transition` writes the value presented
//! now and starts the transition; each later frame writes the next value as
//! a presentation-only commit, and layout follows it as it follows any
//! commit. The web links none of this: the browser plays the CSS
//! `transition` the row emits.
use super::*;
use exact_kernel::{Op, StyleId, StyleProps};
use exact_motion::{Change, Engine, Property, Value};

/// How a runner makes its font-axis engine, when linked.
pub type TextAxesLink = Option<fn() -> Engine>;

/// The font-axis engine, made at the first change a transition covers.
#[derive(Default)]
pub(super) struct TextAxes(Option<Box<Engine>>);

const AXES: [(StyleId, Property); 2] = [
    (StyleId::FontWeight, Property::FontWeight),
    (StyleId::FontStretch, Property::FontStretch),
];

fn row(style: &StyleProps, id: StyleId) -> f64 {
    match id {
        StyleId::FontWeight => f64::from(style.font_weight),
        _ => f64::from(style.font_stretch),
    }
}

fn set_row(style: &mut StyleProps, id: StyleId, value: f64) {
    match id {
        // A weight is a whole number on the wire: the steps are 1 of 1000.
        StyleId::FontWeight => style.font_weight = value.round().clamp(1.0, 1000.0) as u16,
        _ => style.font_stretch = value as f32,
    }
}

impl TextAxes {
    /// Whether a font axis is moving.
    pub(super) fn running(&self) -> bool {
        self.0.as_ref().is_some_and(|e| !e.quiescent())
    }

    /// When the last one ends, milliseconds.
    pub(super) fn settle_ms(&self) -> Option<f64> {
        self.0.as_ref()?.settle_time().map(|s| s * 1000.0)
    }
}

impl<D: DataSource> Runner<D> {
    /// Before `ops` apply: each font-axis change on a live node is observed,
    /// and one a transition covers is written as the value presented now.
    /// A node the engine has not seen and no transition covers costs
    /// nothing, so a frame task that sets a weight each frame stays cheap.
    /// The nodes it observed are returned for [`Runner::unwind_text_axes`].
    pub(super) fn prepare_text_axes(&mut self, ops: &mut [Op]) -> Vec<u64> {
        let mut observed = Vec::new();
        let Some(make) = self.links.text_axes else {
            return observed;
        };
        let now = self.now_ms / 1000.0;
        for op in ops.iter_mut() {
            let (id, patch) = match op {
                Op::SetStyle { id, patch } => (*id, patch),
                Op::DestroyView { id } => {
                    self.forget_text_axes(*id);
                    continue;
                }
                _ => continue,
            };
            if !AXES.iter().any(|(row, _)| patch.mask.has(*row)) {
                continue;
            }
            let Some(node) = self.kernel.node(id) else {
                continue;
            };
            let own = self.kernel.arena().style(node.key.index);
            let transitions = if patch.mask.has(StyleId::Transition) {
                patch.transition.clone()
            } else {
                own.transition.clone()
            };
            let key = u64::from(id);
            for (style_row, property) in AXES {
                if !patch.mask.has(style_row) {
                    continue;
                }
                let covered = transitions
                    .matching(property)
                    .is_some_and(|t| t.governing(property).starts());
                let seen = self
                    .text_axes
                    .0
                    .as_ref()
                    .is_some_and(|e| e.value(key, property).is_some());
                if !covered && !seen {
                    continue;
                }
                let engine = self.text_axes.0.get_or_insert_with(|| Box::new(make()));
                let at = now.max(engine.now());
                let _ = engine.advance(at);
                let change = |value| Change {
                    node: key,
                    property,
                    value: Value::scalar(value),
                    velocity: None,
                };
                if !seen {
                    // CSS's before-change style: the computed value, inherited
                    // or the row's own.
                    let before = node.computed_row(style_row, |s| row(s, style_row));
                    let _ = engine.observe(change(before));
                }
                observed.push(key);
                let _ = engine.set_transitions(key, transitions.clone());
                if engine.observe(change(row(patch, style_row))).is_ok() {
                    if let Some(shown) = engine.value(key, property) {
                        set_row(patch, style_row, shown.x);
                    }
                }
            }
        }
        observed
    }

    /// After a refused commit: the nodes it observed start again from what
    /// the kernel holds, as a node the engine never saw does.
    pub(super) fn unwind_text_axes(&mut self, observed: Vec<u64>) {
        if let Some(engine) = self.text_axes.0.as_mut() {
            for key in observed {
                engine.remove(key);
            }
        }
    }

    fn forget_text_axes(&mut self, id: ViewId) {
        let (Some(engine), Some(node)) = (self.text_axes.0.as_mut(), self.kernel.node(id)) else {
            return;
        };
        let arena = self.kernel.arena();
        for slot in arena.subtree(node.key.index) {
            engine.remove(u64::from(arena.local_id(slot)));
        }
    }

    /// Each moving font axis at the clock, one commit for all of them.
    pub(super) fn tick_text_axes(&mut self) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(engine) = self.text_axes.0.as_mut() else {
            return Ok(None);
        };
        let now = (self.now_ms / 1000.0).max(engine.now());
        let _ = engine.advance(now);
        let mut ops: Vec<Op> = Vec::new();
        for shown in engine.frame() {
            let Ok(id) = ViewId::try_from(shown.node) else {
                continue;
            };
            let Some((style_row, _)) = AXES.iter().find(|(_, p)| *p == shown.property) else {
                continue;
            };
            if self.kernel.node(id).is_none() {
                continue;
            }
            let at = ops
                .iter()
                .position(|op| matches!(op, Op::SetStyle { id: i, .. } if *i == id));
            let at = at.unwrap_or_else(|| {
                ops.push(Op::SetStyle {
                    id,
                    patch: Box::default(),
                });
                ops.len() - 1
            });
            if let Op::SetStyle { patch, .. } = &mut ops[at] {
                set_row(patch, *style_row, shown.value.x);
                patch.mask.set(*style_row);
            }
        }
        if ops.is_empty() {
            return Ok(None);
        }
        self.batch += 1;
        Ok(Some(self.kernel.apply(0, self.batch, &ops)?))
    }

    /// When the last font-axis transition ends, milliseconds; a host's
    /// `settle` takes the later of this and its own engine's.
    pub fn text_axes_settle_ms(&self) -> Option<f64> {
        self.text_axes.settle_ms()
    }
}
