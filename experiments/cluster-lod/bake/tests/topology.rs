use clod_bake::{bake, procedural};
use clod_format::{Bounds, Config, ORIGINAL, Reader};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

struct Edges {
    keys: Vec<u64>,
    clusters: Vec<Vec<(usize, i32)>>,
    uses: Vec<i32>,
    bad: BTreeSet<usize>,
    selected: Vec<bool>,
    marks: Vec<u32>,
    generation: u32,
}
impl Edges {
    fn new(mesh: &clod_bake::Mesh, reader: &Reader<'_>) -> Self {
        let ids: HashMap<_, _> = mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, p)| (p.map(f32::to_bits), i as u32))
            .collect();
        let mut edges = Vec::new();
        for ci in 0..reader.clusters.len() {
            let (vs, ts) = reader.geometry(ci).unwrap();
            for t in ts.chunks_exact(3) {
                let t: [u32; 3] =
                    std::array::from_fn(|i| ids[&vs[t[i] as usize].position.map(f32::to_bits)]);
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    edges.push((((a.min(b) as u64) << 32) | a.max(b) as u64, ci));
                }
            }
        }
        edges.sort_unstable();
        let mut keys = Vec::new();
        let mut clusters = vec![Vec::new(); reader.clusters.len()];
        let mut i = 0;
        while i < edges.len() {
            let (key, ci) = edges[i];
            if keys.last() != Some(&key) {
                keys.push(key);
            }
            let mut end = i + 1;
            while end < edges.len() && edges[end] == edges[i] {
                end += 1;
            }
            clusters[ci].push((keys.len() - 1, (end - i) as i32));
            i = end;
        }
        Self {
            uses: vec![0; keys.len()],
            marks: vec![0; keys.len()],
            generation: 0,
            keys,
            clusters,
            bad: BTreeSet::new(),
            selected: vec![false; reader.clusters.len()],
        }
    }
    fn update(&mut self, selected: &[bool]) {
        // Check every decoded edge, including cluster interiors; update only changed clusters.
        let mut touched = Vec::new();
        self.generation += 1;
        for (ci, &s) in selected.iter().enumerate() {
            if s == self.selected[ci] {
                continue;
            }
            for &(edge, count) in &self.clusters[ci] {
                self.uses[edge] += if s { count } else { -count };
                if self.marks[edge] != self.generation {
                    self.marks[edge] = self.generation;
                    touched.push(edge);
                }
            }
        }
        for edge in touched {
            if self.uses[edge] == 0 || self.uses[edge] == 2 {
                self.bad.remove(&edge);
            } else {
                self.bad.insert(edge);
            }
        }
        self.selected.copy_from_slice(selected);
    }
}
fn overlaps(reader: &Reader<'_>, selected: &[bool]) -> Vec<[usize; 2]> {
    let mut ancestor = vec![None; reader.groups.len()];
    for (i, (c, &s)) in reader.clusters.iter().zip(selected).enumerate() {
        if s && c.refined != ORIGINAL {
            ancestor[c.refined as usize] = Some(i);
        }
    }
    let mut pairs = Vec::new();
    for (gi, g) in reader.groups.iter().enumerate().rev() {
        if let Some(a) = ancestor[gi] {
            for ci in g.first_cluster as usize..(g.first_cluster + g.cluster_count) as usize {
                if selected[ci] {
                    pairs.push([a, ci]);
                }
                let c = &reader.clusters[ci];
                if c.refined != ORIGINAL {
                    ancestor[c.refined as usize] = Some(a);
                }
            }
        }
    }
    pairs
}
fn unit(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 40) as f32 / (1u32 << 24) as f32
}
fn rotate(p: [f32; 3], q: [f32; 4]) -> [f32; 3] {
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let v = [q[0], q[1], q[2]];
    let t = cross(v, p).map(|x| x * 2.0);
    let u = cross(v, t);
    std::array::from_fn(|i| p[i] + q[3] * t[i] + u[i])
}
#[test]
fn multi_fixture_closed_cuts() {
    let start = Instant::now();
    let mut failed_cuts = 0;
    let mut checked = 0;
    let mut checked_triangles = 0u64;
    let mut fixtures = 0;
    for sub in 3..=9 {
        for seed in 0..3 {
            let fixture_start = Instant::now();
            let mut mesh = procedural::octasphere_seeded(sub, seed).unwrap();
            let baked = bake(&mut mesh, Config::default(), [0; 32]).unwrap();
            let reader = Reader::new(&baked.bytes).unwrap();
            let mut edges = Edges::new(&mesh, &reader);
            let mut rng = 0xa512_a971_2000_0421;
            let mut distinct = BTreeSet::new();
            let mut mixed = 0;
            let mut bad_uniform = 0;
            let mut bad_camera = 0;
            let mut bounds_violations = 0;
            let mut terminal_depths = BTreeSet::new();
            for c in reader.clusters {
                if c.simplified.error == f32::MAX {
                    terminal_depths.insert(c.depth);
                }
                if c.refined != ORIGINAL {
                    let d = c
                        .simplified
                        .center
                        .iter()
                        .zip(c.refined_bounds.center)
                        .map(|(&a, b)| (a as f64 - b as f64).powi(2))
                        .sum::<f64>()
                        .sqrt();
                    if d + c.refined_bounds.radius as f64 > c.simplified.radius as f64 + 1e-6 {
                        bounds_violations += 1;
                    }
                }
            }
            let thresholds = [
                0.0, 0.00001, 0.00003, 0.0001, 0.0003, 0.001, 0.003, 0.01, 0.03, 0.1, 0.3, 1.0,
                3.0, 10.0, 100.0,
            ];
            for camera in 0..515 {
                let direction =
                    clod_bake::normalize(std::array::from_fn(|_| unit(&mut rng) * 2.0 - 1.0));
                let radius = 10f32.powf(unit(&mut rng) * 2.5 - 0.5);
                let eye = direction.map(|x| x * radius);
                let cot = 1.0 / ((20.0 + unit(&mut rng) * 100.0).to_radians() * 0.5).tan();
                let near = 0.001 + unit(&mut rng) * 0.099;
                let height = 720.0 + unit(&mut rng) * 1440.0;
                let threshold = if camera < 15 {
                    thresholds[camera]
                } else {
                    10f32.powf(unit(&mut rng) * 3.0 - 1.0)
                };
                let q: [f32; 4] = std::array::from_fn(|_| unit(&mut rng) * 2.0 - 1.0);
                let length = q.iter().map(|x| x * x).sum::<f32>().sqrt();
                let q = q.map(|x| x / length);
                let project = |b: &Bounds| {
                    if camera < 15 {
                        b.error
                    } else {
                        let b = Bounds {
                            center: rotate(b.center, q),
                            ..*b
                        };
                        clod_format::projection::Projection {
                            eye: rotate(eye, q),
                            cot,
                            near,
                            height,
                            orthographic_span: None,
                        }
                        .projected(&b, b.center, 1.0)
                    }
                };
                let errors: Vec<_> = reader
                    .groups
                    .iter()
                    .map(|g| project(&g.simplified))
                    .collect();
                let selected: Vec<_> = reader
                    .clusters
                    .iter()
                    .map(|c| {
                        errors[c.group as usize] > threshold
                            && (c.refined == ORIGINAL || errors[c.refined as usize] <= threshold)
                    })
                    .collect();
                edges.update(&selected);
                let overlap = overlaps(&reader, &selected);
                let tris: u64 = reader
                    .clusters
                    .iter()
                    .zip(&selected)
                    .filter(|(_, s)| **s)
                    .map(|(c, _)| c.triangle_count as u64)
                    .sum();
                let depths: BTreeSet<_> = reader
                    .clusters
                    .iter()
                    .zip(&selected)
                    .filter(|(_, s)| **s)
                    .map(|(c, _)| c.depth)
                    .collect();
                checked += 1;
                checked_triangles += tris;
                distinct.insert(tris);
                mixed += usize::from(depths.len() > 1);
                if !edges.bad.is_empty() || !overlap.is_empty() || tris == 0 {
                    failed_cuts += 1;
                    if camera < 15 {
                        bad_uniform += 1;
                    } else {
                        bad_camera += 1;
                    }
                    let record = |ci: usize| {
                        let c = &reader.clusters[ci];
                        json!({"cluster":ci,"group":c.group,"refined":c.refined,"depth":c.depth,"selected":selected[ci],"simplified_projected":project(&c.simplified),"refined_projected":project(&c.refined_bounds)})
                    };
                    for &edge in &edges.bad {
                        let sides: Vec<_> = edges
                            .clusters
                            .iter()
                            .enumerate()
                            .filter_map(|(ci, es)| {
                                es.binary_search_by_key(&edge, |x| x.0)
                                    .ok()
                                    .map(|i| json!({"uses":es[i].1,"record":record(ci)}))
                            })
                            .collect();
                        println!(
                            "{}",
                            json!({"failure":"edge","subdivision":sub,"seed":seed,"camera":camera,"uniform":camera<15,"eye":eye,"orientation":q,"cot":cot,"near":near,"height":height,"threshold":threshold,"edge":[edges.keys[edge]>>32,edges.keys[edge] as u32 as u64],"uses":edges.uses[edge],"sides_all_generations":sides})
                        );
                    }
                    for pair in overlap {
                        println!(
                            "{}",
                            json!({"failure":"overlap","subdivision":sub,"seed":seed,"camera":camera,"threshold":threshold,"pair":pair.map(record)})
                        );
                    }
                }
            }
            fixtures += 1;
            if distinct.len() < 3 || (sub >= 5 && mixed == 0) {
                failed_cuts += 1;
                println!(
                    "{}",
                    json!({"failure":"vacuous_lod","subdivision":sub,"seed":seed,"distinct":distinct.len(),"mixed":mixed})
                );
            }
            println!(
                "{}",
                json!({"oracle":"topology_fixture","subdivision":sub,"seed":seed,"source_triangles":mesh.indices.len()/3,"clusters":reader.clusters.len(),"groups":reader.groups.len(),"bounds_violations":bounds_violations,"terminal_depths":terminal_depths,"uniform_cuts":15,"cameras":500,"bad_uniform":bad_uniform,"bad_camera":bad_camera,"distinct_cuts":distinct.len(),"mixed_cuts":mixed,"seconds":fixture_start.elapsed().as_secs_f64()})
            );
        }
    }
    println!(
        "{}",
        json!({"oracle":"topology_summary","fixtures":fixtures,"cuts":checked,"triangles":checked_triangles,"failed_cuts":failed_cuts,"seconds":start.elapsed().as_secs_f64()})
    );
    assert_eq!(failed_cuts, 0);
}
