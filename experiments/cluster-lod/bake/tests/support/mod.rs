pub mod bvh;
use clod_bake::Mesh;
use clod_format::{ORIGINAL, Reader};
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
#[derive(Default, Debug)]
pub struct Edges {
    pub bad: usize,
    pub boundary: usize,
    pub max_use: usize,
    pub degenerate: usize,
}
pub fn edges(triangles: &[[u32; 3]]) -> Edges {
    let mut edges = Vec::with_capacity(triangles.len() * 3);
    let mut stats = Edges::default();
    for t in triangles {
        if t[0] == t[1] || t[1] == t[2] || t[2] == t[0] {
            stats.degenerate += 1;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.push(((a.min(b) as u64) << 32) | a.max(b) as u64);
        }
    }
    edges.sort_unstable();
    let mut i = 0;
    while i < edges.len() {
        let mut end = i + 1;
        while end < edges.len() && edges[end] == edges[i] {
            end += 1;
        }
        let uses = end - i;
        stats.max_use = stats.max_use.max(uses);
        if uses != 2 {
            stats.bad += 1;
            if uses == 1 {
                stats.boundary += 1;
            }
        }
        i = end;
    }
    stats
}
pub fn overlaps(reader: &Reader<'_>, selected: &[bool]) -> usize {
    // Mark every descendant group of every selected cluster in reverse topological order.
    let mut descendant = vec![false; reader.groups.len()];
    for (c, &s) in reader.clusters.iter().zip(selected) {
        if s && c.refined != ORIGINAL {
            descendant[c.refined as usize] = true;
        }
    }
    let mut count = 0;
    for (gi, g) in reader.groups.iter().enumerate().rev() {
        if !descendant[gi] {
            continue;
        }
        let range = g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize;
        for (c, &is_selected) in reader.clusters[range.clone()].iter().zip(&selected[range]) {
            count += usize::from(is_selected);
            if c.refined != ORIGINAL {
                descendant[c.refined as usize] = true;
            }
        }
    }
    count
}
