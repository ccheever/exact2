//! An `svg`'s content in the paint walk (LLP 1055 D4, LLP 1055.000 D1): the
//! kernel resolves the scene, and this paints it inside the content box,
//! clipped to the box when the `svg`'s overflow is not visible, each shape
//! through [`Backend::svg_path`]; group opacity is a group, as in CSS.

use super::{effective_overflow, rgba, Painter, Rect4, Shape, Walk};
use exact_kernel::svg::scene::{self, Item, Kind, ShapePaint};
use exact_kernel::svg::Path;
use exact_kernel::{NodeKey, NodeRef, Overflow};
use exact_motion::{Property, Value};
use tiny_skia::Transform;

/// One shape as a backend paints it: path in user units, paint resolved.
pub struct SvgPaint<'a> {
    /// The path.
    pub path: &'a Path,
    /// Fill colour, or none.
    pub fill: Option<[u8; 4]>,
    /// `fill-rule: evenodd`.
    pub even_odd: bool,
    /// Stroke colour, or none.
    pub stroke: Option<[u8; 4]>,
    /// `stroke-width`.
    pub width: f32,
    /// `stroke-linecap`: 0 butt, 1 round, 2 square.
    pub cap: u8,
    /// `stroke-linejoin`: 0 miter, 1 round, 2 bevel.
    pub join: u8,
    /// `stroke-miterlimit`.
    pub miter: f32,
    /// The dash pattern in path units, even length; empty is solid.
    pub dash: Vec<f32>,
    /// The dash offset in path units.
    pub phase: f32,
}

fn affine(t: [f32; 6]) -> Transform {
    Transform::from_row(t[0], t[1], t[2], t[3], t[4], t[5])
}

impl Painter {
    pub(super) fn svg(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        rect: Rect4,
        content: Rect4,
        ts: Transform,
    ) {
        let kernel = walk.scene.kernel;
        let presented = walk.scene.presented;
        let value = |key: NodeKey, p: Property| -> Option<Value> {
            let id = kernel.node_by_key(key)?.id;
            let v = presented(id);
            Some(match p {
                Property::Opacity => Value::scalar(v.opacity as f64),
                Property::Translate => Value::new(v.translate.0 as f64, v.translate.1 as f64),
                Property::Scale => Value::scalar(v.scale as f64),
                Property::Rotate => Value::scalar(v.rotate as f64),
                Property::R => Value::scalar(v.svg.0? as f64),
                Property::StrokeDashoffset => Value::scalar(v.svg.1? as f64),
                Property::Height => return None,
            })
        };
        let scene = scene::resolve(kernel, node, content, &value);
        if scene.view.is_none() {
            return;
        }
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        if clips {
            self.backend.push_clip(&Shape::rect(rect), ts);
        }
        let origin = ts.pre_translate(content.0, content.1);
        let space = origin.pre_concat(affine(
            scene.view.unwrap_or(exact_kernel::svg::transform::IDENTITY),
        ));
        for item in &scene.items {
            self.svg_item(item, space, origin);
        }
        if clips {
            self.backend.pop_clip();
        }
    }

    /// One item in its parent's space `ts`; `origin` is the content box's
    /// space, where a non-scaling stroke is drawn.
    fn svg_item(&mut self, item: &Item, ts: Transform, origin: Transform) {
        if item.opacity <= 0.0 {
            return;
        }
        if item.opacity < 1.0 {
            self.backend.push_opacity(item.opacity);
        }
        let own = match &item.transform {
            Some(t) => ts.pre_concat(affine(t.affine())),
            None => ts,
        };
        match &item.kind {
            Kind::Group(children) => {
                for child in children {
                    self.svg_item(child, own, origin);
                }
            }
            Kind::Viewport {
                rect,
                view,
                clip,
                children,
            } => {
                if let Some(view) = view {
                    if *clip {
                        self.backend.push_clip(&Shape::rect(*rect), own);
                    }
                    let inner = own.pre_concat(affine(*view));
                    for child in children {
                        self.svg_item(child, inner, origin);
                    }
                    if *clip {
                        self.backend.pop_clip();
                    }
                }
            }
            Kind::Shape(shape) => {
                let color = |p: Option<&ShapePaint>| {
                    p.map(|p| {
                        let mut c = rgba(p.color.resolve(self.dark));
                        c[3] = (c[3] as f32 * p.opacity).round() as u8;
                        c
                    })
                };
                let mapped;
                let (path, space) = if shape.non_scaling {
                    // Drawn in the content box's space, the stroke unscaled
                    // (LLP 1055.000 D5).
                    mapped = shape.path.transformed(item.ctm);
                    (&mapped, origin)
                } else {
                    (&shape.path, own)
                };
                let paint = SvgPaint {
                    path,
                    fill: color(shape.fill.as_ref()),
                    even_odd: shape.fill_rule == exact_kernel::FillRule::Evenodd,
                    stroke: color(shape.stroke.as_ref()),
                    width: shape.width,
                    cap: shape.cap as u8,
                    join: shape.join as u8,
                    miter: shape.miter,
                    dash: shape.dash.clone(),
                    phase: shape.dash_offset,
                };
                self.backend.svg_path(&paint, space);
            }
        }
        if item.opacity < 1.0 {
            self.backend.pop_opacity();
        }
    }
}
