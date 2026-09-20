//! Resolve an authored screen anchor to the nearest real source triangle once, at load time.
use crate::scene::Camera;
use clod_format::{ORIGINAL, Reader};
use glam::{Mat4, Vec3};
pub fn target(
    reader: &Reader<'_>,
    base: Mat4,
    min: Vec3,
    max: Vec3,
    washington: bool,
) -> (Vec3, Vec3) {
    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    let fov = 45f32.to_radians();
    let eye = center + Vec3::new(0.85, -1.6, 0.85).normalize() * radius / (fov * 0.5).sin() * 1.1;
    let gaul = reader.header.source_sha256
        == [
            66, 70, 235, 208, 143, 170, 208, 211, 168, 58, 223, 157, 119, 225, 17, 122, 80, 158,
            152, 207, 233, 58, 254, 94, 60, 163, 106, 188, 30, 247, 194, 178,
        ];
    let anchor = if washington {
        [0.442, 0.277]
    } else if gaul {
        [0.379, 0.364]
    } else {
        [0.5, 0.45]
    };
    let camera = Camera::perspective(eye, center, 16.0 / 9.0, fov, 0.002, radius * 12.0 + 10.0);
    let far = camera.matrix.inverse().project_point3(Vec3::new(
        anchor[0] * 2.0 - 1.0,
        1.0 - anchor[1] * 2.0,
        0.9,
    ));
    let ray = (far - eye).normalize();
    let inverse = base.inverse();
    let origin = inverse.transform_point3(eye);
    let direction = inverse.transform_vector3(ray).normalize();
    let mut nearest = f32::INFINITY;
    for (id, c) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.refined == ORIGINAL)
    {
        let center = Vec3::new(c.sphere[0], c.sphere[1], c.sphere[2]);
        let offset = center - origin;
        if offset.cross(direction).length_squared() > c.sphere[3] * c.sphere[3] {
            continue;
        }
        let (vertices, indices) = reader.geometry(id).expect("validated source geometry");
        for tri in indices.chunks_exact(3) {
            let a = Vec3::from_array(vertices[tri[0] as usize].position);
            let b = Vec3::from_array(vertices[tri[1] as usize].position);
            let c = Vec3::from_array(vertices[tri[2] as usize].position);
            let e1 = b - a;
            let e2 = c - a;
            let h = direction.cross(e2);
            let det = e1.dot(h);
            if det <= 1e-12 {
                continue;
            }
            let s = origin - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = direction.dot(q) / det;
            let t = e2.dot(q) / det;
            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0 && t < nearest {
                nearest = t;
            }
        }
    }
    let target = if nearest.is_finite() {
        base.transform_point3(origin + direction * nearest)
    } else {
        center
    };
    (target, (eye - target).normalize())
}
