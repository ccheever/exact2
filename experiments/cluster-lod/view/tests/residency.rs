#[path = "../src/bin/readback.rs"]
mod readback;
#[path = "../src/bin/selection_readback.rs"]
mod selection_readback;
use clod_format::{Config, Reader, oracle};
use clod_view::{Mode, Renderer, View, residency, scene::Scene, select};
use std::collections::BTreeMap;

#[test]
fn resident_cuts_preserve_chains_and_scan_coverage() {
    let mut mesh = clod_bake::procedural::octasphere(6).unwrap();
    let bake = clod_bake::bake(
        &mut mesh,
        Config {
            page_bytes: 65536,
            ..Config::default()
        },
        [0; 32],
    )
    .unwrap();
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(gpu) => gpu,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            println!("SKIP residency GPU: {e}; GPU frames=0");
            // CPU chain test still runs below.
            cpu_chains(&bake.bytes);
            return;
        }
        Err(e) => panic!("{e}"),
    };
    cpu_chains(&bake.bytes);
    let cache = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out/gaul-4.clod");
    let mut failures = Vec::new();
    let mut total_frames = 0;
    for (name, bytes) in [
        ("procedural", Some(bake.bytes)),
        ("gaul", std::fs::read(cache).ok()),
    ] {
        let Some(bytes) = bytes else {
            println!("SKIP {name}: missing asset; frames=0");
            continue;
        };
        let r = Reader::new(&bytes).unwrap();
        let metadata = Reader::metadata(&bytes[..r.header.geometry_offset as usize]).unwrap();
        let scene = Scene::layout(&r, "single").unwrap();
        let mut gpu = Renderer::new(
            device.clone(),
            queue.clone(),
            &metadata,
            None,
            &scene,
            [256; 2],
            Mode::Cluster,
        )
        .unwrap();
        gpu.enable_gpu_selection(&metadata, None).unwrap();
        let mut cpu = Renderer::new(
            device.clone(),
            queue.clone(),
            &r,
            None,
            &scene,
            [256; 2],
            Mode::Cluster,
        )
        .unwrap();
        let mut frames = 0;
        let mut interior = 0;
        let mut missing = 0;
        let mut pixels_different = 0;
        let mut max_mean = 0.0f64;
        let mut max_fraction = 0.0f64;
        for prefix in 1..=r.pages.len() {
            gpu.upload_page(&metadata, prefix - 1, r.page_bytes(prefix - 1).unwrap())
                .unwrap();
            let mask: Vec<_> = (0..r.pages.len()).map(|i| i < prefix).collect();
            let ready = residency::groups(&r, &mask).unwrap();
            gpu.set_residency(&r, &mask).unwrap();
            for step in 0..4 {
                let camera = scene.camera(step as f32 / 3.0, 1.0, 45f32.to_radians());
                let cut =
                    select::select_resident(&r, &scene.instances, &camera, 256, 1.0, true, &ready);
                let gf = gpu
                    .render_gpu(&scene, &camera, View::Coverage, 1.0, true, false)
                    .unwrap();
                let gp = readback::read(&gpu, &gf).unwrap().0;
                let actual =
                    selection_readback::selections(&gpu, r.pages.len(), 128, true).unwrap();
                let cf = cpu
                    .render(&scene, &camera, View::Coverage, &cut, &cut)
                    .unwrap();
                let cp = readback::read(&cpu, &cf).unwrap().0;
                let difference = readback::difference(&gp, &cp);
                max_mean = max_mean.max(difference.mean);
                max_fraction = max_fraction.max(difference.fraction);
                pixels_different += gp
                    .chunks_exact(4)
                    .zip(cp.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count();
                if actual[0].pages != cut.pages || actual[0].overflow != 0 || difference.max != 0 {
                    failures.push(format!(
                        "{name} prefix={prefix} step={step}: cut/image mismatch max={} overflow={}",
                        difference.max, actual[0].overflow
                    ));
                }
                for y in 1..255 {
                    for x in 1..255 {
                        if (y - 1..=y + 1)
                            .all(|a| (x - 1..=x + 1).all(|b| cp[(a * 256 + b) * 4 + 1] == 255))
                        {
                            interior += 1;
                            missing += usize::from(gp[(y * 256 + x) * 4 + 1] == 0);
                        }
                    }
                }
                frames += 1;
            }
        }
        let image_dir = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join("Library/Caches/exact2-cluster-lod/out/web");
        std::fs::create_dir_all(&image_dir).unwrap();
        let camera = scene.camera(0.5, 1.0, 45f32.to_radians());
        let cut = select::select(&r, &scene.instances, &camera, 256, 1.0);
        let shadow = select::select(
            &r,
            &scene.instances,
            &scene.light_camera(),
            clod_view::SHADOW_SIZE,
            2.0,
        );
        let gf = gpu
            .render_gpu(&scene, &camera, View::Lit, 1.0, true, false)
            .unwrap();
        let pixels = readback::read(&gpu, &gf).unwrap().0;
        std::fs::write(
            image_dir.join(format!("residency-{name}.png")),
            readback::png_bytes(&pixels, 256, 256).unwrap(),
        )
        .unwrap();
        let cf = cpu
            .render(&scene, &camera, View::Lit, &cut, &shadow)
            .unwrap();
        readback::read(&cpu, &cf).unwrap();
        if selection_readback::shadow(&gpu).unwrap() != selection_readback::shadow(&cpu).unwrap() {
            failures.push(format!("{name}: shadow mismatch"));
        }
        total_frames += frames;
        println!(
            "residency asset={name} pages={} frames={frames} interior={interior} missing={missing} pixels_different={pixels_different} max_mean={max_mean} max_fraction={max_fraction} shadow_pairs=1",
            r.pages.len()
        );
        if interior == 0 || missing != 0 {
            failures.push(format!(
                "{name}: coverage interior={interior} missing={missing}"
            ));
        }
    }
    println!("residency GPU frames={total_frames} failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}
fn cpu_chains(bytes: &[u8]) {
    let r = Reader::new(bytes).unwrap();
    let scene = Scene::layout(&r, "single").unwrap();
    let mut seed = 719u32;
    let mut failures = Vec::new();
    let mut triangles = 0;
    let mut bad = 0;
    let mut overlaps = 0;
    for case in 0..96 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let prefix = 1 + seed as usize % r.pages.len();
        let mut mask: Vec<_> = (0..r.pages.len()).map(|p| p < prefix).collect();
        if case >= 64 {
            for p in &mut mask[1..] {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *p = seed & 4 != 0;
            }
        }
        let ready = residency::groups(&r, &mask).unwrap();
        let camera = scene.camera(case as f32 / 95.0, 1.0, 45f32.to_radians());
        let cut = select::select_resident(
            &r,
            &scene.instances,
            &camera,
            512,
            [0., 1., 8., 32.][case % 4],
            false,
            &ready,
        );
        let mut ids = BTreeMap::new();
        let mut ts = Vec::new();
        let mut selected = vec![false; r.clusters.len()];
        for pair in cut.pages.iter().flatten() {
            let id = pair[0] as usize;
            selected[id] = true;
            if !mask[r.clusters[id].page as usize] {
                failures.push(format!("unavailable cluster {id}"));
            }
            let (vs, is) = r.geometry(id).unwrap();
            for t in is.chunks_exact(3) {
                ts.push(std::array::from_fn(|i| {
                    let key = vs[t[i] as usize].position.map(f32::to_bits);
                    let next = ids.len() as u32;
                    *ids.entry(key).or_insert(next)
                }));
            }
        }
        let edges = oracle::edges(&ts);
        triangles += ts.len();
        bad += edges.bad;
        overlaps += oracle::overlaps(&r, &selected);
    }
    if bad != 0 || overlaps != 0 || triangles == 0 {
        failures.push(format!(
            "chain={bad} overlaps={overlaps} triangles={triangles}"
        ));
    }
    println!(
        "residency CPU pages={} cuts=96 random_prefixes=64 arbitrary_masks=32 triangles={triangles} winding_errors={bad} overlaps={overlaps} failures={failures:?}",
        r.pages.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
}
