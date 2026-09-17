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

use crate::image::Bitmap;
use crate::text::{Paragraph, Run, RunPaint, Shared, Spec, TextEngine};
use exact_kernel::{
    Dimension, Display, Kernel, NodeRef, NodeType, ObjectFit, Overflow, PropId, StyleId, StyleMask,
    StyleProps, ViewId,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{Pixmap, Point, Transform};
mod region;

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
    pub images: &'a BTreeMap<ViewId, Arc<Bitmap>>,
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
    fn image(&mut self, image: &Arc<Bitmap>, dst: Rect4, clips: &[Shape], ts: Transform);
    /// Paint a paragraph with its top-left at `origin`.
    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
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
    // One lease per actually accepted owner, not one global width per string.
    // Retained while a subsequent backend frame fails.
    accepted_text: BTreeMap<exact_kernel::NodeKey, Rc<Paragraph>>,
    region_picture: Option<Rc<region::Picture>>,
    region_frame: Option<region::Published>,
}

struct Walk<'a, 'b> {
    scene: &'b Scene<'a>,
    boxes: Vec<PaintedBox>,
    text: BTreeMap<exact_kernel::NodeKey, Rc<Paragraph>>,
    skip: Option<exact_kernel::NodeKey>,
    // Record traversal alone supplies native exact paragraphs. The normal
    // shell path and ordinary opt-out remain unchanged.
    region: Option<&'b exact_kernel::RegionPublication>,
    capture: Option<&'b region::Capture>,
    replay: Option<&'b region::Replay<'b>>,
    region_error: Option<&'static str>,
}

impl Painter {
    /// A painter over a backend.
    pub fn new(text: Shared, scale: f32, backend: Box<dyn Backend>) -> Painter {
        Painter {
            text,
            scale,
            dark: false,
            backend,
            accepted_text: BTreeMap::new(),
            region_picture: None,
            region_frame: None,
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

    /// Accepted leases outside the current text catalog, deduplicated by Rc.
    /// A catalog swap followed by failed frames retains the previous accepted
    /// set until successful replacement. Diagnostic-only; not total residency.
    pub fn retiring_text_residency(&self) -> crate::text::RetiringResidency {
        self.text
            .borrow()
            .retiring_accepted(self.accepted_text.values())
    }

    /// Paint the scene into a viewport of the given size (points).
    pub fn paint(&mut self, scene: &Scene<'_>, viewport: (f32, f32)) -> Result<Frame, String> {
        self.paint_selected(scene, viewport, None, None)
    }

    /// Paint exactly the registered selected branch. An accepted publication
    /// requires its native snapshot; live candidate text is never a fallback.
    pub fn paint_region(
        &mut self,
        scene: &Scene<'_>,
        viewport: (f32, f32),
        region: &crate::content_region::ContentRegionState,
    ) -> Result<Frame, String> {
        region.validate_scale(self.scale)?;
        self.validate_region_presentation(scene, region)?;
        if self.backend.name() == "gpu" {
            return Err("content-region trial requires CPU painting".into());
        }
        let receipt = region
            .receipt()
            .ok_or("content region has no successful layout")?;
        match &receipt.selection {
            exact_kernel::RegionSelection::Pending(key) if *key == region.binding().pending => {
                let frame =
                    self.paint_selected(scene, viewport, Some(region.binding().content), None)?;
                self.region_picture = None;
                self.region_frame = Some(region::Published {
                    incarnation: region.incarnation().clone(),
                    selection: None,
                });
                Ok(frame)
            }
            exact_kernel::RegionSelection::Pending(_) => {
                Err("content region placeholder identity mismatch".into())
            }
            exact_kernel::RegionSelection::Accepted(publication) => {
                let picture = if receipt.current {
                    // A flat native paint/hit snapshot, never an app/layout
                    // graph. No UTF-8 copy or cold text lookup is permitted.
                    region::Picture::capture(self, scene, region, publication)?
                } else {
                    self.region_picture
                        .as_ref()
                        .filter(|p| p.belongs_to(region))
                        .cloned()
                        .ok_or("retained content has no matching native picture")?
                };
                let replay = region::Replay {
                    picture: &picture,
                    origin: receipt.origin,
                    content: region.binding().content,
                    viewport,
                };
                let frame = self.paint_selected(
                    scene,
                    viewport,
                    Some(region.binding().pending),
                    Some(&replay),
                )?;
                self.region_frame = Some(region::Published {
                    incarnation: region.incarnation().clone(),
                    selection: Some((picture.publication().clone(), receipt.origin)),
                });
                self.region_picture = Some(picture);
                Ok(frame)
            }
        }
    }

    fn paint_selected(
        &mut self,
        scene: &Scene<'_>,
        viewport: (f32, f32),
        skip: Option<exact_kernel::NodeKey>,
        replay: Option<&region::Replay<'_>>,
    ) -> Result<Frame, String> {
        self.backend.begin(viewport.0, viewport.1, self.scale);
        let mut walk = Walk {
            scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip,
            region: None,
            capture: None,
            replay,
            region_error: None,
        };
        for root in scene.roots {
            self.node(&mut walk, *root, Transform::identity(), scene.page, None);
        }
        if let Some((px, py)) = scene.pointer {
            self.backend.pointer(px, py);
        }
        let finished = self.backend.finish().and_then(|p| match walk.region_error {
            Some(error) => Err(error.into()),
            None => Ok(p),
        });
        // Publication is the ownership boundary. On Err the previous accepted
        // set remains intact; candidate leases simply unwind with `walk`.
        if finished.is_ok() {
            self.accepted_text = walk.text;
        } else {
            drop(walk.text);
        }
        // Pending measurements end with every paint attempt, including failure.
        self.text.borrow_mut().finish_text_frame();
        let pixmap = finished?;
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
        if walk.region_error.is_some() {
            return;
        }
        if let Some(replay) = walk.replay.filter(|r| {
            walk.scene
                .kernel
                .node(id)
                .is_some_and(|n| n.key == r.content)
        }) {
            replay.paint(self, walk, ts, offset, clip_rect);
            return;
        }
        let Some(node) = walk.scene.kernel.node(id) else {
            return;
        };
        if walk.skip == Some(node.key) {
            return;
        }
        if (walk.scene.hidden)(id)
            || node.is_inline_run()
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
        if let Some(capture) = walk.capture {
            capture.hit(node.key, *walk.boxes.last().unwrap());
        }
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
                    if let Some(dst) = object_fit(img.natural(), s.object_fit, content) {
                        self.backend
                            .image(img, dst, &[Shape::rect(content), outer], ts);
                    }
                }
            }
            NodeType::Text => {
                if let Some(publication) = walk.region {
                    if let Some(artifact) = publication.paint_artifact(node.key) {
                        if let Some(native) =
                            artifact.payload::<crate::content_region::NativeText>()
                        {
                            if let Some(paragraph) = native.paragraph() {
                                walk.text.insert(node.key, paragraph.clone());
                                self.backend.text(
                                    &mut self.text.borrow_mut(),
                                    paragraph,
                                    &native.palette(self.dark),
                                    (content.0, content.1),
                                    ts,
                                );
                            }
                        }
                    }
                } else {
                    // The kernel measures a Text subtree as one paragraph. Inline
                    // descendants deliberately have zero frames, not paint boxes.
                    let build = || {
                        let mut spec = text_spec(&node.computed_style(StyleMask::INHERITED), "");
                        spec.runs = node
                            .text_runs()
                            .iter()
                            .map(|run| Run::from_style(run.text, run.style))
                            .collect();
                        spec
                    };
                    let paragraph = if let Some(stamp) = node.paragraph_stamp() {
                        self.text
                            .borrow_mut()
                            .paragraph_identified(&stamp, Some(content.2), build)
                    } else {
                        let spec = build();
                        (!spec.is_empty())
                            .then(|| self.text.borrow_mut().paragraph(&spec, Some(content.2)))
                    };
                    if let Some(paragraph) = paragraph {
                        let mut palette = Vec::new();
                        text_palette(walk.scene.kernel, node, self.dark, &mut palette);
                        walk.text.insert(node.key, paragraph.clone());
                        let mut engine = self.text.borrow_mut();
                        self.backend.text(
                            &mut engine,
                            &paragraph,
                            &palette,
                            (content.0, content.1),
                            ts,
                        );
                    }
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
                walk.text.insert(node.key, paragraph.clone());
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
                    self.backend.text(
                        &mut engine,
                        &paragraph,
                        &[RunPaint {
                            color: ink,
                            source: node.id,
                        }],
                        (content.0, oy),
                        ts,
                    );
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
        let scrolls = ox == Overflow::Scroll || oy == Overflow::Scroll;
        let child_offset = if scrolls {
            let (sx, sy) = walk
                .scene
                .scroll
                .get(&node.id)
                .copied()
                .unwrap_or((0.0, 0.0));
            if let Some(capture) = walk.capture {
                capture.scroll(node.key, (sx, sy));
                offset
            } else {
                (offset.0 + sx, offset.1 + sy)
            }
        } else {
            offset
        };
        for child in node.children() {
            self.node(walk, child, ts, child_offset, child_rect);
        }
        if scrolls {
            if let Some(capture) = walk.capture {
                capture.end_scroll();
            }
        }
        if clips {
            self.backend.pop_clip();
        }
    }
}

/// Where a picture goes under CSS `object-fit`, centred in the content box:
/// `fill` stretches, `contain`/`cover` keep the ratio, `none` is the natural
/// size, `scale-down` the smaller of none and contain (LLP 1011 §4).
pub fn object_fit(natural: (u32, u32), fit: ObjectFit, content: Rect4) -> Option<Rect4> {
    let (nw, nh) = (natural.0 as f32, natural.1 as f32);
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

/// Mirror the canonical run ownership (own text suppresses descendants),
/// retaining paint-only information without adding it to the metric ABI.
fn text_palette(kernel: &Kernel, node: &NodeRef<'_>, dark: bool, out: &mut Vec<RunPaint>) {
    if node.props.str(PropId::Text).is_some() {
        out.push(RunPaint {
            color: rgba(node.text_color().resolve(dark)),
            source: node.id,
        });
    } else {
        for child in node.children() {
            if let Some(child) = kernel.node(child).filter(|c| c.node_type == NodeType::Text) {
                text_palette(kernel, &child, dark, out);
            }
        }
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

#[cfg(test)]
mod paragraph_tests {
    use super::*;
    use crate::presenter::{PainterChoice, Presenter};
    use exact_runner::{DataError, DataSource, Value};
    use std::rc::Rc;

    #[derive(Default)]
    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
            Err(DataError::UnknownSource(source.into()))
        }
    }

    fn fixture(text: &str) -> Presenter<NoData> {
        let source = format!("component App\n  view\n    column width=\"100%\" padding=20 box-sizing=\"border-box\"\n{text}");
        let plan = contract::compile(&source).unwrap();
        let (presenter, error) = Presenter::boot_with(
            &plan.encode(),
            NoData,
            (300.0, 300.0),
            1.0,
            std::path::PathBuf::new(),
            PainterChoice::Cpu,
        )
        .unwrap();
        assert!(error.is_none(), "{error:?}");
        presenter
    }

    #[test]
    fn nested_text_paints_the_same_paragraph_that_layout_measured() {
        let mut plain = fixture("      text \"Alpha beta gamma delta. Another line wraps here.\" testId=\"paragraph\" font-size=16 line-height=1.5 color=\"#234567\"\n");
        let mut nested = fixture("      text testId=\"paragraph\" font-size=16 line-height=1.5 color=\"#234567\"\n        text \"Alpha beta \"\n        text\n          text \"gamma delta. \"\n          text \"Another line wraps here.\"\n");
        for width in [300.0, 160.0, 240.0] {
            assert!(plain.resize(width, 300.0).is_none());
            assert!(nested.resize(width, 300.0).is_none());
            let frame = |p: &Presenter<NoData>| {
                let kernel = p.host().kernel();
                kernel
                    .node_by_key(kernel.find_by_test_id("paragraph")[0])
                    .unwrap()
                    .frame
            };
            assert_eq!(
                frame(&plain),
                frame(&nested),
                "kernel paragraph geometry agrees at {width}"
            );
            let expected = plain.frame();
            let actual = nested.frame();
            assert!(expected
                .data()
                .chunks_exact(4)
                .any(|p| p != [255, 255, 255, 255]));
            assert!(
                actual.data() == expected.data(),
                "nested run pixels differ despite identical measured paragraph at width {width}"
            );
        }
    }

    fn scene_frame(p: &Presenter<NoData>, dark: bool, backend: Box<dyn Backend>) -> Frame {
        let kernel = p.host().kernel();
        let scene = Scene {
            kernel,
            roots: &kernel.roots(),
            hidden: &|_| false,
            presented: &|_| Presented::IDENTITY,
            scroll: &BTreeMap::new(),
            page: (0.0, 0.0),
            images: &BTreeMap::new(),
            focus: None,
            pointer: None,
        };
        let mut painter = Painter::new(p.text().clone(), 1.0, backend);
        painter.dark = dark;
        painter.paint(&scene, (300.0, 300.0)).unwrap()
    }

    const COLORS: &str = "      text testId=\"paragraph\" color=\"#00000000\" font-size=24 line-height=1.5\n        text color=\"light-dark(#ff0000,#008000)\"\n          text \"MMMM \" font-weight=700 testId=\"red\"\n        text color=\"light-dark(#0000ff,#800080)\" href=\"https://example.com/\" testId=\"link\"\n          text \"WWWW\" font-family=\"monospace\" font-style=\"italic\" font-size=18 testId=\"blue\"\n";

    fn has_color(frame: &tiny_skia::Pixmap, color: [u8; 4]) -> bool {
        frame.data().chunks_exact(4).filter(|p| *p == color).count() > 10
    }

    #[test]
    fn nested_run_colors_survive_inheritance_and_appearance() {
        let p = fixture(COLORS);
        for (dark, colors) in [
            (false, [[255, 0, 0, 255], [0, 0, 255, 255]]),
            (true, [[0, 128, 0, 255], [128, 0, 128, 255]]),
        ] {
            let frame = scene_frame(&p, dark, Box::new(crate::raster::Raster::new()));
            for color in colors {
                assert!(
                    has_color(&frame.pixmap, color),
                    "missing {color:?}, dark={dark}"
                );
            }
        }
    }

    #[test]
    fn styled_paragraph_metrics_and_cpu_gpu_glyph_batches_agree() {
        let mut p = fixture(COLORS);
        for width in [300.0, 160.0, 240.0] {
            assert!(p.resize(width, 300.0).is_none());
            let kernel = p.host().kernel();
            let node = kernel
                .node_by_key(kernel.find_by_test_id("paragraph")[0])
                .unwrap();
            let canonical = node.text_runs();
            let mut spec = text_spec(&node.computed_style(StyleMask::INHERITED), "");
            spec.runs = canonical
                .iter()
                .map(|r| Run::from_style(r.text, r.style))
                .collect();
            assert_eq!(spec.runs[0].weight, 700);
            assert!(spec.runs[1].italic);
            assert_eq!(spec.runs[1].size, 18.0);
            let mut light = Vec::new();
            let mut dark = Vec::new();
            text_palette(kernel, &node, false, &mut light);
            text_palette(kernel, &node, true, &mut dark);
            assert_eq!(light.len(), canonical.len());
            assert_eq!(dark.len(), canonical.len());
            let leaf = kernel.node(light[1].source).unwrap();
            assert_eq!(leaf.props.str(PropId::TestId), Some("blue"));
            let link = kernel.node(leaf.parent.unwrap()).unwrap();
            assert_eq!(link.props.str(PropId::Href), Some("https://example.com/"));
            let mut engine = p.text().borrow_mut();
            let paragraph = engine.paragraph(&spec, Some(node.frame.width));
            let metrics =
                engine.measure(&spec, exact_kernel::AxisOffer::Definite(node.frame.width));
            assert_eq!(paragraph.height, metrics.height);
            assert_eq!(Some(paragraph.first_baseline), metrics.first_baseline);
            assert_eq!(paragraph.height, node.frame.height);
            for palette in [&light, &dark] {
                let cpu: Vec<_> = paragraph
                    .paint_glyphs(palette)
                    .map(|(g, baseline, ink)| {
                        (
                            g.glyph_id as u32,
                            g.x + g.x_offset * g.font_size,
                            baseline + g.y - g.y_offset * g.font_size,
                            g.metadata,
                            ink,
                            g.cache_key_flags
                                .contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC),
                        )
                    })
                    .collect();
                let gpu: Vec<_> = engine
                    .glyph_runs(&paragraph, palette)
                    .iter()
                    .flat_map(|run| {
                        run.glyphs.iter().map(|(id, x, y)| {
                            (*id, *x, *y, run.run_index, run.paint, run.synthetic_italic)
                        })
                    })
                    .collect();
                assert!(!cpu.is_empty());
                assert_eq!(cpu, gpu, "common colored glyph stream at width {width}");
            }
            assert!(
                Rc::ptr_eq(&paragraph, &engine.paragraph(&spec, Some(node.frame.width))),
                "appearance/source metadata must not split the shaping cache"
            );
        }
    }

    #[test]
    fn own_text_suppresses_inline_descendants_in_measurement_and_paint() {
        let mut plain = fixture("      text \"Owner\" font-size=24 color=\"#ff0000\"\n");
        let mut nested = fixture("      text \"Owner\" font-size=24 color=\"#ff0000\"\n        text \"Must not paint\" color=\"#0000ff\"\n");
        assert_eq!(plain.frame().data(), nested.frame().data());
    }

    #[test]
    fn identical_fonts_keep_distinct_run_colors_and_source_nodes() {
        let style = StyleProps {
            font_size: 24.0,
            ..StyleProps::default()
        };
        let mut spec = text_spec(&style, "MMMM ");
        spec.runs.push(Run::from_style(
            "WWWW",
            exact_kernel::TextStyle::from_style(&style),
        ));
        let palette = [
            RunPaint {
                color: [255, 0, 0, 255],
                source: 1,
            },
            RunPaint {
                color: [0, 0, 255, 255],
                source: 2,
            },
        ];
        let mut engine = TextEngine::new();
        let paragraph = engine.paragraph(&spec, Some(260.0));
        let batches = engine.glyph_runs(&paragraph, &palette);
        assert_eq!(
            batches.len(),
            2,
            "same font must not merge distinct ink/source runs"
        );
        for (index, batch) in batches.iter().enumerate() {
            assert_eq!(batch.run_index, index);
            assert_eq!(batch.paint, palette[index]);
        }
        let mut raster = crate::raster::Raster::new();
        raster.begin(300.0, 80.0, 1.0);
        raster.text(
            &mut engine,
            &paragraph,
            &palette,
            (20.0, 20.0),
            Transform::identity(),
        );
        let pixels = raster.finish().unwrap();
        assert!(palette.iter().all(|ink| has_color(&pixels, ink.color)));
    }

    #[test]
    fn styled_paragraph_pixels_on_real_gpu_when_available() {
        let gpu = match crate::gpu::Gpu::new() {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!(
                    "GPU paragraph pixels NOT checked: {error}; common batch test still runs"
                );
                return;
            }
        };
        eprintln!("GPU paragraph pixels: {} / {}", gpu.adapter, gpu.api);
        let p = fixture(COLORS);
        let mut painter = Painter::new(p.text().clone(), 1.0, Box::new(gpu));
        let kernel = p.host().kernel();
        let scene = Scene {
            kernel,
            roots: &kernel.roots(),
            hidden: &|_| false,
            presented: &|_| Presented::IDENTITY,
            scroll: &BTreeMap::new(),
            page: (0.0, 0.0),
            images: &BTreeMap::new(),
            focus: None,
            pointer: None,
        };
        for (dark, colors) in [
            (false, [[255, 0, 0, 255], [0, 0, 255, 255]]),
            (true, [[0, 128, 0, 255], [128, 0, 128, 255]]),
        ] {
            painter.dark = dark;
            let frame = painter.paint(&scene, (300.0, 300.0)).unwrap();
            let cpu = scene_frame(&p, dark, Box::new(crate::raster::Raster::new()));
            for color in colors {
                assert!(
                    has_color(&frame.pixmap, color),
                    "GPU missing {color:?}, dark={dark}"
                );
                let bounds = |pixmap: &Pixmap| {
                    let mut bounds = [u32::MAX, u32::MAX, 0, 0];
                    for (i, pixel) in pixmap.data().chunks_exact(4).enumerate() {
                        if pixel == color {
                            let (x, y) = (i as u32 % pixmap.width(), i as u32 / pixmap.width());
                            bounds = [
                                bounds[0].min(x),
                                bounds[1].min(y),
                                bounds[2].max(x),
                                bounds[3].max(y),
                            ];
                        }
                    }
                    bounds
                };
                for (cpu, gpu) in bounds(&cpu.pixmap).into_iter().zip(bounds(&frame.pixmap)) {
                    assert!(
                        cpu.abs_diff(gpu) <= 2,
                        "CPU/GPU colored ink bounds differ: {cpu}, {gpu}"
                    );
                }
            }
        }
    }
}
