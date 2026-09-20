#[path = "../../bake/tests/support/bvh.rs"]
mod bvh;
#[path = "../src/bin/prepare.rs"]
mod prepare;
#[path = "../src/bin/readback.rs"]
mod readback;
use clod_format::{Config, ORIGINAL, Reader, oracle, projection::Projection};
use clod_view::{Mode, Renderer, View, scene::Camera, scene::Scene, select::Selection};
use glam::Vec3;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn crop(pixels: &[u8]) -> Vec<u8> {
    (240..272)
        .flat_map(|y| {
            pixels[(y * 512 + 240) * 4..(y * 512 + 272) * 4]
                .iter()
                .copied()
        })
        .collect()
}
fn unit(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64
}
#[test]
fn aimed_pinch_images_and_error() {
    let mut mesh = clod_bake::procedural::octasphere_seeded(6, 0).unwrap();
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    // Freeze three valid cuts from the worst fixture, covering all four distinct
    // pinched edges seen by the sweep. Close inspection must not refine them away.
    let cases = [
        (
            36,
            2,
            1.754_066_7,
            Some(Projection {
                eye: [-11.888_74, 14.020_278, 16.240_192],
                cot: 1.708_568_6,
                near: 0.091_965_765,
                height: 1855.7228,
                orthographic_span: None,
            }),
        ),
        (9, 1, 0.1, None),
        (
            16,
            1,
            0.773_455,
            Some(Projection {
                eye: [3.717_523, -5.198_781_5, 88.913_26],
                cot: 0.607_951_46,
                near: 0.082_286_95,
                height: 1371.7046,
                orthographic_span: None,
            }),
        ),
    ];
    let mut failures = Vec::new();
    let mut unique_edges = BTreeSet::new();
    for (sweep_camera, expected_pinches, threshold, project) in cases {
        let above: Vec<_> = reader
            .groups
            .iter()
            .map(|g| {
                project.as_ref().map_or(g.simplified.error, |p| {
                    p.projected(&g.simplified, g.simplified.center, 1.0)
                }) > threshold
            })
            .collect();
        unique_edges.extend(inspect(
            &mesh,
            &reader,
            &above,
            sweep_camera,
            expected_pinches,
            &mut failures,
        ));
    }
    if unique_edges.len() != 4 {
        failures.push(format!(
            "expected four distinct pinches, got {}",
            unique_edges.len()
        ));
    }
    println!(
        "{}",
        json!({"oracle":"pinch_all","cuts":3,"unique_edges":unique_edges.len(),"edges":unique_edges,"failures":failures})
    );
    assert!(failures.is_empty(), "{failures:?}");
}
fn inspect(
    mesh: &clod_bake::Mesh,
    reader: &Reader<'_>,
    above: &[bool],
    sweep_camera: u32,
    expected_pinches: usize,
    failures: &mut Vec<String>,
) -> Vec<[u32; 2]> {
    let selected: Vec<_> = reader
        .clusters
        .iter()
        .map(|c| above[c.group as usize] && (c.refined == ORIGINAL || !above[c.refined as usize]))
        .collect();
    let ids: BTreeMap<_, _> = mesh
        .positions
        .iter()
        .enumerate()
        .map(|(i, p)| (p.map(f32::to_bits), i as u32))
        .collect();
    let mut triangles = Vec::new();
    let mut cut = Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Default::default()
    };
    let mut claimed = 0.0f32;
    for (ci, c) in reader
        .clusters
        .iter()
        .enumerate()
        .filter(|(ci, _)| selected[*ci])
    {
        cut.pages[c.page as usize].push([ci as u32, 0]);
        cut.clusters += 1;
        cut.triangles += c.triangle_count as u64;
        cut.padded_triangles += (128 - c.triangle_count) as u64;
        claimed = claimed.max(c.refined_bounds.error);
        let (vs, ts) = reader.geometry(ci).unwrap();
        triangles.extend(
            ts.chunks_exact(3).map(|t| {
                std::array::from_fn(|i| ids[&vs[t[i] as usize].position.map(f32::to_bits)])
            }),
        );
    }
    let edges = oracle::edges(&triangles);
    let overlaps = oracle::overlaps(reader, &selected);
    if edges.pinches.len() != expected_pinches || edges.bad != 0 || overlaps != 0 {
        failures.push(format!("cut changed: {edges:?}, overlaps {overlaps}"));
    }
    let tree = bvh::Bvh::new(mesh);
    let areas = bvh::areas(mesh, &triangles);
    let total = *areas.last().unwrap();
    let mut rng = 0xa512_a971_2000_0421;
    let mut max = 0.0f64;
    let mut sum = 0.0;
    for _ in 0..100_000 {
        let area_target = unit(&mut rng) * total;
        let ti = areas
            .partition_point(|a| *a < area_target)
            .min(triangles.len() - 1);
        let p = bvh::point(mesh, triangles[ti], unit(&mut rng), unit(&mut rng));
        let distance = tree.distance_squared(p);
        max = max.max(distance);
        sum += distance;
    }
    let mut edge_max = 0.0f64;
    for &[a, b] in &edges.pinches {
        for i in 0..=1024 {
            let p = std::array::from_fn(|k| {
                mesh.positions[a as usize][k] as f64 * (1.0 - i as f64 / 1024.0)
                    + mesh.positions[b as usize][k] as f64 * i as f64 / 1024.0
            });
            edge_max = edge_max.max(tree.distance_squared(p));
        }
    }
    let limit = f64::from(claimed) * 4.0 + 1e-6;
    if max.sqrt() > limit || edge_max.sqrt() > limit {
        failures.push("pinch cut exceeds sampled error gate".into());
    }
    println!(
        "{}",
        json!({"oracle":"pinch_error","subdivision":6,"seed":0,"sweep_camera":sweep_camera,"triangles":triangles.len(),"pinched_edges":edges.pinches,"nonzero_winding":edges.bad,"overlaps":overlaps,"samples":100000,"edge_samples":edges.pinches.len()*1025,"claimed_error":claimed,"max_distance":max.sqrt(),"rms_distance":(sum/100000.0).sqrt(),"edge_max_distance":edge_max.sqrt(),"max_over_claim":max.sqrt()/f64::from(claimed),"gate":limit})
    );
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(gpu) => gpu,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP pinch images: {e}; cameras=0");
            return edges.pinches;
        }
        Err(e) => panic!("{e}"),
    };
    let scene = Scene::layout(reader, "single").unwrap();
    let baseline = prepare::baseline(reader);
    let mut cluster = Renderer::new(
        device.clone(),
        queue.clone(),
        reader,
        None,
        &scene,
        [512; 2],
        Mode::Cluster,
    )
    .unwrap();
    let mut naive = Renderer::new(
        device,
        queue,
        reader,
        Some(&baseline),
        &scene,
        [512; 2],
        Mode::Naive,
    )
    .unwrap();
    cluster.shadows = false;
    naive.shadows = false;
    cluster.culling = false;
    naive.culling = false;
    let out = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out/F2/pinches")
        .join(format!("cut-{sweep_camera}"));
    std::fs::create_dir_all(&out).unwrap();
    let empty = Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Default::default()
    };
    let mut cameras = 0;
    for (edge, &[a, b]) in edges.pinches.iter().enumerate() {
        let local = (Vec3::from_array(mesh.positions[a as usize])
            + Vec3::from_array(mesh.positions[b as usize]))
            * 0.5;
        let model = scene.instances[0].transform();
        let target = model.transform_point3(local);
        let radial = model.transform_vector3(local.normalize()).normalize();
        let tangent = radial.cross(Vec3::Z).normalize();
        for angle in [-0.35f32, 0.0, 0.35] {
            let eye = target + (radial + tangent * angle).normalize() * 1.2;
            let camera = Camera::perspective(eye, target, 1.0, 45f32.to_radians(), 0.002, 100.0);
            let cf = cluster
                .render(&scene, &camera, View::Lit, &cut, &empty)
                .unwrap();
            let (cp, _) = readback::read(&cluster, &cf).unwrap();
            let nf = naive
                .render(&scene, &camera, View::Lit, &empty, &empty)
                .unwrap();
            let (np, _) = readback::read(&naive, &nf).unwrap();
            let cc = crop(&cp);
            let nc = crop(&np);
            let difference = readback::difference(&cc, &nc);
            let cf = cluster
                .render(&scene, &camera, View::Coverage, &cut, &empty)
                .unwrap();
            let (cm, _) = readback::read(&cluster, &cf).unwrap();
            let nf = naive
                .render(&scene, &camera, View::Coverage, &empty, &empty)
                .unwrap();
            let (nm, _) = readback::read(&naive, &nf).unwrap();
            let mut interior = 0;
            let mut missing = 0;
            for y in 240..272 {
                for x in 240..272 {
                    if (y - 1..=y + 1)
                        .all(|row| (x - 1..=x + 1).all(|col| nm[(row * 512 + col) * 4 + 1] == 255))
                    {
                        interior += 1;
                        missing += usize::from(cm[(y * 512 + x) * 4 + 1] == 0);
                    }
                }
            }
            if interior == 0 || missing > 0 {
                failures.push(format!(
                    "edge {edge}/angle {angle}: interior {interior}, missing {missing}"
                ));
            }
            let debug = cluster
                .render(&scene, &camera, View::Clusters, &cut, &empty)
                .unwrap();
            let (debug, _) = readback::read(&cluster, &debug).unwrap();
            // Save the original crops and a direct three-panel comparison (no resampling).
            let dc = crop(&debug);
            let panel: Vec<u8> = (0..32)
                .flat_map(|y| {
                    [&nc, &cc, &dc]
                        .into_iter()
                        .flat_map(move |p| p[y * 128..(y + 1) * 128].iter().copied())
                })
                .collect();
            for (name, pixels, w, h) in [
                ("cluster", cp, 512, 512),
                ("naive", np, 512, 512),
                ("crops", panel, 96, 32),
            ] {
                std::fs::write(
                    out.join(format!("edge-{edge}-camera-{cameras}-{name}.png")),
                    readback::png_bytes(&pixels, w, h).unwrap(),
                )
                .unwrap();
            }
            println!(
                "{}",
                json!({"oracle":"pinch_image","sweep_camera":sweep_camera,"edge":edge,"vertices":[a,b],"camera":cameras,"angle":angle,"eye":eye.to_array(),"target":target.to_array(),"window":[240,240,32,32],"mean_abs":difference.mean,"max_byte":difference.max,"fraction_gt_2":difference.fraction,"interior_pixels":interior,"missing":missing,"frozen_cut_triangles":cut.triangles})
            );
            cameras += 1;
        }
    }
    println!(
        "{}",
        json!({"oracle":"pinch_summary","sweep_camera":sweep_camera,"cameras":cameras,"pinched_edges":edges.pinches.len(),"failures":failures})
    );
    edges.pinches
}
