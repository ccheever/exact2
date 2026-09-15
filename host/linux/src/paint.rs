//! The painter: the kernel tree is the display list.
//!
//! @ref LLP 1015 §2; LLP 1014 D4 (a host that paints has the exact
//! invalidation answer because it *is* the display list)
//!
//! One walk of the live tree in preorder emits every node to a [`Backend`]:
//! background (the border box, per-corner radii), borders, an image by
//! `object-fit`, a text node's paragraph (the one the kernel measured,
//! [`crate::text`]), an input's value or placeholder and caret, then the
//! children — clipped when the node's effective overflow is not `visible`,
//! offset by its scroll position. Motion presentation values become a
//! transform about the box's center (CSS `translate` · `rotate` · `scale`)
//! and a group opacity (a layer, only when it is not 1). The walk also
//! records every node's painted box — the transformed bounding box in
//! viewport points and the clip it was painted under — which is what the
//! agent's `layout` reports and what hit-testing reads: no second geometry.
//!
//! Two backends draw what the walk emits: [`crate::gpu`] (vello over wgpu,
//! the main one) and [`crate::raster`] (tiny-skia on the CPU — the fallback
//! where there is no adapter, and the deterministic oracle for pixels).

use crate::text::{Paragraph, Run, Shared, Spec, TextEngine};
use exact_kernel::{
    Dimension, Display, Kernel, NodeRef, NodeType, ObjectFit, Overflow, PropId, StyleId, StyleMask,
    StyleProps, ViewId,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use tiny_skia::{Pixmap, Point, Transform};

/// A node's presentation values: what the motion engine says to paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presented {
    /// Points.
    pub translate: (f32, f32),
    /// Uniform.
    pub scale: f32,
    /// Degrees.
    pub rotate: f32,
    /// Zero to one.
    pub opacity: f32,
}

impl Presented {
    /// Nothing moved.
    pub const IDENTITY: Presented = Presented {
        translate: (0.0, 0.0),
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
    };

    /// The committed style's values (what the engine starts from).
    pub fn from_style(s: &StyleProps) -> Presented {
        Presented {
            translate: (s.translate.x, s.translate.y),
            scale: s.scale,
            rotate: s.rotate,
            opacity: s.opacity,
        }
    }

    fn moves(&self) -> bool {
        self.translate != (0.0, 0.0) || self.scale != 1.0 || self.rotate != 0.0
    }
}

/// A rectangle as (x, y, w, h).
pub type Rect4 = (f32, f32, f32, f32);

/// A rectangle with per-corner radii (top-left, top-right, bottom-right,
/// bottom-left), each clamped so neighbours never overlap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// The box.
    pub rect: Rect4,
    /// The radii.
    pub radii: [f32; 4],
}

impl Shape {
    /// A box with radii, clamped.
    pub fn new(rect: Rect4, radii: [f32; 4]) -> Shape {
        let limit = (rect.2 / 2.0).min(rect.3 / 2.0).max(0.0);
        Shape {
            rect,
            radii: radii.map(|r| r.max(0.0).min(limit)),
        }
    }

    /// A plain box.
    pub fn rect(rect: Rect4) -> Shape {
        Shape {
            rect,
            radii: [0.0; 4],
        }
    }

    /// Whether any corner is rounded.
    pub fn rounded(&self) -> bool {
        self.radii.iter().any(|r| *r > 0.0)
    }

    /// The same box inset on every side (radii shrink with it).
    pub fn inset(&self, by: f32) -> Shape {
        Shape::new(
            (
                self.rect.0 + by,
                self.rect.1 + by,
                (self.rect.2 - 2.0 * by).max(0.0),
                (self.rect.3 - 2.0 * by).max(0.0),
            ),
            self.radii.map(|r| (r - by).max(0.0)),
        )
    }
}

/// A node's box as painted: its transformed bounding box in viewport
/// points, the clip it was painted under (viewport points, axis-aligned),
/// and its scroll offset when it is a scroll container.
#[derive(Debug, Clone, Copy)]
pub struct PaintedBox {
    /// The node.
    pub id: ViewId,
    /// The box.
    pub rect: Rect4,
    /// The clip, `None` when unclipped.
    pub clip: Option<Rect4>,
    /// The scroll offset for a scroll container.
    pub scroll: Option<(f32, f32)>,
}

impl PaintedBox {
    /// Whether a point (viewport points) is inside the box and its clip.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let inside = |r: Rect4| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
        inside(self.rect) && self.clip.is_none_or(inside)
    }
}

/// What a frame is painted from.
pub struct Scene<'a> {
    /// The kernel: frames, styles, props.
    pub kernel: &'a Kernel,
    /// @ref LLP 1038 D6 — hidden route subtrees paint no pixels or hit boxes.
    pub hidden: &'a dyn Fn(ViewId) -> bool,
    /// The roots, in order.
    pub roots: &'a [ViewId],
    /// A node's presentation values.
    pub presented: &'a dyn Fn(ViewId) -> Presented,
    /// Scroll offsets of scroll containers (host state, LLP 1010).
    pub scroll: &'a BTreeMap<ViewId, (f32, f32)>,
    /// The page's scroll offset: the window is a viewport over a document.
    pub page: (f32, f32),
    /// Decoded images by node.
    pub images: &'a BTreeMap<ViewId, Rc<Pixmap>>,
    /// The focused input, if any (its caret is painted).
    pub focus: Option<ViewId>,
    /// The pointer, in viewport points, when the host draws one.
    pub pointer: Option<(f32, f32)>,
}

/// One painted frame.
pub struct Frame {
    /// The pixels, premultiplied RGBA at the device scale.
    pub pixmap: Pixmap,
    /// Every node's box, in paint order (a later box is above an earlier).
    pub boxes: Vec<PaintedBox>,
}

/// What draws the walk's output. Coordinates are viewport points with a
/// transform (points → points); a backend applies the device scale itself.
pub trait Backend {
    /// `"gpu"` or `"cpu"`.
    fn name(&self) -> &'static str;
    /// A new frame of this size in points at this scale, cleared to white.
    fn begin(&mut self, width: f32, height: f32, scale: f32);
    /// Fill a shape.
    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform);
    /// Stroke a shape's outline, centred on it.
    fn stroke(&mut self, shape: &Shape, width: f32, color: [u8; 4], ts: Transform);
    /// Draw a picture scaled into `dst`, clipped to every shape in `clips`.
    fn image(&mut self, image: &Rc<Pixmap>, dst: Rect4, clips: &[Shape], ts: Transform);
    /// Paint a paragraph with its top-left at `origin`.
    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        color: [u8; 4],
        origin: (f32, f32),
        ts: Transform,
    );
    /// Clip everything until the matching pop to a shape.
    fn push_clip(&mut self, shape: &Shape, ts: Transform);
    /// End a clip.
    fn pop_clip(&mut self);
    /// Composite everything until the matching pop at an opacity.
    fn push_opacity(&mut self, alpha: f32);
    /// End an opacity layer.
    fn pop_opacity(&mut self);
    /// The pointer arrow at a point.
    fn pointer(&mut self, x: f32, y: f32);
    /// The frame's pixels.
    fn finish(&mut self) -> Result<Pixmap, String>;
    /// The last frame's (encode + render, readback) milliseconds, on a
    /// backend that has them.
    fn last_frame_ms(&self) -> Option<(f64, f64)> {
        None
    }
}

/// The painter: the walk over one backend.
pub struct Painter {
    /// The text engine, shared with the kernel's measurer.
    pub text: Shared,
    /// Device pixels per point.
    pub scale: f32,
    /// Which appearance a `light-dark()` colour resolves to (LLP 1034 D2).
    /// This host has no system appearance of its own, so it is whatever the
    /// app's `setScheme` last said; `light` until it says otherwise.
    pub dark: bool,
    backend: Box<dyn Backend>,
}

struct Walk<'a, 'b> {
    scene: &'b Scene<'a>,
    boxes: Vec<PaintedBox>,
}

impl Painter {
    /// A painter over a backend.
    pub fn new(text: Shared, scale: f32, backend: Box<dyn Backend>) -> Painter {
        Painter {
            text,
            scale,
            dark: false,
            backend,
        }
    }

    /// The backend's name.
    pub fn backend(&self) -> &'static str {
        self.backend.name()
    }

    /// Another backend from here on (the CPU's, once the GPU's failed a
    /// frame).
    pub fn replace_backend(&mut self, backend: Box<dyn Backend>) {
        self.backend = backend;
    }

    /// The last frame's (encode + render, readback) milliseconds, on the GPU.
    pub fn last_frame_ms(&self) -> Option<(f64, f64)> {
        self.backend.last_frame_ms()
    }

    /// Paint the scene into a viewport of the given size (points).
    pub fn paint(&mut self, scene: &Scene<'_>, viewport: (f32, f32)) -> Result<Frame, String> {
        self.backend.begin(viewport.0, viewport.1, self.scale);
        let mut walk = Walk {
            scene,
            boxes: Vec::new(),
        };
        for root in scene.roots {
            self.node(&mut walk, *root, Transform::identity(), scene.page, None);
        }
        if let Some((px, py)) = scene.pointer {
            self.backend.pointer(px, py);
        }
        let pixmap = self.backend.finish()?;
        Ok(Frame {
            pixmap,
            boxes: walk.boxes,
        })
    }

    fn node(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip_rect: Option<Rect4>,
    ) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        if (walk.scene.hidden)(id)
            || node.style.display == Display::None
            || node.props.str(PropId::SemanticTag) == Some("dialog")
        {
            return;
        }
        let f = node.frame;
        let (x, y, w, h) = (f.x - offset.0, f.y - offset.1, f.width, f.height);
        let p = (walk.scene.presented)(id);
        let ts = if p.moves() {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            ts.pre_concat(
                Transform::from_translate(cx + p.translate.0, cy + p.translate.1)
                    .pre_rotate(p.rotate)
                    .pre_scale(p.scale, p.scale)
                    .pre_translate(-cx, -cy),
            )
        } else {
            ts
        };
        let scrolls = {
            let (ox, oy) = effective_overflow(&node);
            ox == Overflow::Scroll || oy == Overflow::Scroll
        };
        walk.boxes.push(PaintedBox {
            id,
            rect: bbox(ts, (x, y, w, h)),
            clip: clip_rect,
            scroll: scrolls.then(|| walk.scene.scroll.get(&id).copied().unwrap_or((0.0, 0.0))),
        });
        let opacity = p.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return;
        }
        if opacity < 1.0 {
            self.backend.push_opacity(opacity);
        }
        self.content(walk, &node, (x, y, w, h), ts, offset, clip_rect);
        if opacity < 1.0 {
            self.backend.pop_opacity();
        }
    }

    fn content(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        rect: Rect4,
        ts: Transform,
        offset: (f32, f32),
        clip_rect: Option<Rect4>,
    ) {
        let s = node.style;
        let (x, y, w, h) = rect;
        let outer = Shape::new(
            rect,
            [
                s.border_radius_top_left,
                s.border_radius_top_right,
                s.border_radius_bottom_right,
                s.border_radius_bottom_left,
            ],
        );
        if s.background_color.resolve(self.dark).a() > 0 && w > 0.0 && h > 0.0 {
            self.backend
                .fill(&outer, rgba(s.background_color.resolve(self.dark)), ts);
        }
        // Borders: a uniform border with a radius is a stroke inset by half
        // its width; anything else is four side rectangles (as the Apple
        // presenter draws them).
        let widths = s.border_widths();
        let current = node
            .computed_style(StyleMask::of(StyleId::TextColor))
            .text_color;
        let colors = s.border_colors(current);
        if widths.iter().any(|b| *b > 0.0) {
            let uniform =
                widths.iter().all(|b| *b == widths[0]) && colors.iter().all(|c| *c == colors[0]);
            if uniform && outer.rounded() && colors[0].resolve(self.dark).a() > 0 {
                let bw = widths[0];
                self.backend.stroke(
                    &outer.inset(bw / 2.0),
                    bw,
                    rgba(colors[0].resolve(self.dark)),
                    ts,
                );
            } else {
                let sides = [
                    (x, y, w, widths[0]),
                    (x + w - widths[1], y, widths[1], h),
                    (x, y + h - widths[2], w, widths[2]),
                    (x, y, widths[3], h),
                ];
                for (i, side) in sides.iter().enumerate() {
                    if widths[i] > 0.0
                        && colors[i].resolve(self.dark).a() > 0
                        && side.2 > 0.0
                        && side.3 > 0.0
                    {
                        self.backend.fill(
                            &Shape::rect(*side),
                            rgba(colors[i].resolve(self.dark)),
                            ts,
                        );
                    }
                }
            }
        }
        // The content box: inside the borders and the padding.
        let env = walk.scene.kernel.env();
        let pad = |d: Dimension| match d.resolve(&env) {
            Dimension::Points(p) => p,
            Dimension::Percent(p) => w * p / 100.0,
            Dimension::Auto | Dimension::Env(..) => 0.0,
        };
        let content = (
            x + widths[3] + pad(s.padding_left),
            y + widths[0] + pad(s.padding_top),
            (w - widths[3] - widths[1] - pad(s.padding_left) - pad(s.padding_right)).max(0.0),
            (h - widths[0] - widths[2] - pad(s.padding_top) - pad(s.padding_bottom)).max(0.0),
        );
        match node.node_type {
            NodeType::Image => {
                if let Some(img) = walk.scene.images.get(&node.id) {
                    if let Some(dst) = object_fit(img, s.object_fit, content) {
                        self.backend
                            .image(img, dst, &[Shape::rect(content), outer], ts);
                    }
                }
            }
            NodeType::Text => {
                if let Some(text) = node.props.str(PropId::Text) {
                    // Painted with the computed rows: what the kernel measured
                    // with, inherited font and colour included (LLP 1035.000).
                    let spec = text_spec(&node.computed_style(StyleMask::INHERITED), text);
                    let paragraph = self.text.borrow_mut().paragraph(&spec, Some(content.2));
                    let mut engine = self.text.borrow_mut();
                    self.backend.text(
                        &mut engine,
                        &paragraph,
                        rgba(node.text_color().resolve(self.dark)),
                        (content.0, content.1),
                        ts,
                    );
                }
            }
            NodeType::TextInput => {
                let value = node.props.str(PropId::Value).unwrap_or("");
                let placeholder = value.is_empty();
                let shown = if placeholder {
                    node.props.str(PropId::Placeholder).unwrap_or("")
                } else {
                    value
                };
                let computed = node.computed_style(StyleMask::INHERITED);
                let spec = text_spec(&computed, shown);
                let multiline = node.props.str(PropId::SemanticTag) == Some("textarea");
                let paragraph = self
                    .text
                    .borrow_mut()
                    .paragraph(&spec, multiline.then_some(content.2));
                let oy = content.1
                    + if multiline {
                        0.0
                    } else {
                        ((content.3 - paragraph.height) / 2.0).max(0.0)
                    };
                let ink = if placeholder {
                    [0x75, 0x75, 0x75, 0xff]
                } else {
                    rgba(node.text_color().resolve(self.dark))
                };
                {
                    let mut engine = self.text.borrow_mut();
                    self.backend
                        .text(&mut engine, &paragraph, ink, (content.0, oy), ts);
                }
                if walk.scene.focus == Some(node.id) {
                    let caret_x = content.0 + if placeholder { 0.0 } else { paragraph.width };
                    let caret_h = if paragraph.height > 0.0 {
                        paragraph.height
                    } else {
                        computed.font_size * 1.2
                    };
                    self.backend.fill(
                        &Shape::rect((caret_x, oy, 1.0, caret_h)),
                        rgba(
                            computed
                                .caret_color
                                .unwrap_or(node.text_color())
                                .resolve(self.dark),
                        ),
                        ts,
                    );
                }
            }
            _ => {}
        }
        // Children: clipped by this box when its overflow is not visible,
        // moved by its scroll offset when it scrolls.
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        let mut child_rect = clip_rect;
        if clips {
            self.backend.push_clip(&outer, ts);
            let own = bbox(ts, rect);
            child_rect = Some(match clip_rect {
                Some(c) => intersect(c, own),
                None => own,
            });
        }
        let child_offset = if ox == Overflow::Scroll || oy == Overflow::Scroll {
            let (sx, sy) = walk
                .scene
                .scroll
                .get(&node.id)
                .copied()
                .unwrap_or((0.0, 0.0));
            (offset.0 + sx, offset.1 + sy)
        } else {
            offset
        };
        for child in node.children() {
            self.node(walk, child, ts, child_offset, child_rect);
        }
        if clips {
            self.backend.pop_clip();
        }
    }
}

/// Where a picture goes under CSS `object-fit`, centred in the content box:
/// `fill` stretches, `contain`/`cover` keep the ratio, `none` is the natural
/// size, `scale-down` the smaller of none and contain (LLP 1011 §4).
pub fn object_fit(img: &Pixmap, fit: ObjectFit, content: Rect4) -> Option<Rect4> {
    let (nw, nh) = (img.width() as f32, img.height() as f32);
    if nw <= 0.0 || nh <= 0.0 || content.2 <= 0.0 || content.3 <= 0.0 {
        return None;
    }
    let (sx, sy) = (content.2 / nw, content.3 / nh);
    let s = match fit {
        ObjectFit::Contain => Some(sx.min(sy)),
        ObjectFit::Cover => Some(sx.max(sy)),
        ObjectFit::None => Some(1.0),
        ObjectFit::ScaleDown => Some(sx.min(sy).min(1.0)),
        ObjectFit::Fill => None,
    };
    let (dw, dh) = match s {
        Some(s) => (nw * s, nh * s),
        None => (content.2, content.3),
    };
    Some((
        content.0 + (content.2 - dw) / 2.0,
        content.1 + (content.3 - dh) / 2.0,
        dw,
        dh,
    ))
}

/// A text node's paragraph spec from its rows (the kernel's defaults are
/// CSS's, so every row reads directly).
pub fn text_spec(s: &StyleProps, text: &str) -> Spec {
    Spec {
        strut: Run::from_style("", exact_kernel::TextStyle::from_style(s)),
        runs: vec![Run::from_style(
            text,
            exact_kernel::TextStyle::from_style(s),
        )],
        align: s.text_align,
        line_clamp: s.line_clamp,
        overflow_wrap: s.overflow_wrap,
    }
}

/// A node's effective overflow per axis — the kernel's own rule: a
/// `ScrollView`/`List` scrolls on y unless its row says otherwise, and an
/// unset axis beside a non-visible one is scrollable (CSS Overflow §3).
pub fn effective_overflow(node: &NodeRef<'_>) -> (Overflow, Overflow) {
    let s = node.style;
    let mut y = if s.mask.has(StyleId::OverflowY) {
        s.overflow_y
    } else if node.node_type.scrolls_by_default() {
        Overflow::Scroll
    } else {
        Overflow::Visible
    };
    let mut x = if s.mask.has(StyleId::OverflowX) {
        s.overflow_x
    } else {
        Overflow::Visible
    };
    if x == Overflow::Visible && y != Overflow::Visible {
        x = Overflow::Scroll;
    } else if y == Overflow::Visible && x != Overflow::Visible {
        y = Overflow::Scroll;
    }
    (x, y)
}

/// A scroll container's content extent: the kernel's scrollable overflow,
/// floored with the direct children's extent plus the end padding (Taffy's
/// block containers do not always count end-edge padding), never less than
/// the box itself.
pub fn content_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    let env = kernel.env();
    let pad = |d: Dimension, against: f32| match d.resolve(&env) {
        Dimension::Points(p) => p,
        Dimension::Percent(p) => against * p / 100.0,
        Dimension::Auto | Dimension::Env(..) => 0.0,
    };
    let pad_right = pad(node.style.padding_right, node.frame.width);
    let pad_bottom = pad(node.style.padding_bottom, node.frame.width);
    let mut w = node.frame.width.max(node.content.0);
    let mut h = node.frame.height.max(node.content.1);
    for child in node.children() {
        if let Some(c) = kernel.node(child) {
            w = w.max(c.frame.x - node.frame.x + c.frame.width + pad_right);
            h = h.max(c.frame.y - node.frame.y + c.frame.height + pad_bottom);
        }
    }
    (w, h)
}

/// A kernel color's channels.
pub fn rgba(c: exact_kernel::Color) -> [u8; 4] {
    [c.r(), c.g(), c.b(), c.a()]
}

/// The bounding box of a rectangle under a transform.
pub fn bbox(ts: Transform, r: Rect4) -> Rect4 {
    if ts.is_identity() {
        return r;
    }
    let corners = [
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0, r.1 + r.3),
        (r.0 + r.2, r.1 + r.3),
    ];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (cx, cy) in corners {
        let mut p = Point::from_xy(cx, cy);
        ts.map_point(&mut p);
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

fn intersect(a: Rect4, b: Rect4) -> Rect4 {
    let x0 = a.0.max(b.0);
    let y0 = a.1.max(b.1);
    let x1 = (a.0 + a.2).min(b.0 + b.2);
    let y1 = (a.1 + a.3).min(b.1 + b.3);
    (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

/// The pointer arrow's outline, at the origin, in points.
pub const POINTER: [(f32, f32); 7] = [
    (0.0, 0.0),
    (0.0, 16.0),
    (4.0, 12.5),
    (7.0, 19.0),
    (9.5, 18.0),
    (6.5, 11.5),
    (11.5, 11.5),
];
