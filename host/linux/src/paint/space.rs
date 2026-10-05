//! CSS's 3D transforms (LLP 1077 D8): a box turned out of the screen's plane
//! or moved along z, under its parent's `perspective`, painted flat apart and
//! warped into the frame through the plane's homography — the placement
//! route canvas children take (`placed.rs`, `placement.rs`), so paint and hit
//! testing share one map. Its own children flatten into its plane, CSS's
//! initial `transform-style: flat`.
use super::*;

/// A 4×4 matrix, row-major, acting on column vectors.
type M4 = [[f32; 4]; 4];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut out = [[0.0; 4]; 4];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}
fn translate(x: f32, y: f32, z: f32) -> M4 {
    [
        [1., 0., 0., x],
        [0., 1., 0., y],
        [0., 0., 1., z],
        [0., 0., 0., 1.],
    ]
}
fn scale(s: f32) -> M4 {
    [
        [s, 0., 0., 0.],
        [0., s, 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}
/// CSS `rotate3d()` (Transforms 2 §13): a turn of `deg` about the axis.
fn rotate(axis: [f32; 3], deg: f32) -> M4 {
    let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let [x, y, z] = axis.map(|v| v / n);
    let (s, c) = deg.to_radians().sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y, 0.],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x, 0.],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c, 0.],
        [0., 0., 0., 1.],
    ]
}

/// `a` after `b`, two 3×3 maps row-major: what a point goes through when it
/// is mapped by `b` and then by `a`.
fn mul3(a: &[f32; 9], b: &[f32; 9]) -> [f32; 9] {
    let mut out = [0.0; 9];
    for i in 0..3 {
        for j in 0..3 {
            out[i * 3 + j] = (0..3).map(|k| a[i * 3 + k] * b[k * 3 + j]).sum();
        }
    }
    out
}

/// How far a box's outer shadows paint past it: left, top, right, bottom.
fn shadow_reach(node: &exact_kernel::NodeRef<'_>) -> (f32, f32, f32, f32) {
    let mut out = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for s in node.style.box_shadow.shadows().iter().filter(|s| !s.inset) {
        let r = 1.5 * s.blur + s.spread.max(0.0) + 1.0;
        out.0 = out.0.max(r - s.offset.x);
        out.1 = out.1.max(r - s.offset.y);
        out.2 = out.2.max(r + s.offset.x);
        out.3 = out.3.max(r + s.offset.y);
    }
    out
}

/// How far past its border box `node` paints, left, top, right, bottom: its
/// outer shadows' reach whole and, unless it clips, its descendants' painted
/// boxes — each moved by its presented translate, grown to its diagonal
/// when turned, scaled or in a plane of its own, and by its own shadows —
/// bounded by `cap`.
fn reach(
    walk: &Walk<'_, '_>,
    node: &exact_kernel::NodeRef<'_>,
    (x, y, w, h): Rect4,
    offset: (f32, f32),
    cap: f32,
) -> (f32, f32, f32, f32) {
    let mut out = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (ox, oy) = effective_overflow(node);
    if ox == Overflow::Visible && oy == Overflow::Visible {
        let mut stack = node.children();
        while let Some(id) = stack.pop() {
            let Some(child) = walk.scene.kernel.node(id) else {
                continue;
            };
            let q = (walk.scene.presented)(id);
            let (cx, cy, cw, ch) = paint_rect(child.frame, offset);
            let (mx, my) = (
                cx + cw / 2.0 + q.translate.0 + q.layout[0],
                cy + ch / 2.0 + q.translate.1 + q.layout[1],
            );
            let k = (q.scale * q.press).abs().max(1.0);
            let turned = q.rotate % 360.0 != 0.0
                || child.style.rotate_axis.is_3d()
                || child.style.translate_z != 0.0;
            let (hw, hh) = if turned {
                let d = k * cw.hypot(ch);
                (d, d)
            } else {
                (k * cw / 2.0, k * ch / 2.0)
            };
            let sh = shadow_reach(&child);
            out.0 = out.0.max(x - (mx - hw - sh.0));
            out.1 = out.1.max(y - (my - hh - sh.1));
            out.2 = out.2.max(mx + hw + sh.2 - (x + w));
            out.3 = out.3.max(my + hh + sh.3 - (y + h));
            let (cox, coy) = effective_overflow(&child);
            if cox == Overflow::Visible && coy == Overflow::Visible {
                stack.extend(child.children());
            }
        }
    }
    // Bounded: a runaway descendant must not ask for an unbounded island;
    // the box's own shadow is never cut.
    let own = shadow_reach(node);
    (
        out.0.min(cap).max(own.0),
        out.1.min(cap).max(own.1),
        out.2.min(cap).max(own.2),
        out.3.min(cap).max(own.3),
    )
}

/// The bounds of `r`'s corners mapped through `h`; `None` when a corner is
/// behind the viewer.
fn mapped_bounds(h: &[f32; 9], r: Rect4) -> Option<Rect4> {
    let mut b = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for (px, py) in [
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0, r.1 + r.3),
        (r.0 + r.2, r.1 + r.3),
    ] {
        if h[6] * px + h[7] * py + h[8] <= 0.0 {
            return None;
        }
        let (mx, my) = crate::placement::map(h, px, py);
        b = (b.0.min(mx), b.1.min(my), b.2.max(mx), b.3.max(my));
    }
    Some((b.0, b.1, b.2 - b.0, b.3 - b.1))
}

impl Painter {
    /// Paint `node` through its plane's homography when it is turned or
    /// moved in space; `false` when it is not, and the caller paints it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spatial(
        &mut self,
        walk: &mut Walk<'_, '_>,
        node: &exact_kernel::NodeRef<'_>,
        p: &Presented,
        (x, y, w, h): Rect4,
        ts: Transform,
        offset: (f32, f32),
        clip: Option<Rect4>,
    ) -> bool {
        let s = node.style;
        let axis = s.rotate_axis.0;
        if self.flatten == Some(node.id) || !(s.rotate_axis.is_3d() || s.translate_z != 0.0) {
            return false;
        }
        if w <= 0.0 || h <= 0.0 {
            return true;
        }
        // The box's own transform about `transform-origin`, as CSS orders
        // the individual properties: translate, rotate, scale.
        let (ox, oy) = s.transform_origin.resolve(w, h);
        let (cx, cy) = (x + ox, y + oy);
        let [dx, dy, ..] = p.layout;
        let mut m = translate(
            cx + dx + p.translate.0,
            cy + dy + p.translate.1,
            s.translate_z,
        );
        m = mul(&m, &rotate(axis, p.rotate));
        m = mul(&m, &scale(p.scale * p.press));
        m = mul(&m, &translate(-cx, -cy, 0.0));
        // Its parent's `perspective`, about the parent's `perspective-origin`.
        if let Some(parent) = node.parent.and_then(|id| walk.scene.kernel.node(id)) {
            let d = parent.style.perspective;
            if d > 0.0 {
                let (px, py, pw, ph) = paint_rect(parent.frame, offset);
                let (pox, poy) = parent.style.rare.perspective_origin.resolve(pw, ph);
                let (ax, ay) = (px + pox, py + poy);
                let mut persp = translate(ax, ay, 0.0);
                persp = mul(
                    &persp,
                    &[
                        [1., 0., 0., 0.],
                        [0., 1., 0., 0.],
                        [0., 0., 1., 0.],
                        [0., 0., -1.0 / d, 1.],
                    ],
                );
                persp = mul(&persp, &translate(-ax, -ay, 0.0));
                m = mul(&persp, &m);
            }
        }
        // The island covers what the box paints past itself: its outer
        // shadows and any overflowing descendant (nothing past a clip).
        let cap = (2.0 * w.max(h).max(64.0)).max(self.viewport.0.max(self.viewport.1));
        let (left, top, right, bottom) = reach(walk, node, (x, y, w, h), offset, cap);
        let (iw, ih) = (w + left + right, h + top + bottom);
        // The plane z = 0 of the box, from its island (-left..w+right, …).
        let m = mul(&m, &translate(x - left, y - top, 0.0));
        let plane = [
            m[0][0], m[0][1], m[0][3], m[1][0], m[1][1], m[1][3], m[3][0], m[3][1], m[3][3],
        ];
        // Turned away from the viewer: `backface-visibility: hidden` paints
        // nothing (the plane's normal is its z column after the turn).
        let facing = m[2][2] * if m[3][3] < 0.0 { -1.0 } else { 1.0 };
        let turned = (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * m[3][3] < 0.0 || facing < 0.0;
        if turned && s.backface_visibility == exact_kernel::BackfaceVisibility::Hidden {
            return true;
        }
        let device = crate::placement::compose(plane, ts, 0.0, 0.0);
        let Some(inv) = crate::placement::inverse(device) else {
            return true;
        };
        let mut painter = Painter::new(
            self.text.clone(),
            self.scale,
            Box::new(crate::raster::Raster::transparent()),
        );
        painter.dark = self.dark;
        painter.placements = self.placements.clone();
        painter.canvases = self.canvases.clone();
        painter.flatten = Some(node.id);
        painter.viewport = (iw, ih);
        painter.backend.begin(iw, ih, self.scale);
        let mut child_walk = Walk {
            scene: walk.scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip: None,
            replay: None,
            ranks: Rc::clone(&walk.ranks),
        };
        let f = node.frame;
        painter.node(
            &mut child_walk,
            node.id,
            Transform::identity(),
            (f.x - left, f.y - top),
            None,
        );
        walk.text.extend(child_walk.text);
        let planes = [[0., 0., 1.]; 2];
        if let Ok(source) = painter.backend.finish() {
            if let Some((pixels, rect)) =
                crate::placement::warp_soft(&source, device, planes, self.scale, self.viewport)
            {
                self.backend.surface_image(Arc::new(pixels), rect);
            }
        }
        for mut b in child_walk.boxes {
            // A box already in its own plane (a 3D child) maps through both:
            // this island's inverse, then its own (LLP 1077 D8).
            // A clip between the two planes (an overflow inside this
            // island) goes into the inner plane too, as its corners' bounds.
            let (local, local_clip, inv, planes) = match b.projective {
                Some((inner, r, c, p)) => {
                    let mid = b.clip.and_then(|m| mapped_bounds(&inner, m));
                    let c = match (c, mid) {
                        (Some(c), Some(m)) => Some(intersect(c, m)),
                        (c, m) => c.or(m),
                    };
                    (r, c, mul3(&inner, &inv), p)
                }
                None => (b.rect, b.clip, inv, planes),
            };
            if let Some(map) = crate::placement::inverse(inv) {
                b.rect = crate::placement::clipped_bounds(&map, planes, local);
            }
            b.projective = Some((inv, local, local_clip, planes));
            b.clip = clip;
            walk.boxes.push(b);
        }
        true
    }
}
