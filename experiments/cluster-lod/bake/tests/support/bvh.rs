use clod_bake::Mesh;
type V = [f64; 3];
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn scale(a: V, s: f64) -> V {
    a.map(|v| v * s)
}
fn dot(a: V, b: V) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn distance(p: V, t: [V; 3]) -> f64 {
    // Closest point on triangle, including vertex and edge Voronoi regions.
    let [a, b, c] = t;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot(ap, ap);
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot(bp, bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = sub(p, add(a, scale(ab, v)));
        return dot(q, q);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot(cp, cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = sub(p, add(a, scale(ac, w)));
        return dot(q, q);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let q = sub(p, add(b, scale(sub(c, b), w)));
        return dot(q, q);
    }
    let denom = 1.0 / (va + vb + vc);
    let q = sub(p, add(a, add(scale(ab, vb * denom), scale(ac, vc * denom))));
    dot(q, q)
}
struct Node {
    min: V,
    max: V,
    start: usize,
    count: usize,
    children: Option<[usize; 2]>,
}
pub struct Bvh {
    triangles: Vec<[V; 3]>,
    order: Vec<usize>,
    nodes: Vec<Node>,
}
impl Bvh {
    pub fn new(mesh: &Mesh) -> Self {
        let triangles: Vec<_> = mesh
            .indices
            .chunks_exact(3)
            .map(|t| std::array::from_fn(|i| mesh.positions[t[i] as usize].map(f64::from)))
            .collect();
        let mut b = Self {
            order: (0..triangles.len()).collect(),
            triangles,
            nodes: Vec::new(),
        };
        b.build(0, b.order.len());
        b
    }
    fn build(&mut self, start: usize, count: usize) -> usize {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for &i in &self.order[start..start + count] {
            for v in &self.triangles[i] {
                for k in 0..3 {
                    min[k] = min[k].min(v[k]);
                    max[k] = max[k].max(v[k]);
                }
            }
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            min,
            max,
            start,
            count,
            children: None,
        });
        if count > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| (max[a] - min[a]).total_cmp(&(max[b] - min[b])))
                .expect("axis");
            let triangles = &self.triangles;
            self.order[start..start + count].select_nth_unstable_by(count / 2, |&a, &b| {
                let a = triangles[a].iter().map(|v| v[axis]).sum::<f64>();
                let b = triangles[b].iter().map(|v| v[axis]).sum::<f64>();
                a.total_cmp(&b)
            });
            let left = self.build(start, count / 2);
            let right = self.build(start + count / 2, count - count / 2);
            self.nodes[id].children = Some([left, right]);
        }
        id
    }
    fn box_distance(&self, node: usize, p: V) -> f64 {
        let n = &self.nodes[node];
        (0..3)
            .map(|i| (n.min[i] - p[i]).max(p[i] - n.max[i]).max(0.0).powi(2))
            .sum()
    }
    fn visit(&self, id: usize, p: V, best: &mut f64) {
        let n = &self.nodes[id];
        if let Some([a, b]) = n.children {
            let da = self.box_distance(a, p);
            let db = self.box_distance(b, p);
            let ordered = if da < db {
                [(a, da), (b, db)]
            } else {
                [(b, db), (a, da)]
            };
            for (child, d) in ordered {
                if d < *best {
                    self.visit(child, p, best);
                }
            }
        } else {
            for &i in &self.order[n.start..n.start + n.count] {
                *best = best.min(distance(p, self.triangles[i]));
            }
        }
    }
    pub fn distance_squared(&self, p: V) -> f64 {
        let mut best = f64::INFINITY;
        self.visit(0, p, &mut best);
        best
    }
}
pub fn areas(mesh: &Mesh, cut: &[[u32; 3]]) -> Vec<f64> {
    let mut sum = 0.0;
    cut.iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize].map(f64::from));
            let ab = sub(b, a);
            let ac = sub(c, a);
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            sum += dot(cross, cross).sqrt() * 0.5;
            sum
        })
        .collect()
}
pub fn point(mesh: &Mesh, t: [u32; 3], u: f64, v: f64) -> V {
    let s = u.sqrt();
    let weights = [1.0 - s, s * (1.0 - v), s * v];
    std::array::from_fn(|i| {
        (0..3)
            .map(|k| mesh.positions[t[k] as usize][i] as f64 * weights[k])
            .sum()
    })
}
