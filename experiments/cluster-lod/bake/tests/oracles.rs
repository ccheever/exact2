mod support;
use clod_bake::{bake, procedural};
use clod_format::{Config, Reader, unpack_normal};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::Instant;
use support::{Geometry, Random, bvh, canonical, edges, overlaps};

#[test]
fn numerical_oracles() {
    let start = Instant::now();
    let mut failures = Vec::new();
    let mut mesh = procedural::octasphere(8).expect("procedural mesh");
    let mut hasher = Sha256::new();
    hasher.update(bytemuck::cast_slice(&mesh.positions));
    hasher.update(bytemuck::cast_slice(&mesh.indices));
    let digest = hasher.finalize().into();
    // Small enough to force copies across pages and exercise page zero independently.
    let config = Config {
        page_bytes: 256 * 1024,
        ..Config::default()
    };
    let bake_start = Instant::now();
    let first = bake(&mut mesh, config, digest).expect("first bake");
    let second = bake(&mut mesh, config, digest).expect("second bake");
    let equal = first.bytes == second.bytes;
    if !equal {
        failures.push("determinism: bytes differ".into());
    }
    println!(
        "{}",
        json!({"oracle":"determinism","triangles":mesh.indices.len()/3,"vertices":mesh.positions.len(),"bakes":2,"equal":equal,"bytes":first.bytes.len(),"seconds":bake_start.elapsed().as_secs_f64(),"sha256":format!("{:x}",Sha256::digest(&first.bytes))})
    );
    drop(second);
    let reader = Reader::new(&first.bytes).expect("round trip");
    let geo = Geometry::new(&mesh, &reader);
    if geo.unknown_positions > 0 {
        failures.push(format!(
            "{} decoded vertices differ from source",
            geo.unknown_positions
        ));
    }
    if reader.pages.len() < 2 {
        failures.push("fixture did not exercise multiple pages".into());
    }
    let mut normal_max = 0.0f64;
    for ci in 0..reader.clusters.len() {
        let (vs, _) = reader.geometry(ci).expect("geometry");
        for v in vs {
            let n = unpack_normal(v.normal);
            let length = n.iter().map(|x| x * x).sum::<f32>().sqrt();
            if !length.is_finite() || (length - 1.0).abs() > 1e-5 {
                failures.push("invalid decoded normal".into());
                break;
            }
        }
    }
    for &n in &mesh.normals {
        let decoded = unpack_normal(clod_format::pack_normal(n));
        let d = n
            .iter()
            .zip(decoded)
            .map(|(a, b)| (*a as f64 - b as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        normal_max = normal_max.max(d);
    }
    if normal_max > 0.0002 {
        failures.push(format!("packed normal error {normal_max}"));
    }
    println!(
        "{}",
        json!({"oracle":"encoding","pages":reader.pages.len(),"clusters":reader.clusters.len(),"groups":reader.groups.len(),"dag_depth":reader.header.root_count-1,"vertex_copies":geo.copies,"position_bit_mismatches":geo.unknown_positions,"vertex_bytes":20,"float_normal_vertex_bytes":28,"saved_bytes":geo.copies*8,"max_normal_vector_error":normal_max,"bytes_per_source_triangle":first.bytes.len() as f64/(mesh.indices.len()/3) as f64})
    );
    let original = reader
        .clusters
        .iter()
        .map(|c| c.selected(0.0))
        .collect::<Vec<_>>();
    let mut cut0 = geo
        .cut(&original)
        .into_iter()
        .map(canonical)
        .collect::<Vec<_>>();
    cut0.sort_unstable();
    let mut source = mesh
        .indices
        .chunks_exact(3)
        .map(|t| canonical([t[0], t[1], t[2]]))
        .collect::<Vec<_>>();
    source.sort_unstable();
    if cut0 != source {
        failures.push("threshold-zero oriented triangle multiset differs".into());
    }
    let source_edges = edges(&source);
    if source_edges.bad != 0 || source_edges.degenerate != 0 {
        failures.push(format!("source fixture invalid: {source_edges:?}"));
    }
    println!(
        "{}",
        json!({"oracle":"identity","source_triangles":source.len(),"cut_triangles":cut0.len(),"oriented_multisets_equal":cut0==source,"source_bad_edges":source_edges.bad})
    );
    let thresholds = [
        0.0, 0.00001, 0.00003, 0.0001, 0.0003, 0.001, 0.003, 0.01, 0.03, 0.1, 0.3, 1.0, 3.0, 10.0,
        100.0,
    ];
    let mut previous = usize::MAX;
    let mut worst_bad = 0;
    let mut worst_use = 0;
    let mut worst_overlap = 0;
    let mut evaluated_triangles = 0usize;
    for threshold in thresholds {
        let selected = reader
            .clusters
            .iter()
            .map(|c| c.selected(threshold))
            .collect::<Vec<_>>();
        let cut = geo.cut(&selected);
        let e = edges(&cut);
        let overlap = overlaps(&reader, &selected);
        worst_bad = worst_bad.max(e.bad);
        worst_use = worst_use.max(e.max_use);
        worst_overlap = worst_overlap.max(overlap);
        evaluated_triangles += cut.len();
        if e.bad > 0 || e.degenerate > 0 || overlap > 0 || cut.is_empty() {
            failures.push(format!(
                "uniform {threshold}: {e:?}, overlaps {overlap}, triangles {}",
                cut.len()
            ));
        }
        if cut.len() > previous {
            failures.push(format!(
                "triangle count increased at {threshold}: {previous} -> {}",
                cut.len()
            ));
        }
        previous = cut.len();
        println!(
            "{}",
            json!({"oracle":"uniform_cut","threshold":threshold,"triangles":cut.len(),"clusters":selected.iter().filter(|s|**s).count(),"bad_edges":e.bad,"max_edge_use":e.max_use,"overlaps":overlap})
        );
    }
    let terminal = reader
        .clusters
        .iter()
        .map(|c| c.selected(1e30))
        .collect::<Vec<_>>();
    let missing = reader
        .clusters
        .iter()
        .zip(&terminal)
        .filter(|(c, s)| **s && c.page != 0)
        .count();
    if missing > 0 {
        failures.push(format!(
            "coarse cut missing {missing} clusters from page zero"
        ));
    }
    println!(
        "{}",
        json!({"oracle":"page_zero","bytes":reader.pages[0].byte_length,"coarse_triangles":geo.cut(&terminal).len(),"missing_clusters":missing})
    );
    let mut rng = Random::new();
    let mut min_tri = usize::MAX;
    let mut max_tri = 0;
    let mut distinct = std::collections::BTreeSet::new();
    let camera_start = Instant::now();
    for camera in 0..240 {
        let direction =
            clod_bake::normalize(std::array::from_fn(|_| (rng.unit() * 2.0 - 1.0) as f32));
        let radius = (10f64.powf(rng.unit() * 2.5 - 0.5)) as f32;
        let position = direction.map(|x| x * radius);
        let fov = (20.0 + rng.unit() * 100.0) as f32;
        let cot = 1.0 / (fov.to_radians() * 0.5).tan();
        let near = (0.001 + rng.unit() * 0.099) as f32;
        let height = (720.0 + rng.unit() * 1440.0) as f32;
        let threshold = 10f64.powf(rng.unit() * 3.0 - 1.0) as f32;
        // Random unit orientation: the vendor projection formula is invariant under it.
        let q: [f64; 4] = std::array::from_fn(|_| rng.unit() * 2.0 - 1.0);
        let qlen = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        let orientation = q.map(|v| v / qlen);
        if orientation.iter().any(|v| !v.is_finite()) {
            failures.push(format!("invalid orientation {camera}"));
        }
        let selected = reader
            .clusters
            .iter()
            .map(|c| c.selected_camera(threshold, position, cot, near, height))
            .collect::<Vec<_>>();
        let cut = geo.cut(&selected);
        let e = edges(&cut);
        let overlap = overlaps(&reader, &selected);
        distinct.insert(cut.len());
        min_tri = min_tri.min(cut.len());
        max_tri = max_tri.max(cut.len());
        worst_bad = worst_bad.max(e.bad);
        worst_use = worst_use.max(e.max_use);
        worst_overlap = worst_overlap.max(overlap);
        evaluated_triangles += cut.len();
        if e.bad > 0 || e.degenerate > 0 || overlap > 0 || cut.is_empty() {
            failures.push(format!("camera {camera}, p={position:?}, q={orientation:?}, fov={fov}, threshold={threshold}: {e:?}, overlaps {overlap}, triangles {}",cut.len()));
        }
    }
    println!(
        "{}",
        json!({"oracle":"camera_cuts","cameras":240,"distinct_triangle_counts":distinct.len(),"min_triangles":min_tri,"max_triangles":max_tri,"evaluated_triangles_including_uniform":evaluated_triangles,"worst_bad_edges":worst_bad,"worst_edge_use":worst_use,"worst_ancestor_overlaps":worst_overlap,"seconds":camera_start.elapsed().as_secs_f64()})
    );
    let bvh_start = Instant::now();
    let bvh = bvh::Bvh::new(&mesh);
    println!(
        "{}",
        json!({"oracle":"source_bvh","triangles":mesh.indices.len()/3,"seconds":bvh_start.elapsed().as_secs_f64()})
    );
    for threshold in [0.001, 0.01, 0.1, 1.0] {
        let sample_start = Instant::now();
        let selected = reader
            .clusters
            .iter()
            .map(|c| c.selected(threshold))
            .collect::<Vec<_>>();
        let cut = geo.cut(&selected);
        let claimed = reader
            .clusters
            .iter()
            .zip(&selected)
            .filter(|(_, s)| **s)
            .map(|(c, _)| c.refined_bounds.error)
            .fold(0.0f32, f32::max) as f64;
        let areas = bvh::areas(&mesh, &cut);
        let area = areas.last().copied().unwrap_or(0.0);
        let mut max = 0.0f64;
        let mut sum = 0.0;
        for _ in 0..100_000 {
            let target = rng.unit() * area;
            let index = areas.partition_point(|v| *v < target).min(cut.len() - 1);
            let p = bvh::point(&mesh, cut[index], rng.unit(), rng.unit());
            let d = bvh.distance_squared(p);
            max = max.max(d);
            sum += d;
        }
        let max = max.sqrt();
        let rms = (sum / 100_000.0).sqrt();
        let limit = claimed * 4.0 + 1e-6;
        if !max.is_finite() || max > limit {
            failures.push(format!(
                "error honesty {threshold}: max {max}, claim {claimed}, limit {limit}"
            ));
        }
        println!(
            "{}",
            json!({"oracle":"error_honesty","threshold":threshold,"triangles":cut.len(),"samples":100_000,"claimed_error":claimed,"max_distance":max,"rms_distance":rms,"max_over_claim":max/claimed,"gate":limit,"seconds":sample_start.elapsed().as_secs_f64()})
        );
    }
    println!(
        "{}",
        json!({"oracle":"summary","failures":failures,"seconds":start.elapsed().as_secs_f64()})
    );
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
