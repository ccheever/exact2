mod ffi;
pub mod loaders;
mod packing;
pub mod procedural;
pub use packing::{Bake, bake};
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Default, Clone)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Option<Vec<[u8; 4]>>,
    pub indices: Vec<u32>,
}
impl Mesh {
    pub fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();
        if self.positions.is_empty() || self.positions.len() > u32::MAX as usize {
            errors.push("empty or oversized vertex buffer");
        }
        if self.indices.is_empty()
            || !self.indices.len().is_multiple_of(3)
            || self.indices.len() > u32::MAX as usize
        {
            errors.push("empty, oversized or non-triangular index buffer");
        }
        if self.positions.iter().flatten().any(|x| !x.is_finite()) {
            errors.push("nonfinite position");
        }
        if self
            .indices
            .iter()
            .any(|v| *v as usize >= self.positions.len())
        {
            errors.push("index outside vertex buffer");
        }
        if !self.normals.is_empty()
            && (self.normals.len() != self.positions.len()
                || self.normals.iter().flatten().any(|x| !x.is_finite()))
        {
            errors.push("invalid normals");
        }
        if self
            .colors
            .as_ref()
            .is_some_and(|c| c.len() != self.positions.len())
        {
            errors.push("invalid colors");
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }
    pub fn compute_normals(&mut self) {
        if !self.normals.is_empty() {
            for n in &mut self.normals {
                *n = normalize(*n);
            }
            return;
        }
        self.normals.resize(self.positions.len(), [0.0; 3]);
        for t in self.indices.chunks_exact(3) {
            let [a, b, c] = [
                self.positions[t[0] as usize],
                self.positions[t[1] as usize],
                self.positions[t[2] as usize],
            ];
            let n = cross(sub(b, a), sub(c, a));
            for i in t {
                for (v, x) in self.normals[*i as usize].iter_mut().zip(n) {
                    *v += x;
                }
            }
        }
        for n in &mut self.normals {
            *n = normalize(*n);
        }
    }
}
pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
pub fn normalize(n: [f32; 3]) -> [f32; 3] {
    let l = dot(n, n).sqrt();
    if l > 0.0 {
        n.map(|x| x / l)
    } else {
        [0.0, 0.0, 1.0]
    }
}
