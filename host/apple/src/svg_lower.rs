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
                spec(play, i, p, underlying(p), *axis, key, &mut s);
            }
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
    axis: usize,
    key: &str,
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
        // presenter against the box as it stands; a lowered track would
        // freeze it at one size.
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
            || percent
            || engine.timeline_bound(*node)
            || under_box_filter(kernel, &n)
            // A glass group ignores the opacity Core Animation plays between
            // it and its glass; the host isolates on what it is told.
            || (props.contains(&Property::Opacity) && in_glass_group(kernel, &n))
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
        && s.svg_mask.url().is_none()
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
        && s.clip_path.url().is_none()
        && s.svg_mask.url().is_none()
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
            || a.style.svg_mask.url().is_some()
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
            || !s.layout_transition.0.is_empty()
            || subtree_any(kernel, n.id, interactive))
    {
        return false;
    }
    if props.contains(&Property::BackgroundColor) {
        // A side paints only with a style: `border-width`'s initial is 3.
        let bordered = [
            (s.border_width_top, s.border_style_top),
            (s.border_width_right, s.border_style_right),
            (s.border_width_bottom, s.border_style_bottom),
            (s.border_width_left, s.border_style_left),
        ]
        .iter()
        .any(|(w, style)| *w > 0.0 && *style != exact_kernel::BorderStyle::None);
        if bordered || s.background_image.gradient().is_some() {
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
