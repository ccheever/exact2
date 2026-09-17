use crate::Shape;
use exact_game::{Affine3A, Entity, Parent, Transform, Vec3, World};
use rapier3d::{
    math::{Pose, Rotation, Vector},
    parry::{
        shape::{HeightFieldFlags, SharedShape, TriMeshFlags},
        utils::Array2,
    },
};

// All engine ↔ executor math crosses this module. Both currently use glam 0.33.
pub(crate) fn vector(v: Vec3) -> Vector {
    Vector::from_array(v.to_array())
}
pub(crate) fn vec3(v: Vector) -> Vec3 {
    Vec3::from_array(v.to_array())
}
pub(crate) fn pose(t: Transform) -> Pose {
    assert!(
        t.position.is_finite()
            && t.rotation.is_finite()
            && (t.rotation.length_squared() - 1.0).abs() < 1e-3,
        "physics: invalid pose"
    );
    Pose::from_parts(
        vector(t.position),
        Rotation::from_array(t.rotation.to_array()),
    )
}
pub(crate) fn transform(p: &Pose, scale: Vec3) -> Transform {
    Transform {
        position: vec3(p.translation),
        rotation: exact_game::Quat::from_array(p.rotation.to_array()),
        scale,
    }
}
pub(crate) fn shape(s: &Shape, scale: Vec3) -> SharedShape {
    assert!(
        scale.is_finite() && scale.min_element() > 0.0,
        "physics: invalid scale"
    );
    let curved = |radius: f32, height: f32| {
        assert!(
            radius.is_finite()
                && radius > 0.0
                && height.is_finite()
                && height > 0.0
                && scale.x == scale.y
                && scale.y == scale.z,
            "physics: curved shapes require positive dimensions and uniform scale"
        );
    };
    match s {
        Shape::Sphere { radius } => {
            curved(*radius, 1.0);
            SharedShape::ball(radius * scale.x)
        }
        Shape::Capsule { radius, height } => {
            curved(*radius, *height);
            assert!(*height >= 2.0 * radius);
            SharedShape::capsule_y((height * 0.5 - radius) * scale.y, radius * scale.x)
        }
        Shape::Cylinder { radius, height } => {
            curved(*radius, *height);
            SharedShape::cylinder(height * 0.5 * scale.y, radius * scale.x)
        }
        Shape::Box { half } => {
            assert!(
                half.is_finite() && half.min_element() > 0.0,
                "physics: invalid box"
            );
            let h = *half * scale;
            SharedShape::cuboid(h.x, h.y, h.z)
        }
        Shape::Heightfield {
            rows,
            cols,
            heights,
            scale: extents,
        } => {
            assert!(
                *rows >= 2
                    && *cols >= 2
                    && u64::from(*rows) * u64::from(*cols) == heights.len() as u64
                    && heights.iter().all(|x| x.is_finite())
                    && extents.is_finite()
                    && extents.min_element() > 0.0,
                "physics: invalid heightfield"
            );
            let samples = Array2::from_fn(*rows as usize, *cols as usize, |r, c| {
                heights[r * *cols as usize + c]
            });
            SharedShape::heightfield_with_flags(
                samples,
                vector(*extents * scale),
                HeightFieldFlags::FIX_INTERNAL_EDGES,
            )
        }
        Shape::Mesh { vertices, indices } => {
            assert!(
                vertices.iter().all(|v| v.is_finite())
                    && indices
                        .iter()
                        .flatten()
                        .all(|i| (*i as usize) < vertices.len()),
                "physics: invalid mesh"
            );
            SharedShape::trimesh_with_flags(
                vertices.iter().map(|v| vector(*v * scale)).collect(),
                indices.clone(),
                TriMeshFlags::FIX_INTERNAL_EDGES,
            )
            .expect("physics: invalid mesh")
        }
    }
}
// Read current component writes, even before the engine propagates hierarchy.
pub(crate) fn world_pose(world: &World, entity: Entity) -> Transform {
    let t = world
        .get::<Transform>(entity)
        .map(|p| *p)
        .unwrap_or_default();
    if !world.has::<Parent>(entity) {
        return t;
    }
    let mut affine = Affine3A::from_scale_rotation_translation(t.scale, t.rotation, t.position);
    let mut e = entity;
    for _ in 0..world.len() {
        let Some(parent) = world.get::<Parent>(e) else {
            break;
        };
        if !world.contains(parent.0) {
            break;
        }
        e = parent.0;
        assert_ne!(e, entity, "physics: transform cycle");
        let p = world.get::<Transform>(e).map(|p| *p).unwrap_or_default();
        affine =
            Affine3A::from_scale_rotation_translation(p.scale, p.rotation, p.position) * affine;
    }
    let (scale, rotation, position) = affine.to_scale_rotation_translation();
    let rebuilt = Affine3A::from_scale_rotation_translation(scale, rotation, position);
    assert!(
        (affine.matrix3.x_axis - rebuilt.matrix3.x_axis).length() < 1e-4
            && (affine.matrix3.y_axis - rebuilt.matrix3.y_axis).length() < 1e-4
            && (affine.matrix3.z_axis - rebuilt.matrix3.z_axis).length() < 1e-4,
        "physics: sheared collider"
    );
    Transform {
        position,
        rotation,
        scale,
    }
}
