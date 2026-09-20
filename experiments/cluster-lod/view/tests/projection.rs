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
