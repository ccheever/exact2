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
//! transform about its `transform-origin` (CSS `translate` · `rotate` · `scale`)
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
pub mod border;
pub(crate) mod control;
pub(crate) mod damage;
pub mod gradient;
pub use gradient::GradientPaint;
mod inline;
mod placed;
mod presented;
mod region;
mod shadow;
mod space;
mod svg;
mod text_clip;
mod text_shadow;
mod text_stroke;
use inline::{presented_color, presented_text_colors, text_backgrounds, text_palette};
pub use presented::{PaintValues, Presented};
pub(crate) use region::{ActionNode, ActionSlot, RegionActions, ScrollBounds};
pub use svg::{resolve_with, Ink, SvgPaint};

// Only an explicit row makes a raster a template; motion may supply its ink.
fn image_tint(style: &StyleProps, presented: &Presented, dark: bool) -> Option<[u8; 4]> {
    style.mask.has(StyleId::TintColor).then(|| {
        presented
            .colors
            .color(exact_motion::Property::TintColor)
            .unwrap_or_else(|| rgba(style.tint_color.resolve(dark)))
    })
}

/// A rectangle as (x, y, w, h).
pub type Rect4 = (f32, f32, f32, f32);

/// A rectangle with per-corner radii (top-left, top-right, bottom-right,
/// bottom-left), reduced by CSS's one factor so neighbours never overlap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    /// The box.
    pub rect: Rect4,
    /// The radii.
    pub radii: [(f32, f32); 4],
    /// CSS `corner-shape` when any corner is not `round` (LLP 1077 D1).
    pub corners: Option<exact_kernel::corner::CornerShape>,
}

impl Shape {
    /// A box with radii, reduced as CSS reduces them: every corner by the
    /// one factor that fits the tightest edge.
    pub fn new(rect: Rect4, radii: [f32; 4]) -> Shape {
        Self::elliptical(rect, radii.map(|r| (r.max(0.0), r.max(0.0))))
    }

    /// A box with independent horizontal and vertical corner radii.
    pub fn elliptical(rect: Rect4, radii: [(f32, f32); 4]) -> Shape {
        Shape {
            rect,
            radii: border::reduced(radii, rect.2, rect.3),
            corners: None,
        }
    }

    /// A plain box.
    pub fn rect(rect: Rect4) -> Shape {
        Shape {
            rect,
            radii: [(0.0, 0.0); 4],
            corners: None,
        }
    }

    /// The same box with these corner shapes; `round` ones are none.
    pub fn with_corners(mut self, corners: Option<exact_kernel::corner::CornerShape>) -> Shape {
        self.corners = corners.filter(|c| !c.is_round());
        self
    }

    /// Whether any corner is rounded.
    pub fn rounded(&self) -> bool {
        self.radii.iter().any(|r| r.0 > 0.0 && r.1 > 0.0)
    }

    /// The same box inset on every side (radii shrink with it).
    pub fn inset(&self, by: f32) -> Shape {
        Shape::elliptical(
            (
                self.rect.0 + by,
                self.rect.1 + by,
                (self.rect.2 - 2.0 * by).max(0.0),
                (self.rect.3 - 2.0 * by).max(0.0),
            ),
            self.radii
                .map(|(x, y)| ((x - by).max(0.0), (y - by).max(0.0))),
        )
        .with_corners(self.corners)
    }
}

// Frozen numeric/style operands. Capture resolves environment and appearance
// once; geometry is evaluated at the published frame with ordinary f32 order.
struct BoxPaint {
    radii: [Dimension; 4],
    /// CSS `background-clip` (LLP 1077 D6).
    clip: exact_kernel::BackgroundClip,
    corners: Option<exact_kernel::corner::CornerShape>,
    widths: [f32; 4],
    colors: [[u8; 4]; 4],
    background: [u8; 4],
    gradients: Vec<gradient::Captured>,
    padding: [f32; 4],
    shadows: Vec<shadow::ShadowPaint>,
    /// `backdrop-filter: blur(σ)`, σ in points; 0 for none, or under a
    /// host material, which wins (LLP 1053.000 D3).
    backdrop: f32,
}
struct BoxGeometry {
    outer: Shape,
    content: Rect4,
    // Left and top padding + border, summed as the kernel's measure closure
    // sums them: flowed text is measured and painted around the same bits.
    inset: (f32, f32),
}
fn paint_rect(frame: exact_kernel::Frame, offset: (f32, f32)) -> Rect4 {
    (
        frame.x - offset.0,
        frame.y - offset.1,
        frame.width,
        frame.height,
    )
}
impl BoxPaint {
    fn capture(node: &NodeRef<'_>, kernel: &Kernel, dark: bool, w: f32) -> Self {
        let s = node.style;
        let widths = s.border_widths();
        let current = node
            .computed_style(StyleMask::of(StyleId::TextColor))
            .text_color;
        let colors = s.border_colors(current);
        let env = kernel.env();
        let pad = |d: Dimension| match d.resolve(&env) {
            Dimension::Points(p) => p,
            Dimension::Percent(p) => w * p / 100.0,
            Dimension::Calc(p, x) => w * p / 100.0 + x,
            Dimension::Auto | Dimension::Env(..) => 0.0,
        };
        // @ref LLP 1053.000 D4 — a material wins over `backdrop-filter`; a
        // name the table lacks draws ultra-thin ([`material_note`]).
        let material = node.props.str(PropId::BackgroundMaterial).map(|name| {
            exact_kernel::generated::material(name)
                .or_else(|| exact_kernel::generated::material("ultra-thin"))
                .expect("the schema declares ultra-thin")
        });
        Self {
            radii: [
                s.border_radius_top_left,
                s.border_radius_top_right,
                s.border_radius_bottom_right,
                s.border_radius_bottom_left,
            ]
            .map(|d| d.resolve(&env)),
            corners: Some(s.corner_shape).filter(|c| !c.is_round()),
            clip: s.background_clip,
            widths,
            colors: colors.map(|c| rgba(c.resolve(dark))),
            background: match material {
                // The material's tint where the author painted no background,
                // as the web's rule sits under an inline one (LLP 1053.000 D4).
                Some(m) if s.background_color.resolve(dark).a() == 0 => {
                    if dark {
                        m.dark
                    } else {
                        m.light
                    }
                }
                _ => rgba(s.background_color.resolve(dark)),
            },
            gradients: gradient::Captured::capture(s, dark),
            shadows: shadow::ShadowPaint::capture(s, dark),
            backdrop: material.map_or(s.backdrop_blur.max(0.0), |m| m.blur),
            padding: [
                pad(s.padding_top),
                pad(s.padding_right),
                pad(s.padding_bottom),
                pad(s.padding_left),
            ],
        }
    }
    fn geometry(&self, rect: Rect4) -> BoxGeometry {
        let (x, y, w, h) = rect;
        let widths = self.widths;
        let pad = self.padding;
        BoxGeometry {
            inset: (pad[3] + widths[3], pad[0] + widths[0]),
            outer: Shape::elliptical(
                rect,
                self.radii.map(|d| {
                    let resolve = |basis| match d {
                        Dimension::Points(x) => x,
                        Dimension::Percent(p) => basis * p / 100.0,
                        Dimension::Calc(p, x) => basis * p / 100.0 + x,
                        _ => 0.0,
                    };
                    (resolve(w).max(0.0), resolve(h).max(0.0))
                }),
            )
            .with_corners(self.corners),
            content: (
                x + widths[3] + pad[3],
                y + widths[0] + pad[0],
                (w - widths[3] - widths[1] - pad[3] - pad[1]).max(0.0),
                (h - widths[0] - widths[2] - pad[0] - pad[2]).max(0.0),
            ),
        }
    }
    fn paint(&self, backend: &mut dyn Backend, geometry: &BoxGeometry, ts: Transform) {
        for band in self.shadow_fills(geometry) {
            backend.fill_border(&band, ts);
        }
        // @ref LLP 1053.000 D2 — the backdrop blurs under the background.
        if self.backdrop > 0.0 {
            backend.backdrop_blur(&geometry.outer, self.backdrop, ts);
        }
        self.emit(geometry, |shape, color| backend.fill(&shape, color, ts));
        // The last layer first, so the first is on top (LLP 1077 D5), within
        // the background's clip (D6).
        if let Some(clip) = self.background_shape(geometry) {
            for g in self.gradients.iter().rev() {
                gradient::paint(g, &geometry.outer, &clip, self.widths, backend, ts);
            }
        }
        for band in self.inset_shadow_fills(geometry) {
            backend.fill_border(&band, ts);
        }
        for part in self.borders(geometry) {
            backend.fill_border(&part, ts);
        }
    }
    /// The background colour: within its `background-clip` (LLP 1077 D6).
    fn emit(&self, geometry: &BoxGeometry, mut emit: impl FnMut(Shape, [u8; 4])) {
        let Some(shape) = self.background_shape(geometry) else {
            return;
        };
        if self.background[3] > 0 && shape.rect.2 > 0.0 && shape.rect.3 > 0.0 {
            emit(shape, self.background);
        }
    }
    /// Where the background paints: the border box, the padding box or the
    /// content box, each with its radii less what it is inset by; `None` for
    /// `text`, which the paragraph paints (`text_clip`).
    fn background_shape(&self, geometry: &BoxGeometry) -> Option<Shape> {
        use exact_kernel::BackgroundClip;
        let outer = geometry.outer;
        let inset = match self.clip {
            BackgroundClip::BorderBox => return Some(outer),
            BackgroundClip::Text => return None,
            BackgroundClip::PaddingBox => self.widths,
            BackgroundClip::ContentBox => {
                let (w, p) = (self.widths, self.padding);
                [w[0] + p[0], w[1] + p[1], w[2] + p[2], w[3] + p[3]]
            }
        };
        let (x, y, w, h) = outer.rect;
        let [t, r, b, l] = inset;
        let less = |(a, c): (f32, f32), dx: f32, dy: f32| ((a - dx).max(0.0), (c - dy).max(0.0));
        Some(
            Shape::elliptical(
                (x + l, y + t, (w - l - r).max(0.0), (h - t - b).max(0.0)),
                [
                    less(outer.radii[0], l, t),
                    less(outer.radii[1], r, t),
                    less(outer.radii[2], r, b),
                    less(outer.radii[3], l, b),
                ],
            )
            .with_corners(outer.corners),
        )
    }
    /// The outer `box-shadow`s, under everything else (LLP 1064 D2), the
    /// list's first on top (LLP 1077 D4).
    fn shadow_fills(&self, geometry: &BoxGeometry) -> Vec<border::BorderFill> {
        self.shadows
            .iter()
            .rev()
            .filter(|s| !s.inset())
            .flat_map(|s| s.fills(&geometry.outer, self.widths))
            .collect()
    }
    /// The inset `box-shadow`s, over the background and under the border.
    fn inset_shadow_fills(&self, geometry: &BoxGeometry) -> Vec<border::BorderFill> {
        self.shadows
            .iter()
            .rev()
            .filter(|s| s.inset())
            .flat_map(|s| s.fills(&geometry.outer, self.widths))
            .collect()
    }
    /// The border, one fill per colour, joined as the web joins sides.
    fn borders(&self, geometry: &BoxGeometry) -> Vec<border::BorderFill> {
        border::border_fills(&geometry.outer, self.widths, self.colors)
    }
}
type ProjectiveHit = ([f32; 9], Rect4, Option<Rect4>, [[f32; 3]; 2]);

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
    projective: Option<ProjectiveHit>,
    affine: Option<(Transform, Rect4)>,
    press: f32,
}

impl PaintedBox {
    /// The surface's box through the same transform the painter used.
    /// Projective canvas placements have no affine surface observation.
    pub fn surface(&self, shown: Presented) -> Option<Rect4> {
        if self.projective.is_some() {
            return None;
        }
        self.affine.map(|(ts, rect)| bbox(ts, shown.surface(rect)))
    }

    /// Project the local center, which need not be the projected AABB center.
    pub fn center(&self) -> (f32, f32) {
        if let Some((inv, rect, _, _)) = self.projective {
            if let Some(h) = crate::placement::inverse(inv) {
                return crate::placement::map(&h, rect.0 + rect.2 / 2., rect.1 + rect.3 / 2.);
            }
        }
        (
            self.rect.0 + self.rect.2 / 2.,
            self.rect.1 + self.rect.3 / 2.,
        )
    }

    /// Whether a point (viewport points) is inside the box and its clip.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let inside = |r: Rect4| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
        let (local_x, local_y) = self
            .projective
            .map_or((x, y), |(inv, ..)| crate::placement::map(&inv, x, y));
        let affine_hit = self.affine.is_none_or(|(ts, rect)| {
            ts.invert().is_some_and(|inverse| {
                let mut point = tiny_skia::Point::from_xy(local_x, local_y);
                inverse.map_point(&mut point);
                point.x >= rect.0
                    && point.x < rect.0 + rect.2
                    && point.y >= rect.1
                    && point.y < rect.1 + rect.3
            })
        });
        affine_hit
            && inside(self.rect)
            && self.clip.is_none_or(inside)
            && self.projective.is_none_or(|(inv, rect, clip, planes)| {
                let (x, y) = crate::placement::map(&inv, x, y);
                let inside = |r: Rect4| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
                inside(rect) && clip.is_none_or(inside) && crate::placement::accepts(&planes, x, y)
            })
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
    /// Unbound checkboxes' own states, which the host keeps (LLP 1069.001 D4).
    pub controls: &'a BTreeMap<ViewId, bool>,
    /// A select's open menu, painted over everything (LLP 1069.001 D7).
    pub menu: Option<control::MenuPaint>,
    /// The pointer, in viewport points, when the host draws one.
    pub pointer: Option<(f32, f32)>,
}

/// One painted frame.
pub struct Frame {
    /// Immutable pixels, shared with repaint history; premultiplied RGBA at device scale.
    pub pixmap: Arc<Pixmap>,
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
    /// Seed a clipped repaint from an accepted frame; false means full repaint.
    fn damage(&mut self, _previous: &Pixmap, _rects: &[Rect4]) -> bool {
        false
    }
    /// Begin from an accepted frame, clipped; false leaves a white frame to repaint.
    fn begin_damage(
        &mut self,
        width: f32,
        height: f32,
        scale: f32,
        previous: &Pixmap,
        rects: &[Rect4],
    ) -> bool {
        self.begin(width, height, scale);
        self.damage(previous, rects)
    }
    /// Fill a shape.
    fn fill(&mut self, shape: &Shape, color: [u8; 4], ts: Transform);
    /// Fill a shape with a gradient placed in its coordinates (LLP 1066).
    fn fill_gradient(&mut self, shape: &Shape, gradient: &gradient::GradientPaint, ts: Transform);
    /// CSS `backdrop-filter: blur(σ)` (LLP 1053.000 D2): what is painted
    /// under `shape` so far, blurred (σ in points, mirrored edges) and put
    /// back inside it, under the current clip.
    fn backdrop_blur(&mut self, _shape: &Shape, _sigma: f32, _ts: Transform) {}
    /// Fill one colour's share of a border (LLP 1053 G2): its region even-odd, clipped (non-zero).
    fn fill_border(&mut self, part: &border::BorderFill, ts: Transform);
    /// Draw a picture scaled into `dst`, clipped to every shape in `clips`.
    /// An explicit tint replaces its RGB through the picture's alpha alone.
    fn image(
        &mut self,
        image: &Arc<Bitmap>,
        dst: Rect4,
        clips: &[Shape],
        ts: Transform,
        tint: Option<[u8; 4]>,
    );
    /// An SVG island's pixels over `dst` in `ts`'s space, blended by `mode`.
    fn island_image(&mut self, _pixels: Arc<Pixmap>, _dst: Rect4, _ts: Transform, _mode: u8) {}
    /// Composite a rendered canvas child (not a decoded image asset).
    fn surface_image(&mut self, _pixels: Arc<Pixmap>, _dst: Rect4) {}
    /// Composite a 2D canvas's bitmap into `dst`, clipped (LLP 1056 D7).
    fn canvas(&mut self, _pixels: &Arc<Pixmap>, _dst: Rect4, _clips: &[Shape], _ts: Transform) {}
    /// Paint a paragraph with its top-left at `origin`.
    fn text(
        &mut self,
        text: &mut TextEngine,
        paragraph: &Paragraph,
        palette: &[RunPaint],
        origin: (f32, f32),
        ts: Transform,
    );
    /// Paint one SVG shape in `ts`'s space (LLP 1055 D4).
    fn svg_path(&mut self, _shape: &SvgPaint<'_>, _ts: Transform) {}
    /// Clip to an SVG `clipPath` in `ts`'s space until as many pops as this
    /// returns (LLP 1055.000 D10): the union of its shapes, intersected with
    /// its own clip and with whatever clip is in force.
    fn push_svg_clip(&mut self, _clip: &exact_kernel::svg::scene::Clip, _ts: Transform) -> usize {
        0
    }
    /// Clip everything until the matching pop to a shape.
    fn push_clip(&mut self, shape: &Shape, ts: Transform);
    /// Push the kernel's validated CSS path in border-box coordinates.
    /// Returns whether a clip was pushed (recording/custom backends may opt out).
    fn push_css_clip(&mut self, _path: &exact_kernel::clip::ClipPath, _ts: Transform) -> bool {
        false
    }
    /// End a clip.
    fn pop_clip(&mut self);
    /// Composite everything until the matching pop at an opacity.
    fn push_opacity(&mut self, alpha: f32);
    /// CSS `mask-image` (LLP 1077 D2): what paints until [`Backend::pop_mask`]
    /// is one group, clipped to `shape`, the border box.
    fn push_mask(&mut self, _shape: &Shape, _ts: Transform) {
        self.push_opacity(1.0);
    }
    /// The group, kept where `mask` (a gradient, or one colour) is opaque
    /// over `shape`, the border box, and nowhere outside it.
    fn pop_mask(&mut self, _shape: &Shape, _mask: &Result<GradientPaint, [u8; 4]>, _ts: Transform) {
        self.pop_opacity();
    }
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

/// Where a fully transparent subtree draws: nowhere.
struct Unpainted;
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
    pub(crate) placements: BTreeMap<ViewId, crate::placement::Placement>,
    /// Each 2D canvas's latest bitmap (LLP 1056).
    pub(crate) canvases: BTreeMap<ViewId, crate::canvas2d::CanvasPaint>,
    viewport: (f32, f32),
    cpu_ms: Option<f64>,
    // One generational source, lifted only inside its existing List clip.
    pub(crate) arrange_lift: Option<(exact_kernel::NodeKey, exact_kernel::NodeKey)>,
    // One lease per actually accepted owner, not one global width per string.
    // Retained while a subsequent backend frame fails.
    accepted_text: BTreeMap<exact_kernel::NodeKey, Rc<Paragraph>>,
    region_picture: Option<Rc<region::Picture>>,
    region_frame: Option<region::Published>,
    damage: damage::Retained,
    /// `backgroundMaterial` names the schema lacks, and those not yet logged.
    materials: (std::collections::BTreeSet<String>, Vec<String>),
    /// The node a 3D island paints flat, its own transform being the warp's
    /// (LLP 1077 D8).
    pub(crate) flatten: Option<ViewId>,
}

// O(painted owners) references and numeric publication metadata, not copied
// glyphs/commands or a new render graph. The display retains acknowledged A
// here while one submitted B owns its corresponding leases.
#[cfg(any(target_os = "linux", target_os = "android", test))]
pub(crate) struct Presentation {
    text: BTreeMap<exact_kernel::NodeKey, Rc<Paragraph>>,
    picture: Option<Rc<region::Picture>>,
    frame: Option<region::Published>,
}

struct Walk<'a, 'b> {
    scene: &'b Scene<'a>,
    boxes: Vec<PaintedBox>,
    text: BTreeMap<exact_kernel::NodeKey, Rc<Paragraph>>,
    skip: Option<exact_kernel::NodeKey>,
    replay: Option<&'b region::Replay<'b>>,
}

impl Painter {
    /// Whether `node` has a material (which blurs its backdrop); notes a name
    /// the schema lacks, once, for the host's log (LLP 1053.000 D4).
    fn material_note(&mut self, node: &NodeRef<'_>) -> bool {
        let Some(name) = node.props.str(PropId::BackgroundMaterial) else {
            return false;
        };
        if exact_kernel::generated::material(name).is_none() && self.materials.0.insert(name.into())
        {
            self.materials.1.push(format!(
                "backgroundMaterial `{name}` is not a material; drawing ultra-thin"
            ));
        }
        true
    }

    /// The lines [`Painter::material_note`] noted since the last call.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.materials.1)
    }

    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(crate) fn presentation(&self) -> Presentation {
        Presentation {
            text: self.accepted_text.clone(),
            picture: self.region_picture.clone(),
            frame: self.region_frame.as_ref().map(|f| region::Published {
                incarnation: f.incarnation.clone(),
                selection: f.selection.clone(),
            }),
        }
    }
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(crate) fn replace_presentation(&mut self, mut state: Presentation) -> Presentation {
        std::mem::swap(&mut self.accepted_text, &mut state.text);
        std::mem::swap(&mut self.region_picture, &mut state.picture);
        std::mem::swap(&mut self.region_frame, &mut state.frame);
        state
    }
    /// A painter over a backend.
    pub fn new(text: Shared, scale: f32, backend: Box<dyn Backend>) -> Painter {
        Painter {
            text,
            scale,
            dark: false,
            backend,
            accepted_text: BTreeMap::new(),
            arrange_lift: None,
            region_picture: None,
            region_frame: None,
            damage: Default::default(),
            materials: Default::default(),
            placements: BTreeMap::new(),
            canvases: BTreeMap::new(),
            viewport: (0., 0.),
            cpu_ms: None,
            flatten: None,
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

    /// The last frame's (paint, readback) milliseconds; CPU readback is zero.
    pub fn last_frame_ms(&self) -> Option<(f64, f64)> {
        self.backend
            .last_frame_ms()
            .or(self.cpu_ms.map(|ms| (ms, 0.)))
    }

    /// Accepted leaves that fell back after an incomplete bounded walk.
    pub fn flow_failures(&self) -> impl Iterator<Item = exact_kernel::NodeKey> + '_ {
        self.accepted_text
            .iter()
            .filter_map(|(key, p)| p.flow_incomplete().then_some(*key))
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
    pub(crate) fn paint_region(
        &mut self,
        scene: &Scene<'_>,
        viewport: (f32, f32),
        region: &crate::content_region::ContentRegionState,
        collection_limits: &BTreeMap<ViewId, f32>,
        actions: &mut RegionActions<'_>,
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
                    // graph. Only bounded action scalars are copied; paragraph
                    // UTF-8 and cold text lookup remain outside this path.
                    region::Picture::capture(
                        self,
                        scene,
                        region,
                        publication,
                        collection_limits,
                        actions,
                    )?
                } else {
                    self.region_picture
                        .as_ref()
                        .filter(|p| p.belongs_to(region))
                        .cloned()
                        .ok_or("retained content has no matching native picture")?
                };
                // Validate source, project once, and preflight every prepared
                // text query before backend.begin or any shell/content emission.
                let replay = region::Replay::prepare(
                    &picture,
                    scene,
                    receipt.origin,
                    region.binding().content,
                    viewport,
                    self.scale,
                    actions,
                )?;
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
        let started = std::time::Instant::now();
        self.viewport = viewport;
        let partial = if let Some(previous) = self
            .damage
            .pixels
            .as_ref()
            .filter(|_| !self.damage.next.is_empty())
        {
            self.backend.begin_damage(
                viewport.0,
                viewport.1,
                self.scale,
                previous,
                &self.damage.next,
            )
        } else {
            self.backend.begin(viewport.0, viewport.1, self.scale);
            false
        };
        if partial {
            self.damage.last = self.damage.next.clone();
        } else {
            self.damage.last.clear();
        }
        self.damage.next.clear();
        self.damage.unsupported = false;
        let mut walk = Walk {
            scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip,
            replay,
        };
        for root in scene.roots {
            self.node(&mut walk, *root, Transform::identity(), scene.page, None);
        }
        if let Some(menu) = &scene.menu {
            self.menu(menu);
        }
        if let Some((px, py)) = scene.pointer {
            self.backend.pointer(px, py);
        }
        let finished = self.backend.finish();
        if self.backend.name() == "cpu" {
            self.cpu_ms = Some(started.elapsed().as_secs_f64() * 1000.);
        }
        // Publication is the ownership boundary. On Err the previous accepted
        // set remains intact; candidate leases simply unwind with `walk`.
        if finished.is_ok() {
            self.accepted_text = walk.text;
        } else {
            drop(walk.text);
        }
        // Pending measurements end with every paint attempt, including failure.
        self.text.borrow_mut().finish_text_frame();
        let pixmap = Arc::new(finished?);
        self.damage.pixels = self
            .accepted_text
            .values()
            .any(|p| !p.fragments().is_empty())
            .then(|| pixmap.clone());
        self.damage.dark = self.dark;
        self.damage.caret = scene.focus.is_some_and(|id| {
            scene
                .kernel
                .node(id)
                .is_some_and(|n| n.node_type == NodeType::TextInput)
        });
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
        if self.placed(walk, id, ts, offset, clip_rect) {
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
        // An SVG element is its `svg`'s content, painted there (LLP 1055 D4).
        if walk.skip == Some(node.key) || node.node_type.is_svg_element() {
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
        let (x, y, w, h) = paint_rect(f, offset);
        let p = (walk.scene.presented)(id);
        // @ref LLP 1077 D8 — turned or moved in space: painted apart, warped.
        if self.spatial(walk, &node, &p, (x, y, w, h), ts, offset, clip_rect) {
            self.damage.unsupported = true;
            return;
        }
        // @ref LLP 1043.000 §3 D7 — collect damage eligibility during the
        // existing paint walk, not an extra whole-document walk per flow tick.
        // A shadow paints outside the node's box, where damage never looks.
        self.damage.unsupported |= p.moves()
            || p.dark.is_some_and(|dark| dark != self.dark)
            || p.opacity != 1.0
            || node.node_type == NodeType::Image
            || !node.style.box_shadow.0.is_empty()
            // A backdrop reads what is under it, beyond any damage.
            || node.style.backdrop_blur > 0.0
            || self.material_note(&node)
            || !p.colors.is_empty();
        let ts = if p.moves() && self.flatten != Some(id) {
            // About `transform-origin`, the centre unless authored (LLP 1061 D6).
            ts.pre_concat(p.transform((x, y, w, h), node.style.transform_origin.resolve(w, h)))
        } else {
            ts
        };
        let scrolls = {
            let (ox, oy) = effective_overflow(&node);
            ox != Overflow::Visible || oy != Overflow::Visible
        };
        walk.boxes.push(PaintedBox {
            id,
            projective: None,
            affine: Some((ts, (x, y, w, h))),
            press: p.press,
            rect: bbox(ts, (x, y, w, h)),
            clip: clip_rect,
            scroll: scrolls.then(|| walk.scene.scroll.get(&id).copied().unwrap_or((0.0, 0.0))),
        });
        let opacity = p.opacity.clamp(0.0, 1.0);
        // CSS opacity is paint only: a transparent subtree is still hit (the
        // walk records its boxes) and draws through a backend that draws nothing.
        let drawn =
            (opacity <= 0.0).then(|| std::mem::replace(&mut self.backend, Box::new(Unpainted)));
        if drawn.is_none() && opacity < 1.0 {
            self.backend.push_opacity(opacity);
        }
        // @ref LLP 1077 D2 — the mask is the border box's gradient's alpha.
        let mask = gradient::Captured::mask(node.style, self.dark).map(|m| m.place((x, y, w, h)));
        if mask.is_some() {
            self.backend.push_mask(&Shape::rect((x, y, w, h)), ts);
        }
        // @ref LLP 1043.000 §3 D7 — polygon demo ink and exclusion share an outline.
        let path_clip = !node.style.clip_path.commands().is_empty()
            && self
                .backend
                .push_css_clip(&node.style.clip_path, ts.pre_translate(x, y));
        let previous = self.dark;
        self.dark = p.dark.unwrap_or(previous);
        self.content(walk, &node, (x, y, w, h), ts, offset, clip_rect);
        self.dark = previous;
        if path_clip {
            self.backend.pop_clip();
        }
        if let Some(mask) = &mask {
            self.backend.pop_mask(&Shape::rect((x, y, w, h)), mask, ts);
        }
        match drawn {
            Some(backend) => self.backend = backend,
            None if opacity < 1.0 => self.backend.pop_opacity(),
            None => {}
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
        let shown = (walk.scene.presented)(node.id);
        // Paint motion's values over the captured box (LLP 1055.000 D6,
        // LLP 1062 D5): background, border sides and shadow.
        let paint =
            BoxPaint::capture(node, walk.scene.kernel, self.dark, rect.2).presented(&shown.colors);
        let geometry = paint.geometry(rect);
        // @ref LLP 1063 — a layout transition's size is the surface's alone.
        let surface = paint.geometry(shown.surface(rect));
        paint.paint(self.backend.as_mut(), &surface, ts);
        let outer = geometry.outer;
        let content = geometry.content;
        let s = node.style;
        match node.node_type {
            // @ref LLP 1056 D7 — a 2D canvas's kept bitmap fills its content box.
            NodeType::Canvas => {
                if let Some(c) = self.canvases.get(&node.id) {
                    let clips = [Shape::rect(content), outer];
                    self.backend.canvas(&c.pixels, content, &clips, ts);
                }
            }
            NodeType::Image => {
                if let Some(img) = walk.scene.images.get(&node.id) {
                    if let Some(dst) = object_fit(img.natural(), s.object_fit, content) {
                        self.backend.image(
                            img,
                            dst,
                            &[Shape::rect(content), outer],
                            ts,
                            image_tint(s, &shown, self.dark),
                        );
                    }
                }
            }
            NodeType::Text => {
                // The kernel measures a Text subtree as one paragraph. Inline
                // descendants deliberately have zero frames, not paint boxes.
                let build = || {
                    let mut spec = text_spec(&node.computed_style(StyleMask::INHERITED), "");
                    spec.runs = node
                        .text_runs()
                        .iter()
                        .map(|run| Run::from_style(&run.text, run.style))
                        .collect();
                    spec.collapse_white_space()
                };
                let paragraph = if let Some(stamp) = node.paragraph_stamp() {
                    // @ref LLP 1043.000 §3 D7 — ordinary text takes the same
                    // retained identity path; flow only adds exclusion geometry.
                    self.text.borrow_mut().flow_identified(
                        &stamp,
                        content.2,
                        &node
                            .flow_shapes()
                            .iter()
                            .map(|s| s.translate(-geometry.inset.0, -geometry.inset.1))
                            .collect::<Vec<_>>(),
                        self.accepted_text.get(&node.key),
                        build,
                    )
                } else {
                    let spec = build();
                    (!spec.is_empty())
                        .then(|| self.text.borrow_mut().paragraph(&spec, Some(content.2)))
                };
                if let Some(paragraph) = paragraph {
                    let mut palette = Vec::new();
                    text_palette(walk.scene.kernel, node, self.dark, &mut palette);
                    presented_text_colors(walk, node, &mut palette);
                    walk.text.insert(node.key, paragraph.clone());
                    // CSS `text-overflow: ellipsis` in a clipping box: an
                    // over-wide line ends in "…" (LLP 1053 G5; paint only).
                    let shown = (s.text_overflow == exact_kernel::TextOverflow::Ellipsis
                        && effective_overflow(node).0 != Overflow::Visible)
                        .then(|| paragraph.ellipsized(content.2))
                        .flatten()
                        .unwrap_or_else(|| paragraph.clone());
                    // Inline backgrounds, under the glyphs, per line fragment.
                    let mut backgrounds = Vec::new();
                    text_backgrounds(walk.scene.kernel, node, None, self.dark, &mut backgrounds);
                    for (r, color) in shown.run_backgrounds(&backgrounds) {
                        self.backend.fill(
                            &Shape::rect((content.0 + r.0, content.1 + r.1, r.2, r.3)),
                            color,
                            ts,
                        );
                    }
                    self.text_shadow(node, &shown, &palette, (content.0, content.1), ts);
                    let kernel = walk.scene.kernel;
                    self.text_clip(
                        node,
                        kernel,
                        &shown,
                        &palette,
                        (content.0, content.1),
                        rect,
                        ts,
                    );
                    if !self.text_stroke(node, &shown, &palette, (content.0, content.1), ts) {
                        let mut engine = self.text.borrow_mut();
                        self.backend.text(
                            &mut engine,
                            &shown,
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
                // A password is masked, one bullet a character, as the web and
                // Apple's secure fields draw it.
                let masked;
                let shown = if placeholder {
                    node.props.str(PropId::Placeholder).unwrap_or("")
                } else if node.props.str(PropId::Type) == Some("password") {
                    masked = "\u{2022}".repeat(value.chars().count());
                    &masked
                } else {
                    value
                };
                let mut computed = node.computed_style(StyleMask::INHERITED);
                // A field's value is never collapsed, as the kernel measures it.
                if !computed.white_space.model().preserves() {
                    computed.white_space = exact_kernel::WhiteSpace::PreWrap;
                }
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
                    presented_color(walk, node)
                        .unwrap_or_else(|| rgba(node.text_color().resolve(self.dark)))
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
            NodeType::Svg => self.svg(walk, node, rect, content, ts),
            NodeType::Control if node.props.str(PropId::Type) == Some("select") => {
                let label = walk.scene.kernel.select_chosen(node.id).map(|c| c.label);
                self.field_control(node, content, ts, label.as_deref().unwrap_or(""), true);
            }
            // @ref LLP 1069.001 D7 — a date control is a field showing its
            // ISO value, HTML's placeholder form when empty; `type` sets it.
            NodeType::Control
                if matches!(
                    node.props.str(PropId::Type),
                    Some("date" | "time" | "datetime-local")
                ) =>
            {
                let value = node.props.str(PropId::Value).unwrap_or("");
                let shown = match (value, node.props.str(PropId::Type)) {
                    ("", Some("date")) => "yyyy-mm-dd",
                    ("", Some("time")) => "--:--",
                    ("", _) => "yyyy-mm-ddT--:--",
                    (v, _) => v,
                };
                self.field_control(node, content, ts, shown, false);
            }
            NodeType::Control if node.props.str(PropId::Type) == Some("range") => {
                self.range_control(node, content, ts)
            }
            NodeType::Control if node.props.str(PropId::Type) == Some("button") => {
                let title = walk.scene.kernel.press_face(node.id).and_then(|f| f.title);
                self.button_control(node, content, ts, title.as_deref().unwrap_or(""));
            }
            NodeType::Control => control::paint(
                self.backend.as_mut(),
                node,
                content,
                ts,
                self.dark,
                walk.scene.controls.get(&node.id).copied(),
            ),
            _ => {}
        }
        // Children: clipped by this box when its overflow is not visible,
        // moved by its scroll offset when it scrolls.
        let (ox, oy) = effective_overflow(node);
        let clips = ox != Overflow::Visible || oy != Overflow::Visible;
        let mut child_rect = clip_rect;
        if clips {
            self.backend.push_clip(&surface.outer, ts);
            let own = bbox(ts, surface.outer.rect);
            child_rect = Some(match clip_rect {
                Some(c) => intersect(c, own),
                None => own,
            });
        }
        let scrolls = ox != Overflow::Visible || oy != Overflow::Visible;
        let child_offset = if scrolls {
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
        let lift = self
            .arrange_lift
            .filter(|(list, _)| *list == node.key)
            .and_then(|(_, key)| walk.scene.kernel.node_by_key(key))
            .filter(|source| source.parent == Some(node.id))
            .map(|source| source.id);
        // A native button's children are its face, painted above (LLP 1069.011 D5).
        let native =
            node.node_type == NodeType::Control && node.props.str(PropId::Type) == Some("button");
        let mut children: Vec<_> = node
            .children()
            .into_iter()
            .filter(|id| Some(*id) != lift && !native)
            .collect();
        children.sort_by(
            |a, b| match (self.placements.get(a), self.placements.get(b)) {
                (Some(a), Some(b)) => a.depth().total_cmp(&b.depth()),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            },
        );
        for child in children {
            self.node(walk, child, ts, child_offset, child_rect);
        }
        if let Some(child) = lift {
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
/// Its white space is collapsed per CSS unless the row preserves it; the
/// caller that replaces the runs collapses again ([`Spec::collapse_white_space`]).
pub fn text_spec(s: &StyleProps, text: &str) -> Spec {
    Spec {
        strut: Run::from_style("", exact_kernel::TextStyle::from_style(s)),
        runs: vec![Run::from_style(
            text,
            exact_kernel::TextStyle::from_style(s),
        )],
        align: s.text_align.physical(s.direction),
        line_clamp: s.line_clamp,
        overflow_wrap: s.overflow_wrap,
        white_space: s.white_space,
        direction: s.direction,
    }
    .collapse_white_space()
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
        Dimension::Calc(p, x) => against * p / 100.0 + x,
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
mod paragraph_tests;
