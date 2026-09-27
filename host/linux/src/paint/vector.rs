//! A `path` node (LLP 1065 D7): the kernel's path data fitted into the
//! content box by the view box, filled, then stroked between the engine's
//! `stroke-start` and `stroke-end` — trimmed by the kernel along the whole
//! path in subpath order, the order the web and Core Animation draw in. Both
//! painters draw vectors, so nothing is rasterized at one size and scaled.

use super::{rgba, Painter, Shape, Walk};
use exact_kernel::vector::{fit, parse_view_box, Command, PathData};
use exact_kernel::{NodeRef, PropId, StrokeLinecap, StrokeLinejoin, StyleMask};
use tiny_skia::Transform;

/// What a backend draws for one path, in path units.
pub struct VectorPaint {
    /// The fill colour and the whole path, when the fill is not `none`.
    pub fill: Option<([u8; 4], Vec<Command>)>,
    /// The stroke colour and the visible part of the path, when there is one.
    pub stroke: Option<([u8; 4], Vec<Command>)>,
    /// The stroke's width, in path units: `fit` scales it with the path.
    pub width: f32,
    /// SVG's `stroke-linecap`.
    pub cap: StrokeLinecap,
    /// SVG's `stroke-linejoin`; the miter limit is SVG's initial 4.
    pub join: StrokeLinejoin,
    /// Path units into the viewport: the view box fitted `xMidYMid meet`
    /// into the content box.
    pub fit: Transform,
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
        let (scale, dx, dy) = fit(
            node.props.str(PropId::ViewBox).and_then(parse_view_box),
            w,
            h,
        );
        let style = node.computed_style(StyleMask::INHERITED);
        let (start, end) = (walk.scene.presented)(node.id).stroke;
        let paint = |c: Option<exact_kernel::ColorValue>| c.map(|c| rgba(c.resolve(self.dark)));
        let trimmed = data.trimmed(f64::from(start), f64::from(end));
        let vector = VectorPaint {
            fill: paint(style.fill).map(|c| (c, data.commands().to_vec())),
            stroke: paint(style.stroke)
                .filter(|_| !trimmed.is_empty())
                .map(|c| (c, trimmed)),
            width: style.stroke_width,
            cap: style.stroke_linecap,
            join: style.stroke_linejoin,
            fit: Transform::from_row(scale, 0.0, 0.0, scale, x + dx, y + dy),
        };
        // SVG clips a path to its viewport, the content box.
        self.backend.push_clip(&Shape::rect(content), ts);
        self.backend.vector(&vector, ts);
        self.backend.pop_clip();
    }
}
