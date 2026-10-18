//! What each drawn entity displays, as a world-space box (LLP 1127.000 D3, D4):
//! what an orbit or plan look frame frames, and the extent `visible` tests
//! against the view. Primitives by their dimensions, models by their meshes'
//! boxes as the renderer holds them, sprites by their quad about the anchor,
//! emitters by the reach their particles' own cull uses (`quads::emitter_reach`).
use exact_game::{Emitter, Entity, Mesh, Sprite, Vec3, World};
use glam::Mat4;

/// A box, `lo` to `hi`.
pub(crate) type Aabb = (Vec3, Vec3);

/// The box of what `e` displays, or `None` when it is not drawn or displays nothing.
/// `models` holds each model record's box in its entity's space, by entity index
/// (`Renderer::model_boxes`).
pub(crate) fn displayed(w: &World, e: Entity, models: &[(u32, Aabb)]) -> Option<Aabb> {
    let pose = Mat4::from(w.drawn(e)?.pose);
    let mut out: Option<Aabb> = None;
    let mut grow = |b: Aabb| {
        out = Some(out.map_or(b, |(lo, hi)| (lo.min(b.0), hi.max(b.1))));
    };
    let origin = pose.w_axis.truncate();
    if let Some(mesh) = w.get::<Mesh>(e) {
        if let Mesh::Asset(_) = &*mesh {
            for (_, b) in models.iter().filter(|(t, _)| *t == e.index()) {
                grow(transformed(pose, *b));
            }
        } else {
            let half = primitive_half(&mesh);
            grow(transformed(pose, (-half, half)));
        }
    }
    if let Some(s) = w.get::<Sprite>(e) {
        // A camera-facing quad turns about its anchor: the ball through its
        // farthest corner holds it from every view.
        let scale = Vec3::new(pose.x_axis.length(), pose.y_axis.length(), 0.);
        let corner = glam::Vec2::new(
            (s.anchor.x - 0.5).abs() + 0.5,
            (s.anchor.y - 0.5).abs() + 0.5,
        ) * glam::Vec2::new(s.size.x * scale.x, s.size.y * scale.y);
        grow(ball(origin, corner.length()));
    }
    if let Some(emitter) = w.get::<Emitter>(e) {
        let reach = crate::quads::emitter_reach(&emitter, &pose);
        grow(ball(origin, reach));
        // World-space particles stay where they were born.
        for (_, born) in &emitter.origins {
            grow(ball(Vec3::from(born.translation), reach));
        }
    }
    out
}

/// The drawn scene's centre and radius: the box around every drawn entity's
/// displayed geometry.
#[cfg(test)]
pub(crate) fn scene(w: &World, models: &[(u32, Aabb)]) -> (Vec3, f32) {
    centre_radius(scene_box(w, models))
}

/// The box around every drawn entity's displayed geometry.
pub(crate) fn scene_box(w: &World, models: &[(u32, Aabb)]) -> Option<Aabb> {
    let mut all: Option<Aabb> = None;
    let mut grow = |e: Entity| {
        if let Some(b) = displayed(w, e, models) {
            all = Some(all.map_or(b, |(lo, hi)| (lo.min(b.0), hi.max(b.1))));
        }
    };
    w.query::<&Mesh>().iter().for_each(|(e, _)| grow(e));
    w.query::<&Sprite>().iter().for_each(|(e, _)| grow(e));
    w.query::<&Emitter>().iter().for_each(|(e, _)| grow(e));
    all
}

/// The box around what the cull's account leaves out: sprites and emitters,
/// and the meshes whose slots `outside` names (blended models, kept-whole
/// groups), by entity index (a primitive's slot, a model record's transform).
pub(crate) fn unculled_box(w: &World, models: &[(u32, Aabb)], outside: &[u32]) -> Option<Aabb> {
    let mut all: Option<Aabb> = None;
    let mut grow = |e: Entity| {
        if let Some(b) = displayed(w, e, models) {
            all = Some(all.map_or(b, |(lo, hi)| (lo.min(b.0), hi.max(b.1))));
        }
    };
    (w.query::<&Mesh>().iter())
        .filter(|(e, _)| outside.contains(&e.index()))
        .for_each(|(e, _)| grow(e));
    w.query::<&Sprite>().iter().for_each(|(e, _)| grow(e));
    w.query::<&Emitter>().iter().for_each(|(e, _)| grow(e));
    all
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
    use exact_game::{Transform, Vec2};

    #[test]
    fn bounds_hold_a_large_plane_a_model_and_an_anchored_sprite() {
        let mut w = World::new(60, 1);
        // One 200 m plane: its origin alone would give a radius of 1.
        w.spawn((Transform::default(), Mesh::plane(200., 200.)));
        let (centre, radius) = scene(&w, &[]);
        assert!(centre.length() < 1e-3 && radius > 140., "{centre} {radius}");

        let mut w = World::new(60, 1);
        let model = w.spawn((Transform::at(10., 0., 0.), Mesh::asset("tower.model")));
        let tower = (Vec3::new(-1., 0., -1.), Vec3::new(1., 30., 1.));
        let b = displayed(&w, model, &[(model.index(), tower)]).unwrap();
        assert!(
            (b.1.y - 30.).abs() < 1e-4 && (b.0.x - 9.).abs() < 1e-4,
            "{b:?}"
        );

        let mut w = World::new(60, 1);
        let sprite = w.spawn((
            Transform::default(),
            Sprite {
                anchor: Vec2::ZERO,
                ..Sprite::new("wide.tex", [30., 2.])
            },
        ));
        let (lo, hi) = displayed(&w, sprite, &[]).unwrap();
        assert!(hi.x >= 30. && lo.x <= -30., "{lo} {hi}");
    }
}
