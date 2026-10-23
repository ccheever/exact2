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
use exact_motion::{AnimationPlay, Easing, Engine, LinearStop, PlayedCurve, Property, Value};
use std::fmt::Write as _;

/// One CA keyframe track: key times, values and a cubic per segment.
struct Track {
    times: Vec<f64>,
    values: Vec<Value>,
    curves: Vec<[f64; 4]>,
}

pub(crate) fn bezier(e: &Easing) -> Option<[f64; 4]> {
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

/// A key's distance before a jump it approaches, in an interval's progress,
/// at most half the gap to the key before it.
const HAIR: f64 = 1e-4;

/// One forward iteration of `property`, keyframe easings as cubics; a
/// `steps()` or `linear()` interval becomes linear sub-keyframes at its own
/// breakpoints ([`stepped_keys`]). Each interval starts with a key at its
/// own keyframe's value, as the engine samples it at the keyframe's offset:
/// an interval that ends elsewhere (a `linear()` extrapolated past its last
/// stop) keeps its end a hair before the boundary ([`begin`]).
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
        // Two keyframes at one offset: the later one holds from there, which
        // the next interval's start says.
        if b.0 <= a.0 {
            continue;
        }
        let easing = a.1.unwrap_or(default);
        // Core Animation interpolates colours in the display's colour space,
        // not CSS's sRGB: a colour interval is sampled every sixteenth, so
        // its own interpolation only bridges near neighbours.
        let colour = property.is_color();
        match bezier(easing).filter(|_| !colour) {
            Some(c) => {
                begin(&mut t, a.0, a.2);
                push(&mut t, b.0, b.2, c);
            }
            None => stepped_keys(&mut t, a.0, b.0, easing, colour, |p| a.2.lerp(b.2, p)),
        }
    }
    t
}

/// The interval from `from` to `to` under an easing Core Animation can't
/// say, as linear keys. `steps()` is the `linear()` it equals (each step a
/// pair of stops at one input); a cubic, which only a colour reaches here,
/// is sampled. Between keys the curve is straight, so Core Animation plays
/// it exactly; a colour also has a key every sixteenth.
///
/// A jump is two stops at one input, never inferred from values. Its key at
/// the input has the curve's output there (the last of those stops), and a
/// key a hair before it the curve's output at that point, on the line that
/// approaches the jump. Keys are placed in increasing progress, each after
/// the one before it, so none replaces another.
///
/// A colour extrapolated out of gamut is clamped at its keys, and Core
/// Animation draws a straight line across where the engine's clamp bends
/// it, within a sixteenth of the interval, as with colour space.
fn stepped_keys(
    t: &mut Track,
    from: f64,
    to: f64,
    easing: &Easing,
    colour: bool,
    value: impl Fn(f64) -> Value,
) {
    let stops: Vec<LinearStop> = match easing {
        Easing::PiecewiseLinear(stops) => stops.clone(),
        Easing::Steps { count, .. } => {
            let n = (*count).max(1) as f64;
            let mut stops: Vec<LinearStop> = (0..*count)
                .flat_map(|k| {
                    let output = easing.progress((k as f64 + 0.5) / n);
                    [k as f64 / n, (k as f64 + 1.0) / n].map(|input| LinearStop { input, output })
                })
                .collect();
            stops.push(LinearStop {
                input: 1.0,
                output: easing.progress(1.0),
            });
            stops
        }
        _ => (0..=16)
            .map(|j| j as f64 / 16.0)
            .map(|input| LinearStop {
                input,
                output: easing.progress(input),
            })
            .collect(),
    };
    // Keys visit inputs in order, including the hair before a jump.
    let mut cursor = 0;
    let mut progress = |x: f64| {
        if stops.is_empty() {
            return x;
        }
        while cursor + 1 < stops.len() && stops[cursor + 1].input <= x {
            cursor += 1;
        }
        if stops.len() == 1 || stops[cursor].input == x {
            return stops[cursor].output;
        }
        let (a, b) = if x < stops[0].input {
            (stops[0], stops[1])
        } else if cursor + 1 < stops.len() {
            (stops[cursor], stops[cursor + 1])
        } else {
            (stops[stops.len() - 2], stops[stops.len() - 1])
        };
        if a.input == b.input {
            b.output
        } else {
            a.output + (b.output - a.output) * (x - a.input) / (b.input - a.input)
        }
    };
    let span = to - from;
    begin(t, from, value(progress(0.0)));
    // Each input in (0, 1], ascending, and whether stops share it (a jump):
    // a stop's, or a colour's sixteenth.
    let mut inputs: Vec<(f64, bool)> = Vec::new();
    for s in &stops {
        match inputs.last_mut() {
            Some((x, jump)) if *x == s.input => *jump = true,
            _ if s.input > 0.0 => inputs.push((s.input, false)),
            _ => {}
        }
    }
    if colour {
        inputs.extend((1..16).map(|j| (j as f64 / 16.0, false)));
        inputs.sort_by(|a, b| a.0.total_cmp(&b.0));
        inputs.dedup_by(|later, kept| {
            let same = later.0 == kept.0;
            kept.1 |= same && later.1;
            same
        });
    }
    if inputs.last().is_none_or(|(x, _)| *x < 1.0) {
        inputs.push((1.0, false));
    }
    let mut prev = 0.0;
    for (x, jump) in inputs {
        if jump {
            let at = x - HAIR.min((x - prev) / 2.0);
            push(t, from + span * at, value(progress(at)), LINEAR);
        }
        push(t, from + span * x, value(progress(x)), LINEAR);
        prev = x;
    }
}

/// Start an interval at `at` with `v`: the track's first key takes it; an
/// interval that ended at another value keeps that end a hair (of its last
/// segment, straight wherever an end can differ) before `at`, at the value
/// its segment has there.
fn begin(t: &mut Track, at: f64, v: Value) {
    let n = t.times.len();
    if n == 1 {
        t.values[0] = v;
    } else if t.values[n - 1] != v {
        t.times[n - 1] -= HAIR * (t.times[n - 1] - t.times[n - 2]);
        t.values[n - 1] = t.values[n - 1].lerp(t.values[n - 2], HAIR);
        push(t, at, v, LINEAR);
    }
}

fn push(t: &mut Track, at: f64, v: Value, curve: [f64; 4]) {
    t.times.push(at);
    t.values.push(v);
    t.curves.push(curve);
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
/// `[{"id","k","s","dl","d","n","t":[…],"v":[…],"c":[[…],…],"fill","h"}]`;
/// `"c"` is empty when every interval is linear.
/// `underlying(p)` is the property's own value and a factor into CA units:
/// a dash offset's length over `pathLength`, or a paint's opacity folded
/// into a colour's alpha. A colour's values are `[r,g,b,a]` bytes.
pub(crate) fn specs(
    engine: &Engine,
    key: u64,
    props: &[Property],
    underlying: &dyn Fn(Property) -> (Value, f64),
) -> String {
    specs_in(engine, key, props, underlying, None)
}

/// [`specs`] for a box whose border box is `size`: a `translate` given as a
/// percentage of it is resolved to points here (LLP 1115.003 D4), and the
/// host sends the box's specs again when it resizes.
pub(crate) fn specs_in(
    engine: &Engine,
    key: u64,
    props: &[Property],
    underlying: &dyn Fn(Property) -> (Value, f64),
    size: Option<(f64, f64)>,
) -> String {
    if !crate::host::svg::render_server() {
        return "[]".into();
    }
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
            // A translation is two tracks, one per axis.
            let parts: &[(usize, &str)] = match p {
                Property::Translate => &[
                    (0, "transform.translation.x"),
                    (1, "transform.translation.y"),
                ],
                Property::Opacity => &[(0, "opacity")],
                Property::StrokeDashoffset => &[(0, "lineDashPhase")],
                // A centred circle's layer sits at its centre.
                Property::Cx => &[(0, "position.x")],
                Property::Cy => &[(0, "position.y")],
                Property::Fill => &[(0, "fillColor")],
                Property::Stroke => &[(0, "strokeColor")],
                Property::Scale => &[(0, "transform.scale")],
                Property::Rotate => &[(0, "transform.rotation.z")],
                Property::BackgroundColor => &[(0, "backgroundColor")],
                _ => &[(0, "r")],
            };
            for (axis, key) in parts {
                if !first {
                    s.push(',');
                }
                first = false;
                spec(play, i, p, underlying(p), *axis, key, size, &mut s);
            }
        }
    }
    s.push(']');
    s
}

#[allow(clippy::too_many_arguments)]
fn spec(
    play: &AnimationPlay,
    index: usize,
    p: Property,
    (base, scale): (Value, f64),
    axis: usize,
    key: &str,
    size: Option<(f64, f64)>,
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
    let (times, values, curves) = track_json(&track, p, scale, axis, size);
    let _ = write!(
        s,
        "{{\"id\":\"{}#{index}#{key}\",\"k\":\"{key}\",\"s\":{},\"dl\":{},\"d\":{},\"n\":{},\"t\":[{}],\"v\":[{}],\"c\":[{}],\"fill\":{},\"h\":",
        a.name.replace(['"', '\\'], ""),
        play.start,
        a.delay,
        period,
        if repeat.is_infinite() { -1.0 } else { repeat },
        times,
        values,
        curves,
        a.fill as u8,
    );
    // A play waiting for the first presented frame is held where it waits
    // (LLP 1003.001 D4): Core Animation never runs ahead of the engine.
    match play.hold.or(play.pending.map(|begin| begin - play.start)) {
        Some(h) => {
            let _ = write!(s, "{h}");
        }
        None => s.push_str("null"),
    }
    s.push('}');
}

/// A track's key times, values and curves as a spec's `t`, `v` and `c`.
fn track_json(
    track: &Track,
    p: Property,
    scale: f64,
    axis: usize,
    size: Option<(f64, f64)>,
) -> (String, String, String) {
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
            } else if p == Property::Translate {
                // Points plus the percentage of the box along the axis.
                let (w, h) = size.unwrap_or_default();
                let v = if axis == 1 {
                    v.y + v.w / 100.0 * h
                } else {
                    v.x + v.z / 100.0 * w
                };
                num((v * scale) as f32)
            } else {
                num(((if axis == 1 { v.y } else { v.x }) * scale) as f32)
            }
        })
        .collect();
    // Every interval linear (a `steps()` track is ~2n linear keys) says
    // nothing: Core Animation's keyframes are linear without timing
    // functions, and a list row's playhead then carries no 4n curve numbers
    // to build, parse, hash and turn into timing-function objects.
    let curves: Vec<String> = if track.curves.iter().all(|c| *c == LINEAR) {
        Vec::new()
    } else {
        track
            .curves
            .iter()
            .map(|c| format!("[{}]", list(c)))
            .collect()
    };
    (list(&track.times), values.join(","), curves.join(","))
}

/// Per node, whether Core Animation can play its lowered animations as
/// CSS does (LLP 1055.000 D15, LLP 1055.001); a node it cannot is sampled by the engine.
/// An inherited property animated on a container reaches its descendants,
/// which only a sampled scene shows; a colour lowers only when every colour
/// in the track, its underlying one included, has one alpha (then Core
/// Animation's unpremultiplied interpolation is CSS's premultiplied one).
///
/// A box's transform (`box_motion`, iOS) lowers when it turns about its
/// centre, no layout transition moves it, and nothing in it takes input:
/// Core Animation moves only the presentation, and UIKit hit-tests the
/// model, where a browser hit-tests what it shows. Its background colour
/// lowers when the layer paints it: no border and no `background-image`
/// (else the view draws its box), every colour of one alpha. An SVG
/// element's transform is its scene's, which samples it.
pub(crate) fn eligibility(
    kernel: &Kernel,
    engine: &mut Engine,
    sync: &MotionSync,
    box_motion: bool,
    interactive: &dyn Fn(exact_kernel::ViewId) -> bool,
) {
    for (node, animations) in &sync.animations {
        let key = NodeKey {
            index: *node as u32,
            generation: (*node >> 32) as u32,
        };
        let Some(n) = kernel.node_by_key(key) else {
            continue;
        };
        let props = animations.properties();
        // A shape painted by a server draws parts Core Animation's lowered
        // keys do not reach: its scene is sampled.
        let served = |id: exact_kernel::StyleId| {
            matches!(
                n.computed(id),
                exact_kernel::RowValue::Paint(exact_kernel::svg::Paint::Url(..))
            )
        };
        // A `light-dark()` keyframe plays the appearance its animation
        // started under (LLP 1062 D9), which a lowered track cannot say.
        let paired = animations
            .0
            .iter()
            .any(|a| a.keyframes.0.iter().any(|f| !f.dark.is_empty()));
        let svg = n.node_type.is_svg_element() || n.node_type == NodeType::Svg;
        let boxed = box_motion && props.iter().any(|p| super::svg::BOX_LOWERED.contains(p));
        // A drag timeline's consumer (LLP 1057.003 D2) runs on the drag,
        // not Core Animation's clock: the engine samples it in the frame
        // (and the hold's reply) that moves its source.
        let moves = props
            .iter()
            .any(|p| matches!(p, Property::Cx | Property::Cy));
        // A percentage of the box (chess diary #4) is resolved by the
        // presenter against the box as it stands. A lowered box's track
        // resolves it when its spec is made and is made again when the box
        // resizes (`svg.rs`); an SVG element's scene samples it.
        let percent = n.style.translate_percent.x != 0.0
            || n.style.translate_percent.y != 0.0
            || animations.0.iter().any(|a| {
                a.keyframes.0.iter().any(|f| {
                    f.values
                        .iter()
                        .any(|(p, v)| *p == Property::Translate && (v.z != 0.0 || v.w != 0.0))
                })
            });
        let sampled = if paired
            || (percent && !(boxed && !svg))
            || engine.timeline_bound(*node)
            || under_box_filter(kernel, &n)
            // A glass group ignores the opacity Core Animation plays between
            // it and its glass; the host isolates on what it is told.
            || (props.contains(&Property::Opacity) && in_glass_group(kernel, &n))
            // What the host paints over the node's own (a press, a look: LLP
            // 1061 D7, LLP 1121 D8) composes on the engine's sample.
            || props.iter().any(|p| crate::looks::paints_over(&n, *p))
            // Drawn into an island's pixels, which Core Animation does not
            // animate (except a live filter picture on iOS).
            || (svg && in_picture(kernel, &n, box_motion))
            || (boxed && svg && !svg_turns(&n, &props))
            || (moves && !circle_moves(&n, &props))
        {
            true
        } else if boxed && !svg {
            !box_eligible(kernel, &n, &props, interactive)
        } else if n.node_type.is_svg_shape()
            && (served(exact_kernel::StyleId::Fill) || served(exact_kernel::StyleId::Stroke))
        {
            true
        } else if n.node_type.is_svg_shape() {
            let targets: Vec<(Property, Option<Value>)> =
                exact_kernel::motion::color_targets(&n, false);
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

/// LLP 1115.003 D4: each running `d` transition Core Animation can play as
/// CSS does goes to it ([`Engine::play_transition`]): the engine presents the
/// target, and the shape's scene carries a `path` animation between the two
/// ends (`svg.rs`), so the morph plays on the render server whatever the
/// main thread is doing. A shape keeps its sampled morph when Core
/// Animation cannot draw what the engine would: a paint server (its parts
/// are drawn by Core Graphics), a non-scaling stroke or markers (both
/// follow the path's vertices), an island's pixels, a box filter, or two
/// ends whose segments differ (an arc flattens to as many cubics as its
/// sweep needs; Core Animation interpolates only paths of the same
/// elements). An `svg`'s morphs go together or not at all, so a line never
/// runs on while the area under it is sampled (the chart's).
pub(crate) fn hand_over_paths(kernel: &Kernel, engine: &mut Engine, live: bool) {
    use exact_kernel::svg::Paint;
    if !crate::host::svg::render_server() {
        return;
    }
    let moving: Vec<u64> = engine
        .running_transitions()
        .filter(|(_, p)| *p == Property::D)
        .map(|(node, _)| node)
        .collect();
    let mut scenes: std::collections::BTreeMap<Option<exact_kernel::ViewId>, (bool, Vec<u64>)> =
        Default::default();
    for node in moving {
        let key = NodeKey {
            index: node as u32,
            generation: (node >> 32) as u32,
        };
        let Some(n) = kernel.node_by_key(key) else {
            continue;
        };
        let s = n.style;
        let served = |p: &Paint| matches!(p, Paint::Url(..));
        let plain = n.node_type == NodeType::SvgPath
            && !served(&s.fill)
            && !served(&s.stroke)
            && s.vector_effect != exact_kernel::VectorEffect::NonScalingStroke
            && !marked(kernel, &n)
            && !in_picture(kernel, &n, live)
            && !under_box_filter(kernel, &n);
        let same = engine.path_ends(node).is_some_and(|(from, to)| {
            let (a, b) = (
                exact_kernel::svg::parse_d(&from.to_d()),
                exact_kernel::svg::parse_d(&to.to_d()),
            );
            a.0.len() == b.0.len()
                && a.0
                    .iter()
                    .zip(&b.0)
                    .all(|(x, y)| std::mem::discriminant(x) == std::mem::discriminant(y))
        });
        let mut root = n.parent;
        while let Some(r) = root.and_then(|id| kernel.node(id)) {
            if r.node_type == NodeType::Svg {
                break;
            }
            root = r.parent;
        }
        let entry = scenes.entry(root).or_insert((true, Vec::new()));
        entry.0 &= plain && same;
        entry.1.push(node);
    }
    for (playable, nodes) in scenes.into_values() {
        if playable {
            for node in nodes {
                engine.play_transition(node, Property::D);
            }
        }
    }
}

/// The properties whose transitions Core Animation plays for a box
/// ([`hand_over_transitions`]).
const BOX_PLAYED: [Property; 5] = [
    Property::Opacity,
    Property::BackgroundColor,
    Property::Translate,
    Property::Scale,
    Property::Rotate,
];

/// LLP 1055.001 §3 — on iOS each running opacity transition of a box, and
/// each eligible transform or background-colour one, goes to Core Animation
/// ([`Engine::play_transition`]) as LLP 1055.001 lowers a box's keyframes:
/// the engine presents the target, the box's specs carry the transition
/// ([`transition_specs`]), and nothing samples it, so a fade keeps no
/// display link awake. One waiting for its first frame goes at that frame
/// (LLP 1003.001 D4), and none goes unless `new` (the display drives the
/// clock). Every batch, a played transition that no longer qualifies
/// ([`playable`]: an ancestor gained a filter, the box a border), or any once
/// `new` is off (the agent took the clock), comes back to the engine at the
/// value its curve has reached, and its spec goes in the same batch. Returns
/// the views either way.
pub(crate) fn hand_over_transitions(
    kernel: &Kernel,
    engine: &mut Engine,
    skip: &dyn Fn(exact_kernel::ViewId) -> bool,
    interactive: &dyn Fn(exact_kernel::ViewId) -> bool,
    new: bool,
) -> Vec<exact_kernel::ViewId> {
    let played: Vec<(u64, Property)> = engine
        .played_transitions()
        .filter(|(_, p)| BOX_PLAYED.contains(p))
        .collect();
    let moving: Vec<(u64, Property)> = engine
        .running_transitions()
        .filter(|(_, p)| BOX_PLAYED.contains(p))
        .collect();
    let mut views = Vec::new();
    let node = |key: u64| kernel.node_by_key(exact_kernel::motion::node_key(key));
    for (key, p) in played {
        if (!new || !playable(kernel, engine, key, p, skip, interactive))
            && engine.reclaim_transition(key, p)
        {
            views.extend(node(key).map(|n| n.id));
        }
    }
    for (key, p) in moving.into_iter().filter(|_| new) {
        let waits = engine.transition_ends(key, p).is_none_or(|t| t.waits);
        if !waits
            && playable(kernel, engine, key, p, skip, interactive)
            && engine.play_transition(key, p).is_some()
        {
            views.extend(node(key).map(|n| n.id));
        }
    }
    views
}

/// Whether Core Animation can play the transition on `p` of box `key` as
/// CSS does: not what a press or a look paints over, not a property an
/// animation also plays (it composes over the transition), no box under a
/// CSS filter (its picture is drawn from the model), no opacity in a glass
/// group, a background colour only where the layer paints it with one alpha
/// at both ends (Core Animation interpolates unpremultiplied), and no view
/// `skip` names.
fn playable(
    kernel: &Kernel,
    engine: &Engine,
    key: u64,
    p: Property,
    skip: &dyn Fn(exact_kernel::ViewId) -> bool,
    interactive: &dyn Fn(exact_kernel::ViewId) -> bool,
) -> bool {
    let Some(n) = kernel.node_by_key(exact_kernel::motion::node_key(key)) else {
        return false;
    };
    if n.node_type.is_svg_element()
        || n.node_type == NodeType::Svg
        || skip(n.id)
        || crate::looks::paints_over(&n, p)
        || under_box_filter(kernel, &n)
        || engine
            .animation_plays(key)
            .iter()
            .any(|play| play.animation.keyframes.properties().contains(&p))
    {
        return false;
    }
    match p {
        Property::Opacity => !in_glass_group(kernel, &n),
        Property::Translate | Property::Scale | Property::Rotate => {
            box_eligible(kernel, &n, &[p], interactive)
        }
        Property::BackgroundColor => {
            layer_paints_background(&n)
                && background_on_layer(kernel, &n)
                && engine
                    .transition_ends(key, p)
                    .is_some_and(|t| t.from.w == t.to.w)
        }
        _ => false,
    }
}

/// Whether the background colour a box's layer would paint is the box's
/// fill as UIKit shows it (`BoxLayerIOS.applyBoxLayer`'s `bg`): a fixed or
/// light-dark colour (a platform or role colour may draw as vibrancy, a wide
/// one in its own space), over the border box, with one circular radius, a
/// round or continuous corner, no material, no layout transition that moves
/// it to a surface, and shown. Anything UIKit rasterizes stays sampled.
fn background_on_layer(kernel: &Kernel, n: &exact_kernel::NodeRef<'_>) -> bool {
    use exact_kernel::{ColorValue, Dimension};
    let s = n.style;
    let radii: Vec<Option<f32>> = [
        s.border_radius_top_left,
        s.border_radius_top_right,
        s.border_radius_bottom_right,
        s.border_radius_bottom_left,
    ]
    .iter()
    .map(|d| match d {
        Dimension::Points(r) => Some(*r),
        Dimension::Percent(r) if *r == 0.0 => Some(0.0),
        _ => None,
    })
    .collect();
    let set: Vec<f32> = radii
        .iter()
        .flatten()
        .copied()
        .filter(|r| *r > 0.0)
        .collect();
    let one_radius = radii.iter().all(Option::is_some)
        && set.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01)
        && set
            .first()
            .is_none_or(|r| *r <= n.frame.width.min(n.frame.height) / 2.0 + 0.01);
    let shown = {
        let mut at = Some(n.id);
        loop {
            let Some(node) = at.and_then(|id| kernel.node(id)) else {
                break true;
            };
            if node.style.mask.has(exact_kernel::StyleId::Visibility) {
                break node.style.visibility == exact_kernel::Visibility::Visible;
            }
            at = node.parent;
        }
    };
    matches!(
        s.background_color,
        Some(ColorValue::Fixed(_) | ColorValue::LightDark(..))
    ) && s.cold.background_clip == exact_kernel::BackgroundClip::BorderBox
        && (s.rare.corner_shape.is_round() || s.rare.corner_shape.is_apple_continuous())
        && s.rare.layout_transition.0.is_empty()
        && n.props
            .str(exact_kernel::PropId::BackgroundMaterial)
            .is_none()
        && one_radius
        && shown
}

#[derive(Debug)]
pub(crate) struct PlayedSpec {
    ends: exact_motion::TransitionEnds,
    size: (f32, f32),
    text: String,
}

/// A box's played transitions ([`hand_over_transitions`]) as specs, one
/// iteration of each transition's curve from when it starts moving, holding
/// its start through a delay (a backwards fill); after it ends, the target
/// the engine presents shows. A spec is made once per transition and kept in
/// `made` until the transition changes: a spring's track samples its whole
/// curve.
pub(crate) fn transition_specs(
    engine: &Engine,
    key: u64,
    size: (f32, f32),
    made: &mut std::collections::BTreeMap<(u64, usize), PlayedSpec>,
) -> Vec<String> {
    let mut out = Vec::new();
    for p in BOX_PLAYED {
        let slot = (key, p as usize);
        let ends = engine
            .transition_ends(key, p)
            .filter(|_| engine.is_played(key, p));
        let Some(ends) = ends else {
            made.remove(&slot);
            continue;
        };
        if let Some(spec) = made
            .get(&slot)
            .filter(|kept| kept.ends == ends && kept.size == size)
        {
            out.push(spec.text.clone());
            continue;
        }
        let Some((t, _)) = engine.transition(key, p) else {
            continue;
        };
        let (duration, mut track) = match &t.curve {
            PlayedCurve::Easing { easing, duration } => {
                let end = |offset: f64, v: Value| Keyframe {
                    offset,
                    easing: None,
                    values: vec![(p, v)],
                    dark: Vec::new(),
                };
                let ends = [end(0.0, t.from), end(1.0, t.to)];
                (*duration, forward(&ends, easing, p, t.from))
            }
            PlayedCurve::Frames { duration, values } => {
                let last = values.len().saturating_sub(1).max(1) as f64;
                let track = Track {
                    times: (0..values.len()).map(|i| i as f64 / last).collect(),
                    values: values.clone(),
                    curves: vec![LINEAR; values.len().saturating_sub(1)],
                };
                (*duration, track)
            }
        };
        // Backwards fill holds the uneased value through a transition's delay.
        // Keep the eased start just inside the active interval, as at a jump.
        if track.values[0] != t.from && track.times.len() > 1 {
            track.times[0] = HAIR * track.times[1];
            track.values[0] = track.values[0].lerp(track.values[1], HAIR);
            track.times.insert(0, 0.0);
            track.values.insert(0, t.from);
            track.curves.insert(0, LINEAR);
        }
        let parts: &[(usize, &str)] = match p {
            Property::Translate => &[
                (0, "transform.translation.x"),
                (1, "transform.translation.y"),
            ],
            Property::Scale => &[(0, "transform.scale")],
            Property::Rotate => &[(0, "transform.rotation.z")],
            Property::Opacity => &[(0, "opacity")],
            _ => &[(0, "backgroundColor")],
        };
        let scale = if p == Property::Rotate {
            std::f64::consts::PI / 180.0
        } else {
            1.0
        };
        let specs: Vec<String> = parts.iter().map(|(axis, path)| {
            let (times, values, curves) = track_json(&track, p, scale, *axis, Some((size.0 as f64, size.1 as f64)));
            format!(
                "{{\"id\":\"transition#{path}\",\"k\":\"{path}\",\"s\":{},\"dl\":0,\"d\":{duration},\"n\":1,\"t\":[{times}],\"v\":[{values}],\"c\":[{curves}],\"fill\":2,\"h\":null}}", t.start,
            )
        }).collect();
        let spec = specs.join(",");
        made.insert(
            slot,
            PlayedSpec {
                ends,
                size,
                text: spec.clone(),
            },
        );
        out.push(spec);
    }
    out
}

/// Whether a box's layer paints its background colour: no side has a
/// border that paints and there is no `background-image` (else the view
/// draws its box in `draw(_:)`).
fn layer_paints_background(n: &exact_kernel::NodeRef<'_>) -> bool {
    let s = n.style;
    // A side paints only with a style: `border-width`'s initial is 3.
    let bordered = [
        (s.border_width_top, s.border_style_top),
        (s.border_width_right, s.border_style_right),
        (s.border_width_bottom, s.border_style_bottom),
        (s.border_width_left, s.border_style_left),
    ]
    .iter()
    .any(|(w, style)| *w > 0.0 && *style != exact_kernel::BorderStyle::None);
    !bordered && s.background_image.gradient().is_none()
}

/// Whether markers draw on this shape: its own or an inherited
/// `marker-start`, `marker-mid` or `marker-end`.
fn marked(kernel: &Kernel, n: &exact_kernel::NodeRef<'_>) -> bool {
    let mut at = Some(n.id);
    while let Some(node) = at.and_then(|id| kernel.node(id)) {
        let s = node.style;
        if [&s.marker_start, &s.marker_mid, &s.marker_end]
            .iter()
            .any(|m| m.url().is_some())
        {
            return true;
        }
        if node.node_type == NodeType::Svg {
            return false;
        }
        at = node.parent;
    }
    false
}

/// Whether an SVG element's `translate`, `rotate` and `scale` animations
/// play on its transform pair's outer layer (iOS; LLP 1055.001 as a box's):
/// an element, not the `svg` (a box), with nothing drawn for one scale: no
/// non-scaling stroke, filter or mask.
fn svg_turns(n: &exact_kernel::NodeRef<'_>, props: &[Property]) -> bool {
    let s = n.style;
    n.node_type.is_svg_element()
        && !props.contains(&Property::BackgroundColor)
        && s.vector_effect != exact_kernel::VectorEffect::NonScalingStroke
        && s.filter.is_none()
        && s.rare.svg_mask.url().is_none()
}

/// Whether a `cx`/`cy` animation plays as a circle layer's position: a
/// circle (its layer sits at its centre) whose drawing does not depend on
/// where the centre is in its user space: no transform (a fill-box origin
/// follows the centre), clip, mask, filter or paint server.
fn circle_moves(n: &exact_kernel::NodeRef<'_>, props: &[Property]) -> bool {
    use exact_kernel::svg::Paint;
    let s = n.style;
    let served = |p: &Paint| matches!(p, Paint::Url(..));
    n.node_type == NodeType::SvgCircle
        && !props
            .iter()
            .any(|p| matches!(p, Property::Translate | Property::Rotate | Property::Scale))
        && s.translate.x == 0.0
        && s.translate.y == 0.0
        && s.rotate == 0.0
        && s.scale == 1.0
        && exact_kernel::svg::transform::is_identity(s.transform.matrix())
        && s.vector_effect != exact_kernel::VectorEffect::NonScalingStroke
        && s.rare.clip_path.url().is_none()
        && s.rare.svg_mask.url().is_none()
        && s.filter.is_none()
        && !served(&s.fill)
        && !served(&s.stroke)
}

/// Whether an element is inside a glass group (LLP 1053.000.000 D4): an
/// ancestor, not itself, has `glassGroup`. The group's own opacity reaches
/// its glass; a descendant's must come to the host as a value.
fn in_glass_group(kernel: &Kernel, n: &exact_kernel::NodeRef<'_>) -> bool {
    let mut up = n.parent;
    while let Some(a) = up.and_then(|id| kernel.node(id)) {
        if a.props.get(exact_kernel::PropId::GlassGroup).is_some() {
            return true;
        }
        up = a.parent;
    }
    false
}

/// LLP 1053.000.000 D4: every running opacity animation inside a glass
/// group is sampled, not only the ones a commit touched, so a group set on
/// an ancestor after the animation started reaches it too. Returns the views
/// whose Core Animation specs must be withdrawn. A node that leaves every
/// group stays sampled: its pixels are right, at a little more work.
pub(crate) fn glass_sampling(kernel: &Kernel, engine: &mut Engine) -> Vec<exact_kernel::ViewId> {
    let lowered: Vec<u64> = engine
        .animated_nodes()
        .filter(|&node| {
            !engine.node_sampled(node)
                && engine.animation_plays(node).iter().any(|play| {
                    play.animation
                        .keyframes
                        .properties()
                        .into_iter()
                        .any(|p| p == Property::Opacity)
                })
        })
        .collect();
    let mut switched = Vec::new();
    for node in lowered {
        let key = NodeKey {
            index: node as u32,
            generation: (node >> 32) as u32,
        };
        if let Some(n) = kernel.node_by_key(key) {
            if in_glass_group(kernel, &n) {
                engine.set_node_sampled(node, true);
                switched.push(n.id);
            }
        }
    }
    switched
}

/// Whether a node is drawn into a filtered box's picture (CSS `filter` on
/// a box, `BoxFilter` on Apple): it or a box above it has a filter, so its
/// animations are sampled and each frame redraws the picture.
fn under_box_filter(kernel: &Kernel, n: &exact_kernel::NodeRef<'_>) -> bool {
    let filtered =
        |a: &exact_kernel::NodeRef<'_>| !a.node_type.is_svg_element() && !a.style.filter.is_none();
    if filtered(n) {
        return true;
    }
    let mut up = n.parent;
    while let Some(a) = up.and_then(|id| kernel.node(id)) {
        if filtered(&a) {
            return true;
        }
        up = a.parent;
    }
    false
}

/// Whether an element is drawn only into pixels, where a layer's own
/// animation would not show: under a filtered or masked element (an
/// island), or inside a definition (a mask's, pattern's, marker's or clip
/// path's content, `defs`, a `symbol`).
///
/// A CSS function filter (`blur()`, `drop-shadow()`, the colour functions)
/// is an exception on iOS (`live`): its picture follows its content on the
/// GPU (`SvgFilterLive`), where Core Animation renders the content with its
/// animations each frame, so what is inside it lowers as anywhere else.
fn in_picture(kernel: &Kernel, n: &exact_kernel::NodeRef<'_>, live: bool) -> bool {
    use exact_kernel::svg::filter::FilterFn;
    let followed = |f: &exact_kernel::svg::filter::FilterList| {
        live && !f.0.iter().any(|x| matches!(x, FilterFn::Url(_)))
    };
    let mut up = n.parent;
    while let Some(a) = up.and_then(|id| kernel.node(id)) {
        match a.node_type {
            NodeType::Svg => return false,
            NodeType::SvgMask
            | NodeType::SvgPattern
            | NodeType::SvgMarker
            | NodeType::SvgClipPath
            | NodeType::SvgDefs
            | NodeType::SvgSymbol => return true,
            _ => {}
        }
        if (!a.style.filter.is_none() && !followed(&a.style.filter))
            || a.style.rare.svg_mask.url().is_some()
        {
            return true;
        }
        up = a.parent;
    }
    false
}

/// Whether Core Animation plays a box's lowered transform and background
/// colour as CSS does (see [`eligibility`]).
fn box_eligible(
    kernel: &Kernel,
    n: &exact_kernel::NodeRef<'_>,
    props: &[Property],
    interactive: &dyn Fn(exact_kernel::ViewId) -> bool,
) -> bool {
    let s = n.style;
    let turns = props
        .iter()
        .any(|p| matches!(p, Property::Translate | Property::Scale | Property::Rotate));
    if turns
        && (!s.transform_origin.centred()
            || !s.rare.layout_transition.0.is_empty()
            || subtree_any(kernel, n.id, &|id| {
                interactive(id)
                    || kernel.node(id).is_some_and(|node| {
                        matches!(
                            node.node_type,
                            NodeType::Control
                                | NodeType::Pressable
                                | NodeType::TextInput
                                | NodeType::ScrollView
                                | NodeType::List
                                | NodeType::WebView
                                | NodeType::NativeView
                                | NodeType::Video
                        ) || node.props.link().is_some()
                    })
            }))
    {
        return false;
    }
    if props.contains(&Property::BackgroundColor) {
        if !layer_paints_background(n) || !background_on_layer(kernel, n) {
            return false;
        }
        // The row's own colour is interpolated only where a keyframe at 0% or
        // 100% leaves it out.
        let ends = |a: &exact_motion::Animation| {
            [0.0, 1.0].iter().all(|at| {
                a.keyframes.0.iter().any(|f| {
                    f.offset == *at
                        && f.values
                            .iter()
                            .any(|(p, _)| *p == Property::BackgroundColor)
                })
            })
        };
        let bg = |a: &&exact_motion::Animation| {
            a.keyframes
                .properties()
                .contains(&Property::BackgroundColor)
        };
        let own = s.animation.0.iter().filter(bg).any(|a| !ends(a));
        let targets = if own {
            exact_kernel::motion::color_targets(n, false)
        } else {
            Vec::new()
        };
        let alphas: Vec<Option<f64>> = s
            .animation
            .0
            .iter()
            .flat_map(|a| a.keyframes.0.iter())
            .flat_map(|f| f.values.iter())
            .filter(|(p, _)| *p == Property::BackgroundColor)
            .map(|(_, v)| Some(v.w))
            .chain(
                targets
                    .iter()
                    .filter(|(p, _)| *p == Property::BackgroundColor)
                    .map(|(_, v)| v.map(|v| v.w)),
            )
            .collect();
        // Core Animation interpolates unpremultiplied: CSS's premultiplied
        // interpolation only where every colour has one alpha.
        if alphas
            .iter()
            .any(|a| a.is_none_or(|a| Some(a) != alphas[0]))
        {
            return false;
        }
    }
    true
}

/// Whether `id` or anything under it satisfies `f`.
fn subtree_any(
    kernel: &Kernel,
    id: exact_kernel::ViewId,
    f: &dyn Fn(exact_kernel::ViewId) -> bool,
) -> bool {
    f(id)
        || kernel
            .node(id)
            .is_some_and(|n| n.children().into_iter().any(|c| subtree_any(kernel, c, f)))
}

#[cfg(test)]
mod stepped_cost_tests {
    use super::*;

    #[test]
    fn the_largest_steps_curve_keeps_every_jump() {
        let easing = Easing::Steps {
            count: 65535,
            position: exact_motion::StepPosition::JumpEnd,
        };
        let mut track = Track {
            times: vec![0.0],
            values: vec![Value::ZERO],
            curves: Vec::new(),
        };
        let started = std::time::Instant::now();
        stepped_keys(&mut track, 0.0, 1.0, &easing, false, Value::scalar);
        let elapsed = started.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "the largest steps curve exceeded its 3 s construction budget: {elapsed:?}"
        );
        assert_eq!(track.times.len(), 131071);
        assert_eq!(track.values.last(), Some(&Value::scalar(1.0)));
        assert!(track.times.windows(2).all(|w| w[0] < w[1]));
    }
}
