#![cfg(not(target_arch = "wasm32"))]

use exact_game_render::{shapes, Batch, Environment, FrameInput, PointLightInput, Renderer, Sun};
use exact_gpu::{fixture, wgpu, Gpu};
use glam::camera::rh::{proj::directx, view};
use glam::{Quat, Vec3};

fn gpu() -> Option<Gpu> {
    match fixture::device() {
        Ok(gpu) => {
            eprintln!("GPU: {:?}", gpu.adapter.get_info());
            Some(gpu)
        }
        Err(reason) => {
            eprintln!("SKIP exact-game-render GPU test: {reason}");
            None
        }
    }
}

fn target(gpu: &Gpu, size: (u32, u32), format: wgpu::TextureFormat) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("game test"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn render(
    gpu: &Gpu,
    renderer: &mut Renderer,
    target: &wgpu::Texture,
    frame: &FrameInput<'_>,
) -> fixture::Pixels {
    let stats = renderer.draw(
        &gpu.device,
        &gpu.queue,
        &target.create_view(&Default::default()),
        target.format(),
        (target.width(), target.height()),
        frame,
    );
    assert!(stats.draws >= 1 && stats.triangles >= 1);
    fixture::read(gpu, target).unwrap()
}

fn transform(position: Vec3, rotation: Quat, scale: Vec3) -> [f32; 10] {
    [
        position.x, position.y, position.z, rotation.x, rotation.y, rotation.z, rotation.w,
        scale.x, scale.y, scale.z,
    ]
}

fn material(color: [f32; 3], emissive: f32) -> [f32; 12] {
    [
        color[0], color[1], color[2], 1.0, 0.0, 0.65, emissive, emissive, emissive, 0.0, 0.0, 0.0,
    ]
}

fn frame() -> FrameInput<'static> {
    FrameInput {
        view: view::look_at_mat4(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y),
        proj: directx::orthographic(-4.0, 4.0, -2.5, 2.5, 0.1, 100.0),
        camera_position: Vec3::new(0.0, 0.0, 10.0),
        alpha: 1.0,
        sun: None,
        points: &[],
        environment: Environment {
            sky: [0.0; 3],
            ground: [0.0; 3],
            ambient: 0.0,
        },
        exposure: 1.0,
    }
}

fn tone(x: f32) -> u8 {
    let x = (x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    let srgb = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

fn project(frame: &FrameInput<'_>, point: Vec3, pixels: &fixture::Pixels) -> (u32, u32) {
    let clip = frame.proj * frame.view * point.extend(1.0);
    (
        ((clip.x / clip.w * 0.5 + 0.5) * pixels.width as f32) as u32,
        ((0.5 - clip.y / clip.w * 0.5) * pixels.height as f32) as u32,
    )
}

#[test]
fn lit_scene_and_output_transfer() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let mut batches = Vec::new();
    for (slot, (vertices, indices)) in [
        shapes::plane(),
        shapes::cube(),
        shapes::sphere(32),
        shapes::capsule(0.4, 1.8, 32),
    ]
    .into_iter()
    .enumerate()
    {
        let mesh = renderer.add_mesh(&vertices, &indices);
        let (center, radius) = renderer.mesh_bounds(mesh);
        assert!(vertices
            .iter()
            .all(|v| Vec3::from_array(v.position).distance(center) <= radius + 1e-6));
        batches.push(Batch {
            mesh,
            slots: slot as u32..slot as u32 + 1,
        });
    }
    for (slot, (p, s, color)) in [
        (Vec3::ZERO, Vec3::new(10.0, 1.0, 8.0), [0.35, 0.38, 0.4]),
        (Vec3::new(-1.5, 0.5, 0.0), Vec3::ONE, [0.65; 3]),
        (
            Vec3::new(0.0, 0.55, 0.0),
            Vec3::splat(1.1),
            [0.75, 0.3, 0.08],
        ),
        (Vec3::new(1.6, 0.9, 0.0), Vec3::ONE, [0.65; 3]),
    ]
    .into_iter()
    .enumerate()
    {
        renderer.write_transforms_both(slot as u32, &transform(p, Quat::IDENTITY, s));
        renderer.write_materials(slot as u32, &material(color, 0.0));
    }
    renderer.set_batches(&batches, &[0, 1, 2, 3]);
    let mut frame = frame();
    frame.camera_position = Vec3::new(5.0, 4.0, 8.0);
    frame.view = view::look_at_mat4(frame.camera_position, Vec3::new(0.0, 0.6, 0.0), Vec3::Y);
    frame.proj = directx::perspective(45.0_f32.to_radians(), 1.5, 0.1, 100.0);
    frame.sun = Some(Sun {
        direction: Vec3::new(1.0, -2.0, -3.0),
        color: Vec3::ONE,
        illuminance: 3.0,
    });
    frame.environment = Environment {
        sky: [0.12, 0.18, 0.28],
        ground: [0.04, 0.025, 0.02],
        ambient: 0.35,
    };
    let texture = target(&gpu, (600, 400), format);
    let unlit_point = render(&gpu, &mut renderer, &texture, &frame);
    let points = [PointLightInput {
        position: Vec3::new(1.6, 1.2, 1.4),
        color: Vec3::new(0.02, 0.1, 1.0),
        intensity: 12.0,
        range: 3.5,
    }];
    frame.points = &points;
    let pixels = render(&gpu, &mut renderer, &texture, &frame);
    pixels.save("lit-scene");
    let sky = pixels.at(5, 5);
    for (actual, expected) in sky[..3].iter().zip(frame.environment.sky.map(tone)) {
        assert!(actual.abs_diff(expected) <= 2, "sky {sky:?}");
    }
    assert!(pixels.count(|p| p != sky) > 30_000);
    let front = project(&frame, Vec3::new(-1.5, 0.55, 0.501), &pixels);
    let side = project(&frame, Vec3::new(-0.999, 0.55, 0.0), &pixels);
    let lit = pixels.at(front.0, front.1);
    let dark = pixels.at(side.0, side.1);
    eprintln!("cube lit={lit:?}, shadowed={dark:?}, sky={sky:?}");
    assert!(u32::from(lit[0]) > u32::from(dark[0]) + 50);
    let near = project(&frame, Vec3::new(1.6, 1.0, 0.4), &pixels);
    let blue = pixels.at(near.0, near.1);
    let neutral = unlit_point.at(near.0, near.1);
    eprintln!("point nearby: {neutral:?} -> {blue:?}");
    assert!(
        i32::from(blue[2]) - i32::from(neutral[2])
            > i32::from(blue[0]) - i32::from(neutral[0]) + 15
    );

    // An sRGB target must encode exactly once, matching the non-sRGB shader path.
    let srgb_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut srgb = Renderer::new(&gpu.device, &gpu.queue, srgb_format);
    srgb.set_batches(&[], &[]);
    let background = render(
        &gpu,
        &mut srgb,
        &target(&gpu, (32, 24), srgb_format),
        &frame,
    );
    assert!(background
        .at(5, 5)
        .iter()
        .zip(sky)
        .all(|(a, b)| a.abs_diff(b) <= 2));
}

fn silhouette(pixels: &fixture::Pixels) -> (f32, u32) {
    let mut total = 0u64;
    let mut count = 0u64;
    let mut min = pixels.width;
    let mut max = 0;
    for y in 0..pixels.height {
        for x in 0..pixels.width {
            if pixels.at(x, y)[0] > 100 {
                total += u64::from(x);
                count += 1;
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    assert!(count > 0);
    (total as f32 / count as f32, max - min + 1)
}

#[test]
fn interpolation_teleport_untouched_and_growth() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let (v, i) = shapes::cube();
    let cube = renderer.add_mesh(&v, &i);
    renderer.write_transforms_both(
        7,
        &transform(Vec3::new(-2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    renderer.write_materials(7, &material([0.5; 3], 1.0));
    renderer.set_batches(
        &[Batch {
            mesh: cube,
            slots: 1..2,
        }],
        &[7, 7],
    );
    renderer.begin_tick();
    renderer.write_transforms(
        7,
        &transform(Vec3::new(2.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    let target = target(&gpu, (256, 160), format);
    let mut frame = frame();
    for (alpha, expected, name) in [
        (0.0, 63.5, "interpolation-left"),
        (0.5, 127.5, "interpolation-middle"),
        (1.0, 191.5, "interpolation-right"),
    ] {
        frame.alpha = alpha;
        let pixels = render(&gpu, &mut renderer, &target, &frame);
        pixels.save(name);
        assert!((silhouette(&pixels).0 - expected).abs() < 1.0);
    }
    renderer.write_transforms_both(7, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE));
    renderer.begin_tick();
    renderer.write_transforms(
        7,
        &transform(
            Vec3::ZERO,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::ONE,
        ),
    );
    frame.alpha = 0.5;
    let rotated = render(&gpu, &mut renderer, &target, &frame);
    rotated.save("rotation-eighth-turn");
    assert!(silhouette(&rotated).1 > 42);
    renderer.write_transforms_both(
        7,
        &transform(
            Vec3::ZERO,
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
            Vec3::ONE,
        ),
    );
    let reference = render(&gpu, &mut renderer, &target, &frame);
    assert_eq!(
        rotated.data, reference.data,
        "nlerp quarter turn midpoint must be an eighth turn"
    );
    // Antipodal quaternion representations denote the same rotation.
    renderer.begin_tick();
    renderer.write_transforms(
        7,
        &transform(
            Vec3::ZERO,
            -Quat::from_rotation_y(std::f32::consts::FRAC_PI_4),
            Vec3::ONE,
        ),
    );
    assert_eq!(
        reference.data,
        render(&gpu, &mut renderer, &target, &frame).data
    );

    renderer.write_transforms_both(
        7,
        &transform(Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    for tick in 0..3 {
        if tick != 0 {
            renderer.begin_tick();
        }
        for alpha in [0.0, 0.5, 1.0] {
            frame.alpha = alpha;
            assert!(
                (silhouette(&render(&gpu, &mut renderer, &target, &frame)).0 - 159.5).abs() < 1.0
            );
        }
    }
    // Grow every storage arena and both geometry arenas, after uploading live data.
    renderer.write_transforms_both(
        1_000_000,
        &transform(Vec3::new(-1.0, 0.0, 0.0), Quat::IDENTITY, Vec3::ONE),
    );
    renderer.write_materials(1_000_000, &material([0.5; 3], 1.0));
    let (v, i) = shapes::sphere(64);
    let sphere = renderer.add_mesh(&v, &i);
    let mut slots = vec![7; 100];
    slots[99] = 1_000_000;
    renderer.set_batches(
        &[
            Batch {
                mesh: cube,
                slots: 0..1,
            },
            Batch {
                mesh: sphere,
                slots: 99..100,
            },
        ],
        &slots,
    );
    renderer.begin_tick();
    for alpha in [0.0, 0.5, 1.0] {
        frame.alpha = alpha;
        let pixels = render(&gpu, &mut renderer, &target, &frame);
        assert!(pixels.at(160, 80)[0] > 200 && pixels.at(96, 80)[0] > 200);
        pixels.save("millionth-slot-growth");
    }
}

#[test]
fn nonuniform_scale_matches_baked_normal_matrix() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let positions = [
        Vec3::new(-1.0, -0.8, -0.5),
        Vec3::new(1.0, -0.8, 0.5),
        Vec3::new(0.0, 0.8, 0.0),
    ];
    let normal = (positions[1] - positions[0])
        .cross(positions[2] - positions[0])
        .normalize();
    let vertices = positions.map(|p| exact_game_render::Vertex {
        position: p.to_array(),
        normal: normal.to_array(),
        uv: [0.0; 2],
    });
    let rotation = Quat::from_rotation_y(0.3) * Quat::from_rotation_z(0.4);
    let scale = Vec3::new(1.2, 0.7, 2.5);
    let baked = vertices.map(|v| exact_game_render::Vertex {
        position: (rotation * (Vec3::from_array(v.position) * scale)).to_array(),
        normal: (rotation * (normal / scale)).normalize().to_array(),
        uv: v.uv,
    });
    let source = renderer.add_mesh(&vertices, &[0, 1, 2]);
    let reference = renderer.add_mesh(&baked, &[0, 1, 2]);
    renderer.write_materials(0, &material([0.6; 3], 0.0));
    renderer.write_transforms_both(0, &transform(Vec3::ZERO, rotation, scale));
    renderer.set_batches(
        &[Batch {
            mesh: source,
            slots: 0..1,
        }],
        &[0],
    );
    let mut frame = frame();
    frame.environment = Environment {
        sky: [0.3, 0.4, 0.5],
        ground: [0.02; 3],
        ambient: 1.0,
    };
    frame.sun = Some(Sun {
        direction: Vec3::new(1.0, -1.0, -2.0),
        color: Vec3::ONE,
        illuminance: 3.0,
    });
    let target = target(&gpu, (256, 160), format);
    let actual = render(&gpu, &mut renderer, &target, &frame);
    renderer.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE));
    renderer.set_batches(
        &[Batch {
            mesh: reference,
            slots: 0..1,
        }],
        &[0],
    );
    let expected = render(&gpu, &mut renderer, &target, &frame);
    assert!(actual.at(128, 80)[0] > 150);
    let mean_error = actual
        .data
        .iter()
        .zip(&expected.data)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / actual.data.len() as f64;
    assert!(mean_error < 0.1, "normal matrix pixel error: {mean_error}");
}

#[test]
#[ignore = "200k cubes, 600 frames; run in release on a GPU host"]
fn timing_200k() {
    let Some(gpu) = gpu() else {
        return;
    };
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format);
    let (v, i) = shapes::cube();
    let cube = renderer.add_mesh(&v, &i);
    const N: usize = 200_000;
    let mut transforms = Vec::with_capacity(N * 10);
    let mut materials = Vec::with_capacity(N * 12);
    for i in 0..N {
        let p = Vec3::new(
            (i % 59) as f32,
            ((i / 59) % 59) as f32,
            (i / (59 * 59)) as f32,
        ) * 2.0
            - Vec3::splat(58.0);
        transforms.extend(transform(p, Quat::IDENTITY, Vec3::ONE));
        materials.extend(material([0.35, 0.55, 0.8], 0.0));
    }
    renderer.write_transforms_both(0, &transforms);
    renderer.write_materials(0, &materials);
    renderer.set_batches(
        &[Batch {
            mesh: cube,
            slots: 0..N as u32,
        }],
        &(0..N as u32).collect::<Vec<_>>(),
    );
    let target = target(&gpu, (1280, 720), format);
    let view = target.create_view(&Default::default());
    let mut frame = frame();
    frame.camera_position = Vec3::new(115.0, 80.0, 160.0);
    frame.view = view::look_at_mat4(frame.camera_position, Vec3::ZERO, Vec3::Y);
    frame.proj = directx::perspective(60.0_f32.to_radians(), 1280.0 / 720.0, 0.1, 500.0);
    frame.sun = Some(Sun {
        direction: Vec3::new(-1.0, -2.0, -3.0),
        color: Vec3::ONE,
        illuminance: 3.0,
    });
    frame.environment = Environment {
        sky: [0.12, 0.18, 0.28],
        ground: [0.04; 3],
        ambient: 0.5,
    };
    for _ in 0..10 {
        renderer.draw(&gpu.device, &gpu.queue, &view, format, (1280, 720), &frame);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
    let mut encode_us = 0.0;
    let mut upload_ms = 0.0;
    let mut total_ms = 0.0;
    for index in 0..600 {
        if index % 2 == 0 {
            // Simulation work is deliberately outside the upload timer.
            let rotation = Quat::from_rotation_y(index as f32 / 120.0);
            for transform in transforms.chunks_exact_mut(10) {
                transform[3..7].copy_from_slice(&rotation.to_array());
            }
        }
        let start = std::time::Instant::now();
        if index % 2 == 0 {
            let upload = std::time::Instant::now();
            renderer.begin_tick();
            renderer.write_transforms(0, &transforms);
            upload_ms += upload.elapsed().as_secs_f64() * 1000.0;
        }
        frame.alpha = if index % 2 == 0 { 0.0 } else { 0.5 };
        let stats = renderer.draw(&gpu.device, &gpu.queue, &view, format, (1280, 720), &frame);
        assert_eq!(
            (stats.draws, stats.instances, stats.triangles),
            (2, N as u64, N as u64 * 12 + 1)
        );
        encode_us += stats.encode_us;
        // Bound outstanding work; wall time includes GPU completion, encode does not.
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        total_ms += start.elapsed().as_secs_f64() * 1000.0;
        if index % 100 == 99 {
            eprintln!("timing: {} / 600 frames", index + 1);
        }
    }
    eprintln!("200000 cubes, 1280x720, 4x MSAA, 600 frames: CPU encode {:.4} ms/frame; tick copy+upload {:.4} ms/tick; GPU-completed wall {:.4} ms/frame",
        encode_us / 600_000.0, upload_ms / 300.0, total_ms / 600.0);
    fixture::read(&gpu, &target).unwrap().save("timing-200k");
}
