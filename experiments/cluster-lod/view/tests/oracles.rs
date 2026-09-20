#[path = "../src/bin/prepare.rs"]
mod prepare;
#[path = "../src/bin/readback.rs"]
mod readback;
use clod_format::{Bounds, Config, Reader};
use clod_view::{
    Mode, Renderer, View, device_descriptor,
    scene::{Camera, Instance, Scene},
    select::{self, Selection},
    wgpu,
};
use glam::{Mat4, Quat, Vec3};
use serde_json::json;

#[test]
fn image_oracles() {
    let mut failures = Vec::new();
    let mut check = |ok: bool, message: &str| {
        if !ok {
            failures.push(message.to_owned())
        }
    };
    let descriptor = device_descriptor(false);
    check(
        descriptor.required_features == wgpu::Features::empty(),
        "nonempty features",
    );
    check(
        descriptor.required_limits == wgpu::Limits::default(),
        "nondefault limits",
    );
    check(
        device_descriptor(true).required_features == wgpu::Features::TIMESTAMP_QUERY,
        "timing feature set",
    );
    println!(
        "{}",
        json!({"oracle":"descriptor","features":format!("{:?}",descriptor.required_features),"storage_buffers":descriptor.required_limits.max_storage_buffers_per_shader_stage,"storage_bytes":descriptor.required_limits.max_storage_buffer_binding_size,"max_buffer":descriptor.required_limits.max_buffer_size})
    );
    let camera = Camera::perspective(Vec3::new(0.0, -5.0, 2.0), Vec3::ZERO, 1.0, 1.0, 0.01, 100.0);
    let bounds = Bounds {
        center: [0.0; 3],
        radius: 1.0,
        error: 0.1,
    };
    let base = select::projected(&bounds, Mat4::IDENTITY, 1.0, &camera, 512);
    let transformed = Instance::new(Vec3::new(3.0, 7.0, 1.0), Quat::from_rotation_z(0.6), 2.5);
    let model = transformed.transform();
    let camera2 = Camera {
        eye: model.transform_point3(camera.eye),
        near: camera.near * 2.5,
        ..camera
    };
    let scaled = select::projected(&bounds, model, 2.5, &camera2, 512);
    check((base - scaled).abs() < 1e-4, "scaled projection mismatch");
    println!(
        "{}",
        json!({"oracle":"transform_projection","original":base,"transformed":scaled,"difference":(base-scaled).abs()})
    );
    let (device, queue, adapter) = match pollster::block_on(clod_view::request_device(false)) {
        Ok(result) => result,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!(
                    "SKIP GPU ORACLES: {e}; executed descriptor and transform tests, 0 images\n"
                ),
            );
            assert!(failures.is_empty(), "{failures:?}");
            return;
        }
        Err(e) => panic!("device request failed: {e}"),
    };
    check(
        device.features().is_empty(),
        "device enabled extra features",
    );
    println!(
        "{}",
        json!({"oracle":"adapter","name":adapter.name,"backend":format!("{:?}",adapter.backend)})
    );
    let mut mesh = clod_bake::procedural::octasphere(8).expect("procedural mesh");
    let baked = clod_bake::bake(
        &mut mesh,
        Config {
            page_bytes: 1024 * 1024,
            ..Config::default()
        },
        [0; 32],
    )
    .expect("bake");
    let reader = Reader::new(&baked.bytes).expect("reader");
    let scene = Scene::layout(&reader, "single").expect("scene");
    let moved_camera = Camera {
        matrix: camera.matrix * model.inverse(),
        ..camera2
    };
    let mut transformed_cuts = 0;
    for threshold in [0.0, 0.25, 1.0, 4.0, 32.0] {
        let original = select::select(
            &reader,
            &[Instance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)],
            &camera,
            512,
            threshold,
        );
        let moved = select::select(&reader, &[transformed], &moved_camera, 512, threshold);
        check(
            original.pages == moved.pages,
            "rotation/translation/scale changes equivalent frustum cut",
        );
        transformed_cuts += 1;
    }
    println!(
        "{}",
        json!({"oracle":"transformed_culling","cuts":transformed_cuts,"scale":2.5})
    );

    let baseline = prepare::baseline(&reader);
    let size = 384;
    let mut cluster = Renderer::new(
        device.clone(),
        queue.clone(),
        &reader,
        None,
        &scene,
        [size, size],
        Mode::Cluster,
    )
    .expect("cluster renderer");
    let mut naive = Renderer::new(
        device,
        queue,
        &reader,
        Some(&baseline),
        &scene,
        [size, size],
        Mode::Naive,
    )
    .expect("naive renderer");
    let camera = scene.camera(0.25, 1.0, 45f32.to_radians());
    let draw = |renderer: &mut Renderer, threshold: f32, view: View| {
        let selection = if renderer.mode == Mode::Cluster {
            select::select(&reader, &scene.instances, &camera, size, threshold)
        } else {
            Selection::default()
        };
        let shadow = if renderer.mode == Mode::Cluster {
            select::select(
                &reader,
                &scene.instances,
                &scene.light_camera(),
                clod_view::SHADOW_SIZE,
                threshold * 2.0,
            )
        } else {
            Selection::default()
        };
        let frame = renderer
            .render(&scene, &camera, view, &selection, &shadow)
            .expect("render");
        let (pixels, _) = readback::read(renderer, &frame).expect("readback");
        (pixels, frame.stats)
    };
    let (c0, s0) = draw(&mut cluster, 0.0, View::Lit);
    let (n0, _) = draw(&mut naive, 0.0, View::Lit);
    let d = readback::difference(&c0, &n0);
    println!(
        "{}",
        json!({"oracle":"threshold_zero","source_triangles":reader.header.source_triangles,"cluster_triangles":s0.triangles,"pages":reader.pages.len(),"draws":s0.draws,"max_byte":d.max,"mean_abs":d.mean,"fraction_gt_2":d.fraction})
    );
    check(d.max == 0, "threshold-zero must be pixel-exact");
    let (repeat, _) = draw(&mut cluster, 0.0, View::Lit);
    let png_a = readback::png_bytes(&c0, size, size).expect("png");
    let png_b = readback::png_bytes(&repeat, size, size).expect("png");
    check(png_a == png_b, "nondeterministic PNG");
    println!(
        "{}",
        json!({"oracle":"repeat_png","bytes":png_a.len(),"identical":png_a==png_b})
    );
    let (c1, s1) = draw(&mut cluster, 1.0, View::Lit);
    check(
        s1.triangles < s0.triangles / 2,
        "LOD must reduce procedural triangles by at least 50% at 1 px",
    );
    let d = readback::difference(&c1, &n0);
    println!(
        "{}",
        json!({"oracle":"one_pixel","max_byte":d.max,"mean_abs":d.mean,"fraction_gt_2":d.fraction,"triangles":s1.triangles,"padding":s1.padded_triangles,"draws":s1.draws})
    );
    check(
        d.mean < 0.008 && d.fraction < 0.20,
        "one-pixel image exceeds measured-envelope bound",
    );
    check(
        s0.draws == s1.draws,
        "draw count depends on selected clusters",
    );
    let mut debug_count = 0;
    for view in [
        View::Clusters,
        View::Depth,
        View::Triangles,
        View::Instances,
        View::Overdraw,
    ] {
        let (pixels, _) = draw(&mut cluster, 1.0, view);
        let diff = readback::difference(&pixels, &c1);
        debug_count += 1;
        check(
            diff.mean > 0.001,
            &format!("{view:?} indistinguishable from lit"),
        );
        println!(
            "{}",
            json!({"oracle":"debug_view","view":format!("{view:?}"),"mean_abs":diff.mean})
        );
    }
    println!(
        "{}",
        json!({"oracle":"image_summary","images":9,"debug_views":debug_count,"shader_files_validated_at_build":2,"failures":failures})
    );
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn grid_distance_depth_and_scene_contract() {
    let mut mesh = clod_bake::Mesh::default();
    for (y, color) in [(0.01, [0, 255, 0, 255]), (0.0, [255, 0, 0, 255])] {
        let base = mesh.positions.len() as u32;
        mesh.positions
            .extend([[-20.0, y, -20.0], [20.0, y, -20.0], [0.0, y, 20.0]]);
        mesh.indices.extend([base, base + 1, base + 2]);
        mesh.colors.get_or_insert_with(Vec::new).extend([color; 3]);
    }
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    let mut scene = Scene::layout(&reader, "single").unwrap();
    scene.instances = vec![Instance::new(Vec3::ZERO, Quat::IDENTITY, 1.0)];
    let camera = Camera::perspective(
        Vec3::new(0.0, -100.0, 0.0),
        Vec3::ZERO,
        1.0,
        45f32.to_radians(),
        0.002,
        1000.0,
    );
    let depths = [0.0, 0.01].map(|y| camera.matrix.project_point3(Vec3::new(0.0, y, 0.0)).z);
    let mut failures = Vec::new();
    if depths[0] == depths[1] {
        failures.push("depth separation rounds to the same float".into());
    }
    let (device, queue, _) = pollster::block_on(clod_view::request_device(false)).unwrap();
    let baseline = prepare::baseline(&reader);
    for mode in [Mode::Cluster, Mode::Naive] {
        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            Some(&baseline),
            &scene,
            [64, 64],
            mode,
        )
        .unwrap();
        let cut = select::select_culled(&reader, &scene.instances, &camera, 64, 0.0, false);
        let frame = renderer
            .render(&scene, &camera, View::Lit, &cut, &cut)
            .unwrap();
        let (pixels, _) = readback::read(&renderer, &frame).unwrap();
        let p = &pixels[(24 * 64 + 32) * 4..(24 * 64 + 32) * 4 + 4];
        println!(
            "depth mode={mode:?} distance=100 separation=0.01 projected={depths:?} pixel={p:?}"
        );
        if p[0] <= p[1] {
            failures.push(format!("depth {mode:?} far green wins {p:?}"));
        }
    }
    scene.instances[0].matrix = Mat4::from_scale(Vec3::new(0.5, 2.0, 2.0)).to_cols_array();
    let rejects = Renderer::new(
        device.clone(),
        queue.clone(),
        &reader,
        Some(&baseline),
        &scene,
        [16, 16],
        Mode::Naive,
    )
    .is_err();
    println!("nonuniform_scene_rejected={rejects}");
    if !rejects {
        failures.push("nonuniform scene accepted".into());
    }
    scene.instances = vec![
        Instance::new(Vec3::ZERO, Quat::IDENTITY, 1.0),
        Instance::new(Vec3::splat(10000.0), Quat::IDENTITY, 1.0),
    ];
    let mut renderer = Renderer::new(
        device,
        queue,
        &reader,
        Some(&baseline),
        &scene,
        [16, 16],
        Mode::Naive,
    )
    .unwrap();
    let frame = renderer
        .render(
            &scene,
            &camera,
            View::Lit,
            &Selection::default(),
            &Selection::default(),
        )
        .unwrap();
    println!(
        "naive_cull instances=2 expected_triangles=2 actual={}",
        frame.stats.triangles
    );
    if frame.stats.triangles != 2 {
        failures.push("naive instance culling absent".into());
    }
    let mut zero = clod_bake::Mesh {
        positions: vec![[0.0; 3]; 3],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let zero = clod_bake::bake(&mut zero, Config::default(), [0; 32]).unwrap();
    let rejects = Scene::layout(&Reader::new(&zero.bytes).unwrap(), "single").is_err();
    println!("zero_extent_rejected={rejects}");
    if !rejects {
        failures.push("zero extent accepted".into());
    }
    println!("regression_cases=5 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn baseline_limits_precede_gpu_allocation() {
    let mut mesh = clod_bake::procedural::octasphere(0).unwrap();
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let mut reader = Reader::new(&baked.bytes).unwrap();
    let scene = Scene::layout(&reader, "single").unwrap();
    let mut baseline = prepare::baseline(&reader);
    let (device, queue, _) = pollster::block_on(clod_view::request_device(false)).unwrap();
    let oversized = vec![
        reader.clusters[0];
        128 * 1024 * 1024 / std::mem::size_of::<clod_format::Cluster>() + 1
    ];
    let original = reader.clusters;
    reader.clusters = &oversized;
    let accepted = Renderer::new(
        device.clone(),
        queue.clone(),
        &reader,
        Some(&baseline),
        &scene,
        [16, 16],
        Mode::Naive,
    )
    .is_ok();
    println!(
        "naive_metadata_bytes={} accepted={accepted}",
        std::mem::size_of_val(reader.clusters)
    );
    reader.clusters = original;
    baseline[0].indices = vec![0; device.limits().max_buffer_size as usize / 4 + 1];
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let rejected = Renderer::new(
        device.clone(),
        queue,
        &reader,
        Some(&baseline),
        &scene,
        [16, 16],
        Mode::Naive,
    )
    .is_err();
    let gpu_error = pollster::block_on(scope.pop());
    println!(
        "baseline_index_bytes={} returned_error={rejected} gpu_allocation_error={gpu_error:?}",
        std::mem::size_of_val(baseline[0].indices.as_slice())
    );
    assert!(accepted && rejected && gpu_error.is_none());
}

#[test]
fn timestamped_passes_with_and_without_shadows() {
    let mut mesh = clod_bake::procedural::octasphere(3).unwrap();
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    let scene = Scene::layout(&reader, "single").unwrap();
    let camera = scene.camera(0.0, 1.0, 1.0);
    let baseline = prepare::baseline(&reader);
    let (device, queue, _) = match pollster::block_on(clod_view::request_device(true)) {
        Ok(gpu) => gpu,
        Err(e) if e.starts_with("NO ADAPTER:") => {
            eprintln!("SKIP timestamp GPU test: {e}; cases=0");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let mut failures = Vec::new();
    for mode in [Mode::Cluster, Mode::Naive] {
        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            Some(&baseline),
            &scene,
            [64, 64],
            mode,
        )
        .unwrap();
        if mode == Mode::Cluster {
            renderer.enable_gpu_selection(&reader, None).unwrap();
        }
        for shadows in [true, false, true, false] {
            renderer.shadows = shadows;
            let f = if mode == Mode::Cluster {
                renderer.render_gpu(&scene, &camera, View::Lit, 1.0, true, false)
            } else {
                renderer.render(
                    &scene,
                    &camera,
                    View::Lit,
                    &Selection::default(),
                    &Selection::default(),
                )
            }
            .unwrap();
            match readback::read(&renderer, &f) {
                Ok((_, Some(times))) => {
                    println!("timestamp_render mode={mode:?} shadows={shadows} times_ms={times:?}");
                    if !shadows && (times[0] != 0.0 || times[3] != 0.0 || f.stats.shadow_draws != 0)
                    {
                        failures.push(format!("disabled shadow timing {mode:?}"));
                    }
                }
                other => failures.push(format!(
                    "{mode:?} shadows={shadows}: {}",
                    other.err().unwrap_or("missing timestamps".into())
                )),
            }
        }
    }
    println!("timestamp_render_cases=8 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}
