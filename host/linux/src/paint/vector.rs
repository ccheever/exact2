//! A `path` node (LLP 1065 D7): the kernel's path data fitted into the
//! content box by the view box and `preserveAspectRatio`, filled by its
//! `fill-rule`, then stroked between the engine's `stroke-start` and
//! `stroke-end` — trimmed by the kernel along the whole path in subpath
//! order, the order the web and Core Animation draw in. A dashed stroke is
//! the kernel's dashes of the whole path, shown only where the trimmed,
//! undashed stroke would paint (the web's mask, Core Animation's too), so
//! the trim reveals the dashes rather than moving them. Under
//! `vector-effect: non-scaling-stroke` the stroke is drawn in the box's
//! pixels: the kernel maps the path there first, and the width and dashes
//! are not scaled. Both painters draw vectors, so nothing is rasterized at
//! one size and scaled.

use super::{rgba, Painter, Shape, Walk};
use exact_kernel::vector::{fit, Command, PathData, ShapePaint};
use exact_kernel::{
    FillRule, NodeRef, PropId, RowValue, StrokeLinecap, StrokeLinejoin, StyleId, StyleMask,
    VectorEffect,
};
use exact_motion::Property;
use tiny_skia::Transform;

/// What a backend draws for one path.
pub struct VectorPaint {
    /// The fill colour and the whole path in path units (under `fit`), when
    /// the fill is not `none`.
    pub fill: Option<([u8; 4], Vec<Command>)>,
    /// SVG's `fill-rule`.
    pub rule: FillRule,
    /// The stroke colour and what it draws (under `stroke_fit`): the visible
    /// part of the path, or, dashed, every dash of it.
    pub stroke: Option<([u8; 4], Vec<Command>)>,
    /// A dashed stroke shows only where this undashed, trimmed stroke — the
    /// same width, caps and joins — would paint.
    pub reveal: Option<Vec<Command>>,
    /// The stroke's width, in the stroke's units.
    pub width: f32,
    /// SVG's `stroke-linecap`.
    pub cap: StrokeLinecap,
    /// SVG's `stroke-linejoin`.
    pub join: StrokeLinejoin,
    /// SVG's `stroke-miterlimit`.
    pub miter: f32,
    /// Path units into the viewport: the view box fitted into the content
    /// box.
    pub fit: Transform,
    /// The stroke's units into the viewport: `fit`, or for a non-scaling
    /// stroke the content box's origin alone.
    pub stroke_fit: Transform,
}

impl Painter {
    pub(super) fn vector(
        &mut self,
        walk: &Walk<'_, '_>,
        node: &NodeRef<'_>,
        content: super::Rect4,
        ts: Transform,
    ) {
        let (x, y, w, h) = content;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let data = PathData::parse(node.props.str(PropId::PathData).unwrap_or(""));
        // A path in a path draws in the outermost one's coordinate system.
        let (view_box, aspect) = node.path_viewport();
        let at = fit(view_box, aspect, w, h);
        let [sx, sy, dx, dy] = at;
        let fitted = Transform::from_row(sx, 0.0, 0.0, sy, x + dx, y + dy);
        let style = node.computed_style(StyleMask::INHERITED);
        let presented = (walk.scene.presented)(node.id);
        // Paint motion's value over the row while it moves (LLP 1062): the
        // property's own, or for `currentcolor` a moving `color`.
        let paint = |row: StyleId, property: Property| {
            let color = node.paint(row)?;
            let current = matches!(node.computed(row), RowValue::Enum("currentcolor"));
            let moving = presented.paint.color(property).or_else(|| {
                current
                    .then(|| presented.paint.color(Property::Color))
                    .flatten()
            });
            Some(moving.unwrap_or_else(|| rgba(color.resolve(self.dark))))
        };
        // A non-scaling stroke is measured and trimmed where it is drawn,
        // as Core Animation trims the path it is handed.
        let (lines, stroke_fit) = match node.style.vector_effect {
            VectorEffect::NonScalingStroke => {
                (data.transformed(at), Transform::from_translate(x, y))
            }
            VectorEffect::None => (data.clone(), fitted),
        };
        let (start, end) = presented.stroke;
        let trimmed = lines.trimmed(f64::from(start), f64::from(end));
        let (drawn, reveal) = match node.computed(StyleId::StrokeDasharray) {
            RowValue::DashArray(d) if d.dashes() && !trimmed.is_empty() => {
                (lines.dashed(&d.0, style.stroke_dashoffset), Some(trimmed))
            }
            _ => (trimmed, None),
        };
        let vector = VectorPaint {
            fill: paint(StyleId::Fill, Property::Fill).map(|c| (c, data.commands().to_vec())),
            rule: style.fill_rule,
            stroke: paint(StyleId::Stroke, Property::Stroke)
                .filter(|_| !drawn.is_empty())
                .map(|c| (c, drawn)),
            reveal,
            width: style.stroke_width,
            cap: style.stroke_linecap,
            join: style.stroke_linejoin,
            miter: style.stroke_miterlimit,
            fit: fitted,
            stroke_fit,
        };
        // SVG clips a path to its viewport, the content box.
        self.backend.push_clip(&Shape::rect(content), ts);
        self.backend.vector(&vector, ts);
        let context = [
            vector.fill.as_ref().map(|(c, _)| *c),
            paint(StyleId::Stroke, Property::Stroke),
            Some(
                presented
                    .paint
                    .color(Property::Color)
                    .unwrap_or_else(|| rgba(node.text_color().resolve(self.dark))),
            ),
        ];
        self.markers(node, &data, (start, end), fitted, context, ts);
        self.backend.pop_clip();
    }

    /// The path's markers (LLP 1065 D11), each clipped to its viewport, in
    /// SVG's order; one shows while the trim holds its vertex. `context` is
    /// the path's fill, stroke and `color`, for context paints.
    fn markers(
        &mut self,
        node: &NodeRef<'_>,
        data: &PathData,
        (start, end): (f32, f32),
        fitted: Transform,
        [fill, stroke, current]: [Option<[u8; 4]>; 3],
        ts: Transform,
    ) {
        let total = data.length();
        let (from, to) = (f64::from(start) * total, f64::from(end) * total);
        for (def, placed) in node.path_markers(data) {
            if !(from <= placed.at + 1e-9 && placed.at <= to + 1e-9 && start < end) {
                continue;
            }
            let [a, b, c, d, e, f] = placed.outer;
            let outer = fitted.pre_concat(Transform::from_row(a, b, c, d, e, f));
            let [cx, cy, cw, ch] = placed.clip;
            self.backend
                .push_clip(&Shape::rect((cx, cy, cw, ch)), ts.pre_concat(outer));
            let [sx, sy, dx, dy] = placed.fit;
            let inner = outer.pre_concat(Transform::from_row(sx, 0.0, 0.0, sy, dx, dy));
            for shape in &def.shapes {
                let color = |p: ShapePaint| match p {
                    ShapePaint::None => None,
                    ShapePaint::ContextFill => fill,
                    ShapePaint::ContextStroke => stroke,
                    ShapePaint::CurrentColor => current,
                    ShapePaint::Color(c) => Some(rgba(c.resolve(self.dark))),
                };
                let commands = || shape.data.commands().to_vec();
                let paint = VectorPaint {
                    fill: color(shape.fill).map(|c| (c, commands())),
                    rule: shape.rule,
                    stroke: color(shape.stroke).map(|c| (c, commands())),
                    reveal: None,
                    width: shape.width,
                    cap: shape.cap,
                    join: shape.join,
                    miter: shape.miter,
                    fit: inner,
                    stroke_fit: inner,
                };
                self.backend.vector(&paint, ts);
            }
            self.backend.pop_clip();
        }
    }
}
