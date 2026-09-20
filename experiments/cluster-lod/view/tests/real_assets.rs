#[path = "../src/bin/prepare.rs"]
mod prepare;
#[path = "../src/bin/readback.rs"]
mod readback;
use clod_format::{ORIGINAL, Reader};
use clod_view::{
    Baseline, Mode, Renderer, View,
    scene::Scene,
    select::{Selection, select},
};
#[test]
fn real_asset_raster_oracles() {
    let mut failures = Vec::new();
    let mut comparisons = 0;
    let mut assets = 0;
    let out = std::path::PathBuf::from(std::env::var("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out");
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(result) => result,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("SKIP real-asset GPU tests: {e}; 0 comparisons\n"),
            );
            return;
        }
        Err(e) => panic!("{e}"),
    };
    for asset in ["gaul", "washington"] {
        let path = out.join(format!("{asset}-1.clod"));
        if !path.exists() {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!(
                    "SKIP optional real asset {}: baked fixture absent\n",
                    path.display()
                ),
            );
            continue;
        }
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                failures.push(format!("{asset}: {e}"));
                continue;
            }
        };
        let reader = match Reader::new(&bytes) {
            Ok(reader) => reader,
            Err(e) => {
                failures.push(format!("{asset}: {e}"));
                continue;
            }
        };
        let scene = Scene::layout(&reader, "single").unwrap();
        assets += 1;
        let mut raw = Vec::new();
        for page in reader.pages {
            let mut mesh = Baseline {
                vertices: Vec::new(),
                indices: Vec::new(),
            };
            for id in page.first_cluster..page.first_cluster + page.cluster_count {
                if reader.clusters[id as usize].refined != ORIGINAL {
                    continue;
                }
                let (v, i) = reader.geometry(id as usize).unwrap();
                let base = mesh.vertices.len() as u32;
                mesh.vertices.extend_from_slice(v);
                mesh.indices.extend(i.iter().map(|i| base + *i as u32));
            }
            if !mesh.indices.is_empty() {
                raw.push(mesh);
            }
        }
        let baseline = prepare::baseline(&reader);
        let mut c = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            None,
            &scene,
            [2560, 1440],
            Mode::Cluster,
        )
        .unwrap();
        let mut optimized = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            Some(&baseline),
            &scene,
            [2560, 1440],
            Mode::Naive,
        )
        .unwrap();
        let mut ordered = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            Some(&raw),
            &scene,
            [2560, 1440],
            Mode::Naive,
        )
        .unwrap();
        for t in [0.0, 0.5, 1.0] {
            let camera = scene.camera(t, 16.0 / 9.0, 45f32.to_radians());
            let selected = select(&reader, &scene.instances, &camera, 1440, 0.0);
            let shadow = select(&reader, &scene.instances, &scene.light_camera(), 2048, 0.0);
            let cf = c
                .render(&scene, &camera, View::Lit, &selected, &shadow)
                .unwrap();
            let (cp, _) = readback::read(&c, &cf).unwrap();
            for (name, r) in [
                ("cache_optimized", &mut optimized),
                ("same_triangle_order", &mut ordered),
            ] {
                let f = r
                    .render(
                        &scene,
                        &camera,
                        View::Lit,
                        &Selection::default(),
                        &Selection::default(),
                    )
                    .unwrap();
                let (p, _) = readback::read(r, &f).unwrap();
                let d = readback::difference(&cp, &p);
                comparisons += 1;
                let different_pixels = (d.fraction * 2560.0 * 1440.0).round() as u64;
                let ok = if name == "same_triangle_order" {
                    d.max == 0
                } else {
                    d.max <= 20 && d.mean < 1e-7 && different_pixels <= 8
                };
                if !ok {
                    failures.push(format!(
                        "{asset} t={t} {name}: max={}, mean={}, >2 pixels={different_pixels}",
                        d.max, d.mean
                    ));
                }
                println!(
                    "{}",
                    serde_json::json!({"oracle":"real_asset_zero","asset":asset,"t":t,"baseline":name,"max_byte":d.max,"mean_abs":d.mean,"fraction_gt_2":d.fraction,"pixels_gt_2":different_pixels,"png_bytes":readback::png_bytes(&p,2560,1440).unwrap().len()})
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"oracle":"real_asset_summary","assets":assets,"comparisons":comparisons,"gpu_frames":comparisons/2*3,"failures":failures})
    );
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
