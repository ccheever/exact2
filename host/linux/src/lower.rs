//! Animations the Canvas host's reader plays (LLP 1076 on Android, after LLP
//! 1055 D7's lowering on Apple): an `opacity`, `translate`, `scale` or
//! `rotate` animation of a box, and an `opacity` or `r` animation of a filled
//! circle, are not sampled by the engine. The painter records the node once
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
use exact_motion::Property;
#[cfg(target_os = "android")]
use exact_motion::{Easing, StepPosition, Value};
use exact_runner::DataSource;

/// The properties a reader plays.
pub(crate) const LOWERED: [Property; 5] = [
    Property::Opacity,
    Property::Translate,
    Property::Scale,
    Property::Rotate,
    Property::R,
];

/// [`Presented::lowered`] bits: the reader plays the node's opacity…
pub const LOWER_OPACITY: u8 = 1;
/// …its `translate`, `rotate` and `scale` (all three, about its origin)…
pub const LOWER_TRANSFORM: u8 = 2;
/// …or a filled circle's radius, as a scale about its centre.
pub const LOWER_R: u8 = 4;

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
            if self.lowered.remove(node).is_some() {
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
            }
        }
    }

    /// The [`Presented::lowered`] bits of a node: what its reader plays.
    pub(crate) fn lowered_mask(&self, key: NodeKey) -> u8 {
        if !self.lowering || self.lowered.is_empty() {
            return 0;
        }
        self.lowered.get(&node_u64(key)).copied().unwrap_or(0)
    }

    /// Bumped whenever a lowered node's plays may have changed: the reader's
    /// tracks are encoded again only then.
    #[cfg(target_os = "android")]
    pub(crate) fn lowered_epoch(&self) -> u64 {
        self.lowered_epoch
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
        let r0 = self
            .runner
            .kernel()
            .node_by_key(key)
            .and_then(|n| match n.style.r {
                Dimension::Points(r) => Some(r),
                _ => None,
            })
            .unwrap_or(0.0);
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
        out
    }
}

/// The [`Presented::lowered`] bits for these animated properties.
fn mask(props: &[Property]) -> u8 {
    props.iter().fold(0, |m, p| {
        m | match p {
            Property::Opacity => LOWER_OPACITY,
            Property::Translate | Property::Scale | Property::Rotate => LOWER_TRANSFORM,
            Property::R => LOWER_R,
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
        // An element's transform and geometry are its scene's.
        if turns
            || !s.filter.is_none()
            || s.svg_mask.url().is_some()
            || s.clip_path.url().is_some()
            || served(&s.fill)
            || served(&s.stroke)
        {
            return false;
        }
        if props.contains(&Property::R) {
            return n.node_type == NodeType::SvgCircle
                && matches!(s.stroke, Paint::None)
                && matches!(s.r, Dimension::Points(r) if r > 0.0);
        }
        return true;
    }
    !props.contains(&Property::R)
}
