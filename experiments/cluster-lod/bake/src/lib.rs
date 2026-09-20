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
        let mut normals = vec![[0.0f64; 3]; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let [a, b, c] = [
                self.positions[t[0] as usize].map(f64::from),
                self.positions[t[1] as usize].map(f64::from),
                self.positions[t[2] as usize].map(f64::from),
            ];
            let u: [f64; 3] = std::array::from_fn(|i| b[i] - a[i]);
            let v: [f64; 3] = std::array::from_fn(|i| c[i] - a[i]);
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            for i in t {
                for (v, x) in normals[*i as usize].iter_mut().zip(n) {
                    *v += x;
                }
            }
        }
        self.normals = normals
            .into_iter()
            .map(|n| {
                let length = n.iter().map(|x| x * x).sum::<f64>().sqrt();
                if length > 0.0 {
                    n.map(|x| (x / length) as f32)
                } else {
                    [0.0, 0.0, 1.0]
                }
            })
            .collect();
    }
}
pub fn normalize(n: [f32; 3]) -> [f32; 3] {
    let l = n.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if l > 0.0 {
        n.map(|x| (x as f64 / l) as f32)
    } else {
        [0.0, 0.0, 1.0]
    }
}
