//! The painter: the kernel tree is the display list.
//!
//! @ref LLP 1015 §2; LLP 1014 D4 (a host that paints has the exact
//! invalidation answer because it *is* the display list)
//!
//! One walk of the live tree in preorder paints every node into a
//! premultiplied RGBA pixmap with tiny-skia: background (the border box,
//! per-corner radii), borders, an image by `object-fit`, a text node's
//! paragraph (the one the kernel measured, [`crate::text`]), an input's
//! value or placeholder and caret, then the children — clipped when the
//! node's effective overflow is not `visible`, offset by its scroll
//! position. Motion presentation values become a transform about the box's
//! center (CSS `translate` · `rotate` · `scale`) and a group opacity (a
//! layer, only when it is not 1). The walk also records every node's
//! painted box — the transformed bounding box in viewport points and the
//! clip it was painted under — which is what the agent's `layout` reports
//! and what hit-testing reads: no second geometry.

use crate::text::{Run, Shared, Spec};
use exact_kernel::{
    Dimension, Display, FontStyle, Kernel, NodeRef, NodeType, ObjectFit, Overflow, PropId, StyleId,
    StyleProps, ViewId,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use tiny_skia::{
    Color, FillRule, FilterQuality, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Point,
    Rect, Stroke, Transform,
};

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

/// The painter.
pub struct Painter {
    /// The text engine, shared with the kernel's measurer.
    pub text: Shared,
    /// Device pixels per point.
    pub scale: f32,
}

struct Walk<'a, 'b> {
    scene: &'b Scene<'a>,
    boxes: Vec<PaintedBox>,
    width: u32,
    height: u32,
}

impl Painter {
    /// Paint the scene into a viewport of the given size (points).
    pub fn paint(&mut self, scene: &Scene<'_>, viewport: (f32, f32)) -> Frame {
        let width = ((viewport.0 * self.scale).round() as u32).max(1);
        let height = ((viewport.1 * self.scale).round() as u32).max(1);
        let mut pixmap = Pixmap::new(width, height).expect("a viewport has pixels");
        pixmap.fill(Color::WHITE);
        let mut walk = Walk {
            scene,
            boxes: Vec::new(),
            width,
            height,
        };
        for root in scene.roots {
            self.node(
                &mut walk,
                *root,
                Transform::identity(),
                scene.page,
                None,
                None,
                &mut pixmap,
            );
        }
        if let Some((px, py)) = scene.pointer {
            self.pointer(&mut pixmap, px, py);
        }
        Frame {
            pixmap,
            boxes: walk.boxes,
        }
    }

    fn device(&self, ts: Transform) -> Transform {
        Transform::from_scale(self.scale, self.scale).pre_concat(ts)
    }

    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        walk: &mut Walk<'_, '_>,
        id: ViewId,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<&Rc<Mask>>,
        clip_rect: Option<Rect4>,
        target: &mut Pixmap,
    ) {
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        if node.style.display == Display::None {
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
            // Group opacity: the subtree into a layer, composited once.
            let Some(mut layer) = Pixmap::new(walk.width, walk.height) else {
                return;
            };
            self.content(
                walk,
                &node,
                (x, y, w, h),
                ts,
                offset,
                clip,
                clip_rect,
                &mut layer,
            );
            let paint = PixmapPaint {
                opacity,
                ..PixmapPaint::default()
            };
            target.draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
        } else {
            self.content(
                walk,
                &node,
                (x, y, w, h),
                ts,
                offset,
                clip,
                clip_rect,
                target,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn content(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &NodeRef<'_>,
        rect: Rect4,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<&Rc<Mask>>,
        clip_rect: Option<Rect4>,
        target: &mut Pixmap,
    ) {
        let s = node.style;
        let (x, y, w, h) = rect;
        let dev = self.device(ts);
        let mask = clip.map(|m| &**m);
        let radii = [
            s.border_radius_top_left,
            s.border_radius_top_right,
            s.border_radius_bottom_right,
            s.border_radius_bottom_left,
        ];
        let outer = rounded_rect(rect, radii);
        if let Some(outer) = &outer {
            if s.background_color.a() > 0 {
                target.fill_path(
                    outer,
                    &solid(color(s.background_color)),
                    FillRule::Winding,
                    dev,
                    mask,
                );
            }
        }
        // Borders: a uniform border with a radius is a stroke inset by half
        // its width; anything else is four side rectangles (as the Apple
        // presenter draws them).
        let widths = [
            s.border_width_top,
            s.border_width_right,
            s.border_width_bottom,
            s.border_width_left,
        ];
        let colors = [
            s.border_color_top,
            s.border_color_right,
            s.border_color_bottom,
            s.border_color_left,
        ];
        if widths.iter().any(|b| *b > 0.0) {
            let uniform =
                widths.iter().all(|b| *b == widths[0]) && colors.iter().all(|c| *c == colors[0]);
            if uniform && radii.iter().any(|r| *r > 0.0) && colors[0].a() > 0 {
                let bw = widths[0];
                let inset = (
                    x + bw / 2.0,
                    y + bw / 2.0,
                    (w - bw).max(0.0),
                    (h - bw).max(0.0),
                );
                let inner_radii = radii.map(|r| (r - bw / 2.0).max(0.0));
                if let Some(path) = rounded_rect(inset, inner_radii) {
                    let stroke = Stroke {
                        width: bw,
                        ..Stroke::default()
                    };
                    target.stroke_path(&path, &solid(color(colors[0])), &stroke, dev, mask);
                }
            } else {
                let sides = [
                    (x, y, w, widths[0]),
                    (x + w - widths[1], y, widths[1], h),
                    (x, y + h - widths[2], w, widths[2]),
                    (x, y, widths[3], h),
                ];
                for (i, side) in sides.iter().enumerate() {
                    if widths[i] > 0.0 && colors[i].a() > 0 {
                        if let Some(r) = Rect::from_xywh(side.0, side.1, side.2, side.3) {
                            target.fill_rect(r, &solid(color(colors[i])), dev, mask);
                        }
                    }
                }
            }
        }
        // The content box: inside the borders and the padding.
        let pad = |d: Dimension| match d {
            Dimension::Points(p) => p,
            Dimension::Percent(p) => w * p / 100.0,
            Dimension::Auto => 0.0,
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
                    self.image(
                        walk,
                        target,
                        img,
                        s.object_fit,
                        content,
                        outer.as_ref(),
                        dev,
                        clip,
                    );
                }
            }
            NodeType::Text => {
                if let Some(text) = node.props.str(PropId::Text) {
                    let spec = text_spec(s, text);
                    let paragraph = self.text.borrow_mut().paragraph(&spec, Some(content.2));
                    self.text.borrow_mut().paint(
                        target,
                        &paragraph,
                        rgba(s.text_color),
                        (content.0, content.1),
                        self.scale,
                        dev,
                        mask,
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
                let spec = text_spec(s, shown);
                let paragraph = self.text.borrow_mut().paragraph(&spec, None);
                let oy = content.1 + ((content.3 - paragraph.height) / 2.0).max(0.0);
                let ink = if placeholder {
                    [0x75, 0x75, 0x75, 0xff]
                } else {
                    rgba(s.text_color)
                };
                self.text.borrow_mut().paint(
                    target,
                    &paragraph,
                    ink,
                    (content.0, oy),
                    self.scale,
                    dev,
                    mask,
                );
                if walk.scene.focus == Some(node.id) {
                    let caret_x = content.0 + if placeholder { 0.0 } else { paragraph.width };
                    let caret_h = if paragraph.height > 0.0 {
                        paragraph.height
                    } else {
                        s.font_size * 1.2
                    };
                    if let Some(r) = Rect::from_xywh(caret_x, oy, 1.0, caret_h) {
                        target.fill_rect(r, &solid(color(s.text_color)), dev, mask);
                    }
                }
            }
            _ => {}
        }
        // Children: clipped by this box when its overflow is not visible,
        // moved by its scroll offset when it scrolls.
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        let mut child_clip: Option<Rc<Mask>> = clip.cloned();
        let mut child_rect = clip_rect;
        if clips {
            if let Some(outer) = &outer {
                let mut m = match clip {
                    Some(c) => (**c).clone(),
                    None => {
                        let Some(mut m) = Mask::new(walk.width, walk.height) else {
                            return;
                        };
                        m.fill_path(outer, FillRule::Winding, true, dev);
                        m
                    }
                };
                if clip.is_some() {
                    m.intersect_path(outer, FillRule::Winding, true, dev);
                }
                child_clip = Some(Rc::new(m));
            }
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
            self.node(
                walk,
                child,
                ts,
                child_offset,
                child_clip.as_ref(),
                child_rect,
                target,
            );
        }
    }

    /// CSS `object-fit` over the content box, clipped to it and to the
    /// border box's rounded path (LLP 1011 §4).
    #[allow(clippy::too_many_arguments)]
    fn image(
        &mut self,
        walk: &mut Walk<'_, '_>,
        target: &mut Pixmap,
        img: &Pixmap,
        fit: ObjectFit,
        content: Rect4,
        outer: Option<&Path>,
        dev: Transform,
        clip: Option<&Rc<Mask>>,
    ) {
        let (nw, nh) = (img.width() as f32, img.height() as f32);
        if nw <= 0.0 || nh <= 0.0 || content.2 <= 0.0 || content.3 <= 0.0 {
            return;
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
        let ox = content.0 + (content.2 - dw) / 2.0;
        let oy = content.1 + (content.3 - dh) / 2.0;
        let Some(content_rect) = Rect::from_xywh(content.0, content.1, content.2, content.3) else {
            return;
        };
        let mut m = match clip {
            Some(c) => (**c).clone(),
            None => {
                let Some(mut m) = Mask::new(walk.width, walk.height) else {
                    return;
                };
                m.fill_path(
                    &PathBuilder::from_rect(content_rect),
                    FillRule::Winding,
                    true,
                    dev,
                );
                m
            }
        };
        if clip.is_some() {
            m.intersect_path(
                &PathBuilder::from_rect(content_rect),
                FillRule::Winding,
                true,
                dev,
            );
        }
        if let Some(outer) = outer {
            m.intersect_path(outer, FillRule::Winding, true, dev);
        }
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        let ts = dev.pre_concat(Transform::from_translate(ox, oy).pre_scale(dw / nw, dh / nh));
        target.draw_pixmap(0, 0, img.as_ref(), &paint, ts, Some(&m));
    }

    /// A pointer, when the host has no compositor to draw one.
    fn pointer(&self, target: &mut Pixmap, px: f32, py: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0);
        pb.line_to(0.0, 16.0);
        pb.line_to(4.0, 12.5);
        pb.line_to(7.0, 19.0);
        pb.line_to(9.5, 18.0);
        pb.line_to(6.5, 11.5);
        pb.line_to(11.5, 11.5);
        pb.close();
        let Some(path) = pb.finish() else { return };
        let ts = self.device(Transform::from_translate(px, py));
        target.fill_path(&path, &solid(Color::WHITE), FillRule::Winding, ts, None);
        let stroke = Stroke {
            width: 1.0,
            ..Stroke::default()
        };
        target.stroke_path(&path, &solid(Color::BLACK), &stroke, ts, None);
    }
}

/// A text node's paragraph spec from its rows (the kernel's defaults are
/// CSS's, so every row reads directly).
pub fn text_spec(s: &StyleProps, text: &str) -> Spec {
    Spec {
        runs: vec![Run {
            text: text.to_string(),
            size: s.font_size,
            weight: s.font_weight,
            italic: s.font_style != FontStyle::Normal,
            line_height: s.line_height,
            letter_spacing: s.letter_spacing,
        }],
        align: s.text_align,
        line_clamp: s.line_clamp,
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
    let pad = |d: Dimension, against: f32| match d {
        Dimension::Points(p) => p,
        Dimension::Percent(p) => against * p / 100.0,
        Dimension::Auto => 0.0,
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

fn color(c: exact_kernel::Color) -> Color {
    Color::from_rgba8(c.r(), c.g(), c.b(), c.a())
}

fn rgba(c: exact_kernel::Color) -> [u8; 4] {
    [c.r(), c.g(), c.b(), c.a()]
}

fn solid(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

/// The bounding box of a rectangle under a transform.
fn bbox(ts: Transform, r: Rect4) -> Rect4 {
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

/// A rectangle with per-corner radii (top-left, top-right, bottom-right,
/// bottom-left), each clamped so neighbours never overlap.
pub fn rounded_rect(r: Rect4, radii: [f32; 4]) -> Option<Path> {
    let (x, y, w, h) = r;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let limit = (w / 2.0).min(h / 2.0);
    let [tl, tr, br, bl] = radii.map(|r| r.max(0.0).min(limit));
    if tl == 0.0 && tr == 0.0 && br == 0.0 && bl == 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    const K: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    if tr > 0.0 {
        pb.cubic_to(
            x + w - tr + tr * K,
            y,
            x + w,
            y + tr - tr * K,
            x + w,
            y + tr,
        );
    }
    pb.line_to(x + w, y + h - br);
    if br > 0.0 {
        pb.cubic_to(
            x + w,
            y + h - br + br * K,
            x + w - br + br * K,
            y + h,
            x + w - br,
            y + h,
        );
    }
    pb.line_to(x + bl, y + h);
    if bl > 0.0 {
        pb.cubic_to(
            x + bl - bl * K,
            y + h,
            x,
            y + h - bl + bl * K,
            x,
            y + h - bl,
        );
    }
    pb.line_to(x, y + tl);
    if tl > 0.0 {
        pb.cubic_to(x, y + tl - tl * K, x + tl - tl * K, y, x + tl, y);
    }
    pb.close();
    pb.finish()
}
