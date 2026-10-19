//! What a look frame drew outside the cull's account, as a world-space box
//! (LLP 1127.000 D3): what an orbit or plan look frame frames besides the
//! cull's spheres. Each item is placed where the frame drew it: its two ticks'
//! rows at the frame's alpha, or its attachment matrix (`FramePoses`, as
//! `transform.wgsl` and `model_vertex.wgsl` place it); and only what the
//! renderer drew is counted: the slots of its draw groups and blended models,
//! and the sprites and emitters its quads derived, at the matrix they used.
//! Primitives by their dimensions, model records by their meshes' boxes (or
//! their skin palette's), sprites by their quad about the anchor, emitters by
//! the reach their particles' own cull uses (`quads::emitter_reach`).
use crate::quads::QuadFact;
use crate::renderer::probe::{Facts, Reading};
use exact_game::{Emitter, Entity, Mesh, Sprite, Vec3, World};
use glam::Mat4;

/// A box, `lo` to `hi`.
pub(crate) type Aabb = (Vec3, Vec3);

/// Where the frame drew each transform slot.
pub(crate) struct FramePoses<'a> {
    pub alpha: f32,
    /// The previous and current tick's rows, ten floats a slot.
    pub prev: &'a [f32],
    pub curr: &'a [f32],
    /// Each attached slot's matrix, sixteen floats a slot (`attachment_words`).
    pub attachments: &'a [f32],
}

impl FramePoses<'_> {
    /// Slot `slot`'s matrix this frame: its attachment's, else its rows
    /// interpolated as the shaders do (position and scale linearly, rotation by
    /// the shortest normalized lerp); a primitive's scale is absolute.
    pub fn matrix(&self, slot: u32, primitive: bool) -> Option<Mat4> {
        let at = slot as usize * 16;
        if let Some(m) = self.attachments.get(at..at + 16) {
            if m[15] == 1. {
                return Some(Mat4::from_cols_slice(m));
            }
        }
        let at = slot as usize * 10;
        let (p, c) = (self.prev.get(at..at + 10)?, self.curr.get(at..at + 10)?);
        let v3 = |r: &[f32], i: usize| Vec3::new(r[i], r[i + 1], r[i + 2]);
        let q = |r: &[f32]| glam::Vec4::new(r[3], r[4], r[5], r[6]);
        let t = self.alpha.clamp(0., 1.);
        let (q0, q1) = (q(p), q(c));
        let q1 = if q0.dot(q1) < 0. { -q1 } else { q1 };
        let rotation = glam::Quat::from_vec4(q0.lerp(q1, t).try_normalize()?);
        let scale = |r: &[f32]| {
            let s = v3(r, 7);
            if primitive {
                s.abs()
            } else {
                s
            }
        };
        let m = Mat4::from_scale_rotation_translation(
            scale(p).lerp(scale(c), t),
            rotation,
            v3(p, 0).lerp(v3(c, 0), t),
        );
        m.is_finite().then_some(m)
    }
}

/// The box around what the frame drew outside the cull's account: blended
/// model records, sprites and emitters, and when `all` (a direct frame, with
/// no account) every draw group's slots too. `records` holds each model
/// record's box in its entity's space (`look::model_boxes`).
pub(crate) fn drawn_box(
    w: &World,
    facts: &Facts,
    read: &Reading,
    records: &[Option<Aabb>],
    all: bool,
) -> Option<Aabb> {
    let poses = FramePoses {
        alpha: facts.alpha,
        prev: &read.prev,
        curr: &read.curr,
        attachments: &facts.attachments,
    };
    let mut out: Option<Aabb> = None;
    let mut grow = |b: Option<Aabb>| {
        if let Some(b) = b.filter(|(lo, hi)| lo.is_finite() && hi.is_finite()) {
            out = Some(out.map_or(b, |(lo, hi)| (lo.min(b.0), hi.max(b.1))));
        }
    };
    let grouped = (facts.groups.iter().filter(|_| all))
        .flat_map(|g| &facts.slot_list[g.range.start as usize..g.range.end as usize]);
    let slots: Vec<u32> = (facts.blended.iter().map(|(slot, _)| *slot))
        .chain(grouped.copied())
        .collect();
    // A primitive's slot is its entity's index; its dimensions are its Mesh's.
    let primitives: std::collections::BTreeMap<u32, Entity> =
        if slots.iter().any(|&s| s < crate::RENDER_SLOT_BASE) {
            (w.query::<&Mesh>().iter())
                .map(|(e, _)| (e.index(), e))
                .collect()
        } else {
            Default::default()
        };
    for slot in slots {
        match slot.checked_sub(crate::RENDER_SLOT_BASE) {
            Some(record) => {
                let transform = facts.record_slots.get(record as usize).copied();
                let local = records.get(record as usize).copied().flatten();
                let pose = transform.and_then(|t| poses.matrix(t, false));
                grow(local.zip(pose).map(|(b, m)| transformed(m, b)));
            }
            None => {
                let mesh = primitives.get(&slot).and_then(|&e| w.get::<Mesh>(e));
                let half = mesh.map(|m| primitive_half(&m));
                let pose = poses.matrix(slot, true);
                grow(half.zip(pose).map(|(h, m)| transformed(m, (-h, h))));
            }
        }
    }
    for (e, fact, pose, sprite) in &facts.quads {
        if matches!(fact, QuadFact::Pending(_)) {
            continue;
        }
        if *sprite {
            grow(w.get::<Sprite>(*e).map(|s| sprite_box(&s, pose)));
        } else {
            grow(
                w.get::<Emitter>(*e)
                    .map(|emitter| emitter_box(&emitter, pose)),
            );
        }
    }
    out
}

/// A camera-facing quad turns about its anchor: the ball through its farthest
/// corner holds it from every view.
fn sprite_box(s: &Sprite, pose: &Mat4) -> Aabb {
    let scale = Vec3::new(pose.x_axis.length(), pose.y_axis.length(), 0.);
    let corner = glam::Vec2::new(
        (s.anchor.x - 0.5).abs() + 0.5,
        (s.anchor.y - 0.5).abs() + 0.5,
    ) * glam::Vec2::new(s.size.x * scale.x, s.size.y * scale.y);
    ball(pose.w_axis.truncate(), corner.length())
}

/// An emitter's particles within their reach of it, and of every world-space
/// particle's birthplace, where they stay.
fn emitter_box(emitter: &Emitter, pose: &Mat4) -> Aabb {
    let reach = crate::quads::emitter_reach(emitter, pose);
    let (mut lo, mut hi) = ball(pose.w_axis.truncate(), reach);
    for (_, born) in &emitter.origins {
        let b = ball(Vec3::from(born.translation), reach);
        (lo, hi) = (lo.min(b.0), hi.max(b.1));
    }
    (lo, hi)
}

/// A box's centre and radius (at least a metre); the origin's metre without one.
pub(crate) fn centre_radius(b: Option<Aabb>) -> (Vec3, f32) {
    match b {
        Some((lo, hi)) if lo.is_finite() && hi.is_finite() => {
            ((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1.0))
        }
        _ => (Vec3::ZERO, 1.0),
    }
}

/// Half extents of a primitive mesh about its origin (`shapes.rs`).
fn primitive_half(mesh: &Mesh) -> Vec3 {
    match *mesh {
        Mesh::Box { size } => size.abs() * 0.5,
        Mesh::Sphere { radius } => Vec3::splat(radius.abs()),
        Mesh::Capsule { radius, height } => {
            let r = radius.abs();
            Vec3::new(r, (height.abs() * 0.5).max(r), r)
        }
        Mesh::Cylinder { radius, height } => {
            Vec3::new(radius.abs(), height.abs() * 0.5, radius.abs())
        }
        Mesh::Plane { width, depth } => Vec3::new(width.abs() * 0.5, 0., depth.abs() * 0.5),
        Mesh::Ring { outer, .. } => Vec3::new(outer.abs(), 0., outer.abs()),
        Mesh::Torus { radius, tube } => {
            let r = radius.abs() + tube.abs();
            Vec3::new(r, tube.abs(), r)
        }
        Mesh::Cone { radius, height } => Vec3::new(radius.abs(), height.abs() * 0.5, radius.abs()),
        Mesh::Asset(_) => Vec3::ZERO,
    }
}

fn ball(centre: Vec3, radius: f32) -> Aabb {
    (centre - Vec3::splat(radius), centre + Vec3::splat(radius))
}

/// The world box of a local box under `m`.
pub(crate) fn transformed(m: Mat4, (lo, hi): Aabb) -> Aabb {
    let centre = m.transform_point3((lo + hi) * 0.5);
    let half = (hi - lo) * 0.5;
    let reach = m.x_axis.truncate().abs() * half.x
        + m.y_axis.truncate().abs() * half.y
        + m.z_axis.truncate().abs() * half.z;
    (centre - reach, centre + reach)
}

#[cfg(test)]
mod tests {
    use super::*;
    use exact_game::{Quat, Vec2};

    #[test]
    fn frame_poses_interpolate_rows_and_take_attachments_whole() {
        let row = |x: f32, angle: f32, scale: Vec3| {
            let q = Quat::from_rotation_y(angle);
            [x, 0., 0., q.x, q.y, q.z, q.w, scale.x, scale.y, scale.z]
        };
        let prev = row(0., 0., Vec3::ONE);
        let curr = row(100., 1., Vec3::new(3., 3., -1.));
        let mut attachments = vec![0.; 32];
        let joint = Mat4::from_translation(Vec3::new(5., 6., 7.));
        attachments[16..].copy_from_slice(&joint.to_cols_array());
        let poses = FramePoses {
            alpha: 0.25,
            prev: &[prev, prev].concat(),
            curr: &[curr, curr].concat(),
            attachments: &attachments,
        };
        let m = poses.matrix(0, false).unwrap();
        assert!((m.w_axis.x - 25.).abs() < 1e-4, "{m}");
        let x = m.x_axis.truncate();
        assert!((x.length() - 1.5).abs() < 1e-4, "{x}");
        assert!(((-x.z).atan2(x.x) - 0.25).abs() < 0.02, "{x}");
        // A model's scale keeps its sign; a primitive draws its magnitude.
        assert!((m.z_axis.truncate().length() - 0.5).abs() < 1e-4, "{m}");
        let p = poses.matrix(0, true).unwrap();
        assert!((p.z_axis.truncate().length() - 1.).abs() < 1e-4, "{p}");
        assert_eq!(poses.matrix(1, false), Some(joint));
        assert_eq!(poses.matrix(2, false), None);
    }

    #[test]
    fn a_sprite_is_held_from_every_view_and_an_emitter_by_its_reach() {
        let sprite = Sprite {
            anchor: Vec2::ZERO,
            ..Sprite::new("wide.tex", [30., 2.])
        };
        let (lo, hi) = sprite_box(&sprite, &Mat4::IDENTITY);
        assert!(hi.x >= 30. && lo.x <= -30., "{lo} {hi}");
        let emitter = Emitter::sparks();
        let at = Mat4::from_translation(Vec3::new(10., 0., 0.));
        let (lo, hi) = emitter_box(&emitter, &at);
        assert!(
            lo.x < 10. && ((lo + hi).x / 2. - 10.).abs() < 1e-4,
            "{lo} {hi}"
        );
        let plane = primitive_half(&Mesh::plane(200., 200.));
        let (lo, hi) = transformed(Mat4::IDENTITY, (-plane, plane));
        let (centre, radius) = centre_radius(Some((lo, hi)));
        assert!(centre.length() < 1e-3 && radius > 140., "{centre} {radius}");
    }
}
