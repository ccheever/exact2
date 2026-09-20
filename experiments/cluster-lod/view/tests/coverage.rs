#[path = "../src/bin/prepare.rs"]
mod prepare;
#[path = "../src/bin/readback.rs"]
mod readback;
use clod_format::Reader;
use clod_view::{
    Mode, Renderer, View,
    scene::{AssetBounds, Camera, Scene},
    select::{self, Selection},
};
use glam::Vec3;
use serde_json::json;
use std::collections::BTreeSet;

// Both images are white geometry over saturated magenta, with no ground/shadows.
// Partial MSAA coverage is silhouette, too. Erode the fully covered naive mask
// by a one-pixel Chebyshev band. A crack inside
// that mask remains eligible, including a single missing pixel.
fn missing(cluster: &[u8], naive: &[u8], size: usize) -> (usize, usize, Option<usize>) {
    let mut interior = 0;
    let mut holes = 0;
    let mut first = None;
    for y in 1..size - 1 {
        for x in 1..size - 1 {
            if (y - 1..=y + 1)
                .all(|row| (x - 1..=x + 1).all(|col| naive[(row * size + col) * 4 + 1] == 255))
            {
                interior += 1;
                let i = y * size + x;
                first.get_or_insert(i);
                holes += usize::from(cluster[i * 4 + 1] == 0);
            }
        }
    }
    (interior, holes, first)
}
#[test]
fn real_scan_localized_coverage() {
    let out = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out");
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(gpu) => gpu,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP coverage: {e}; assets=0 cameras=0");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let mut failures = Vec::new();
    let mut assets = 0;
    let size = 512;
    for asset in ["gaul", "washington"] {
        let path = out.join(format!("{asset}-5.clod"));
        if !path.exists() {
            eprintln!("SKIP coverage {asset}: asset absent; cameras=0");
            continue;
        }
        let bytes = std::fs::read(path).unwrap();
        let reader = match Reader::new(&bytes) {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{asset}: {e}"));
                continue;
            }
        };
        assets += 1;
        let baseline = prepare::baseline(&reader);
        let bounds = AssetBounds::read(&reader);
        let mut cameras = 0;
        let mut mixed = 0;
        let mut interior_total = 0;
        let mut holes_total = 0;
        let mut controls = 0;
        for layout in ["single", "ring:12", "grid:400"] {
            let scene = Scene::layout(&reader, layout).unwrap();
            let mut cluster = Renderer::new(
                device.clone(),
                queue.clone(),
                &reader,
                None,
                &scene,
                [size as u32; 2],
                Mode::Cluster,
            )
            .unwrap();
            cluster.enable_gpu_selection(&reader, None).unwrap();
            let mut naive = Renderer::new(
                device.clone(),
                queue.clone(),
                &reader,
                Some(&baseline),
                &scene,
                [size as u32; 2],
                Mode::Naive,
            )
            .unwrap();
            for step in 0..16 {
                let camera = if layout == "single" {
                    scene.camera(step as f32 / 15.0, 1.0, 45f32.to_radians())
                } else {
                    let id = if layout == "ring:12" {
                        step % 12
                    } else {
                        190 + (step % 4) * 20 + step / 4
                    };
                    let target = scene.instances[id]
                        .transform()
                        .transform_point3(bounds.center());
                    let eye = if layout == "ring:12" {
                        scene.center + Vec3::new(0.2, -0.3, 0.15)
                    } else {
                        target + Vec3::new(-1.3, -1.6, 0.15)
                    };
                    Camera::perspective(
                        eye,
                        target,
                        1.0,
                        65f32.to_radians(),
                        0.002,
                        scene.radius * 12.0 + 10.0,
                    )
                };
                let selected = select::select(&reader, &scene.instances, &camera, size as u32, 1.0);
                let depths: BTreeSet<_> = selected
                    .pages
                    .iter()
                    .flatten()
                    .map(|p| reader.clusters[p[0] as usize].depth)
                    .collect();
                let cf = cluster
                    .render_gpu(&scene, &camera, View::Coverage, 1.0, true, false)
                    .unwrap();
                let (cp, _) = readback::read(&cluster, &cf).unwrap();
                let nf = naive
                    .render(
                        &scene,
                        &camera,
                        View::Coverage,
                        &Selection::default(),
                        &Selection::default(),
                    )
                    .unwrap();
                let (np, _) = readback::read(&naive, &nf).unwrap();
                let (interior, holes, first) = missing(&cp, &np, size);
                let is_mixed = depths.len() > 1 && interior > 0;
                mixed += usize::from(is_mixed);
                cameras += 1;
                interior_total += interior;
                holes_total += holes;
                if holes > 0 {
                    let locations: Vec<_> = (1..size - 1)
                        .flat_map(|y| (1..size - 1).map(move |x| (x, y)))
                        .filter(|&(x, y)| {
                            cp[(y * size + x) * 4 + 1] == 0
                                && (y - 1..=y + 1).all(|row| {
                                    (x - 1..=x + 1).all(|col| np[(row * size + col) * 4 + 1] > 0)
                                })
                        })
                        .collect();
                    for (x, y) in locations {
                        let neighbors: Vec<_> = (y - 1..=y + 1)
                            .flat_map(|row| (x - 1..=x + 1).map(move |col| (row, col)))
                            .map(|(row, col)| np[(row * size + col) * 4 + 1])
                            .collect();
                        println!(
                            "{}",
                            json!({"oracle":"coverage_failure_location","asset":asset,"camera":step,"x":x,"y":y,"naive_green_3x3":neighbors})
                        );
                    }
                    for (threshold, cull) in [(1.0, false), (0.5, true), (0.0, true)] {
                        let f = cluster
                            .render_gpu(&scene, &camera, View::Coverage, threshold, cull, false)
                            .unwrap();
                        let (p, _) = readback::read(&cluster, &f).unwrap();
                        println!(
                            "{}",
                            json!({"oracle":"coverage_diagnosis","asset":asset,"camera":step,"threshold":threshold,"cull":cull,"missing":missing(&p,&np,size).1})
                        );
                    }
                    failures.push(format!("{asset}/{layout}/{step}: {holes} interior holes"));
                    for (name, pixels) in [("cluster", &cp), ("naive", &np)] {
                        std::fs::write(
                            out.join("F1").join(format!(
                                "coverage-{asset}-{}-{step}-{name}.png",
                                layout.replace(':', "-")
                            )),
                            readback::png_bytes(pixels, size as u32, size as u32).unwrap(),
                        )
                        .unwrap();
                    }
                }
                if controls == 0
                    && let Some(i) = first
                {
                    let mut cracked = np.clone();
                    cracked[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 255, 255]);
                    let d = readback::difference(&cracked, &np);
                    let (_, detected, _) = missing(&cracked, &np, size);
                    println!(
                        "{}",
                        json!({"oracle":"one_pixel_crack_control","asset":asset,"mean":d.mean,"max_byte":d.max,"fraction":d.fraction,"old_gate_accepts":d.mean<0.008 && d.fraction<0.20,"missing_pixels":detected})
                    );
                    if detected != 1 {
                        failures.push(format!("{asset}: injected one-pixel crack escaped"));
                    }
                    controls += 1;
                }
                println!(
                    "{}",
                    json!({"oracle":"coverage_camera","asset":asset,"layout":layout,"camera":step,"depths":depths,"mixed":is_mixed,"interior_pixels":interior,"missing_pixels":holes,"cluster_triangles":selected.triangles,"naive_triangles":nf.stats.triangles})
                );
            }
        }
        if mixed < 32 || controls != 1 {
            failures.push(format!(
                "{asset}: only {mixed} mixed nonempty cameras; controls={controls}"
            ));
        }
        println!(
            "{}",
            json!({"oracle":"coverage_asset","asset":asset,"cameras":cameras,"mixed_cameras":mixed,"interior_pixels":interior_total,"missing_pixels":holes_total,"controls":controls})
        );
    }
    println!(
        "{}",
        json!({"oracle":"coverage_summary","assets":assets,"failures":failures})
    );
    assert!(failures.is_empty(), "{failures:?}");
}
