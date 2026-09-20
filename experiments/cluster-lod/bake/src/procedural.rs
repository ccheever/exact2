//! Deterministic displaced octasphere. Hash maps are lookup-only; allocation order follows faces.
use crate::{Mesh, Result, normalize};
use std::{collections::HashMap, io::Write};

fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x9e3779b9)
        ^ (y as u32).wrapping_mul(0x85ebca6b)
        ^ (z as u32).wrapping_mul(0xc2b2ae35)
        ^ seed.wrapping_mul(0x27d4eb2d);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    (n & 0xffffff) as f32 / 16777215.0 * 2.0 - 1.0
}
fn noise(p: [f32; 3], seed: u32) -> f32 {
    let q = p.map(|x| x.floor() as i32);
    let f = std::array::from_fn::<_, 3, _>(|i| {
        let f = p[i] - q[i] as f32;
        f * f * (3.0 - 2.0 * f)
    });
    let mut value = 0.0;
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let weight = [x, y, z]
                    .iter()
                    .enumerate()
                    .map(|(i, &b)| if b == 0 { 1.0 - f[i] } else { f[i] })
                    .product::<f32>();
                value += weight * hash(q[0] + x, q[1] + y, q[2] + z, seed);
            }
        }
    }
    value
}
pub fn octasphere(subdivisions: u32) -> Result<Mesh> {
    octasphere_seeded(subdivisions, 0)
}
pub fn octasphere_seeded(subdivisions: u32, seed: u32) -> Result<Mesh> {
    if subdivisions > 10 {
        return Err("procedural subdivisions must be <= 10".into());
    }
    let mut mesh = Mesh {
        positions: vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ],
        indices: vec![
            4, 0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 5, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3,
        ],
        ..Mesh::default()
    };
    for _ in 0..subdivisions {
        let mut edges = HashMap::with_capacity(mesh.indices.len() / 2);
        let mut indices = Vec::with_capacity(mesh.indices.len() * 4);
        for t in mesh.indices.chunks_exact(3) {
            let mut mid = |a: u32, b: u32| -> u32 {
                let key = (a.min(b), a.max(b));
                *edges.entry(key).or_insert_with(|| {
                    let p = normalize(std::array::from_fn(|i| {
                        mesh.positions[a as usize][i] + mesh.positions[b as usize][i]
                    }));
                    let id = mesh.positions.len() as u32;
                    mesh.positions.push(p);
                    id
                })
            };
            let [a, b, c] = [t[0], t[1], t[2]];
            let [ab, bc, ca] = [mid(a, b), mid(b, c), mid(c, a)];
            indices.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
        }
        mesh.indices = indices;
    }
    for p in &mut mesh.positions {
        let mut displacement = 0.0;
        for octave in 0..4 {
            let f = (3 << octave) as f32;
            displacement += noise(p.map(|x| x * f), seed) * 0.08 / (1 << octave) as f32;
        }
        *p = p.map(|x| x * (1.0 + displacement));
    }
    Ok(mesh)
}
pub fn write_ply(mesh: &Mesh, mut out: impl Write) -> Result<()> {
    write!(
        out,
        "ply\nformat binary_little_endian 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        mesh.positions.len(),
        mesh.indices.len() / 3
    )?;
    for p in &mesh.positions {
        for v in p {
            out.write_all(&v.to_le_bytes())?;
        }
    }
    for t in mesh.indices.chunks_exact(3) {
        out.write_all(&[3])?;
        for i in t {
            out.write_all(&i.to_le_bytes())?;
        }
    }
    Ok(())
}
