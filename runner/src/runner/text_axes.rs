//! The font rows that move glyphs (LLP 1019 §13 D10): `font-weight`,
//! `font-stretch` and `font-variation-settings` change advances, so no
//! compositor can play them. A `transition` or `@keyframes` that moves one
//! plays in a motion engine of the runner's own; each frame writes the
//! presented value as a presentation-only commit, and layout follows it as
//! it follows any commit. The web links none of this: the browser plays the
//! CSS the rows emit.
//!
//! A change is CSS Transitions §3's. The before-change value is the
//! computed one presented before the commit; the after-change value leaves
//! transitions out (a moving row's target); and a node whose own
//! `transition` covers a row it inherits starts one when an ancestor's
//! value changes. While such a node's row moves the runner writes it, and
//! at rest it clears the write, so the node inherits again.
//!
//! `font-variation-settings` moves axis by axis: the engine keys each axis
//! as a node of its own, its tag above the view's id. Two values that name
//! the same axes interpolate each; two that name different ones change at
//! once, CSS's discrete step for a transition (CSS Fonts 4 §7.2).
use super::*;
use exact_kernel::style::FontVariations;
use exact_kernel::{Op, StyleId, StyleMask, StyleProps};
use exact_motion::{Change, Engine, Property, Transitions, Value};
use std::collections::{BTreeMap, BTreeSet};

mod present;

/// How a runner makes its font-axis engine, when linked.
pub type TextAxesLink = Option<fn() -> Engine>;

/// One font row the runner moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Row {
    Weight,
    Stretch,
    Axes,
}

const ROWS: [Row; 3] = [Row::Weight, Row::Stretch, Row::Axes];

impl Row {
    fn id(self) -> StyleId {
        match self {
            Row::Weight => StyleId::FontWeight,
            Row::Stretch => StyleId::FontStretch,
            Row::Axes => StyleId::FontVariationSettings,
        }
    }

    fn property(self) -> Property {
        match self {
            Row::Weight => Property::FontWeight,
            Row::Stretch => Property::FontStretch,
            Row::Axes => Property::FontVariationSettings,
        }
    }

    fn of(property: Property) -> Option<Row> {
        ROWS.into_iter().find(|r| r.property() == property)
    }
}

/// A row's value.
#[derive(Debug, Clone, PartialEq)]
enum Shown {
    Number(f64),
    Axes(FontVariations),
}

fn read(style: &StyleProps, row: Row) -> Shown {
    match row {
        Row::Weight => Shown::Number(f64::from(style.font_weight)),
        Row::Stretch => Shown::Number(f64::from(style.font_stretch)),
        Row::Axes => Shown::Axes(style.font_variation_settings),
    }
}

fn write(patch: &mut StyleProps, row: Row, value: &Shown) {
    match (row, value) {
        // A weight is a whole number on the wire: the steps are 1 of 1000.
        (Row::Weight, Shown::Number(n)) => patch.font_weight = n.round().clamp(1.0, 1000.0) as u16,
        (Row::Stretch, Shown::Number(n)) => patch.font_stretch = *n as f32,
        (Row::Axes, Shown::Axes(axes)) => patch.font_variation_settings = *axes,
        _ => return,
    }
    patch.mask.set(row.id());
}

/// The engine's key for a view's own rows.
fn key(view: ViewId) -> u64 {
    u64::from(view)
}

/// The engine's key for one variation axis of a view.
fn axis_key(view: ViewId, tag: u32) -> u64 {
    (u64::from(tag) << 32) | u64::from(view)
}

/// The axes as engine values.
fn axis_values(axes: &FontVariations) -> impl Iterator<Item = (u32, Value)> + '_ {
    axes.axes()
        .iter()
        .map(|(tag, v)| (*tag, Value::axis(*tag, f64::from(*v))))
}

fn sorted_tags(axes: &FontVariations) -> Vec<u32> {
    let mut tags: Vec<u32> = axes.axes().iter().map(|(t, _)| *t).collect();
    tags.sort_unstable();
    tags
}

/// A row the runner presents: where it moves to, and whether the node only
/// inherits it (then the runner's write is cleared at rest).
#[derive(Debug)]
struct Moving {
    target: Shown,
    inherited: bool,
}

/// The font-axis engine, made at the first change a transition or an
/// animation moves, and what the runner presents through it.
#[derive(Default)]
pub(super) struct TextAxes {
    engine: Option<Box<Engine>>,
    moving: BTreeMap<(ViewId, Row), Moving>,
    /// Nodes whose own `transition` or `animation` row names a font row.
    watched: BTreeSet<ViewId>,
    /// Per node, the axes its `font-variation-settings` keyframes name.
    keyframed: BTreeMap<ViewId, Vec<u32>>,
}

impl TextAxes {
    /// Whether a font axis is moving.
    pub(super) fn running(&self) -> bool {
        self.engine.as_ref().is_some_and(|e| !e.quiescent())
    }

    /// When the last one ends, milliseconds.
    pub(super) fn settle_ms(&self) -> Option<f64> {
        self.engine.as_ref()?.settle_time().map(|s| s * 1000.0)
    }
}

/// What a commit's ops change, read before they apply.
pub(super) struct AxesBefore {
    /// Each row that may move, and the value presented before the commit.
    rows: Vec<(ViewId, Row, Shown)>,
    /// The rows the ops set on the node itself.
    own: BTreeSet<(ViewId, Row)>,
    /// The views the ops destroy, subtrees included.
    destroyed: Vec<ViewId>,
}

impl<D: DataSource> Runner<D> {
    /// Before `ops` apply: the font rows they change, directly or through
    /// inheritance on a watched node, with their presented values. Nothing
    /// changes here, so a refused commit has nothing to undo.
    pub(super) fn prepare_text_axes(&self, ops: &[Op]) -> Option<AxesBefore> {
        self.links.text_axes?;
        let mut changed: BTreeSet<(ViewId, Row)> = BTreeSet::new();
        let mut own = BTreeSet::new();
        let mut destroyed = Vec::new();
        for op in ops {
            let (id, mask, sets) = match op {
                Op::SetStyle { id, patch } => (*id, patch.mask, true),
                Op::ClearStyle { id, mask, .. } => (*id, *mask, false),
                Op::DestroyView { id } => {
                    if let Some(node) = self.kernel.node(*id) {
                        let arena = self.kernel.arena();
                        let slots = arena.subtree(node.key.index);
                        destroyed.extend(slots.into_iter().map(|s| arena.local_id(s)));
                    }
                    continue;
                }
                _ => continue,
            };
            if self.kernel.node(id).is_none() {
                continue;
            }
            for row in ROWS.into_iter().filter(|r| mask.has(r.id())) {
                changed.insert((id, row));
                if sets {
                    own.insert((id, row));
                }
            }
        }
        if changed.is_empty() && destroyed.is_empty() {
            return None;
        }
        // A watched node below a changed one may inherit the change.
        let mut rows: BTreeSet<(ViewId, Row)> = changed.clone();
        for &view in &self.text_axes.watched {
            let mut cur = self.kernel.node(view).and_then(|n| n.parent);
            while let Some(at) = cur {
                for row in ROWS {
                    if changed.contains(&(at, row)) {
                        rows.insert((view, row));
                    }
                }
                cur = self.kernel.node(at).and_then(|n| n.parent);
            }
        }
        let rows = rows
            .into_iter()
            .filter_map(|(view, row)| {
                let node = self.kernel.node(view)?;
                Some((view, row, node.computed_row(row.id(), |s| read(s, row))))
            })
            .collect();
        Some(AxesBefore {
            rows,
            own,
            destroyed,
        })
    }

    /// After a commit applied: start, retarget or drop what its changes
    /// move, and write the values presented now as one more commit, which
    /// `receipt` takes in.
    pub(super) fn settle_text_axes(
        &mut self,
        before: Option<AxesBefore>,
        receipt: &CommitReceipt,
    ) -> Result<Option<CommitReceipt>, RunnerError> {
        let Some(make) = self.links.text_axes else {
            return Ok(None);
        };
        let mut dirty: BTreeSet<(ViewId, Row)> = BTreeSet::new();
        let before = before.unwrap_or(AxesBefore {
            rows: Vec::new(),
            own: BTreeSet::new(),
            destroyed: Vec::new(),
        });
        for view in &before.destroyed {
            self.forget_text_axes(*view);
        }
        // Who watches: a node's own `transition` and `animation` rows.
        let arena = self.kernel.arena();
        let mut animated = Vec::new();
        for node_key in exact_kernel::motion::receipt_nodes(receipt) {
            let view = arena.local_id(node_key.index);
            let Some(node) = self.kernel.node(view).filter(|n| n.key == node_key) else {
                continue;
            };
            let covers = (node.style.transition.0.iter())
                .any(|t| ROWS.iter().any(|r| t.property.covers(r.property())));
            let animates = !node.style.animation.0.is_empty()
                && (node.style.animation.properties().into_iter()).any(|p| Row::of(p).is_some());
            if covers || animates {
                self.text_axes.watched.insert(view);
            } else {
                self.text_axes.watched.remove(&view);
            }
            let had = self.text_axes.keyframed.contains_key(&view)
                || (self.text_axes.engine.as_ref())
                    .is_some_and(|e| !e.animation_plays(key(view)).is_empty());
            if animates || had {
                animated.push(view);
            }
        }
        // Parents first: a child's after-change value is its parent's.
        let mut rows = before.rows;
        rows.sort_by_cached_key(|(view, row, _)| (self.depth(*view), *view, *row));
        let now = self.now_ms / 1000.0;
        for (view, row, shown) in rows {
            if self.kernel.node(view).is_none() {
                continue;
            }
            let inherits = !before.own.contains(&(view, row)) && {
                let node = self.kernel.node(view).expect("live");
                let ours = self
                    .text_axes
                    .moving
                    .get(&(view, row))
                    .is_some_and(|m| m.inherited);
                !node.style.mask.has(row.id()) || ours
            };
            let target = if inherits {
                self.target_of(view, row, true)
            } else {
                self.kernel
                    .node(view)
                    .map(|n| read(n.style, row))
                    .expect("live")
            };
            let transitions = self
                .kernel
                .node(view)
                .expect("live")
                .style
                .transition
                .clone();
            let engine = self
                .text_axes
                .engine
                .get_or_insert_with(|| Box::new(make()));
            let at = now.max(engine.now());
            let _ = engine.advance(at);
            if moves(engine, view, row, &shown, &target, transitions) {
                self.text_axes.moving.insert(
                    (view, row),
                    Moving {
                        target,
                        inherited: inherits,
                    },
                );
                dirty.insert((view, row));
            }
        }
        for view in animated {
            self.animate_text_axes(view, make, &mut dirty);
        }
        let ops = self.present_text_axes(dirty);
        if ops.is_empty() {
            return Ok(None);
        }
        self.batch += 1;
        Ok(Some(self.kernel.apply(0, self.batch, &ops)?))
    }

    /// How many ancestors a view has.
    fn depth(&self, view: ViewId) -> usize {
        let mut depth = 0;
        let mut cur = self.kernel.node(view).and_then(|n| n.parent);
        while let Some(at) = cur {
            depth += 1;
            cur = self.kernel.node(at).and_then(|n| n.parent);
        }
        depth
    }

    /// A row's after-change value: the nearest own value up the tree, a
    /// moving row's target in place of what the runner presents, else the
    /// initial value. `skip_self` starts at the parent, for a node that
    /// inherits the row.
    fn target_of(&self, view: ViewId, row: Row, skip_self: bool) -> Shown {
        let mut cur = self.kernel.node(view);
        let mut own = !skip_self;
        while let Some(node) = cur {
            if own {
                if let Some(moving) = self.text_axes.moving.get(&(node.id, row)) {
                    return moving.target.clone();
                }
                if node.style.mask.has(row.id()) {
                    return read(node.style, row);
                }
            }
            // A platform control's start stops inheritance (LLP 1104 D4).
            if node.starts(row.id()) {
                break;
            }
            own = true;
            cur = node.parent.and_then(|p| self.kernel.node(p));
        }
        read(&StyleProps::default(), row)
    }

    /// Forget a destroyed view's rows and axes.
    fn forget_text_axes(&mut self, view: ViewId) {
        let axes = &mut self.text_axes;
        axes.watched.remove(&view);
        let mut tags = axes.keyframed.remove(&view).unwrap_or_default();
        for row in ROWS {
            if let Some(Moving {
                target: Shown::Axes(list),
                ..
            }) = axes.moving.remove(&(view, row))
            {
                tags.extend(list.axes().iter().map(|(t, _)| *t));
            }
        }
        if let Some(engine) = axes.engine.as_mut() {
            engine.remove(key(view));
            for tag in tags {
                engine.remove(axis_key(view, tag));
            }
        }
    }

    /// When the last font-axis transition or finite animation ends,
    /// milliseconds; a host's `settle` takes the later of this and its own
    /// engine's.
    pub fn text_axes_settle_ms(&self) -> Option<f64> {
        self.text_axes.settle_ms()
    }
}

/// Feed one row's change to the engine: whether the runner now presents
/// it. A row the engine has not seen and no transition covers costs
/// nothing, so a frame task that sets a weight each frame stays cheap.
fn moves(
    engine: &mut Engine,
    view: ViewId,
    row: Row,
    before: &Shown,
    after: &Shown,
    transitions: Transitions,
) -> bool {
    let property = row.property();
    let covered = transitions
        .matching(property)
        .is_some_and(|t| t.governing(property).starts());
    let change = |node, value| Change {
        node,
        property,
        value,
        velocity: None,
    };
    match (before, after) {
        (Shown::Number(was), Shown::Number(now)) => {
            let node = key(view);
            let seen = engine.value(node, property).is_some();
            if !covered && !seen {
                return false;
            }
            if !seen {
                let _ = engine.observe(change(node, Value::scalar(*was)));
            }
            let _ = engine.set_transitions(node, transitions);
            let _ = engine.observe(change(node, Value::scalar(*now)));
            true
        }
        (Shown::Axes(was), Shown::Axes(now)) => {
            let seen: Vec<u32> = (was.axes().iter().chain(now.axes()))
                .map(|(t, _)| *t)
                .filter(|t| engine.value(axis_key(view, *t), property).is_some())
                .collect();
            if !covered && seen.is_empty() {
                return false;
            }
            // Different axes are a discrete step: no transition runs.
            let same = sorted_tags(was) == sorted_tags(now);
            for (tag, value) in axis_values(now) {
                let node = axis_key(view, tag);
                if same && !seen.contains(&tag) {
                    let at = was.get(&tag.to_be_bytes()).map_or(value.x, f64::from);
                    let _ = engine.observe(change(node, Value::axis(tag, at)));
                }
                let row = if same && covered {
                    transitions.clone()
                } else {
                    Transitions::default()
                };
                let _ = engine.set_transitions(node, row);
                let _ = engine.observe(change(node, value));
            }
            for (tag, _) in was.axes() {
                if now.get(&tag.to_be_bytes()).is_none()
                    && engine.animation_plays(axis_key(view, *tag)).is_empty()
                {
                    engine.remove(axis_key(view, *tag));
                }
            }
            true
        }
        _ => false,
    }
}

/// The rows a font row's clear restores.
fn mask_of(row: Row) -> StyleMask {
    let mut mask = StyleMask::EMPTY;
    mask.set(row.id());
    mask
}
