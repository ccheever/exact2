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

mod clip;
mod filter;
mod hit;
mod island;
mod marker;
mod text;
pub use clip::{Clip, ClipShape};
pub use island::{Mask, Pattern};
pub use text::{TextChunk, TextItem, TextRun};

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
    /// What this host cannot draw and so leaves out, by name: a
    /// `foreignObject` (LLP 1055.000 D13), which is the web's alone.
    pub refused: Option<&'static str>,
}

/// The refusal a native host reports for a `foreignObject`.
pub const FOREIGN_OBJECT: &str = "`foreignObject` renders on the web only (LLP 1055.000 D13): on this host, position a box over the `svg` instead";

/// One rendered element.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The element's node (for an instance under a `use`, the target's
    /// node: events on it go to the `use`, LLP 1055.000 D8).
    pub id: ViewId,
    /// A key unique in the scene: the node id, or for an instance the node
    /// id mixed with the `use`s above it, so two instances of one node are
    /// two layers.
    pub uid: u64,
    /// Its generation-checked key (the motion engine's).
    pub key: NodeKey,
    /// Group opacity, presented.
    pub opacity: f32,
    /// The element's own transform; `None` is the identity.
    pub transform: Option<Transform>,
    /// The element's user space to the `svg`'s content box, its own
    /// transform included (a non-scaling stroke and hit testing use it).
    pub ctm: Affine,
    /// `clip-path: url(#…)`: the clip in the element's user space.
    pub clip: Option<Box<Clip>>,
    /// `mask: url(#…)`: the mask in the element's user space (LLP
    /// 1055.000 D10), applied inside the clip.
    pub mask: Option<Box<Mask>>,
    /// `filter`: the chain, in the element's user space (LLP 1055.000
    /// D14), applied before the clip and the mask.
    pub filter: Option<Box<super::filter::Filter>>,
    /// `mix-blend-mode`, as an index into `filter::BLEND_MODES` (0 is
    /// `normal`), and `isolation: isolate` (LLP 1055.000 D19).
    pub blend: u8,
    /// `isolation: isolate`: the element's content blends within it.
    pub isolate: bool,
    /// A `use`: what it draws is an instance, and hits it as the `use`.
    pub instance: bool,
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
    /// A `text` element (LLP 1055.000 D11).
    Text(Box<TextItem>),
}

/// Paint, resolved: a colour (a `light-dark()` pair kept for the host's
/// appearance) or a gradient, and the paint's opacity
/// (`fill-opacity`/`stroke-opacity`).
#[derive(Debug, Clone, PartialEq)]
pub struct ShapePaint {
    /// The colour, when `server` is `None`.
    pub color: ColorValue,
    /// Zero to one.
    pub opacity: f32,
    /// A gradient in the shape's user space (LLP 1055.000 D7).
    pub server: Option<Box<super::server::Server>>,
    /// A pattern, whose pattern space maps to the shape's user space (LLP
    /// 1055.000 D7). `color` and `server` are unused beside it.
    pub pattern: Option<Box<Pattern>>,
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
    /// `paint-order`: fill (0), stroke (1), markers (2), first to last.
    pub order: [u8; 3],
    /// `pointer-events`, with what it reads: whether the shape is visible
    /// and which of its paints are set (LLP 1055.000 D17).
    pub pointer_events: crate::generated::PointerEvents,
    /// `visibility: visible`.
    pub visible: bool,
    /// Whether `fill` and `stroke` are other than `none`.
    pub painted: (bool, bool),
    /// Marker instances at its vertices, in its user space, painted where
    /// `paint-order` puts markers (LLP 1055.000 D9). They take no hits.
    pub markers: Vec<Item>,
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
    let mut r = Resolver {
        kernel,
        presented,
        uses: Vec::new(),
        refused: None,
    };
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
        refused: r.refused,
    }
}

struct Resolver<'k, 'p> {
    kernel: &'k Kernel,
    presented: Presented<'p>,
    /// The `use` elements (and marker instances) above the item being
    /// resolved, outermost first: its instance path, and what a cycle is
    /// checked against.
    uses: Vec<u64>,
    /// A refusal met on the walk.
    refused: Option<&'static str>,
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
                if node.node_type == NodeType::SvgForeignObject {
                    self.refused = Some(FOREIGN_OBJECT);
                }
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
        // Definitions render only where they are referenced.
        if !node.node_type.is_svg_element() || !node.node_type.renders() {
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
            NodeType::SvgUse => self.instance(node, &style, vp, ctm)?,
            NodeType::SvgText => Kind::Text(Box::new(self.text(node, &style, vp)?)),
            _ => {
                let mut shape = self.shape(node, &style, vp, ctm)?;
                shape.markers = self.markers(node, &style, &shape.path, vp, ctm);
                Kind::Shape(Box::new(shape))
            }
        };
        let clip = if style.clip_path.url().is_some() {
            let bbox = match &kind {
                Kind::Shape(s) => s.path.bounds(),
                _ => self.bbox(node, &style, vp),
            };
            self.clip(node, bbox, vp, 0).map(Box::new)
        } else {
            None
        };
        let mask = if style.svg_mask.url().is_some() {
            let bbox = match &kind {
                Kind::Shape(s) => s.path.bounds(),
                _ => self.bbox(node, &style, vp),
            };
            self.mask(node, bbox, vp, ctm).map(Box::new)
        } else {
            None
        };
        let filter = if style.filter.is_none() {
            None
        } else {
            let (bbox, stroke) = match &kind {
                Kind::Shape(s) => (
                    s.path.bounds(),
                    if s.stroke.is_some() {
                        s.width / 2.0
                    } else {
                        0.0
                    },
                ),
                _ => (self.bbox(node, &style, vp), 0.0),
            };
            self.filter(node, bbox, stroke, vp).map(Box::new)
        };
        Some(Item {
            id: node.id,
            uid: self.uid(node.id),
            clip,
            mask,
            filter,
            blend: style.mix_blend_mode as u8,
            isolate: style.isolation == crate::generated::Isolation::Isolate,
            instance: node.node_type == NodeType::SvgUse,
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
        // @ref LLP 1055.000 D15 — geometry rows moving under an animation
        // or transition: their presented lengths, in user units.
        const MOVING: [(Property, StyleId); 6] = [
            (Property::Cx, StyleId::Cx),
            (Property::Cy, StyleId::Cy),
            (Property::X, StyleId::X),
            (Property::Y, StyleId::Y),
            (Property::Rx, StyleId::Rx),
            (Property::Ry, StyleId::Ry),
        ];
        let mut moved: Option<StyleProps> = None;
        for (p, row) in MOVING {
            if let Some(v) = self.value(node.key, p) {
                let s = moved.get_or_insert_with(|| style.clone());
                let d = Dimension::Points(v.x as f32);
                match row {
                    StyleId::Cx => s.cx = d,
                    StyleId::Cy => s.cy = d,
                    StyleId::X => s.x = d,
                    StyleId::Y => s.y = d,
                    StyleId::Rx => s.rx = d,
                    _ => s.ry = d,
                }
            }
        }
        let style = moved.as_ref().unwrap_or(style);
        if node.node_type == NodeType::SvgCircle {
            let r = self
                .value(node.key, Property::R)
                .map_or(vp.d(style.r), |v| v.x as f32);
            let (cx, cy) = (vp.x(style.cx), vp.y(style.cy));
            return circle(cx, cy, r).map(|p| (p, Some((cx, cy, r))));
        }
        geometry(node.node_type, node.props, style, vp).map(|p| (p, None))
    }

    fn shape(
        &mut self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        vp: Viewport,
        ctm: Affine,
    ) -> Option<Shape> {
        let (path, circle) = self.geometry(node, style, vp)?;
        let hidden = style.visibility != Visibility::Visible;
        let bbox = path.bounds();
        // A pattern paint's tile, resolved first: `Some(None)` paints
        // nothing, `None` is not a pattern (LLP 1055.000 D7).
        let mut patterns = [None, None];
        for (slot, p) in patterns.iter_mut().zip([&style.fill, &style.stroke]) {
            if let (Paint::Url(id, _), false) = (p, hidden) {
                *slot = match self.pattern(node, id, bbox, vp, ctm) {
                    Ok(Some(pat)) => Some(Some(Box::new(pat))),
                    Ok(None) => None,
                    Err(()) => Some(None),
                };
            }
        }
        let [fill_pattern, stroke_pattern] = patterns;
        let paint = |p: &Paint,
                     opacity: f32,
                     pattern: Option<Option<Box<Pattern>>>|
         -> Option<ShapePaint> {
            if hidden {
                return None;
            }
            let opacity = opacity.clamp(0.0, 1.0);
            if let Some(pattern) = pattern {
                return pattern.map(|pat| ShapePaint {
                    color: style.text_color,
                    opacity,
                    server: None,
                    pattern: Some(pat),
                });
            }
            let color = match p {
                Paint::None => return None,
                Paint::CurrentColor => style.text_color,
                Paint::Color(c) => *c,
                Paint::Url(id, _) => {
                    use super::server::{resolve, Resolved};
                    return match resolve(self.kernel, node.id, id, bbox, vp) {
                        Resolved::Server(server) => Some(ShapePaint {
                            color: style.text_color,
                            opacity,
                            server: Some(Box::new(server)),
                            pattern: None,
                        }),
                        Resolved::Color(color, stop) => Some(ShapePaint {
                            color,
                            opacity: opacity * stop,
                            server: None,
                            pattern: None,
                        }),
                        Resolved::Nothing => None,
                        // A missing server paints the fallback, or nothing.
                        Resolved::Fallback => match p.fallback()? {
                            Paint::CurrentColor => Some(ShapePaint {
                                color: style.text_color,
                                opacity,
                                server: None,
                                pattern: None,
                            }),
                            Paint::Color(c) => Some(ShapePaint {
                                color: c,
                                opacity,
                                server: None,
                                pattern: None,
                            }),
                            _ => None,
                        },
                    };
                }
            };
            Some(ShapePaint {
                color,
                opacity,
                server: None,
                pattern: None,
            })
        };
        let width = style.stroke_width.max(0.0);
        let stroke =
            paint(&style.stroke, style.stroke_opacity, stroke_pattern).filter(|_| width > 0.0);
        let scale = dash_scale(&path, node.props);
        let offset = style.stroke_dashoffset;
        Some(Shape {
            fill: paint(&style.fill, style.fill_opacity, fill_pattern),
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
            order: style.paint_order.0,
            pointer_events: style.pointer_events,
            visible: !hidden,
            painted: (
                !matches!(style.fill, Paint::None),
                !matches!(style.stroke, Paint::None),
            ),
            path,
            circle,
            markers: Vec::new(),
        })
    }

    /// A key unique in the scene for `id` under the current `use` path.
    /// Kept within 53 bits, so a host reading it as a JSON number keeps it.
    fn uid(&self, id: ViewId) -> u64 {
        if self.uses.is_empty() {
            return id as u64;
        }
        self.uses.iter().fold(id as u64, |acc, u| {
            (acc ^ (u << 32) ^ (u >> 32))
                .rotate_left(13)
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                & ((1 << 53) - 1)
        })
    }

    /// A `use`: an instance of its target, inheriting from the `use` (SVG 2
    /// §5.6), placed at `x`/`y`. A `symbol` (or `svg`) target is a viewport
    /// sized by the `use`'s `width`/`height` (`auto` is 100%) under the
    /// target's view box (LLP 1055.000 D8). A missing target, or one whose
    /// instance would contain this `use`, renders nothing.
    fn instance(
        &mut self,
        node: &NodeRef<'_>,
        style: &StyleProps,
        vp: Viewport,
        ctm: Affine,
    ) -> Option<Kind> {
        let target_id = node
            .props
            .str(PropId::Href)
            .and_then(|h| h.strip_prefix('#'))
            .and_then(|h| self.kernel.resolve_id(node.id, h))?;
        if self.uses.contains(&(node.id as u64)) || self.uses.len() > 16 {
            return None;
        }
        // The target must not contain the `use` itself.
        let mut up = Some(node.id);
        while let Some(id) = up {
            if id == target_id {
                return None;
            }
            up = self.kernel.node(id).and_then(|n| n.parent);
        }
        let target = self.kernel.node(target_id)?;
        let (x, y) = (vp.x(style.x), vp.y(style.y));
        self.uses.push(node.id as u64);
        let kind = if matches!(
            target.node_type,
            NodeType::SvgSymbol | NodeType::SvgViewport | NodeType::Svg
        ) {
            let size = |d: Dimension, basis: f32| match d {
                Dimension::Auto => basis,
                d => super::length::resolve(d, basis),
            };
            let rect = (
                x,
                y,
                size(style.width, vp.width).max(0.0),
                size(style.height, vp.height).max(0.0),
            );
            let tstyle = cascade(&target, style);
            let vb = view_box(target.props);
            let view = view_box_transform(
                vb,
                target.props.str(PropId::PreserveAspectRatio),
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
            let clip =
                tstyle.overflow_x != Overflow::Visible || tstyle.overflow_y != Overflow::Visible;
            let children = match view {
                Some(v) => self.children(&target, &tstyle, inner, tf::mul(ctm, v)),
                None => Vec::new(),
            };
            Kind::Viewport {
                rect,
                view,
                clip,
                children,
            }
        } else {
            let placed = tf::mul(ctm, tf::translate(x, y));
            let item = self.item_any(&target, style, vp, placed);
            let mut inner = Kind::Group(item.into_iter().collect());
            if (x, y) != (0.0, 0.0) {
                inner = Kind::Viewport {
                    rect: (0.0, 0.0, 0.0, 0.0),
                    view: Some(tf::translate(x, y)),
                    clip: false,
                    children: match inner {
                        Kind::Group(c) => c,
                        _ => Vec::new(),
                    },
                };
            }
            inner
        };
        self.uses.pop();
        Some(kind)
    }

    /// `item`, for an instance's target: a definition (a `symbol`, or an
    /// element inside `defs`) renders here though it does not where it
    /// stands.
    fn item_any(
        &mut self,
        node: &NodeRef<'_>,
        inherited: &StyleProps,
        vp: Viewport,
        ctm: Affine,
    ) -> Option<Item> {
        self.item(node, inherited, vp, ctm)
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
                    Kind::Shape(s) => go(&s.markers, f),
                    Kind::Text(_) => {}
                }
            }
        }
        go(&self.items, f);
    }
}
