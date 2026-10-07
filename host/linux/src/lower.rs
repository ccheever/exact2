//! Animations the Canvas host's reader plays (LLP 1076 on Android, after LLP
//! 1055 D7's lowering on Apple): an `opacity`, `translate`, `scale` or
//! `rotate` animation of a box, an `opacity` or `r` animation of a filled
//! circle, and a stroked shape's `stroke-dashoffset` (a draw-in: the reader
//! records that one path again with the dash phase), are not sampled by the
//! engine. The painter records the node once
//! into a layer at its underlying values ([`Presented::lowered`]); the reader
//! samples the keyframes this module encodes ([`Host::layer_tracks`]) every
//! display frame and moves or fades the layer, as a browser's compositor
//! plays them without the main thread. Rows are not recorded again for them.
//!
//! A node is lowered only when every animation on it is, and nothing the
//! layer cannot show depends on the values: a turned SVG element (its
//! transform is its scene's), a stroked circle (its stroke would scale), a
//! filter, mask or clip, a paint server, a `light-dark()` keyframe, a drag
//! timeline. Anything else is sampled as before.

use super::Host;
#[cfg(target_os = "android")]
use crate::paint::Presented;
use exact_kernel::motion::MotionSync;
use exact_kernel::svg::Paint;
use exact_kernel::{Dimension, NodeKey, NodeType};
#[cfg(target_os = "android")]
use exact_motion::{Easing, StepPosition, Value};
use exact_motion::{PlayedCurve, PlayedTransition, Property};
use exact_runner::DataSource;

/// The properties a reader plays.
pub(crate) const LOWERED: [Property; 8] = [
    Property::Opacity,
    Property::Translate,
    Property::Scale,
    Property::Rotate,
    Property::R,
    Property::StrokeDashoffset,
    Property::Cx,
    Property::Cy,
];

/// [`Presented::lowered`] bits: the reader plays the node's opacity…
pub const LOWER_OPACITY: u8 = 1;
/// …its `translate`, `rotate` and `scale` (all three, about its origin)…
pub const LOWER_TRANSFORM: u8 = 2;
/// …or a filled circle's radius, as a scale about its centre…
pub const LOWER_R: u8 = 4;
/// …or a stroked shape's dash offset, its one path recorded again…
pub const LOWER_DASH: u8 = 8;
/// …or a circle's centre, as a translation from where it is drawn.
pub const LOWER_MOVE: u8 = 16;

/// Track property codes on the wire.
#[cfg(target_os = "android")]
const OPACITY: u32 = 0;
#[cfg(target_os = "android")]
const TX: u32 = 1;
#[cfg(target_os = "android")]
const TY: u32 = 2;
#[cfg(target_os = "android")]
const SCALE: u32 = 3;
#[cfg(target_os = "android")]
const ROTATE: u32 = 4;
#[cfg(target_os = "android")]
const R: u32 = 5;
#[cfg(target_os = "android")]
const DASH: u32 = 6;
#[cfg(target_os = "android")]
const CX: u32 = 7;
#[cfg(target_os = "android")]
const CY: u32 = 8;

impl<D: DataSource> Host<D> {
    /// Lower the reader's properties when the painter is the Canvas host's
    /// (`EXACT_PAINTER=canvas`), unless `EXACT_LOWER=0`. Before the boot
    /// tree's animations start, so none starts sampled.
    pub(super) fn lowering_from_env(&mut self) {
        self.lowering = std::env::var("EXACT_PAINTER").is_ok_and(|p| p == "canvas")
            && !std::env::var("EXACT_LOWER").is_ok_and(|v| v == "0");
        if self.lowering {
            self.engine.set_lowered_properties(&LOWERED);
        }
    }

    /// Per node a sync set animations on: whether the reader can play them
    /// all, else the engine samples them; what it plays, kept by node.
    pub(super) fn lower_eligibility(&mut self, sync: &MotionSync) {
        if !self.lowering {
            return;
        }
        for node in &sync.removed {
            self.lowered_changed.remove(node);
            if self.lowered.remove(node).is_some() | self.played.remove(node).is_some() {
                self.lowered_epoch += 1;
            }
        }
        for (node, animations) in &sync.animations {
            let key = NodeKey {
                index: *node as u32,
                generation: (*node >> 32) as u32,
            };
            let props = animations.properties();
            let playable = !props.is_empty()
                && props.iter().all(|p| LOWERED.contains(p))
                && !animations
                    .0
                    .iter()
                    .any(|a| a.keyframes.0.iter().any(|f| !f.dark.is_empty()))
                && !self.engine.timeline_bound(*node)
                && self
                    .runner
                    .kernel()
                    .node_by_key(key)
                    .is_some_and(|n| playable(&n, &props));
            self.engine.set_node_sampled(*node, !playable);
            let mask = if playable { mask(&props) } else { 0 };
            let old = if mask == 0 {
                self.lowered.remove(node)
            } else {
                self.lowered.insert(*node, mask)
            };
            // Tracks change with the plays: a new start, a pause.
            if old.is_some() || mask != 0 {
                self.lowered_epoch += 1;
                self.lowered_changed.insert(*node, self.lowered_epoch);
            }
        }
    }

    /// The [`Presented::lowered`] bits of a node: what its reader plays.
    pub(crate) fn lowered_mask(&self, key: NodeKey) -> u8 {
        if !self.lowering || (self.lowered.is_empty() && self.played.is_empty()) {
            return 0;
        }
        let node = node_u64(key);
        let played = self
            .played
            .get(&node)
            .map_or(0, |t| mask(&t.iter().map(|(p, _)| *p).collect::<Vec<_>>()));
        self.lowered.get(&node).copied().unwrap_or(0) | played
    }

    /// Hand the reader every transition it can play (opacity, translate,
    /// scale, rotate of a node it can layer): the engine presents their
    /// targets from now on and the reader moves the layer. Before the
    /// commit's presentation, so the painter draws the target. Ended ones go.
    pub(super) fn play_transitions(&mut self) {
        if !self.lowering {
            return;
        }
        let now = self.engine.now();
        let mut ended = Vec::new();
        self.played.retain(|node, t| {
            let before = t.len();
            t.retain(|(_, p)| p.start + played_seconds(p) > now);
            if t.len() != before {
                ended.push(*node);
            }
            !t.is_empty()
        });
        let mut changed = !ended.is_empty();
        let keys: Vec<(u64, Property)> = self
            .engine
            .running_transitions()
            .filter(|(_, p)| {
                matches!(
                    p,
                    Property::Opacity | Property::Translate | Property::Scale | Property::Rotate
                )
            })
            .collect();
        for (node, property) in keys {
            let key = exact_kernel::motion::node_key(node);
            let layered = self
                .runner
                .kernel()
                .node_by_key(key)
                .is_some_and(|n| playable(&n, &[property]));
            if !layered {
                continue;
            }
            if let Some(t) = self.engine.play_transition(node, property) {
                let list = self.played.entry(node).or_default();
                list.retain(|(p, _)| *p != property);
                list.push((property, t));
                ended.push(node);
                changed = true;
            }
        }
        if changed {
            self.lowered_epoch += 1;
            for node in ended {
                self.lowered_changed.insert(node, self.lowered_epoch);
            }
        }
    }

    /// Bumped whenever a lowered node's plays may have changed: the reader's
    /// tracks are encoded again only then.
    #[cfg(target_os = "android")]
    pub(crate) fn lowered_epoch(&self) -> u64 {
        self.lowered_epoch
    }

    /// Whether `key`'s plays changed after `epoch` ([`Host::lowered_epoch`]):
    /// its tracks are encoded again only then, not every layer's at each
    /// change of any (rows mounting in a fling start their own).
    #[cfg(target_os = "android")]
    pub(crate) fn lowered_changed_after(&self, key: NodeKey, epoch: u64) -> bool {
        self.lowered_changed
            .get(&node_u64(key))
            .is_some_and(|at| *at > epoch)
    }

    /// A lowered node's tracks for its reader, as words: per animation and
    /// property (a translation is two, x and y), `[property, start, hold?,
    /// hold, delay, duration, iterations, direction, fill, easing…, frames,
    /// (offset, value, easing…)…]` with every number an `f32`'s bits, the
    /// keyframes with CSS's implicit ones from the underlying `base`. Easing:
    /// `0` none (the animation's), `1` cubic + 4, `2` steps + count +
    /// position, `3` linear() + n + n pairs. Empty when nothing is lowered.
    #[cfg(target_os = "android")]
    pub(crate) fn layer_tracks(&self, key: NodeKey, base: &Presented) -> Vec<u32> {
        let mut out = Vec::new();
        if self.lowered_mask(key) == 0 {
            return out;
        }
        let node = self.runner.kernel().node_by_key(key);
        let r0 = node
            .as_ref()
            .and_then(|n| match n.style.r {
                Dimension::Points(r) => Some(r),
                _ => None,
            })
            .unwrap_or(0.0);
        let point = |d: Dimension| match d {
            Dimension::Points(v) => v,
            _ => 0.0,
        };
        let (cx0, cy0) = node
            .as_ref()
            .map_or((0.0, 0.0), |n| (point(n.style.cx), point(n.style.cy)));
        // Only for a dash track: the one row, where it is set.
        let dash0 = || {
            node.as_ref().map_or(0.0, |n| {
                n.computed_row(exact_kernel::StyleId::StrokeDashoffset, |s| {
                    s.stroke_dashoffset
                })
            })
        };
        for play in self.engine.animation_plays(node_u64(key)) {
            let a = &play.animation;
            for p in a.keyframes.properties() {
                let (underlying, axes): (Value, &[(u32, usize)]) = match p {
                    Property::Opacity => (Value::scalar(base.opacity as f64), &[(OPACITY, 0)]),
                    Property::Translate => (
                        Value::new(base.translate.0 as f64, base.translate.1 as f64),
                        &[(TX, 0), (TY, 1)],
                    ),
                    Property::Scale => (Value::scalar(base.scale as f64), &[(SCALE, 0)]),
                    Property::Rotate => (Value::scalar(base.rotate as f64), &[(ROTATE, 0)]),
                    Property::R => (Value::scalar(r0 as f64), &[(R, 0)]),
                    Property::StrokeDashoffset => (Value::scalar(dash0() as f64), &[(DASH, 0)]),
                    Property::Cx => (Value::scalar(cx0 as f64), &[(CX, 0)]),
                    Property::Cy => (Value::scalar(cy0 as f64), &[(CY, 0)]),
                    _ => continue,
                };
                let track = a.keyframes.track(p, underlying, play.dark);
                for &(code, axis) in axes {
                    out.push(code);
                    f(&mut out, play.start);
                    out.push(play.hold.is_some() as u32);
                    f(&mut out, play.hold.unwrap_or(0.0));
                    f(&mut out, a.delay);
                    f(&mut out, a.duration);
                    f(&mut out, a.iterations);
                    out.push(a.direction as u32);
                    out.push(a.fill as u32);
                    easing(&mut out, Some(&a.easing));
                    out.push(track.len() as u32);
                    for (offset, e, v) in &track {
                        f(&mut out, *offset);
                        f(&mut out, if axis == 1 { v.y } else { v.x });
                        easing(&mut out, *e);
                    }
                }
            }
        }
        // Transitions after animations: CSS's cascade puts them above.
        for (p, t) in self.played.get(&node_u64(key)).into_iter().flatten() {
            let axes: &[(u32, usize)] = match p {
                Property::Opacity => &[(OPACITY, 0)],
                Property::Translate => &[(TX, 0), (TY, 1)],
                Property::Scale => &[(SCALE, 0)],
                Property::Rotate => &[(ROTATE, 0)],
                _ => continue,
            };
            let axis = |v: &Value, a: usize| if a == 1 { v.y } else { v.x };
            for &(code, a) in axes {
                out.push(code);
                f(&mut out, t.start);
                out.push(0);
                f(&mut out, 0.0);
                f(&mut out, 0.0);
                f(&mut out, played_seconds(t));
                f(&mut out, 1.0);
                out.push(0);
                // Backwards: during its delay it shows where it starts.
                out.push(2);
                match &t.curve {
                    PlayedCurve::Easing { easing: e, .. } => {
                        easing(&mut out, Some(e));
                        out.push(2);
                        for (offset, v) in [(0.0, &t.from), (1.0, &t.to)] {
                            f(&mut out, offset);
                            f(&mut out, axis(v, a));
                            out.push(0);
                        }
                    }
                    PlayedCurve::Frames { values, .. } => {
                        easing(&mut out, Some(&Easing::Linear));
                        out.push(values.len() as u32);
                        let last = values.len().saturating_sub(1).max(1) as f64;
                        for (i, v) in values.iter().enumerate() {
                            f(&mut out, i as f64 / last);
                            f(&mut out, axis(v, a));
                            out.push(0);
                        }
                    }
                }
            }
        }
        out
    }
}

/// A played transition's length, seconds.
fn played_seconds(t: &PlayedTransition) -> f64 {
    match &t.curve {
        PlayedCurve::Easing { duration, .. } | PlayedCurve::Frames { duration, .. } => *duration,
    }
}

/// The [`Presented::lowered`] bits for these animated properties.
fn mask(props: &[Property]) -> u8 {
    props.iter().fold(0, |m, p| {
        m | match p {
            Property::Opacity => LOWER_OPACITY,
            Property::Translate | Property::Scale | Property::Rotate => LOWER_TRANSFORM,
            Property::R => LOWER_R,
            Property::StrokeDashoffset => LOWER_DASH,
            Property::Cx | Property::Cy => LOWER_MOVE,
            _ => 0,
        }
    })
}

fn node_u64(key: NodeKey) -> u64 {
    (key.generation as u64) << 32 | key.index as u64
}

#[cfg(target_os = "android")]
fn f(out: &mut Vec<u32>, v: f64) {
    out.push((v as f32).to_bits());
}

#[cfg(target_os = "android")]
fn easing(out: &mut Vec<u32>, e: Option<&Easing>) {
    let cubic = |out: &mut Vec<u32>, c: [f64; 4]| {
        out.push(1);
        for v in c {
            f(out, v);
        }
    };
    match e {
        None => out.push(0),
        Some(Easing::Linear) => cubic(out, [0.0, 0.0, 1.0, 1.0]),
        Some(Easing::Ease) => cubic(out, [0.25, 0.1, 0.25, 1.0]),
        Some(Easing::EaseIn) => cubic(out, [0.42, 0.0, 1.0, 1.0]),
        Some(Easing::EaseOut) => cubic(out, [0.0, 0.0, 0.58, 1.0]),
        Some(Easing::EaseInOut) => cubic(out, [0.42, 0.0, 0.58, 1.0]),
        Some(Easing::CubicBezier { x1, y1, x2, y2 }) => cubic(out, [*x1, *y1, *x2, *y2]),
        Some(Easing::Steps { count, position }) => {
            out.extend([2, *count as u32]);
            out.push(match position {
                StepPosition::JumpStart => 0,
                StepPosition::JumpEnd => 1,
                StepPosition::JumpNone => 2,
                StepPosition::JumpBoth => 3,
            });
        }
        Some(Easing::PiecewiseLinear(stops)) => {
            out.extend([3, stops.len() as u32]);
            for s in stops {
                f(out, s.input);
                f(out, s.output);
            }
        }
    }
}

/// Whether the reader can show these animated properties of `n` as CSS
/// does: a box's transform and opacity; an SVG element's opacity; a filled
/// circle's radius as a scale about its centre.
fn playable(n: &exact_kernel::NodeRef<'_>, props: &[Property]) -> bool {
    let s = n.style;
    let svg = n.node_type.is_svg_element();
    let served = |p: &Paint| matches!(p, Paint::Url(..));
    let turns = props
        .iter()
        .any(|p| matches!(p, Property::Translate | Property::Scale | Property::Rotate));
    if svg {
        // The scene keeps an animated element's transform in parts about its
        // origin; a non-scaling stroke would scale, a clip or effect would not move.
        let moves = props
            .iter()
            .any(|p| matches!(p, Property::Cx | Property::Cy));
        if !s.filter.is_none()
            || s.rare.svg_mask.url().is_some()
            || s.rare.clip_path.url().is_some()
            || served(&s.fill)
            || served(&s.stroke)
            || (turns && s.vector_effect == exact_kernel::VectorEffect::NonScalingStroke)
        {
            return false;
        }
        let point = |d: Dimension| matches!(d, Dimension::Points(_));
        // A circle's centre is a translation of its drawing, when its centre
        // is a length; not with a turn (two pivots).
        if moves && (n.node_type != NodeType::SvgCircle || turns || !point(s.cx) || !point(s.cy)) {
            return false;
        }
        // A radius scales the whole drawing: a fill alone. A dash offset is one
        // stroked path's phase; the two in one layer would scale the dash.
        if props.contains(&Property::R) {
            return n.node_type == NodeType::SvgCircle
                && !turns
                && matches!(s.stroke, Paint::None)
                && !props.contains(&Property::StrokeDashoffset)
                && matches!(s.r, Dimension::Points(r) if r > 0.0);
        }
        if props.contains(&Property::StrokeDashoffset) {
            return !turns && n.node_type.is_svg_shape() && !matches!(s.stroke, Paint::None);
        }
        return true;
    }
    !props.contains(&Property::R)
}
