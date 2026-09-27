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

use super::svg_lower::specs;
use crate::batch::Batch;
use crate::style::num;
use exact_kernel::id::{IdMap, IdSet};
use exact_kernel::motion::motion_node;
use exact_kernel::svg::scene::{content_box, Clip, Item, Kind, Shape, ShapePaint, TextItem};
use exact_kernel::svg::server::{Server, ServerKind, Spread};
use exact_kernel::svg::transform::{self as tf, Affine};
use exact_kernel::svg::{Path, Seg};
use exact_kernel::{ColorValue, Dimension, Kernel, NodeKey, NodeRef, NodeType, StyleProps, ViewId};
use exact_motion::{Engine, Property, Value};
use std::fmt::Write as _;

/// The properties Core Animation plays for a CSS animation on Apple.
pub(crate) const LOWERED: [Property; 5] = [
    Property::Opacity,
    Property::StrokeDashoffset,
    Property::R,
    Property::Fill,
    Property::Stroke,
];

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
    /// SVG elements with a press handler (LLP 1055.000 D17).
    pressable: IdSet<ViewId>,
    /// Whether a `foreignObject` was met: its boxes are then looked for
    /// (LLP 1055.000 D13, refused on this host).
    foreign: bool,
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
            // @ref LLP 1055.000 D13 — a box inside a `foreignObject` is the
            // web's; here it is left out with the element.
            if self.foreign {
                let mut up = node.parent;
                while let Some(p) = up.and_then(|p| kernel.node(p)) {
                    if p.node_type == NodeType::SvgForeignObject {
                        return self.element(kernel, p.id);
                    }
                    up = p.parent;
                }
            }
            if !node.style.animation.0.is_empty() || self.box_sent.contains_key(&id) {
                self.boxes.insert(id);
            }
            return None;
        }
        if node.node_type == NodeType::SvgForeignObject && !self.foreign {
            self.foreign = true;
            eprintln!("exact svg: {}", exact_kernel::svg::scene::FOREIGN_OBJECT);
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

    /// A presented value moved on an `svg` or an SVG element: its scene is
    /// rebuilt. `false` for any other node.
    pub(crate) fn touch(&mut self, kernel: &Kernel, id: ViewId) -> bool {
        match kernel.node(id).map(|n| n.node_type) {
            Some(NodeType::Svg) => {
                self.dirty.insert(id);
                true
            }
            Some(t) if t.is_svg_element() => self.presented(id),
            _ => false,
        }
    }

    /// Whether an SVG element handles presses; its scene says so.
    pub(crate) fn handlers(&mut self, id: ViewId, press: bool) {
        if press {
            self.pressable.insert(id);
        } else {
            self.pressable.remove(&id);
        }
    }

    /// Forget a destroyed node; `true` when it was an SVG element (no view).
    pub(crate) fn destroyed(&mut self, id: ViewId) -> bool {
        self.pressable.remove(&id);
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
            let scene = scene(kernel, engine, &self.pressable, &node, bx);
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
            let specs = specs(engine, key, &[Property::Opacity], &|_| {
                (Value::scalar(base), 1.0)
            });
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
fn scene(
    kernel: &Kernel,
    engine: &Engine,
    press: &IdSet<ViewId>,
    node: &NodeRef<'_>,
    bx: (f32, f32, f32, f32),
) -> String {
    // A sampled animation's value now; a lowered one is Core Animation's,
    // so the scene carries its underlying value and the spec.
    let presented = |key: NodeKey, p: Property| engine.sampled_value(motion_node(key), p);
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
        press,
        &resolved.items,
        resolved.view.unwrap_or(tf::IDENTITY),
        &mut s,
        Role::Item,
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

/// How an item is written: as itself, or as a piece of a marked shape
/// (LLP 1055.000 D9) — the group standing for the shape, one of its paint
/// parts (its opacity is the group's), or a marker (which takes no hits).
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Item,
    Marked,
    Part,
    Marker,
}

fn items(
    engine: &Engine,
    press: &IdSet<ViewId>,
    list: &[Item],
    parent_ctm: Affine,
    s: &mut String,
    parent: Role,
) {
    s.push('[');
    for (i, item) in list.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let role = match (parent, &item.kind) {
            (Role::Marked, Kind::Shape(_)) => Role::Part,
            (Role::Marked, _) => Role::Marker,
            _ => Role::Item,
        };
        element(engine, press, item, parent_ctm, s, role);
    }
    s.push(']');
}

/// A shape with markers, as a group of its paint and its markers in
/// `paint-order`: the group takes the shape's opacity, transform and clip.
fn marked(
    engine: &Engine,
    press: &IdSet<ViewId>,
    item: &Item,
    shape: &Shape,
    parent_ctm: Affine,
    s: &mut String,
) {
    let part = |fill: bool, stroke: bool, salt: u64| {
        let mut sh = shape.clone();
        sh.markers = Vec::new();
        sh.fill = sh.fill.filter(|_| fill);
        sh.stroke = sh.stroke.filter(|_| stroke);
        Item {
            id: item.id,
            uid: item.uid ^ salt,
            key: item.key,
            opacity: 1.0,
            transform: None,
            ctm: item.ctm,
            clip: None,
            mask: None,
            instance: false,
            kind: Kind::Shape(Box::new(sh)),
        }
    };
    let at = shape.order.iter().position(|&o| o == 2).unwrap_or(2);
    let fill_first = shape.order[0] == 0;
    let mut children = Vec::new();
    match at {
        0 => {
            children.extend(shape.markers.iter().cloned());
            children.push(part(true, true, 1 << 52));
        }
        1 => {
            children.push(part(fill_first, !fill_first, 1 << 52));
            children.extend(shape.markers.iter().cloned());
            children.push(part(!fill_first, fill_first, 1 << 51));
        }
        _ => {
            children.push(part(true, true, 1 << 52));
            children.extend(shape.markers.iter().cloned());
        }
    }
    let group = Item {
        kind: Kind::Group(children),
        ..item.clone()
    };
    element(engine, press, &group, parent_ctm, s, Role::Marked);
}

/// One item: `id`, group opacity `o`, its transform `tf` (origin `o`, the
/// individual properties `i`, the list `m`) when it has one, the lowered
/// animations `a`, then what it draws.
fn element(
    engine: &Engine,
    press: &IdSet<ViewId>,
    item: &Item,
    parent_ctm: Affine,
    s: &mut String,
    role: Role,
) {
    if let (Role::Item, Kind::Shape(shape)) = (role, &item.kind) {
        if !shape.markers.is_empty() {
            return marked(engine, press, item, shape, parent_ctm, s);
        }
    }
    let key = motion_node(item.key);
    let opacity = item.opacity as f64;
    let _ = write!(s, "{{\"id\":{},\"o\":{}", item.uid, num(item.opacity));
    // @ref LLP 1055.000 D17 — the presenter hits it: its node, whether it
    // takes presses, and what `pointer-events` reads.
    let _ = write!(s, ",\"n\":{}", item.id);
    if press.contains(&item.id) {
        s.push_str(",\"h\":1");
    }
    if role == Role::Marker
        || matches!(&item.kind, Kind::Shape(sh) if sh.pointer_events == exact_kernel::PointerEvents::None)
    {
        s.push_str(",\"pn\":1");
    }
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
    if let Some(clip) = &item.clip {
        // In the layer's own space: a centred circle's is about its centre,
        // a non-scaling stroke's the content box's.
        let clip = match &item.kind {
            Kind::Shape(sh) if sh.non_scaling => clip.transformed(item.ctm),
            Kind::Shape(sh) => match sh.circle {
                Some((cx, cy, _)) => clip.transformed(tf::translate(-cx, -cy)),
                None => (**clip).clone(),
            },
            _ => (**clip).clone(),
        };
        s.push_str(",\"cl\":");
        clip_json(&clip, s);
    }
    if let Some(mask) = &item.mask {
        // @ref LLP 1055.000 D10 — a mask: its region and content, and the
        // element's user space to its layer's (as the clip's above).
        let local = match &item.kind {
            Kind::Shape(sh) if sh.non_scaling => item.ctm,
            Kind::Shape(sh) => sh
                .circle
                .map_or(tf::IDENTITY, |(cx, cy, _)| tf::translate(-cx, -cy)),
            _ => tf::IDENTITY,
        };
        let (x, y, w, h) = mask.region;
        let _ = write!(
            s,
            ",\"mk\":{{\"r\":[{},{},{},{}],\"l\":{},\"t\":",
            num(x),
            num(y),
            num(w),
            num(h),
            mask.luminance as u8
        );
        affine_json(local, s);
        // Points of the layer per unit, for the island's resolution.
        let det = |m: Affine| (m[0] * m[3] - m[1] * m[2]).abs();
        let k = (det(item.ctm) / det(local).max(1e-12)).sqrt();
        let _ = write!(s, ",\"k\":{}", num(k));
        s.push_str(",\"c\":");
        items(engine, press, &mask.items, item.ctm, s, Role::Item);
        s.push('}');
    }
    match &item.kind {
        Kind::Group(children) => {
            let specs = specs(engine, key, &[Property::Opacity], &|_| {
                (Value::scalar(opacity), 1.0)
            });
            let _ = write!(s, ",\"g\":1,\"a\":{specs},\"c\":");
            items(engine, press, children, item.ctm, s, role);
        }
        Kind::Viewport {
            rect,
            view,
            clip,
            children,
        } => {
            let specs = specs(engine, key, &[Property::Opacity], &|_| {
                (Value::scalar(opacity), 1.0)
            });
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
                Some(v) => items(
                    engine,
                    press,
                    children,
                    tf::mul(item.ctm, *v),
                    s,
                    Role::Item,
                ),
                None => s.push_str("[]"),
            }
        }
        Kind::Shape(shape) => shape_json(
            engine,
            press,
            key,
            opacity,
            item,
            shape,
            s,
            role == Role::Part,
        ),
        Kind::Text(text) => {
            let specs = specs(engine, key, &[Property::Opacity], &|_| {
                (Value::scalar(opacity), 1.0)
            });
            let _ = write!(s, ",\"a\":{specs},\"tx\":");
            text_json(text, s);
        }
    }
    s.push('}');
}

#[allow(clippy::too_many_arguments)]
fn shape_json(
    engine: &Engine,
    press: &IdSet<ViewId>,
    key: u64,
    opacity: f64,
    item: &Item,
    shape: &Shape,
    s: &mut String,
    part: bool,
) {
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
    // A circle drawn about the origin takes its gradients there too.
    let local = |p: Option<&ShapePaint>| -> Option<ShapePaint> {
        let mut p = p?.clone();
        if let (Some((cx, cy, _)), Some(server)) = (centered, p.server.as_mut()) {
            server.transform = tf::mul(tf::translate(-cx, -cy), server.transform);
        }
        Some(p)
    };
    // @ref LLP 1055.000 D7 — a pattern's tile, in the same space.
    let place = match centered {
        Some((cx, cy, _)) => tf::translate(-cx, -cy),
        None if shape.non_scaling => item.ctm,
        None => tf::IDENTITY,
    };
    for (key, paint) in [("f", &shape.fill), ("s", &shape.stroke)] {
        let _ = write!(s, ",\"{key}\":");
        match paint
            .as_ref()
            .and_then(|p| p.pattern.as_ref().map(|t| (p, t)))
        {
            Some((p, pattern)) => {
                let (x, y, w, h) = pattern.tile;
                let _ = write!(
                    s,
                    "{{\"pt\":[{},{},{},{}],\"o\":{},\"t\":",
                    num(x),
                    num(y),
                    num(w),
                    num(h),
                    num(p.opacity)
                );
                affine_json(tf::mul(place, pattern.transform), s);
                s.push_str(",\"c\":");
                let at = tf::mul(item.ctm, pattern.transform);
                items(engine, press, &pattern.items, at, s, Role::Item);
                s.push('}');
            }
            None => paint_json(local(paint.as_ref()).as_ref(), s),
        }
    }
    if shape.order != [0, 1, 2] {
        let _ = write!(
            s,
            ",\"po\":[{},{},{}]",
            shape.order[0], shape.order[1], shape.order[2]
        );
    }
    let drawn = |p: &Option<ShapePaint>| {
        p.as_ref()
            .is_some_and(|p| p.server.is_some() || p.pattern.is_some())
    };
    if drawn(&shape.fill) || drawn(&shape.stroke) {
        // Gradients are drawn at the scale the shape shows at.
        let m = item.ctm;
        let _ = write!(
            s,
            ",\"cs\":{}",
            num((m[0] * m[3] - m[1] * m[2]).abs().sqrt())
        );
    }
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
    let paint = |p: Option<&ShapePaint>| match p {
        Some(ShapePaint {
            color: ColorValue::Fixed(c),
            opacity,
            server: None,
            pattern: None,
        }) => (Value::rgba8(c.r(), c.g(), c.b(), c.a()), *opacity as f64),
        _ => (Value::ZERO, 1.0),
    };
    let underlying = |p: Property| match p {
        Property::Opacity => (Value::scalar(opacity), 1.0),
        Property::StrokeDashoffset => (Value::scalar(offset), scale),
        Property::R => (Value::scalar(r), 1.0),
        Property::Fill => paint(shape.fill.as_ref()),
        Property::Stroke => paint(shape.stroke.as_ref()),
        _ => (Value::ZERO, 1.0),
    };
    let props: &[Property] = if part {
        // A marked shape's paint: its group takes the opacity.
        &[Property::StrokeDashoffset, Property::Fill, Property::Stroke]
    } else if centered.is_some() {
        &LOWERED
    } else {
        &[
            Property::Opacity,
            Property::StrokeDashoffset,
            Property::Fill,
            Property::Stroke,
        ]
    };
    let specs = specs(engine, key, props, &underlying);
    let _ = write!(s, ",\"a\":{specs}");
}

/// A gradient: `{"lg":[x1,y1,x2,y2]}` or `{"rg":[cx,cy,r,fx,fy,fr]}`, its
/// stops `"st":[[offset, colour],…]` (colours as `paint_json` writes them,
/// the paint's opacity folded in), spread `"sp"` (0 pad, 1 reflect, 2
/// repeat) and `"t"`, gradient space to the shape's user space.
fn server_json(server: &Server, opacity: f32, s: &mut String) {
    s.push('{');
    match server.kind {
        ServerKind::Linear { x1, y1, x2, y2 } => {
            let _ = write!(
                s,
                "\"lg\":[{},{},{},{}]",
                num(x1),
                num(y1),
                num(x2),
                num(y2)
            );
        }
        ServerKind::Radial {
            cx,
            cy,
            r,
            fx,
            fy,
            fr,
        } => {
            let _ = write!(
                s,
                "\"rg\":[{},{},{},{},{},{}]",
                num(cx),
                num(cy),
                num(r),
                num(fx),
                num(fy),
                num(fr)
            );
        }
    }
    s.push_str(",\"st\":[");
    for (i, stop) in server.stops.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "[{},", num(stop.offset));
        let paint = ShapePaint {
            color: stop.color,
            opacity: stop.opacity * opacity,
            server: None,
            pattern: None,
        };
        paint_json(Some(&paint), s);
        s.push(']');
    }
    let _ = write!(
        s,
        "],\"sp\":{},\"t\":",
        match server.spread {
            Spread::Pad => 0,
            Spread::Reflect => 1,
            Spread::Repeat => 2,
        }
    );
    affine_json(server.transform, s);
    s.push('}');
}

/// SVG text (LLP 1055.000 D11): `[{"x","y","an","bl","runs":[{"t","dx",
/// "dy","fs","fw","ff","it","ls","f","s","w","cap","join","ml"},…]},…]`,
/// shaped by the presenter with its own fonts.
fn text_json(text: &TextItem, s: &mut String) {
    s.push('[');
    for (i, chunk) in text.chunks.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let opt = |v: Option<f32>| v.map_or("null".to_string(), num);
        let _ = write!(
            s,
            "{{\"x\":{},\"y\":{},\"an\":{},\"bl\":{},\"runs\":[",
            opt(chunk.x),
            opt(chunk.y),
            chunk.anchor as u8,
            chunk.baseline as u8
        );
        for (j, run) in chunk.runs.iter().enumerate() {
            if j > 0 {
                s.push(',');
            }
            s.push_str("{\"t\":");
            let _ = write!(s, "{}", serde_json_string(&run.text));
            let _ = write!(
                s,
                ",\"dx\":{},\"dy\":{},\"fs\":{},\"fw\":{},\"ff\":{},\"it\":{},\"ls\":{},\"f\":",
                num(run.dx),
                num(run.dy),
                num(run.style.font_size),
                run.style.font_weight,
                run.style.font_family,
                (run.style.font_style != exact_kernel::FontStyle::Normal) as u8,
                num(run.style.letter_spacing)
            );
            paint_json(run.fill.as_ref(), s);
            s.push_str(",\"s\":");
            paint_json(run.stroke.as_ref(), s);
            let _ = write!(
                s,
                ",\"w\":{},\"cap\":{},\"join\":{},\"ml\":{}}}",
                num(run.width),
                run.cap as u8,
                run.join as u8,
                num(run.miter)
            );
        }
        s.push_str("]}");
    }
    s.push(']');
}

/// A JSON string literal.
fn serde_json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A clip: `{"s":[[path, evenodd],…],"n":clip|null}`, its shapes' union
/// intersected with `n` (LLP 1055.000 D10).
fn clip_json(clip: &Clip, s: &mut String) {
    s.push_str("{\"s\":[");
    for (i, shape) in clip.shapes.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('[');
        path_json(Some(&shape.path), s);
        let _ = write!(s, ",{}]", shape.even_odd as u8);
    }
    s.push_str("],\"n\":");
    match &clip.then {
        Some(t) => clip_json(t, s),
        None => s.push_str("null"),
    }
    s.push('}');
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
    if let Some(server) = &paint.server {
        return server_json(server, paint.opacity, s);
    }
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
