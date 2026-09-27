//! CSS animations lowered to Core Animation keyframe specs (LLP 1055 D7,
//! LLP 1055.000 D6 and D15).
//!
//! Each lowered property of each playing animation becomes one
//! `CAKeyframeAnimation` spec: key times, values and one cubic per segment,
//! with the direction folded into one explicit period. A colour lowers only
//! where Core Animation's unpremultiplied interpolation equals CSS's
//! premultiplied one: every colour in the track has the same alpha
//! ([`eligible`]); anything else is sampled by the engine.

use crate::style::num;
use exact_kernel::id::NodeKey;
use exact_kernel::motion::MotionSync;
use exact_kernel::{Kernel, NodeType};
use exact_motion::animation::Keyframe;
use exact_motion::{AnimationPlay, Easing, Engine, Property, Value};
use std::fmt::Write as _;

/// One CA keyframe track: key times, values and a cubic per segment.
struct Track {
    times: Vec<f64>,
    values: Vec<Value>,
    curves: Vec<[f64; 4]>,
}

fn bezier(e: &Easing) -> Option<[f64; 4]> {
    Some(match e {
        Easing::Linear => [0.0, 0.0, 1.0, 1.0],
        // CSS `ease`, not Core Animation's default curve.
        Easing::Ease => [0.25, 0.1, 0.25, 1.0],
        Easing::EaseIn => [0.42, 0.0, 1.0, 1.0],
        Easing::EaseOut => [0.0, 0.0, 0.58, 1.0],
        Easing::EaseInOut => [0.42, 0.0, 0.58, 1.0],
        Easing::CubicBezier { x1, y1, x2, y2 } => [*x1, *y1, *x2, *y2],
        Easing::Steps { .. } | Easing::PiecewiseLinear(_) => return None,
    })
}

const LINEAR: [f64; 4] = [0.0, 0.0, 1.0, 1.0];

/// One forward iteration of `property`, keyframe easings as cubics; a
/// `steps()` or `linear()` interval becomes linear sub-keyframes at its own
/// breakpoints (a step is a hold: two keys a hair apart).
fn forward(frames: &[Keyframe], default: &Easing, property: Property, underlying: Value) -> Track {
    let mut pts: Vec<(f64, Option<&Easing>, Value)> = frames
        .iter()
        .filter_map(|f| {
            f.values
                .iter()
                .find(|(p, _)| *p == property)
                .map(|(_, v)| (f.offset, f.easing.as_ref(), *v))
        })
        .collect();
    if pts.first().is_none_or(|p| p.0 > 0.0) {
        pts.insert(0, (0.0, None, underlying));
    }
    if pts.last().is_none_or(|p| p.0 < 1.0) {
        pts.push((1.0, None, underlying));
    }
    let mut t = Track {
        times: vec![pts[0].0],
        values: vec![pts[0].2],
        curves: Vec::new(),
    };
    for w in pts.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let easing = a.1.unwrap_or(default);
        // Core Animation interpolates colours in the display's colour space,
        // not CSS's sRGB: a colour interval is sampled every sixteenth, so
        // its own interpolation only bridges near neighbours.
        let curve = bezier(easing).filter(|_| !property.is_color());
        match curve {
            Some(c) => {
                t.times.push(b.0);
                t.values.push(b.2);
                t.curves.push(c);
            }
            None => {
                let span = b.0 - a.0;
                let mut xs: Vec<f64> = match easing {
                    Easing::Steps { count, .. } => (1..*count)
                        .map(|j| j as f64 / *count as f64)
                        .flat_map(|x| [x - 1e-4, x])
                        .chain([1e-4, 1.0 - 1e-4])
                        .collect(),
                    Easing::PiecewiseLinear(stops) => stops.iter().map(|s| s.input).collect(),
                    _ => Vec::new(),
                };
                if property.is_color() {
                    xs.extend((1..16).map(|j| j as f64 / 16.0));
                }
                xs.push(1.0);
                xs.retain(|x| *x > 0.0 && *x <= 1.0);
                xs.sort_by(f64::total_cmp);
                xs.dedup();
                for x in xs {
                    t.times.push(a.0 + span * x);
                    t.values.push(a.2.lerp(b.2, easing.progress(x)));
                    t.curves.push(LINEAR);
                }
            }
        }
    }
    t
}

/// The same iteration played backwards: times mirrored, each cubic reversed
/// in time (CSS `reverse`: an ease-out interval traversed backwards).
fn reversed(t: &Track) -> Track {
    Track {
        times: t.times.iter().rev().map(|x| 1.0 - x).collect(),
        values: t.values.iter().rev().copied().collect(),
        curves: t
            .curves
            .iter()
            .rev()
            .map(|c| [1.0 - c[2], 1.0 - c[3], 1.0 - c[0], 1.0 - c[1]])
            .collect(),
    }
}

/// Two tracks, one after the other, in one period.
fn joined(a: &Track, b: &Track) -> Track {
    let mut t = Track {
        times: a.times.iter().map(|x| x * 0.5).collect(),
        values: a.values.clone(),
        curves: a.curves.clone(),
    };
    for (i, x) in b.times.iter().enumerate().skip(1) {
        t.times.push(0.5 + x * 0.5);
        t.values.push(b.values[i]);
        t.curves.push(b.curves[i - 1]);
    }
    t
}

/// A node's lowered animations for `props`, as CA specs:
/// `[{"id","k","s","dl","d","n","t":[…],"v":[…],"c":[[…],…],"fill","h"}]`.
/// `underlying(p)` is the property's own value and a factor into CA units:
/// a dash offset's length over `pathLength`, or a paint's opacity folded
/// into a colour's alpha. A colour's values are `[r,g,b,a]` bytes.
pub(crate) fn specs(
    engine: &Engine,
    key: u64,
    props: &[Property],
    underlying: &dyn Fn(Property) -> (Value, f64),
) -> String {
    let mut s = String::from("[");
    let mut first = true;
    for (i, play) in engine.animation_plays(key).iter().enumerate() {
        if engine.node_sampled(key) {
            // The engine samples every animation on this node.
            break;
        }
        for p in play.animation.keyframes.properties() {
            if !props.contains(&p) {
                continue;
            }
            if !first {
                s.push(',');
            }
            first = false;
            spec(play, i, p, underlying(p), &mut s);
        }
    }
    s.push(']');
    s
}

fn spec(
    play: &AnimationPlay,
    index: usize,
    p: Property,
    (base, scale): (Value, f64),
    s: &mut String,
) {
    let a = &play.animation;
    // Keyframes and the underlying value are in the author's units (a dash
    // offset in `pathLength` units); CA's are the path's own.
    let fwd = forward(&a.keyframes.0, &a.easing, p, base);
    let (track, period, repeat) = match a.direction {
        exact_motion::Direction::Normal => (fwd, a.duration, a.iterations),
        exact_motion::Direction::Reverse => (reversed(&fwd), a.duration, a.iterations),
        exact_motion::Direction::Alternate => (
            joined(&fwd, &reversed(&fwd)),
            a.duration * 2.0,
            a.iterations / 2.0,
        ),
        exact_motion::Direction::AlternateReverse => (
            joined(&reversed(&fwd), &fwd),
            a.duration * 2.0,
            a.iterations / 2.0,
        ),
    };
    let key = match p {
        Property::Opacity => "opacity",
        Property::StrokeDashoffset => "lineDashPhase",
        Property::Fill => "fillColor",
        Property::Stroke => "strokeColor",
        _ => "r",
    };
    let list = |v: &[f64]| {
        v.iter()
            .map(|n| num(*n as f32))
            .collect::<Vec<_>>()
            .join(",")
    };
    let values: Vec<String> = track
        .values
        .iter()
        .map(|v| {
            if p.is_color() {
                let [r, g, b, al] = v.to_rgba8();
                format!("[{r},{g},{b},{}]", ((al as f64) * scale).round() as u8)
            } else {
                num((v.x * scale) as f32)
            }
        })
        .collect();
    let curves: Vec<String> = track
        .curves
        .iter()
        .map(|c| format!("[{}]", list(c)))
        .collect();
    let _ = write!(
        s,
        "{{\"id\":\"{}#{index}#{key}\",\"k\":\"{key}\",\"s\":{},\"dl\":{},\"d\":{},\"n\":{},\"t\":[{}],\"v\":[{}],\"c\":[{}],\"fill\":{},\"h\":",
        a.name.replace(['"', '\\'], ""),
        play.start,
        a.delay,
        period,
        if repeat.is_infinite() { -1.0 } else { repeat },
        list(&track.times),
        values.join(","),
        curves.join(","),
        a.fill as u8,
    );
    match play.hold {
        Some(h) => {
            let _ = write!(s, "{h}");
        }
        None => s.push_str("null"),
    }
    s.push('}');
}

/// Per node, whether Core Animation can play its lowered animations as
/// CSS does (LLP 1055.000 D15); a node it cannot is sampled by the engine.
/// An inherited property animated on a container reaches its descendants,
/// which only a sampled scene shows; a colour lowers only when every colour
/// in the track, its underlying one included, has one alpha (then Core
/// Animation's unpremultiplied interpolation is CSS's premultiplied one).
pub(crate) fn eligibility(kernel: &Kernel, engine: &mut Engine, sync: &MotionSync) {
    for (node, animations) in &sync.animations {
        let key = NodeKey {
            index: *node as u32,
            generation: (*node >> 32) as u32,
        };
        let Some(n) = kernel.node_by_key(key) else {
            continue;
        };
        let props = animations.properties();
        let sampled = if n.node_type.is_svg_shape() {
            let targets: Vec<(Property, Option<Value>)> = exact_kernel::motion::color_targets(&n);
            let mut alphas = animations
                .0
                .iter()
                .flat_map(|a| a.keyframes.0.iter())
                .flat_map(|f| f.values.iter())
                .filter(|(p, _)| p.is_color())
                .map(|(_, v)| Some(v.w))
                .chain(
                    targets
                        .iter()
                        .filter(|(p, _)| props.contains(p))
                        .map(|(_, v)| v.map(|v| v.w)),
                );
            let first = alphas.next().flatten();
            alphas.any(|a| a.is_none() || a != first)
        } else if n.node_type.is_svg_element() || n.node_type == NodeType::Svg {
            props.iter().any(|p| {
                matches!(
                    p,
                    Property::Fill
                        | Property::Stroke
                        | Property::Color
                        | Property::StrokeDashoffset
                )
            })
        } else {
            false
        };
        engine.set_node_sampled(*node, sampled);
    }
}
