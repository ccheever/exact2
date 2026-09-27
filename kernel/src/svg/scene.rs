//! The resolved scene: one `svg`'s elements with inheritance, lengths,
//! transforms, paint and presented motion values resolved, which every
//! native host paints without asking a node anything again.
//!
//! @ref LLP 1055.000 D1 (one resolver in Rust), D4 (lengths against the
//! nearest viewport), D5 (the transform sandwich), D19 (`display`,
//! `visibility`)
//!
//! Inherited rows are carried down the walk, not looked up per node, so an
//! instance (a `use`, later) can inherit from somewhere other than its
//! target's parent. Presented values come from the host's motion engine
//! through [`Presented`], read once per resolve, so every host shows the
//! same instant.

use super::length::Viewport;
use super::transform::{self as tf, Affine, TransformOrigin};
use super::{circle, dash_scale, geometry, view_box, view_box_transform, Paint, Path};
use crate::generated::BorderStyle;
use crate::generated::{
    Display, FillRule, NodeType, Overflow, PropId, StrokeLinecap, StrokeLinejoin, StyleId,
    StyleMask, StyleProps, TransformBox, VectorEffect, Visibility,
};
use crate::id::{NodeKey, ViewId};
use crate::kernel::{Kernel, NodeRef};
use crate::style::{ColorValue, Dimension};
use exact_motion::{Property, Value};

/// A circle's centre and radius.
type Circle = (f32, f32, f32);

/// A host's presented value for a node's property: a running transition's
/// or a sampled animation's; `None` shows the row.
pub type Presented<'a> = &'a dyn Fn(NodeKey, Property) -> Option<Value>;

/// One `svg`, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// The content box in the `svg`'s border box: x, y, width, height.
    pub content: (f32, f32, f32, f32),
    /// User units to the content box; `None` renders nothing (a view box
    /// with no area).
    pub view: Option<Affine>,
    /// Whether the `svg` clips to its box (`overflow` not visible).
    pub clip: bool,
    /// The elements, in paint order.
    pub items: Vec<Item>,
}

/// One rendered element.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The element's node.
    pub id: ViewId,
    /// Its generation-checked key (the motion engine's).
    pub key: NodeKey,
    /// Group opacity, presented.
    pub opacity: f32,
    /// The element's own transform; `None` is the identity.
    pub transform: Option<Transform>,
    /// The element's user space to the `svg`'s content box, its own
    /// transform included (a non-scaling stroke and hit testing use it).
    pub ctm: Affine,
    /// What it draws.
    pub kind: Kind,
}

/// CSS Transforms 2 §6.1's composition for one element, kept in parts so a
/// host can animate the individual properties apart from the list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// `transform-origin` in user units (after `transform-box`).
    pub origin: (f32, f32),
    /// `translate`, presented.
    pub translate: (f32, f32),
    /// `rotate` in degrees, presented.
    pub rotate: f32,
    /// `scale`, presented.
    pub scale: f32,
    /// The `transform` list's matrix.
    pub matrix: Affine,
}

impl Transform {
    /// `translate · rotate · scale`: the individual properties.
    pub fn individual(&self) -> Affine {
        tf::mul(
            tf::translate(self.translate.0, self.translate.1),
            tf::mul(tf::rotate(self.rotate), tf::scale(self.scale, self.scale)),
        )
    }

    /// The whole: `T(origin) · translate · rotate · scale · transform ·
    /// T(−origin)`.
    pub fn affine(&self) -> Affine {
        let (ox, oy) = self.origin;
        tf::mul(
            tf::translate(ox, oy),
            tf::mul(
                tf::mul(self.individual(), self.matrix),
                tf::translate(-ox, -oy),
            ),
        )
    }
}

/// What an item draws.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A `g`: its children.
    Group(Vec<Item>),
    /// A nested `svg`: a new viewport.
    Viewport {
        /// x, y, width, height in the parent's user units.
        rect: (f32, f32, f32, f32),
        /// Its user units to the parent's (the rect's origin included);
        /// `None` renders nothing.
        view: Option<Affine>,
        /// Whether it clips to `rect`.
        clip: bool,
        /// Its children.
        children: Vec<Item>,
    },
    /// A shape.
    Shape(Box<Shape>),
}

/// Paint, resolved: a colour (a `light-dark()` pair kept for the host's
/// appearance) and the paint's opacity (`fill-opacity`/`stroke-opacity`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapePaint {
    /// The colour.
    pub color: ColorValue,
    /// Zero to one.
    pub opacity: f32,
}

/// A shape, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// The geometry in the element's user units.
    pub path: Path,
    /// A circle's centre and presented radius: hosts that animate `r` draw
    /// the circle about the origin and place it at the centre.
    pub circle: Option<(f32, f32, f32)>,
    /// Fill, or none.
    pub fill: Option<ShapePaint>,
    /// `fill-rule`.
    pub fill_rule: FillRule,
    /// Stroke, or none (also none at zero width).
    pub stroke: Option<ShapePaint>,
    /// `stroke-width` in user units.
    pub width: f32,
    /// `stroke-linecap`.
    pub cap: StrokeLinecap,
    /// `stroke-linejoin`.
    pub join: StrokeLinejoin,
    /// `stroke-miterlimit`, at least 1.
    pub miter: f32,
    /// The dash pattern in path units, even length; empty is solid.
    pub dash: Vec<f32>,
    /// The presented dash offset in path units.
    pub dash_offset: f32,
    /// Path units per author unit (the length over `pathLength`).
    pub dash_scale: f32,
    /// `vector-effect: non-scaling-stroke`: the stroke is drawn in the
    /// content box's space, the path mapped through the item's `ctm`.
    pub non_scaling: bool,
}

/// The content box inside a box's border box: x, y, width, height. A
/// border takes space only when its style draws one (CSS: `none` and
/// `hidden` compute the width to zero, whatever `medium` says).
pub fn content_box(node: &NodeRef<'_>) -> (f32, f32, f32, f32) {
    let s = node.style;
    let len = |d: Dimension| match d {
        Dimension::Points(p) => p,
        Dimension::Percent(p) => node.frame.width * p / 100.0,
        _ => 0.0,
    };
    let border = |w: f32, style: BorderStyle| match style {
        BorderStyle::None | BorderStyle::Hidden => 0.0,
        _ => w,
    };
    let left = len(s.padding_left) + border(s.border_width_left, s.border_style_left);
    let top = len(s.padding_top) + border(s.border_width_top, s.border_style_top);
    let w = node.frame.width
        - left
        - len(s.padding_right)
        - border(s.border_width_right, s.border_style_right);
    let h = node.frame.height
        - top
        - len(s.padding_bottom)
        - border(s.border_width_bottom, s.border_style_bottom);
    (left, top, w.max(0.0), h.max(0.0))
}

/// Resolve the `svg` `node` whose content box is `content`.
pub fn resolve(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    content: (f32, f32, f32, f32),
    presented: Presented<'_>,
) -> Scene {
    let vb = view_box(node.props);
    let view = view_box_transform(
        vb,
        node.props.str(PropId::PreserveAspectRatio),
        content.2,
        content.3,
    );
    let vp = match vb {
        Some(v) => Viewport {
            width: v.width,
            height: v.height,
        },
        None => Viewport {
            width: content.2,
            height: content.3,
        },
    };
    let clip =
        node.style.overflow_x != Overflow::Visible || node.style.overflow_y != Overflow::Visible;
    let mut inherited = node.computed_style(StyleMask::INHERITED);
    let mut r = Resolver { kernel, presented };
    r.present(node, &mut inherited);
    let items = match view {
        Some(v) => r.children(node, &inherited, vp, v),
        None => Vec::new(),
    };
    Scene {
        content,
        view,
        clip,
        items,
    }
}

struct Resolver<'k, 'p> {
    kernel: &'k Kernel,
    presented: Presented<'p>,
}

/// The rows a child computes from its parent: every inherited row it does
/// not set itself.
fn cascade(node: &NodeRef<'_>, inherited: &StyleProps) -> StyleProps {
    let mut style = node.style.clone();
    style.copy_rows(inherited, StyleMask::INHERITED.minus(node.style.mask));
    style
}

impl Resolver<'_, '_> {
    fn value(&self, key: NodeKey, p: Property) -> Option<Value> {
        (self.presented)(key, p).filter(|v| v.is_finite())
    }

    /// The inherited properties an animation or transition is moving on
    /// this node, over its computed style: its descendants inherit the
    /// presented value, as CSS's computed value (LLP 1055.000 D15).
    ///
    /// A node's own presented value counts only where the node sets the row
    /// or animates the property itself: otherwise its engine slot holds the
    /// row it inherits, and the ancestor's moving value must win.
    fn present(&self, node: &NodeRef<'_>, style: &mut StyleProps) {
        let key = node.key;
        let animated = node.style.animation.properties();
        let own = |p: Property, row: StyleId| node.style.mask.has(row) || animated.contains(&p);
        let color = |p: Property| {
            let row = match p {
                Property::Color => StyleId::TextColor,
                Property::Fill => StyleId::Fill,
                _ => StyleId::Stroke,
            };
            own(p, row).then(|| self.value(key, p)).flatten().map(|v| {
                let [r, g, b, a] = v.to_rgba8();
                crate::style::Color(u32::from_be_bytes([r, g, b, a]))
            })
        };
        if let Some(c) = color(Property::Color) {
            style.text_color = ColorValue::Fixed(c);
        }
        if let Some(c) = color(Property::Fill) {
            style.fill = Paint::Color(ColorValue::Fixed(c));
        }
        if let Some(c) = color(Property::Stroke) {
            style.stroke = Paint::Color(ColorValue::Fixed(c));
        }
        if own(Property::StrokeDashoffset, StyleId::StrokeDashoffset) {
            if let Some(v) = self.value(key, Property::StrokeDashoffset) {
                style.stroke_dashoffset = v.x as f32;
            }
        }
    }

    fn children(
        &mut self,
        parent: &NodeRef<'_>,
        inherited: &StyleProps,
        vp: Viewport,
        ctm: Affine,
    ) -> Vec<Item> {
        let mut out = Vec::new();
        for child in parent.children() {
            if let Some(node) = self.kernel.node(child) {
                if let Some(item) = self.item(&node, inherited, vp, ctm) {
                    out.push(item);
                }
            }
        }
        out
    }

    fn item(
        &mut self,
        node: &NodeRef<'_>,
        inherited: &StyleProps,
        vp: Viewport,
        parent_ctm: Affine,
    ) -> Option<Item> {
        if !node.node_type.is_svg_element() {
            return None;
        }
        let mut style = cascade(node, inherited);
        if style.display == Display::None {
            return None;
        }
        self.present(node, &mut style);
        let key = node.key;
        let opacity = self
            .value(key, Property::Opacity)
            .map_or(style.opacity, |v| v.x as f32)
            .clamp(0.0, 1.0);
        let transform = self.transform(node, &style, vp);
        let ctm = match &transform {
            Some(t) => tf::mul(parent_ctm, t.affine()),
            None => parent_ctm,
        };
        let kind = match node.node_type {
            NodeType::SvgGroup => Kind::Group(self.children(node, &style, vp, ctm)),
            NodeType::SvgViewport => self.viewport(node, &style, vp, ctm),
            _ => Kind::Shape(Box::new(self.shape(node, &style, vp)?)),
        };
        Some(Item {
            id: node.id,
            key,
            opacity,
            transform,
            ctm,
            kind,
        })
    }

    /// The element's transform, or `None` when every part is the identity.
    fn transform(&self, node: &NodeRef<'_>, style: &StyleProps, vp: Viewport) -> Option<Transform> {
        let key = node.key;
        let translate = self
            .value(key, Property::Translate)
            .map_or((style.translate.x, style.translate.y), |v| {
                (v.x as f32, v.y as f32)
            });
        let rotate = self
            .value(key, Property::Rotate)
            .map_or(style.rotate, |v| v.x as f32);
        let scale = self
            .value(key, Property::Scale)
            .map_or(style.scale, |v| v.x as f32);
        let matrix = style.transform.matrix();
        if translate == (0.0, 0.0) && rotate == 0.0 && scale == 1.0 && tf::is_identity(matrix) {
            return None;
        }
        let origin = if style.mask.has(StyleId::TransformOrigin) {
            style.transform_origin
        } else {
            TransformOrigin::SVG
        };
        let reference = match style.transform_box {
            TransformBox::ViewBox => (0.0, 0.0, vp.width, vp.height),
            // CSS Transforms 1 §4: an SVG element has no CSS layout box, so
            // content-box is fill-box and border-box is stroke-box.
            TransformBox::FillBox | TransformBox::ContentBox => {
                self.bbox(node, style, vp).unwrap_or_default()
            }
            TransformBox::StrokeBox | TransformBox::BorderBox => {
                let (x, y, w, h) = self.bbox(node, style, vp).unwrap_or_default();
                let half = if matches!(style.stroke, Paint::None) {
                    0.0
                } else {
                    style.stroke_width.max(0.0) / 2.0
                };
                (x - half, y - half, w + 2.0 * half, h + 2.0 * half)
            }
        };
        Some(Transform {
            origin: origin.point(reference),
            translate,
            rotate,
            scale,
            matrix,
        })
    }

    /// The object bounding box in the element's own user space, before its
    /// transform (SVG 2 §8.10): a shape's geometry, or the union of a
    /// group's children with their transforms.
    fn bbox(
        &self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        vp: Viewport,
    ) -> Option<(f32, f32, f32, f32)> {
        match node.node_type {
            NodeType::SvgGroup => {
                let mut acc: Option<(f32, f32, f32, f32)> = None;
                for child in node.children() {
                    let Some(c) = self.kernel.node(child) else {
                        continue;
                    };
                    let cs = cascade(&c, style);
                    if cs.display == Display::None {
                        continue;
                    }
                    let Some(b) = self.bbox(&c, &cs, vp) else {
                        continue;
                    };
                    let b = match self.transform(&c, &cs, vp) {
                        Some(t) => map_rect(t.affine(), b),
                        None => b,
                    };
                    acc = Some(match acc {
                        None => b,
                        Some(a) => union(a, b),
                    });
                }
                acc
            }
            NodeType::SvgViewport => {
                let (x, y, w, h) = viewport_rect(style, vp);
                Some((x, y, w, h))
            }
            _ => self.geometry(node, style, vp)?.0.bounds(),
        }
    }

    fn geometry(
        &self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        vp: Viewport,
    ) -> Option<(Path, Option<Circle>)> {
        if node.node_type == NodeType::SvgCircle {
            let r = self
                .value(node.key, Property::R)
                .map_or(vp.d(style.r), |v| v.x as f32);
            let (cx, cy) = (vp.x(style.cx), vp.y(style.cy));
            return circle(cx, cy, r).map(|p| (p, Some((cx, cy, r))));
        }
        geometry(node.node_type, node.props, style, vp).map(|p| (p, None))
    }

    fn shape(&self, node: &NodeRef<'_>, style: &StyleProps, vp: Viewport) -> Option<Shape> {
        let (path, circle) = self.geometry(node, style, vp)?;
        let hidden = style.visibility != Visibility::Visible;
        let paint = |p: &Paint, opacity: f32| -> Option<ShapePaint> {
            if hidden {
                return None;
            }
            let color = match p {
                Paint::None => return None,
                Paint::CurrentColor => style.text_color,
                Paint::Color(c) => *c,
            };
            Some(ShapePaint {
                color,
                opacity: opacity.clamp(0.0, 1.0),
            })
        };
        let width = style.stroke_width.max(0.0);
        let stroke = paint(&style.stroke, style.stroke_opacity).filter(|_| width > 0.0);
        let scale = dash_scale(&path, node.props);
        let offset = style.stroke_dashoffset;
        Some(Shape {
            fill: paint(&style.fill, style.fill_opacity),
            fill_rule: style.fill_rule,
            stroke,
            width,
            cap: style.stroke_linecap,
            join: style.stroke_linejoin,
            miter: style.stroke_miterlimit.max(1.0),
            dash: style.stroke_dasharray.pattern(scale),
            dash_offset: offset * scale,
            dash_scale: scale,
            non_scaling: style.vector_effect == VectorEffect::NonScalingStroke,
            path,
            circle,
        })
    }

    fn viewport(
        &mut self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        vp: Viewport,
        ctm: Affine,
    ) -> Kind {
        let rect = viewport_rect(style, vp);
        let vb = view_box(node.props);
        let view = view_box_transform(
            vb,
            node.props.str(PropId::PreserveAspectRatio),
            rect.2,
            rect.3,
        )
        .map(|v| tf::mul(tf::translate(rect.0, rect.1), v));
        let inner = match vb {
            Some(v) => Viewport {
                width: v.width,
                height: v.height,
            },
            None => Viewport {
                width: rect.2,
                height: rect.3,
            },
        };
        let clip = style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible;
        let children = match view {
            Some(v) => self.children(node, style, inner, tf::mul(ctm, v)),
            None => Vec::new(),
        };
        Kind::Viewport {
            rect,
            view,
            clip,
            children,
        }
    }
}

/// A nested `svg`'s rect: `x`, `y`, and `width`/`height` whose `auto` is
/// 100% (SVG 2 §8.2).
fn viewport_rect(style: &StyleProps, vp: Viewport) -> (f32, f32, f32, f32) {
    let size = |d: Dimension, basis: f32| match d {
        Dimension::Auto => basis,
        d => super::length::resolve(d, basis),
    };
    (
        vp.x(style.x),
        vp.y(style.y),
        size(style.width, vp.width).max(0.0),
        size(style.height, vp.height).max(0.0),
    )
}

fn map_rect(m: Affine, (x, y, w, h): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let pts = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)].map(|p| tf::apply(m, p));
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (px, py) in pts {
        x0 = x0.min(px);
        y0 = y0.min(py);
        x1 = x1.max(px);
        y1 = y1.max(py);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

fn union(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

impl Scene {
    /// Every item, depth first in paint order.
    pub fn walk(&self, f: &mut impl FnMut(&Item)) {
        fn go(items: &[Item], f: &mut impl FnMut(&Item)) {
            for item in items {
                f(item);
                match &item.kind {
                    Kind::Group(c) | Kind::Viewport { children: c, .. } => go(c, f),
                    Kind::Shape(_) => {}
                }
            }
        }
        go(&self.items, f);
    }
}
