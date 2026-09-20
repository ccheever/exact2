use clod_format::{Config, Reader};
use clod_view::{
    Mode, Renderer, View,
    scene::{Camera, Instance, Scene},
    select, wgpu,
};
use glam::{Mat4, Quat, Vec3};

#[test]
fn planar_threshold_zero_survives_underflow() {
    let mut mesh = clod_bake::Mesh::default();
    for y in 0..129 {
        for x in 0..129 {
            mesh.positions
                .push([x as f32 / 128.0, y as f32 / 128.0, 0.0]);
        }
    }
    for y in 0..128 {
        for x in 0..128 {
            let a = y * 129 + x;
            mesh.indices
                .extend_from_slice(&[a, a + 1, a + 129, a + 1, a + 130, a + 129]);
        }
    }
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    let mut failures = Vec::new();
    let gpu = pollster::block_on(clod_view::request_device(false));
    for (scale, distance) in [(1.0, 1e20), (1e-20, 1.0), (1e-20, 1e20)] {
        let mut scene = Scene::layout(&reader, "single").unwrap();
        scene.instances = vec![Instance::new(Vec3::ZERO, Quat::IDENTITY, scale)];
        let camera = Camera {
            eye: Vec3::splat(distance),
            matrix: Mat4::IDENTITY,
            near: 0.002,
            cot: 1.0,
            orthographic_span: None,
        };
        let cut = select::select_culled(&reader, &scene.instances, &camera, 192, 0.0, false);
        if cut.triangles != 32768 {
            failures.push(format!(
                "CPU scale={scale} distance={distance}: {} triangles",
                cut.triangles
            ));
        }
        println!(
            "underflow scale={scale} distance={distance} source=32768 cpu={}",
            cut.triangles
        );
        if let Ok((device, queue, _)) = &gpu {
            let mut renderer = Renderer::new(
                device.clone(),
                queue.clone(),
                &reader,
                None,
                &scene,
                [16, 16],
                Mode::Cluster,
            )
            .unwrap();
            renderer.enable_gpu_selection(&reader, None).unwrap();
            for brute in [false, true] {
                renderer
                    .render_gpu(&scene, &camera, View::Lit, 0.0, false, brute)
                    .unwrap();
                let buffers = renderer.selection_readback(false).unwrap();
                for buffer in &buffers {
                    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
                }
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let mapped = buffers[0].slice(..).get_mapped_range().unwrap();
                let words: &[u32] = bytemuck::cast_slice(&mapped);
                let triangles = words[reader.pages.len() * 8 + 1];
                println!(
                    "underflow brute={brute} scale={scale} distance={distance} gpu={triangles}"
                );
                if triangles != 32768 {
                    failures.push(format!(
                        "GPU brute={brute} scale={scale} distance={distance}: {triangles} triangles"
                    ));
                }
            }
        }
    }
    if let Err(e) = gpu {
        eprintln!("SKIP GPU underflow: {e}; GPU cases=0");
    }
    println!("projection_cases=9 failures={failures:?}");
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn positive_threshold_extreme_ranges_match_reference() {
    let mut mesh = clod_bake::procedural::octasphere(5).unwrap();
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    let (device, queue, _) = pollster::block_on(clod_view::request_device(false)).unwrap();
    let mut failures = Vec::new();
    let cases = [
        (1e20, 1e21, 0.5),
        (1.0, 1e19, 1e-20),
        (1e20, 1e21, 1.0),
        (1e20, 1e21, 1e20),
    ];
    for (scale, distance, threshold) in cases {
        let mut scene = Scene::layout(&reader, "single").unwrap();
        scene.instances = vec![Instance::new(Vec3::ZERO, Quat::IDENTITY, scale)];
        let camera = Camera {
            eye: Vec3::splat(distance),
            matrix: Mat4::from_scale(Vec3::splat(1.0 / distance)),
            near: 0.002,
            cot: 1.0,
            orthographic_span: None,
        };
        let cpu = select::select_culled(&reader, &scene.instances, &camera, 100, threshold, false);
        let index = select::CandidateIndex::new(&reader);
        let range = index.range(
            scene.instances[0].transform(),
            scale,
            &camera,
            100,
            threshold,
        );
        if cpu
            .pages
            .iter()
            .flatten()
            .any(|p| !range.contains(&(p[0] as usize)))
        {
            failures.push(format!("CPU range pruned selected cluster {range:?}"));
        }

        let mut renderer = Renderer::new(
            device.clone(),
            queue.clone(),
            &reader,
            None,
            &scene,
            [100, 100],
            Mode::Cluster,
        )
        .unwrap();
        renderer.shadows = false;
        renderer.enable_gpu_selection(&reader, None).unwrap();
        for brute in [false, true] {
            renderer
                .render_gpu(&scene, &camera, View::Coverage, threshold, false, brute)
                .unwrap();
            let buffers = renderer.selection_readback(true).unwrap();
            buffers[0].slice(..).map_async(wgpu::MapMode::Read, |_| {});
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let mapped = buffers[0].slice(..).get_mapped_range().unwrap();
            let words: &[u32] = bytemuck::cast_slice(&mapped);
            let count = words[reader.pages.len() * 8] as usize;
            let pairs: Vec<[u32; 2]> = words[reader.pages.len() * 8 + 4..][..count * 2]
                .chunks_exact(2)
                .map(|p| [p[0], p[1]])
                .collect();
            let expected: Vec<_> = cpu.pages.iter().flatten().copied().collect();
            println!(
                "range scale={scale} distance={distance} threshold={threshold} brute={brute} cpu={} gpu={count}",
                expected.len()
            );
            if pairs != expected {
                failures.push(format!(
                    "scale={scale} distance={distance} threshold={threshold} brute={brute}"
                ));
            }
        }
    }
    println!("range_cases={} failures={failures:?}", cases.len() * 2);
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn admitted_stretch_keeps_intersecting_sphere() {
    let mut mesh = clod_bake::procedural::octasphere(1).unwrap();
    let baked = clod_bake::bake(&mut mesh, Config::default(), [0; 32]).unwrap();
    let reader = Reader::new(&baked.bytes).unwrap();
    let mut scene = Scene::layout(&reader, "single").unwrap();
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(1.0, 1.000009, 1.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -10.00005, 0.0),
    );
    scene.instances = vec![Instance {
        matrix: model.to_cols_array(),
    }];
    scene.validate().unwrap();
    let visible = select::sphere_visible([0.0, 0.0, 0.0, 10.0], model, 1.0, &[glam::Vec4::Y; 6]);
    println!(
        "stretch_cases=1 accepted=1 transformed_tip_y={} visible={visible}",
        model.transform_point3(Vec3::new(0.0, 9.999995, 0.0)).y
    );
    assert!(visible);
}
