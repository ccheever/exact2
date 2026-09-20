pub mod bvh;
use clod_bake::Mesh;
use clod_format::Reader;
use std::collections::HashMap;

pub struct Random(u64);
impl Random {
    pub fn new() -> Self {
        Self(0xa512_a971_2000_0421)
    }
    pub fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / ((1u64 << 53) as f64)
    }
}
pub fn canonical(t: [u32; 3]) -> [u32; 3] {
    if t[0] <= t[1] && t[0] <= t[2] {
        t
    } else if t[1] <= t[2] {
        [t[1], t[2], t[0]]
    } else {
        [t[2], t[0], t[1]]
    }
}
pub struct Geometry {
    pub triangles: Vec<Vec<[u32; 3]>>,
    pub unknown_positions: usize,
    pub copies: usize,
}
impl Geometry {
    pub fn new(mesh: &Mesh, reader: &Reader<'_>) -> Self {
        let positions: HashMap<_, _> = mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, p)| (p.map(f32::to_bits), i as u32))
            .collect();
        let mut triangles = Vec::new();
        let mut unknown_positions = 0;
        let mut copies = 0;
        for i in 0..reader.clusters.len() {
            let (vs, indices) = reader.geometry(i).expect("validated geometry");
            let ids: Vec<_> = vs
                .iter()
                .map(|v| {
                    copies += 1;
                    *positions
                        .get(&v.position.map(f32::to_bits))
                        .unwrap_or_else(|| {
                            unknown_positions += 1;
                            &u32::MAX
                        })
                })
                .collect();
            triangles.push(
                indices
                    .chunks_exact(3)
                    .map(|t| [ids[t[0] as usize], ids[t[1] as usize], ids[t[2] as usize]])
                    .collect(),
            );
        }
        Self {
            triangles,
            unknown_positions,
            copies,
        }
    }
    pub fn cut(&self, selected: &[bool]) -> Vec<[u32; 3]> {
        self.triangles
            .iter()
            .zip(selected)
            .filter(|(_, s)| **s)
            .flat_map(|(t, _)| t.iter().copied())
            .collect()
    }
}
pub use clod_format::oracle::{edges, overlaps};
