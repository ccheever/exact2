//! Layers a reader animates (`crate::host::lower`): the node painted at its
//! underlying values into a layer the backend keeps apart, so the reader
//! moves and fades it every frame without the row being recorded again.

use super::{border, gradient, Backend, Painter, Presented, Rect4, RunPaint, Shape};
use crate::host::lower::{
    LOWER_COLOR, LOWER_DASH, LOWER_MOVE, LOWER_OPACITY, LOWER_R, LOWER_TRANSFORM,
};
use crate::image::Bitmap;
use crate::text::{Paragraph, TextEngine};
use exact_kernel::motion::motion_node;
use std::sync::Arc;
use tiny_skia::{Pixmap, Transform};

/// Where a fully transparent subtree draws: nowhere.
pub(super) struct Unpainted;
impl Backend for Unpainted {
    fn name(&self) -> &'static str {
        "none"
    }
    fn begin(&mut self, _: f32, _: f32, _: f32) {}
    fn fill(&mut self, _: &Shape, _: [u8; 4], _: Transform) {}
    fn fill_gradient(&mut self, _: &Shape, _: &gradient::GradientPaint, _: Transform) {}
    fn fill_border(&mut self, _: &border::BorderFill, _: Transform) {}
    fn image(&mut self, _: &Arc<Bitmap>, _: Rect4, _: &[Shape], _: Transform, _: Option<[u8; 4]>) {}
    fn text(
        &mut self,
        _: &mut TextEngine,
        _: &Paragraph,
        _: &[RunPaint],
        _: (f32, f32),
        _: Transform,
    ) {
    }
    fn push_clip(&mut self, _: &Shape, _: Transform) {}
    fn pop_clip(&mut self) {}
    fn push_opacity(&mut self, _: f32) {}
    fn pop_opacity(&mut self) {}
    fn pointer(&mut self, _: f32, _: f32) {}
    fn finish(&mut self) -> Result<Pixmap, String> {
        Err("a transparent subtree has no frame".into())
    }
}

/// A layer the painter opened: what the node draws without.
#[derive(Clone, Copy)]
pub(super) struct Opened {
    /// The lowered bits the backend took (zero: drawn where it is).
    pub(super) lowered: u8,
}

impl Opened {
    /// Whether the node's own transform is the layer's.
    pub(super) fn transform(self) -> bool {
        self.lowered & LOWER_TRANSFORM != 0
    }
    /// Whether the node's own opacity is the layer's.
    pub(super) fn opacity(self) -> bool {
        self.lowered & LOWER_OPACITY != 0
    }
}

impl Painter {
    /// Open a box's layer when its reader animates it: about its
    /// `transform-origin` (`origin`, in its box) after a layout transition's
    /// offset, in `ts` (its parent's space).
    pub(super) fn box_layer(
        &mut self,
        key: exact_kernel::NodeKey,
        p: &Presented,
        (x, y, _, _): Rect4,
        origin: (f32, f32),
        ts: Transform,
    ) -> Opened {
        if p.lowered == 0 || p.lowered & (LOWER_R | LOWER_DASH | LOWER_MOVE) != 0 {
            return Opened { lowered: 0 };
        }
        let [dx, dy, ..] = p.layout;
        let base = [
            p.translate.0,
            p.translate.1,
            p.scale * p.press,
            p.rotate,
            p.opacity,
            0.0,
            0.0,
        ];
        let pivot = (x + origin.0 + dx, y + origin.1 + dy);
        let taken = self.backend.layer_begin(motion_node(key), ts, pivot, base);
        if taken && p.lowered & LOWER_COLOR != 0 {
            self.backend.layer_recolored();
        }
        Opened {
            lowered: if taken { p.lowered } else { 0 },
        }
    }

    /// Open an SVG element's layer when its reader animates it: its opacity;
    /// its `translate`/`rotate`/`scale` about the scene's origin, in `ts` (its
    /// parent's user space; drawn with only its `transform` list); a filled
    /// circle's radius or centre about its centre, in `own` (its own space).
    /// Also the space its drawing is in.
    pub(super) fn svg_layer(
        &mut self,
        item: &exact_kernel::svg::scene::Item,
        ts: Transform,
        own: Transform,
    ) -> (Opened, Transform) {
        let closed = (Opened { lowered: 0 }, own);
        let Some(p) = self
            .svg_layers
            .iter()
            .find(|(id, _)| *id == item.id)
            .map(|e| e.1)
        else {
            return closed;
        };
        let shape = match &item.kind {
            exact_kernel::svg::scene::Kind::Shape(s) => Some(s),
            _ => None,
        };
        let circle = shape.and_then(|s| s.circle);
        if (p.lowered & (LOWER_R | LOWER_MOVE) != 0 && circle.is_none())
            || (p.lowered & LOWER_DASH != 0 && shape.is_none())
            || (p.lowered & LOWER_TRANSFORM != 0 && item.transform.is_none())
        {
            return closed;
        }
        let (cx, cy, r) = circle.unwrap_or((0.0, 0.0, 0.0));
        let dash = shape.map_or(0.0, |s| s.dash_scale);
        let (space, pivot, drawn, base) = match item.transform {
            Some(t) if p.lowered & LOWER_TRANSFORM != 0 => {
                // The individual properties are the layer's; the list stays drawn.
                let (ox, oy) = t.origin;
                let list = Transform::from_translate(ox, oy)
                    .pre_concat(super::svg::affine(t.matrix))
                    .pre_translate(-ox, -oy);
                let base = [
                    t.translate.0,
                    t.translate.1,
                    t.scale,
                    t.rotate,
                    item.opacity,
                    r,
                    dash,
                ];
                (ts, t.origin, ts.pre_concat(list), base)
            }
            _ => (
                own,
                (cx, cy),
                own,
                [0.0, 0.0, 1.0, 0.0, item.opacity, r, dash],
            ),
        };
        let taken = self
            .backend
            .layer_begin(motion_node(item.key), space, pivot, base);
        if !taken {
            return closed;
        }
        (Opened { lowered: p.lowered }, drawn)
    }

    /// Close a layer [`Painter::box_layer`] or [`Painter::svg_layer`] opened.
    pub(super) fn layer_close(&mut self, opened: Opened) {
        if opened.lowered != 0 {
            self.backend.layer_end();
        }
    }
}
