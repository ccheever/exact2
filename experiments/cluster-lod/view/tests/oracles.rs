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
                2048,
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
    check(
        d.max <= 1 && d.mean < 0.000001,
        "threshold-zero exceeds 1/255 max or .000001 mean bound",
    );
    let (repeat, _) = draw(&mut cluster, 0.0, View::Lit);
    let png_a = readback::png_bytes(&c0, size, size).expect("png");
    let png_b = readback::png_bytes(&repeat, size, size).expect("png");
    check(png_a == png_b, "nondeterministic PNG");
    println!(
        "{}",
        json!({"oracle":"repeat_png","bytes":png_a.len(),"identical":png_a==png_b})
    );
    let (c1, s1) = draw(&mut cluster, 1.0, View::Lit);
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
        json!({"oracle":"image_summary","images":9,"debug_views":debug_count,"shader_files_validated_at_build":1,"failures":failures})
    );
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
