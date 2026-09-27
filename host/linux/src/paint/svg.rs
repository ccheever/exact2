//! An `svg`'s content in the paint walk (LLP 1055 D4): its elements drawn
//! in the view box's space inside the content box, clipped to the box when
//! the `svg`'s overflow is not visible, each shape through
//! [`Backend::svg_path`]; `g` opacity is a group, as in CSS.

use super::{effective_overflow, rgba, Painter, Rect4, Shape, Walk};
use exact_kernel::svg::{circle, dash_scale, geometry, view_box, view_box_transform, Paint, Path};
use exact_kernel::{NodeRef, NodeType, Overflow, PropId, StyleId, StyleMask};
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

const ROWS: [StyleId; 12] = [
    StyleId::Fill,
    StyleId::Stroke,
    StyleId::StrokeWidth,
    StyleId::StrokeLinecap,
    StyleId::StrokeLinejoin,
    StyleId::StrokeMiterlimit,
    StyleId::StrokeDasharray,
    StyleId::StrokeDashoffset,
    StyleId::FillOpacity,
    StyleId::StrokeOpacity,
    StyleId::FillRule,
    StyleId::TextColor,
];

impl Painter {
    pub(super) fn svg(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        rect: Rect4,
        content: Rect4,
        ts: Transform,
    ) {
        let Some(t) = view_box_transform(
            view_box(node.props),
            node.props.str(PropId::PreserveAspectRatio),
            content.2,
            content.3,
        ) else {
            return;
        };
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        if clips {
            self.backend.push_clip(&Shape::rect(rect), ts);
        }
        let space = ts
            .pre_translate(content.0, content.1)
            .pre_concat(Transform::from_row(t[0], t[1], t[2], t[3], t[4], t[5]));
        for child in node.children() {
            self.svg_element(walk, child, space);
        }
        if clips {
            self.backend.pop_clip();
        }
    }

    fn svg_element(&mut self, walk: &mut Walk<'_, '_>, id: exact_kernel::ViewId, ts: Transform) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        let p = (walk.scene.presented)(id);
        let opacity = p.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return;
        }
        if opacity < 1.0 {
            self.backend.push_opacity(opacity);
        }
        if node.node_type == NodeType::SvgGroup {
            for child in node.children() {
                self.svg_element(walk, child, ts);
            }
        } else {
            let mut mask = StyleMask::EMPTY;
            for row in ROWS {
                mask.set(row);
            }
            let s = node.computed_style(mask);
            let path = if node.node_type == NodeType::SvgCircle {
                circle(
                    node.style.cx,
                    node.style.cy,
                    p.svg.0.unwrap_or(node.style.r),
                )
            } else {
                geometry(node.node_type, node.props, &s)
            };
            if let Some(path) = path {
                let scale = dash_scale(&path, node.props);
                let color = |paint: &Paint, alpha: f32| {
                    paint.resolve(s.text_color, self.dark).map(|c| {
                        let mut c = rgba(c);
                        c[3] = (c[3] as f32 * alpha.clamp(0.0, 1.0)).round() as u8;
                        c
                    })
                };
                let shape = SvgPaint {
                    path: &path,
                    fill: color(&s.fill, s.fill_opacity),
                    even_odd: s.fill_rule == exact_kernel::FillRule::Evenodd,
                    stroke: color(&s.stroke, s.stroke_opacity),
                    width: s.stroke_width.max(0.0),
                    cap: s.stroke_linecap as u8,
                    join: s.stroke_linejoin as u8,
                    miter: s.stroke_miterlimit.max(1.0),
                    dash: s.stroke_dasharray.pattern(scale),
                    phase: p.svg.1.unwrap_or(s.stroke_dashoffset) * scale,
                };
                self.backend.svg_path(&shape, ts);
            }
        }
        if opacity < 1.0 {
            self.backend.pop_opacity();
        }
    }
}
