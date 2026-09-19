//! Indexed, counterclockwise primitives centred at the origin, with outward normals.

use crate::Vertex;
use glam::{Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Unit cube with independent face normals: 24 vertices, 12 triangles.
pub fn cube() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (n, u, v) in [
        (Vec3::X, -Vec3::Z, Vec3::Y),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, -Vec3::Z),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, -Vec3::X, Vec3::Y),
    ] {
        let base = vertices.len() as u32;
        for [x, y] in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            vertices.push(Vertex {
                position: (n * 0.5 + u * (x - 0.5) + v * (y - 0.5)).to_array(),
                normal: n.to_array(),
                uv: [0.0; 2],
            });
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

/// Unit quad in XZ with +Y normals and four vertices.
pub fn plane() -> (Vec<Vertex>, Vec<u32>) {
    let vertices = [
        [-0.5, 0.0, 0.5],
        [0.5, 0.0, 0.5],
        [0.5, 0.0, -0.5],
        [-0.5, 0.0, -0.5],
    ]
    .into_iter()
    .zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
    .map(|(position, _uv)| Vertex {
        position,
        normal: [0.0, 1.0, 0.0],
        uv: [0.0; 2],
    })
    .collect();
    (vertices, vec![0, 1, 2, 0, 2, 3])
}

/// UV sphere, radius 0.5; `segments` longitudes and ceil(segments/2) latitudes.
/// At least three segments are required.
pub fn sphere(segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    assert!(segments >= 3);
    let rings = segments.div_ceil(2).max(2);
    let profile: Vec<_> = (0..=rings)
        .map(|i| {
            let angle = PI * i as f32 / rings as f32;
            let [c, s] = Vec2::from_angle(angle).to_array();
            let s = if i == 0 || i == rings { 0.0 } else { s };
            Ring {
                radius: s * 0.5,
                y: c * 0.5,
                radial_normal: s,
                y_normal: c,
            }
        })
        .collect();
    revolve(&profile, segments)
}

/// Cylinder along Y, radius 0.5 and height 1, with flat caps.
/// At least three segments are required.
pub fn cylinder(segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    assert!(segments >= 3);
    let profile = [
        Ring {
            radius: 0.0,
            y: 0.5,
            radial_normal: 0.0,
            y_normal: 1.0,
        },
        Ring {
            radius: 0.5,
            y: 0.5,
            radial_normal: 0.0,
            y_normal: 1.0,
        },
        Ring {
            radius: 0.5,
            y: 0.5,
            radial_normal: 1.0,
            y_normal: 0.0,
        },
        Ring {
            radius: 0.5,
            y: -0.5,
            radial_normal: 1.0,
            y_normal: 0.0,
        },
        Ring {
            radius: 0.5,
            y: -0.5,
            radial_normal: 0.0,
            y_normal: -1.0,
        },
        Ring {
            radius: 0.0,
            y: -0.5,
            radial_normal: 0.0,
            y_normal: -1.0,
        },
    ];
    revolve(&profile, segments)
}

/// Y-axis capsule; `height` is total tip-to-tip height, including hemispheres.
/// Requires finite radius > 0, height >= 2 * radius, and segments >= 3.
pub fn capsule(radius: f32, height: f32, segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    assert!(
        radius.is_finite()
            && radius > 0.0
            && height.is_finite()
            && height >= 2.0 * radius
            && segments >= 3
    );
    let half_stem = height * 0.5 - radius;
    let steps = segments.div_ceil(4).max(1);
    let mut profile = Vec::with_capacity((2 * steps + 2) as usize);
    for lower in [false, true] {
        for i in 0..=steps {
            let angle = (i as f32 / steps as f32 + if lower { 1.0 } else { 0.0 }) * FRAC_PI_2;
            let [c, s] = Vec2::from_angle(angle).to_array();
            let s = if (!lower && i == 0) || (lower && i == steps) {
                0.0
            } else {
                s
            };
            let c = if (!lower && i == steps) || (lower && i == 0) {
                0.0
            } else {
                c
            };
            profile.push(Ring {
                radius: radius * s,
                y: radius * c + if lower { -half_stem } else { half_stem },
                radial_normal: s,
                y_normal: c,
            });
        }
    }
    revolve(&profile, segments)
}

struct Ring {
    radius: f32,
    y: f32,
    radial_normal: f32,
    y_normal: f32,
}

fn revolve(profile: &[Ring], segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    let stride = segments + 1;
    let circle: Vec<_> = (0..=segments)
        .map(|j| Vec2::from_angle(TAU * j as f32 / segments as f32).to_array())
        .collect();
    let mut vertices = Vec::with_capacity(profile.len() * stride as usize);
    let mut indices = Vec::new();
    for (i, ring) in profile.iter().enumerate() {
        for &[c, s] in &circle {
            vertices.push(Vertex {
                position: [ring.radius * c, ring.y, ring.radius * s],
                normal: [
                    ring.radial_normal * c,
                    ring.y_normal,
                    ring.radial_normal * s,
                ],
                uv: [0.0; 2],
            });
        }
        if i == 0 {
            continue;
        }
        let previous = &profile[i - 1];
        if previous.radius == ring.radius && previous.y == ring.y {
            continue;
        }
        for j in 0..segments {
            let a = (i as u32 - 1) * stride + j;
            let b = i as u32 * stride + j;
            if previous.radius > 0.0 {
                indices.extend([a, a + 1, b]);
            }
            if ring.radius > 0.0 {
                indices.extend([a + 1, b + 1, b]);
            }
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_have_outward_triangles_and_unit_normals() {
        assert_eq!(cube().0.len(), 24);
        for (vertices, indices) in [
            cube(),
            plane(),
            sphere(24),
            cylinder(24),
            capsule(0.3, 1.8, 24),
            capsule(0.5, 1.0, 24),
        ] {
            for vertex in &vertices {
                assert!((Vec3::from_array(vertex.normal).length() - 1.0).abs() < 1e-5);
            }
            for triangle in indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| vertices[triangle[i] as usize]);
                let cross = (Vec3::from_array(b.position) - Vec3::from_array(a.position))
                    .cross(Vec3::from_array(c.position) - Vec3::from_array(a.position));
                let normal = Vec3::from_array(a.normal)
                    + Vec3::from_array(b.normal)
                    + Vec3::from_array(c.normal);
                assert!(cross.dot(normal) > 1e-8, "inward or degenerate triangle");
            }
        }
    }
}
