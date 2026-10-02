//! CSS's 3D transforms (LLP 1076 D8): a box turned out of the screen's plane
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
                let (pox, poy) = parent.style.perspective_origin.resolve(pw, ph);
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
        // The plane z = 0 of the box, from its island (0..w, 0..h).
        let m = mul(&m, &translate(x, y, 0.0));
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
        painter.viewport = (w, h);
        painter.backend.begin(w, h, self.scale);
        let mut child_walk = Walk {
            scene: walk.scene,
            boxes: Vec::new(),
            text: BTreeMap::new(),
            skip: None,
            replay: None,
        };
        let f = node.frame;
        painter.node(
            &mut child_walk,
            node.id,
            Transform::identity(),
            (f.x, f.y),
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
            b.projective = Some((inv, b.rect, b.clip, planes));
            b.rect = crate::placement::clipped_bounds(&device, planes, b.rect);
            b.clip = clip;
            walk.boxes.push(b);
        }
        true
    }
}
