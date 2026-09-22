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

pub(crate) fn heightfield(
    rows: u32,
    cols: u32,
    heights: Vec<f32>,
    scale: Vec3,
) -> Result<(exact_game::asset::MeshData, Shape), String> {
    if rows < 2
        || cols < 2
        || u64::from(rows) * u64::from(cols) != heights.len() as u64
        || heights.iter().any(|h| !h.is_finite())
        || !scale.is_finite()
        || scale.min_element() <= 0.
    {
        return Err("heightfield: invalid rows, cols, heights or scale".into());
    }
    let mut mesh = exact_game::asset::MeshData::default();
    let dx = 1. / (cols - 1) as f32;
    let dz = 1. / (rows - 1) as f32;
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for r in 0..rows {
        for c in 0..cols {
            let p = Vec3::new(
                -0.5 + dx * c as f32,
                heights[(r * cols + c) as usize],
                -0.5 + dz * r as f32,
            ) * scale;
            if !p.is_finite() {
                return Err("heightfield: scaled vertex is not finite".into());
            }
            mesh.positions.extend(p.to_array());
            mesh.uvs.extend([dx * c as f32, dz * r as f32]);
            lo = lo.min(p);
            hi = hi.max(p);
            if r + 1 < rows && c + 1 < cols {
                let a = r * cols + c;
                mesh.indices
                    .extend([a, a + cols, a + 1, a + cols, a + cols + 1, a + 1]);
            }
        }
    }
    let mut normals = vec![Vec3::ZERO; heights.len()];
    for t in mesh.indices.chunks_exact(3) {
        let p = |i: u32| Vec3::from_slice(&mesh.positions[i as usize * 3..][..3]);
        let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
        for &i in t {
            normals[i as usize] += n;
        }
    }
    mesh.normals = normals
        .into_iter()
        .flat_map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array())
        .collect();
    mesh.bounds = [lo.x, lo.y, lo.z, hi.x, hi.y, hi.z];
    Ok((
        mesh,
        Shape::Heightfield {
            rows,
            cols,
            heights,
            scale,
        },
    ))
}
