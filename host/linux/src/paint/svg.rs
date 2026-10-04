//! An `svg`'s content in the paint walk (LLP 1055 D4, LLP 1055.000 D1): the
//! kernel resolves the scene, and this paints it inside the content box,
//! clipped to the box when the `svg`'s overflow is not visible, each shape
//! through [`Backend::svg_path`]; group opacity is a group, as in CSS.

use super::{effective_overflow, rgba, Painter, Rect4, Shape, Walk};
use exact_kernel::svg::scene::{self, Item, Kind, ShapePaint};
use exact_kernel::svg::server::Server;
use exact_kernel::svg::Path;
use exact_kernel::{NodeKey, NodeRef, Overflow};
use exact_motion::{Property, Value};
use tiny_skia::Transform;

#[path = "island.rs"]
mod island;

/// What a fill or a stroke paints with.
pub enum Ink<'a> {
    /// One colour, straight RGBA.
    Solid([u8; 4]),
    /// A gradient (LLP 1055.000 D7): its geometry and spread, the stops
    /// with the paint's opacity folded in, and gradient space to the
    /// path's space.
    Gradient {
        /// The resolved server.
        server: &'a Server,
        /// Offsets and straight RGBA colours.
        stops: Vec<(f32, [u8; 4])>,
        /// Gradient space to the path's space.
        transform: [f32; 6],
    },
    /// A pattern (LLP 1055.000 D7): its tile rendered to pixels, repeated;
    /// the transform maps the tile's pixels to the path's space.
    Pattern {
        /// The tile, premultiplied, at device scale.
        tile: std::sync::Arc<tiny_skia::Pixmap>,
        /// Tile pixels to the path's space.
        transform: [f32; 6],
        /// The paint's opacity.
        opacity: f32,
    },
}

/// One shape as a backend paints it: path in user units, paint resolved.
pub struct SvgPaint<'a> {
    /// The path.
    pub path: &'a Path,
    /// Fill, or none.
    pub fill: Option<Ink<'a>>,
    /// `fill-rule: evenodd`.
    pub even_odd: bool,
    /// Stroke, or none.
    pub stroke: Option<Ink<'a>>,
    /// `paint-order`: fill (0), stroke (1), markers (2).
    pub order: [u8; 3],
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

impl Painter {
    /// SVG text (LLP 1055.000 D11): each run shaped as a one-line paragraph
    /// by the text engine, its chunk anchored by the runs' total advance and
    /// set on the baseline `dominant-baseline` names. The fill is painted;
    /// a stroke or a gradient on text is owed on this host.
    fn svg_text(&mut self, item: &exact_kernel::svg::scene::TextItem, ts: Transform) {
        use exact_kernel::{DominantBaseline, TextAnchor, WhiteSpace};
        let mut pen = (0.0f32, 0.0f32);
        for chunk in &item.chunks {
            let shaped: Vec<_> = chunk
                .runs
                .iter()
                .map(|r| {
                    let s = exact_kernel::StyleProps {
                        font_size: r.style.font_size,
                        font_weight: r.style.font_weight,
                        font_style: r.style.font_style,
                        font_family: r.style.font_family,
                        letter_spacing: r.style.letter_spacing,
                        white_space: WhiteSpace::PreWrap,
                        ..Default::default()
                    };
                    let spec = super::text_spec(&s, &r.text);
                    (self.text.borrow_mut().paragraph(&spec, None), r)
                })
                .collect();
            let width: f32 = shaped.iter().map(|(p, r)| p.width + r.dx).sum();
            let anchor = match chunk.anchor {
                TextAnchor::Start => 0.0,
                TextAnchor::Middle => width / 2.0,
                TextAnchor::End => width,
            };
            let mut x = chunk.x.unwrap_or(pen.0) - anchor;
            let mut y = chunk.y.unwrap_or(pen.1);
            // Chrome's baselines from the first run's font: integer ascent
            // and descent, x-height about half an em (LLP 1055.000 D11).
            let (ascent, descent, size) = shaped.first().map_or((0.0, 0.0, 0.0), |(p, r)| {
                let a = p.first_baseline.round();
                (a, (p.height - p.first_baseline).round(), r.style.font_size)
            });
            let shift = match chunk.baseline {
                DominantBaseline::Middle => 0.53 * size / 2.0,
                DominantBaseline::Central => (ascent - descent) / 2.0,
                DominantBaseline::Hanging => 0.8 * ascent,
                DominantBaseline::Ideographic => -descent,
                DominantBaseline::Mathematical => ascent / 2.0,
                _ => 0.0,
            };
            for (p, r) in &shaped {
                x += r.dx;
                y += r.dy;
                if let Some(fill) = &r.fill {
                    let mut c = rgba(fill.color.resolve(self.dark));
                    c[3] = (c[3] as f32 * fill.opacity).round() as u8;
                    let palette = [crate::text::RunPaint {
                        color: c,
                        source: r.id,
                    }];
                    let mut engine = self.text.borrow_mut();
                    self.backend.text(
                        &mut engine,
                        p,
                        &palette,
                        (x, y + shift - p.first_baseline),
                        ts,
                    );
                }
                x += p.width;
            }
            pen = (x, y);
        }
    }
}

/// A fill's or stroke's ink: its colour, or its gradient's stops with the
/// paint's opacity folded in and the gradient mapped into the path's space.
fn ink(p: Option<&ShapePaint>, dark: bool, to_path: [f32; 6]) -> Option<Ink<'_>> {
    let p = p?;
    let fold = |c: exact_kernel::ColorValue, o: f32| {
        let mut c = rgba(c.resolve(dark));
        c[3] = (c[3] as f32 * o).round() as u8;
        c
    };
    Some(match &p.server {
        None => Ink::Solid(fold(p.color, p.opacity)),
        Some(server) => Ink::Gradient {
            server,
            stops: server
                .stops
                .iter()
                .map(|s| (s.offset, fold(s.color, s.opacity * p.opacity)))
                .collect(),
            transform: exact_kernel::svg::transform::mul(to_path, server.transform),
        },
    })
}

pub(super) fn affine(t: [f32; 6]) -> Transform {
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
        let scene = resolve_svg(walk.scene, node, content);
        if let Some(refusal) = scene.refused {
            // @ref LLP 1055.000 D13 — refused by name, once.
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("exact svg: {refusal}");
            }
        }
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        // The elements the reader animates (`crate::host::lower`).
        self.svg_layers.clear();
        let mut stack: Vec<&Item> = scene.items.iter().collect();
        while let Some(item) = stack.pop() {
            let p = (walk.scene.presented)(item.id);
            if p.lowered != 0 {
                self.svg_layers.push((item.id, p));
            }
            if let Kind::Group(children) | Kind::Viewport { children, .. } = &item.kind {
                stack.extend(children);
            }
        }
        self.paint_svg(&scene, clips, rect, content, ts);
        self.svg_layers.clear();
    }

    /// A resolved `svg` scene inside its box: clipped to the border box
    /// `rect` when `clips`, its view box mapped into `content`. A retained
    /// region replays a scene it resolved at capture.
    pub(super) fn paint_svg(
        &mut self,
        scene: &exact_kernel::svg::Scene,
        clips: bool,
        rect: Rect4,
        content: Rect4,
        ts: Transform,
    ) {
        if scene.view.is_none() {
            return;
        }
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
}

/// The scene of the `svg` `node` whose content box is `content`, with the
/// motion engine's presented values (LLP 1055.000 D1).
pub(super) fn resolve_svg(
    walk: &super::Scene<'_>,
    node: &NodeRef<'_>,
    content: Rect4,
) -> exact_kernel::svg::Scene {
    resolve_with(walk.kernel, node, content, walk.presented)
}

/// [`resolve_svg`] with the host's presented values by view.
pub fn resolve_with(
    kernel: &exact_kernel::Kernel,
    node: &NodeRef<'_>,
    content: Rect4,
    presented: &dyn Fn(exact_kernel::ViewId) -> super::Presented,
) -> exact_kernel::svg::Scene {
    {
        let value = |key: NodeKey, p: Property| -> Option<Value> {
            let id = kernel.node_by_key(key)?.id;
            let v = presented(id);
            Some(match p {
                Property::Opacity => Value::scalar(v.opacity as f64),
                Property::Translate => Value::new(v.translate.0 as f64, v.translate.1 as f64),
                Property::Scale => Value::scalar((v.scale * v.press) as f64),
                Property::Rotate => Value::scalar(v.rotate as f64),
                Property::R => Value::scalar(v.svg[0]? as f64),
                Property::StrokeDashoffset => Value::scalar(v.svg[1]? as f64),
                Property::Cx => Value::scalar(v.svg[2]? as f64),
                Property::Cy => Value::scalar(v.svg[3]? as f64),
                Property::X => Value::scalar(v.svg[4]? as f64),
                Property::Y => Value::scalar(v.svg[5]? as f64),
                Property::Rx => Value::scalar(v.svg[6]? as f64),
                Property::Ry => Value::scalar(v.svg[7]? as f64),
                p if Property::PAINT.contains(&p) => v.colors.value(p)?,
                _ => return None,
            })
        };
        scene::resolve(kernel, node, content, &value)
    }
}

impl Painter {
    /// A shape's fill and/or stroke (`parts`), in its own space `own`.
    fn svg_shape(
        &mut self,
        item: &Item,
        shape: &exact_kernel::svg::Shape,
        own: Transform,
        origin: Transform,
        parts: (bool, bool),
    ) {
        let dark = self.dark;
        // Drawn in the content box's space, the stroke unscaled
        // (LLP 1055.000 D5): a gradient maps through the same ctm.
        let to_path = if shape.non_scaling {
            item.ctm
        } else {
            exact_kernel::svg::transform::IDENTITY
        };
        let mapped;
        let (path, space) = if shape.non_scaling {
            mapped = shape.path.transformed(item.ctm);
            (&mapped, origin)
        } else {
            (&shape.path, own)
        };
        let fill = shape.fill.as_ref().filter(|_| parts.0);
        let stroke = shape.stroke.as_ref().filter(|_| parts.1);
        let paint = SvgPaint {
            path,
            fill: match fill {
                Some(p) if p.pattern.is_some() => self.pattern_ink(p, space, to_path),
                p => ink(p, dark, to_path),
            },
            even_odd: shape.fill_rule == exact_kernel::FillRule::Evenodd,
            stroke: match stroke {
                Some(p) if p.pattern.is_some() => self.pattern_ink(p, space, to_path),
                p => ink(p, dark, to_path),
            },
            order: shape.order,
            width: shape.width,
            cap: shape.cap as u8,
            join: shape.join as u8,
            miter: shape.miter,
            dash: shape.dash.clone(),
            phase: shape.dash_offset,
        };
        self.backend.svg_path(&paint, space);
    }

    /// One item in its parent's space `ts`; `origin` is the content box's
    /// space, where a non-scaling stroke is drawn.
    pub(super) fn svg_item(&mut self, item: &Item, ts: Transform, origin: Transform) {
        let own = match &item.transform {
            Some(t) => ts.pre_concat(affine(t.affine())),
            None => ts,
        };
        let (layer, own) = match self.svg_layers.is_empty() {
            true => (super::layer::Opened { lowered: 0 }, own),
            false => self.svg_layer(item, ts, own),
        };
        let opacity = if layer.opacity() { 1.0 } else { item.opacity };
        if opacity <= 0.0 {
            self.layer_close(layer);
            return;
        }
        // @ref LLP 1055.000 D19 — a blended element is drawn alone into an
        // island over the frame, then blended onto what is below it.
        if item.blend != 0 {
            let (vw, vh) = self.viewport;
            let mut plain = item.clone();
            plain.blend = 0;
            if let Some(pixels) = self.island((0.0, 0.0, vw, vh), |p, shift| {
                p.svg_item(&plain, shift.pre_concat(ts), shift.pre_concat(origin));
            }) {
                let rect = (0.0, 0.0, vw, vh);
                self.backend.island_image(
                    std::sync::Arc::new(pixels),
                    rect,
                    Transform::identity(),
                    item.blend,
                );
            }
            return;
        }
        let isolate = item.isolate && opacity >= 1.0;
        if isolate {
            self.backend.push_opacity(1.0);
        }
        if opacity < 1.0 {
            self.backend.push_opacity(opacity);
        }
        let clips = item
            .clip
            .as_ref()
            .map_or(0, |c| self.backend.push_svg_clip(c, own));
        match &item.mask {
            // @ref LLP 1055.000 D10 — a mask is an island.
            Some(mask) => self.svg_masked(item, mask, own, origin),
            None => self.svg_effects(item, own, origin),
        }
        for _ in 0..clips {
            self.backend.pop_clip();
        }
        if opacity < 1.0 {
            self.backend.pop_opacity();
        }
        if isolate {
            self.backend.pop_opacity();
        }
        self.layer_close(layer);
    }

    /// What an item draws, in its own space `own`.
    pub(super) fn svg_kind(&mut self, item: &Item, own: Transform, origin: Transform) {
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
            Kind::Text(text) => self.svg_text(text, own),
            Kind::Shape(shape) => {
                // @ref LLP 1055.000 D9 — markers paint where `paint-order`
                // puts them: before, between, or after the fill and stroke.
                let at = shape.order.iter().position(|&o| o == 2).unwrap_or(2);
                let markers = |this: &mut Self| {
                    for m in &shape.markers {
                        this.svg_item(m, own, origin);
                    }
                };
                match at {
                    0 => {
                        markers(self);
                        self.svg_shape(item, shape, own, origin, (true, true));
                    }
                    1 => {
                        let fill_first = shape.order[0] == 0;
                        self.svg_shape(item, shape, own, origin, (fill_first, !fill_first));
                        markers(self);
                        self.svg_shape(item, shape, own, origin, (!fill_first, fill_first));
                    }
                    _ => {
                        self.svg_shape(item, shape, own, origin, (true, true));
                        markers(self);
                    }
                }
            }
        }
    }
}
