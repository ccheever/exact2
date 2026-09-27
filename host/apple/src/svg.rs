//! Inline SVG and CSS animations on Apple (LLP 1055 D4, D7).
//!
//! An `svg` is an ordinary node view; its SVG elements are not views. Each
//! commit that touches an element (or its `svg`, or the `svg`'s size) sends
//! one `svg` op with the whole scene: the view-box transform and, per
//! element, its flattened path, paint and stroke, and the Core Animation
//! form of its animations. The presenter keeps one `CAShapeLayer` per shape,
//! diffs by element id, and never restarts an animation whose spec is
//! unchanged. A box's own `opacity` animation goes out as an `animations`
//! op on its view. The engine runs lowered for these properties: it tracks
//! each animation's start and pause, and never samples it per frame.

use crate::batch::Batch;
use crate::style::num;
use exact_kernel::id::{IdMap, IdSet};
use exact_kernel::motion::motion_node;
use exact_kernel::svg::scene::{content_box, Item, Kind, Shape, ShapePaint};
use exact_kernel::svg::transform::{self as tf, Affine};
use exact_kernel::svg::{Path, Seg};
use exact_kernel::{ColorValue, Dimension, Kernel, NodeKey, NodeRef, NodeType, StyleProps, ViewId};
use exact_motion::animation::Keyframe;
use exact_motion::{AnimationPlay, Easing, Engine, Property, Value};
use std::fmt::Write as _;

/// The properties Core Animation plays for a CSS animation on Apple.
pub(crate) const LOWERED: [Property; 3] =
    [Property::Opacity, Property::StrokeDashoffset, Property::R];

/// A content box: x, y, width, height.
type Rect = (f32, f32, f32, f32);

/// What the Apple host knows about SVG scenes and lowered animations.
#[derive(Debug, Default)]
pub(crate) struct SvgState {
    /// An SVG element's `svg`.
    elements: IdMap<ViewId, ViewId>,
    /// Scenes to rebuild.
    dirty: IdSet<ViewId>,
    /// The last scene sent per `svg`, and its content box.
    sent: IdMap<ViewId, (String, Rect)>,
    /// Boxes whose lowered animations may have changed, and their last spec.
    boxes: IdSet<ViewId>,
    box_sent: IdMap<ViewId, String>,
}

impl SvgState {
    /// An SVG element's `svg`, marking its scene dirty; `None` for any other node.
    pub(crate) fn element(&mut self, kernel: &Kernel, id: ViewId) -> Option<ViewId> {
        let node = kernel.node(id)?;
        if node.node_type == NodeType::Svg {
            self.dirty.insert(id);
            return None;
        }
        if !node.node_type.is_svg_element() {
            if !node.style.animation.0.is_empty() || self.box_sent.contains_key(&id) {
                self.boxes.insert(id);
            }
            return None;
        }
        let mut root = node.parent;
        while let Some(r) = root {
            let n = kernel.node(r)?;
            if n.node_type == NodeType::Svg {
                self.elements.insert(id, r);
                self.dirty.insert(r);
                return Some(r);
            }
            root = n.parent;
        }
        None
    }

    /// Forget a destroyed node; `true` when it was an SVG element (no view).
    pub(crate) fn destroyed(&mut self, id: ViewId) -> bool {
        self.sent.remove(&id);
        self.box_sent.remove(&id);
        self.boxes.remove(&id);
        match self.elements.remove(&id) {
            Some(root) => {
                self.dirty.insert(root);
                true
            }
            None => false,
        }
    }

    /// A presented value moved on an element: its scene is rebuilt.
    pub(crate) fn presented(&mut self, id: ViewId) -> bool {
        match self.elements.get(&id) {
            Some(root) => {
                self.dirty.insert(*root);
                true
            }
            None => false,
        }
    }

    /// The `svg` and `animations` ops this batch owes.
    pub(crate) fn emit(&mut self, kernel: &Kernel, engine: &Engine, batch: &mut Batch) {
        // A resized `svg` needs a new transform.
        let moved: Vec<ViewId> = self
            .sent
            .iter()
            .filter(|(id, (_, bx))| kernel.node(**id).is_some_and(|n| content_box(&n) != *bx))
            .map(|(id, _)| *id)
            .collect();
        self.dirty.extend(moved);
        let mut dirty: Vec<ViewId> = self.dirty.drain().collect();
        dirty.sort_unstable();
        for root in dirty {
            let Some(node) = kernel.node(root) else {
                self.sent.remove(&root);
                continue;
            };
            let bx = content_box(&node);
            let scene = scene(kernel, engine, &node, bx);
            if self.sent.get(&root).is_none_or(|(s, _)| *s != scene) {
                batch.svg(root, &scene);
                self.sent.insert(root, (scene, bx));
            }
        }
        let mut boxes: Vec<ViewId> = self.boxes.drain().collect();
        boxes.sort_unstable();
        for id in boxes {
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let key = motion_node(node.key);
            let base = engine
                .target(key, Property::Opacity)
                .map_or(node.style.opacity as f64, |v| v.x);
            let specs = specs(engine, key, &[Property::Opacity], &|_| (base, 1.0));
            if self.box_sent.get(&id) != Some(&specs) {
                batch.animations(id, &specs);
                if specs == "[]" {
                    self.box_sent.remove(&id);
                } else {
                    self.box_sent.insert(id, specs);
                }
            }
        }
    }
}

/// `{"box":[x,y,w,h],"t":[a,b,c,d,e,f]|null,"els":[…]}`: `t` null renders
/// nothing (a view box with no area). The scene is the kernel's resolved
/// one (LLP 1055.000 D1); this only serializes it with the lowered
/// animations of each item.
fn scene(kernel: &Kernel, engine: &Engine, node: &NodeRef<'_>, bx: (f32, f32, f32, f32)) -> String {
    let presented = |key: NodeKey, p: Property| engine.value(motion_node(key), p);
    let resolved = exact_kernel::svg::scene::resolve(kernel, node, bx, &presented);
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"box\":[{},{},{},{}],\"t\":",
        num(bx.0),
        num(bx.1),
        num(bx.2),
        num(bx.3)
    );
    match resolved.view {
        Some(t) => affine_json(t, &mut s),
        None => s.push_str("null"),
    }
    s.push_str(",\"els\":");
    items(
        engine,
        &resolved.items,
        resolved.view.unwrap_or(tf::IDENTITY),
        &mut s,
    );
    s.push('}');
    s
}

fn affine_json(t: Affine, s: &mut String) {
    let _ = write!(
        s,
        "[{},{},{},{},{},{}]",
        num(t[0]),
        num(t[1]),
        num(t[2]),
        num(t[3]),
        num(t[4]),
        num(t[5])
    );
}

fn items(engine: &Engine, list: &[Item], parent_ctm: Affine, s: &mut String) {
    s.push('[');
    for (i, item) in list.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        element(engine, item, parent_ctm, s);
    }
    s.push(']');
}

/// One item: `id`, group opacity `o`, its transform `tf` (origin `o`, the
/// individual properties `i`, the list `m`) when it has one, the lowered
/// animations `a`, then what it draws.
fn element(engine: &Engine, item: &Item, parent_ctm: Affine, s: &mut String) {
    let key = motion_node(item.key);
    let opacity = item.opacity as f64;
    let _ = write!(s, "{{\"id\":{},\"o\":{}", item.id, num(item.opacity));
    let shape = match &item.kind {
        Kind::Shape(shape) => Some(shape.as_ref()),
        _ => None,
    };
    let non_scaling = shape.is_some_and(|sh| sh.non_scaling);
    match (&item.transform, non_scaling) {
        (_, true) => {
            // A non-scaling stroke is drawn in the content box's space: the
            // path comes mapped through the item's ctm, and the layer undoes
            // its parents' (LLP 1055.000 D5).
            s.push_str(",\"inv\":");
            affine_json(tf::invert(parent_ctm).unwrap_or(tf::IDENTITY), s);
        }
        (Some(t), false) => {
            let _ = write!(
                s,
                ",\"tf\":{{\"o\":[{},{}],\"i\":",
                num(t.origin.0),
                num(t.origin.1)
            );
            affine_json(t.individual(), s);
            s.push_str(",\"m\":");
            affine_json(t.matrix, s);
            s.push('}');
        }
        (None, false) => {}
    }
    match &item.kind {
        Kind::Group(children) => {
            let specs = specs(engine, key, &[Property::Opacity], &|_| (opacity, 1.0));
            let _ = write!(s, ",\"g\":1,\"a\":{specs},\"c\":");
            items(engine, children, item.ctm, s);
        }
        Kind::Viewport {
            rect,
            view,
            clip,
            children,
        } => {
            let specs = specs(engine, key, &[Property::Opacity], &|_| (opacity, 1.0));
            let _ = write!(
                s,
                ",\"g\":1,\"vp\":[{},{},{},{}],\"clip\":{},\"a\":{specs},\"t\":",
                num(rect.0),
                num(rect.1),
                num(rect.2),
                num(rect.3),
                *clip as u8
            );
            match view {
                // The layer sits at the rect's origin; its sublayers take
                // the view box without that translation.
                Some(v) => affine_json(tf::mul(tf::translate(-rect.0, -rect.1), *v), s),
                None => s.push_str("null"),
            }
            s.push_str(",\"c\":");
            match view {
                Some(v) => items(engine, children, tf::mul(item.ctm, *v), s),
                None => s.push_str("[]"),
            }
        }
        Kind::Shape(shape) => shape_json(engine, key, opacity, item, shape, s),
    }
    s.push('}');
}

fn shape_json(engine: &Engine, key: u64, opacity: f64, item: &Item, shape: &Shape, s: &mut String) {
    // A circle is drawn about the origin and placed at its centre, so a
    // moving pulse and an `r` animation never fight over one path.
    let centered = shape.circle.filter(|_| !shape.non_scaling);
    let path = match centered {
        Some((_, _, r)) => exact_kernel::svg::circle(0.0, 0.0, r),
        None if shape.non_scaling => Some(shape.path.transformed(item.ctm)),
        None => Some(shape.path.clone()),
    };
    s.push_str(",\"p\":");
    path_json(path.as_ref(), s);
    if let Some((cx, cy, _)) = centered {
        let _ = write!(s, ",\"pos\":[{},{}]", num(cx), num(cy));
    }
    s.push_str(",\"f\":");
    paint_json(shape.fill.as_ref(), s);
    s.push_str(",\"s\":");
    paint_json(shape.stroke.as_ref(), s);
    let dash: Vec<String> = shape.dash.iter().map(|v| num(*v)).collect();
    let _ = write!(
        s,
        ",\"w\":{},\"cap\":{},\"join\":{},\"ml\":{},\"rule\":{},\"dash\":[{}],\"ph\":{}",
        num(shape.width),
        shape.cap as u8,
        shape.join as u8,
        num(shape.miter),
        shape.fill_rule as u8,
        dash.join(","),
        num(shape.dash_offset)
    );
    let scale = shape.dash_scale as f64;
    let offset = if scale > 0.0 {
        shape.dash_offset as f64 / scale
    } else {
        0.0
    };
    let r = shape.circle.map_or(0.0, |c| c.2 as f64);
    let underlying = |p: Property| match p {
        Property::Opacity => (opacity, 1.0),
        Property::StrokeDashoffset => (offset, scale),
        Property::R => (r, 1.0),
        _ => (0.0, 1.0),
    };
    let props: &[Property] = if centered.is_some() {
        &LOWERED
    } else {
        &[Property::Opacity, Property::StrokeDashoffset]
    };
    let specs = specs(engine, key, props, &underlying);
    let _ = write!(s, ",\"a\":{specs}");
}

fn path_json(path: Option<&Path>, s: &mut String) {
    s.push('[');
    let mut first = true;
    let mut push = |s: &mut String, vals: &[f32]| {
        for v in vals {
            if !first {
                s.push(',');
            }
            first = false;
            s.push_str(&num(*v));
        }
    };
    for seg in path.map_or(&[][..], |p| p.0.as_slice()) {
        match *seg {
            Seg::Move(x, y) => push(s, &[0.0, x, y]),
            Seg::Line(x, y) => push(s, &[1.0, x, y]),
            Seg::Cubic(a, b, c, d, x, y) => push(s, &[2.0, a, b, c, d, x, y]),
            Seg::Close => push(s, &[3.0]),
        }
    }
    s.push(']');
}

/// Paint as the presenter's colour: `null` for none; the paint's opacity
/// folded into alpha. A `light-dark()` pair stays a pair, resolved by the
/// presenter.
fn paint_json(paint: Option<&ShapePaint>, s: &mut String) {
    let Some(paint) = paint else {
        return s.push_str("null");
    };
    let a = |alpha: u8| ((alpha as f32) * paint.opacity).round() as u8;
    match paint.color {
        ColorValue::Fixed(c) => {
            let _ = write!(s, "[{},{},{},{}]", c.r(), c.g(), c.b(), a(c.a()));
        }
        ColorValue::LightDark(l, d) => {
            let _ = write!(
                s,
                "[[{},{},{},{}],[{},{},{},{}]]",
                l.r(),
                l.g(),
                l.b(),
                a(l.a()),
                d.r(),
                d.g(),
                d.b(),
                a(d.a())
            );
        }
    }
}

/// One CA keyframe track: key times, values and a cubic per segment.
struct Track {
    times: Vec<f64>,
    values: Vec<f64>,
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
fn forward(frames: &[Keyframe], default: &Easing, property: Property, underlying: f64) -> Track {
    let mut pts: Vec<(f64, Option<&Easing>, f64)> = frames
        .iter()
        .filter_map(|f| {
            f.values
                .iter()
                .find(|(p, _)| *p == property)
                .map(|(_, v)| (f.offset, f.easing.as_ref(), v.x))
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
        match bezier(easing) {
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
                xs.push(1.0);
                xs.retain(|x| *x > 0.0 && *x <= 1.0);
                xs.sort_by(f64::total_cmp);
                xs.dedup();
                for x in xs {
                    t.times.push(a.0 + span * x);
                    t.values.push(a.2 + (b.2 - a.2) * easing.progress(x));
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
/// `underlying(p)` is the property's own value and the scale to CA units
/// (a dash offset's length over `pathLength`).
fn specs(
    engine: &Engine,
    key: u64,
    props: &[Property],
    underlying: &dyn Fn(Property) -> (f64, f64),
) -> String {
    let mut s = String::from("[");
    let mut first = true;
    for (i, play) in engine.animation_plays(key).iter().enumerate() {
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
    (base, scale): (f64, f64),
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
        _ => "r",
    };
    let list = |v: &[f64]| {
        v.iter()
            .map(|n| num(*n as f32))
            .collect::<Vec<_>>()
            .join(",")
    };
    let values: Vec<f64> = track.values.iter().map(|v| v * scale).collect();
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
        list(&values),
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

/// A node's own unlowered value for a row, for tests and state.
#[allow(dead_code)]
pub(crate) fn row(style: &StyleProps, p: Property) -> Value {
    match p {
        Property::Opacity => Value::scalar(style.opacity as f64),
        Property::R => Value::scalar(match style.r {
            Dimension::Points(r) => r as f64,
            _ => 0.0,
        }),
        Property::StrokeDashoffset => Value::scalar(style.stroke_dashoffset as f64),
        _ => Value::ZERO,
    }
}
