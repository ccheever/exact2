#[path = "../src/bin/oracle.rs"]
mod oracle;
#[path = "../src/bin/readback.rs"]
mod readback;
#[path = "../src/bin/selection_readback.rs"]
mod selection_readback;

#[test]
fn selection_oracles() {
    let (device, queue, adapter) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(result) => result,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP GPU selection oracles: {e}; cameras=0");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    println!("adapter={} features={:?}", adapter.name, device.features());
    let mut mesh = clod_bake::procedural::octasphere(8).expect("mesh");
    let bake = clod_bake::bake(
        &mut mesh,
        clod_format::Config {
            page_bytes: 256 * 1024,
            ..Default::default()
        },
        [0; 32],
    )
    .expect("bake");
    let reader = clod_format::Reader::new(&bake.bytes).expect("reader");
    // Exercise the shared RGB difference metric as well as exact equality in the sweep.
    let zero = readback::difference(&[0, 0, 0, 255], &[0, 0, 0, 255]);
    assert_eq!(zero.max, 0);
    println!("difference_mean={} fraction={}", zero.mean, zero.fraction);
    oracle::run(&reader, &device, &queue, [192, 192], 64, true).expect("GPU oracles");
}

#[test]
fn scan_shadow_culling_preserves_depth() {
    use clod_view::{Mode, Renderer, View, scene::Scene, select};
    let path = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join("Library/Caches/exact2-cluster-lod/out/washington-2.clod");
    if !path.exists() {
        eprintln!("SKIP scan shadow culling: asset absent; comparisons=0");
        return;
    }
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(gpu) => gpu,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP scan shadow culling: {e}; comparisons=0");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let bytes = std::fs::read(path).unwrap();
    let reader = clod_format::Reader::new(&bytes).unwrap();
    let scene = Scene::layout(&reader, "grid:400").unwrap();
    let camera = scene.camera(0.0, 1.0, 45f32.to_radians());
    let light = scene.light_camera();
    let full = select::select_culled(&reader, &scene.instances, &light, 2048, 2.0, false);
    let culled = select::select(&reader, &scene.instances, &light, 2048, 2.0);
    let mut spheres = select::Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Default::default()
    };
    for (out, pairs) in spheres.pages.iter_mut().zip(&full.pages) {
        for &[id, instance] in pairs {
            let c = &reader.clusters[id as usize];
            let i = &scene.instances[instance as usize];
            if select::sphere_visible(c.sphere, i.transform(), i.scale(), &light.planes()) {
                out.push([id, instance]);
                spheres.clusters += 1;
                spheres.triangles += c.triangle_count as u64;
                spheres.padded_triangles += (128 - c.triangle_count) as u64;
            }
        }
    }
    let mut renderer = Renderer::new(
        device,
        queue,
        &reader,
        None,
        &scene,
        [32, 32],
        Mode::Cluster,
    )
    .unwrap();
    let mut reference = Vec::new();
    let mut failures = Vec::new();
    let empty = select::Selection {
        pages: vec![Vec::new(); reader.pages.len()],
        ..Default::default()
    };
    for (name, cut) in [
        ("unculled", &full),
        ("spheres", &spheres),
        ("configured", &culled),
    ] {
        let frame = renderer
            .render(&scene, &camera, View::Lit, &empty, cut)
            .unwrap();
        let _ = readback::read(&renderer, &frame).unwrap();
        let depth = selection_readback::shadow(&renderer).unwrap();
        if reference.is_empty() {
            reference = depth.clone();
        }
        let differences: Vec<_> = reference.chunks_exact(4).zip(depth.chunks_exact(4))
            .enumerate().filter(|(_, (a,b))| a != b)
            .map(|(pixel,(a,b))| serde_json::json!({"pixel":pixel,"unculled":f32::from_le_bytes(a.try_into().unwrap()),"culled":f32::from_le_bytes(b.try_into().unwrap()),"bits":[u32::from_le_bytes(a.try_into().unwrap()),u32::from_le_bytes(b.try_into().unwrap())]})).collect();
        println!(
            "{}",
            serde_json::json!({"oracle":"shadow_culling","mode":name,"clusters":cut.clusters,"triangles":cut.triangles,"changed_pixels":differences.len(),"differences":differences})
        );
        if !differences.is_empty() {
            failures.push(name);
        }
    }
    println!("shadow_culling_comparisons=2 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}
