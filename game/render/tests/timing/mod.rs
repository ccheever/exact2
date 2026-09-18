use super::*;

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
    renderer.write_transforms_both(0, &transforms).unwrap();
    renderer.write_materials(0, &materials).unwrap();
    renderer
        .set_batches(
            &[Batch {
                mesh: cube,
                casts_shadows: true,
                slots: 0..N as u32,
            }],
            &(0..N as u32).collect::<Vec<_>>(),
        )
        .unwrap();
    let target = target(&gpu, (1280, 720), format);
    let view = target.create_view(&Default::default());
    let mut frame = frame();
    frame.camera_position = Vec3::new(115.0, 80.0, 160.0);
    frame.view = view::look_at_mat4(frame.camera_position, Vec3::ZERO, Vec3::Y);
    frame.proj = directx::perspective(60.0_f32.to_radians(), 1280.0 / 720.0, 0.1, 500.0);
    frame.sun = Some(Sun {
        direction: Vec3::new(-1.0, -2.0, -3.0),
        color: Vec3::ONE,
        shadows: None,
        illuminance: 3.0,
    });
    frame.environment = Environment {
        background: None,
        zenith: [0.12, 0.18, 0.28],
        ground: [0.12, 0.18, 0.28],
        ambient: 0.5,
        horizon: [0.12, 0.18, 0.28],
        sun_disc: 0.0,
        fog: None,
    };
    for _ in 0..10 {
        renderer.draw(&gpu.device, &gpu.queue, &view, format, (1280, 720), &frame);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
    for rewrite in [
        exact_game_render::Rewrite::Some,
        exact_game_render::Rewrite::All,
    ] {
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
                renderer.begin_tick(rewrite);
                renderer.write_transforms(0, &transforms).unwrap();
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
        eprintln!("200000 cubes {rewrite:?}, 1280x720, 4x MSAA, 600 frames: CPU encode {:.4} ms/frame; tick copy+upload {:.4} ms/tick; GPU-completed wall {:.4} ms/frame",
        encode_us / 600_000.0, upload_ms / 300.0, total_ms / 600.0);
    }
    fixture::read(&gpu, &target).unwrap().save("timing-200k");
}

struct Queries {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}
impl Queries {
    fn new(gpu: &Gpu) -> Option<Self> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let set = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("game timing"),
            ty: wgpu::QueryType::Timestamp,
            count: exact_game_render::GPU_PASS_COUNT * 2,
        });
        let buffer = |usage| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("game timing read"),
                size: 256,
                usage,
                mapped_at_creation: false,
            })
        };
        Some(Self {
            set,
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
        })
    }

    fn read(&self, gpu: &Gpu) -> [f64; 17] {
        // This diagnostic resolves in a separate submission. On Metal, counter
        // samples are not texture/buffer hazards: wait for fragment completion
        // before resolving, or the trailing samples can still be zero.
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&self.set, 0..32, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 256);
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.read
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let range = self.read.slice(..).get_mapped_range().unwrap();
        let result = std::array::from_fn(|i| {
            let stamp = |index: usize| {
                u64::from_ne_bytes(range[index * 8..index * 8 + 8].try_into().unwrap())
            };
            let (a, b) = if i == 16 {
                // Total GPU envelope, not the sum: Metal can overlap vertex work
                // from later passes with fragment work from earlier passes.
                let first = (0..32).map(stamp).filter(|&x| x != 0).min().unwrap_or(0);
                let last = (0..32).map(stamp).max().unwrap_or(0);
                (first, last)
            } else {
                (stamp(i * 2), stamp(i * 2 + 1))
            };
            if b < a {
                static REPORTED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    eprintln!("GPU timestamp warning: reversed pair at {}: {a} -> {b}; reporting NaN, not zero", exact_game_render::GPU_PASS_NAMES[i]);
                }
                return f64::NAN;
            }
            (b - a) as f64 * f64::from(gpu.queue.get_timestamp_period()) / 1_000_000.0
        });
        drop(range);
        self.read.unmap();
        result
    }
}

#[test]
#[ignore = "300-object effects scene at 2560x1440, optional per-pass GPU timestamps"]
fn timing_effects_300() {
    use exact_game_render::{Bloom, Fog, Shadows, GPU_PASS_NAMES};
    let Some(mut gpu) = gpu() else {
        return;
    };
    if gpu
        .adapter
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY)
    {
        let (device, queue) =
            exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("game timing"),
                required_features: wgpu::Features::TIMESTAMP_QUERY,
                ..Default::default()
            }))
            .unwrap();
        gpu.device = device;
        gpu.queue = queue;
    } else {
        eprintln!("GPU pass timings unavailable: adapter lacks TIMESTAMP_QUERY");
    }
    let mut r = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let (v, i) = shapes::plane();
    let plane = r.add_mesh(&v, &i);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    let (v, i) = shapes::sphere(32);
    let sphere = r.add_mesh(&v, &i);
    let mut transforms = Vec::new();
    let mut materials = Vec::new();
    transforms.extend(transform(
        Vec3::ZERO,
        Quat::IDENTITY,
        Vec3::new(80.0, 1.0, 80.0),
    ));
    materials.extend(material([0.12, 0.24, 0.16], 0.0));
    for i in 0..298 {
        let x = ((i % 20) as f32 - 9.5) * 2.5;
        let z = ((i / 20) as f32 - 7.0) * 2.5;
        transforms.extend(transform(
            Vec3::new(x, 0.5, z),
            Quat::from_rotation_y(i as f32 * 0.12),
            Vec3::ONE,
        ));
        materials.extend(material([0.45, 0.25, 0.08], 0.0));
    }
    transforms.extend(transform(
        Vec3::new(0.0, 1.2, 4.0),
        Quat::IDENTITY,
        Vec3::splat(1.5),
    ));
    let mut beacon = material([0.0; 3], 0.0);
    beacon[6..9].copy_from_slice(&[20.0, 12.0, 1.0]);
    materials.extend(beacon);
    r.write_transforms_both(0, &transforms).unwrap();
    r.write_materials(0, &materials).unwrap();
    r.set_batches(
        &[
            Batch::new(plane, 0..1),
            Batch::new(cube, 1..299),
            Batch::new(sphere, 299..300),
        ],
        &(0..300).collect::<Vec<_>>(),
    )
    .unwrap();
    let texture = target(&gpu, (2560, 1440), wgpu::TextureFormat::Rgba8Unorm);
    let view = texture.create_view(&Default::default());
    let queries = Queries::new(&gpu);
    let mut f = frame();
    f.camera_position = Vec3::new(10.0, 9.0, 22.0);
    f.view = view::look_at_mat4(f.camera_position, Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
    f.proj = directx::perspective(60f32.to_radians(), 16.0 / 9.0, 0.1, 120.0);
    f.sun = Some(Sun {
        direction: Vec3::new(1.0, -2.0, -1.0),
        color: Vec3::new(1.0, 0.94, 0.8),
        illuminance: 3.0,
        shadows: None,
    });
    let points = [PointLightInput {
        position: Vec3::new(0.0, 1.4, 4.0),
        color: Vec3::new(1.0, 0.65, 0.07),
        intensity: 20.0,
        range: 6.0,
    }];
    f.points = &points;
    let mut cpu = [0.0; 3];
    let mut pass_ms = [[0.0; 17]; 3];
    for mode in 0..3 {
        f.environment = Environment {
            background: None,
            zenith: [0.2; 3],
            horizon: [0.2; 3],
            ground: [0.2; 3],
            ambient: 0.5,
            sun_disc: 0.0,
            fog: None,
        };
        if mode >= 1 {
            f.sun.as_mut().unwrap().shadows = Some(Shadows::default());
        }
        if mode == 2 {
            f.bloom = Some(Bloom::default());
            f.environment = Environment {
                fog: Some(Fog {
                    density: 0.012,
                    ..Default::default()
                }),
                ..Default::default()
            };
        }
        f.timestamps = queries.as_ref().map(|q| &q.set);
        for index in 0..100 {
            let stats = r.draw(
                &gpu.device,
                &gpu.queue,
                &view,
                texture.format(),
                (2560, 1440),
                &f,
            );
            let times = if let Some(q) = &queries {
                q.read(&gpu)
            } else {
                gpu.device
                    .poll(wgpu::PollType::wait_indefinitely())
                    .unwrap();
                [0.0; 17]
            };
            if index >= 10 {
                cpu[mode] += stats.encode_us / 90_000.0;
                for (sum, value) in pass_ms[mode].iter_mut().zip(times) {
                    *sum += value / 90.0;
                }
            }
        }
        eprintln!(
            "300 objects 2560x1440 {}: CPU encode {:.4} ms",
            ["off", "shadows", "all effects"][mode],
            cpu[mode]
        );
        eprintln!(
            "  GPU frame envelope: {:.4} ms (pass intervals overlap)",
            pass_ms[mode][16]
        );
        for (name, ms) in GPU_PASS_NAMES.iter().zip(pass_ms[mode]) {
            if ms != 0.0 {
                eprintln!("  {name}: {ms:.4} GPU ms");
            }
        }
    }
    let shadow_gpu: f64 = pass_ms[1][..3].iter().sum::<f64>() + pass_ms[1][3] - pass_ms[0][3];
    eprintln!(
        "shadow added cost: {:.4} CPU ms, {:.4} GPU ms (depth passes + forward sampling delta)",
        cpu[1] - cpu[0],
        shadow_gpu
    );
    fixture::read(&gpu, &texture).unwrap().save("effects-300");
}

#[test]
#[ignore = "Beacons camera, 40 m plane and cube; GPU timestamps on a GPU host"]
fn timing_beacons_shadows() {
    let Some(mut gpu) = gpu() else {
        return;
    };
    if !gpu
        .adapter
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY)
    {
        eprintln!("SKIP Beacons GPU timing: TIMESTAMP_QUERY unavailable");
        return;
    }
    (gpu.device, gpu.queue) =
        exact_gpu::block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        }))
        .unwrap();
    let (mut r, mut f) = effects::shadow_quality::scene(&gpu);
    f.camera_position = Vec3::new(0.0, 9.0, 13.0);
    f.view = view::look_at_mat4(f.camera_position, Vec3::ZERO, Vec3::Y);
    f.sun.as_mut().unwrap().direction = Vec3::new(-5.0, -10.0, -5.0);
    let texture = target(&gpu, (1280, 720), wgpu::TextureFormat::Rgba8Unorm);
    let view = texture.create_view(&Default::default());
    let queries = Queries::new(&gpu).unwrap();
    f.timestamps = Some(&queries.set);
    let mut spans = Vec::new();
    let mut encodes = Vec::new();
    for i in 0..300 {
        let stats = r.draw(
            &gpu.device,
            &gpu.queue,
            &view,
            texture.format(),
            (1280, 720),
            &f,
        );
        let ms = queries.read(&gpu)[16];
        if i >= 60 {
            spans.push(ms);
            encodes.push(stats.encode_us / 1000.0);
        }
    }
    spans.sort_by(f64::total_cmp);
    encodes.sort_by(f64::total_cmp);
    eprintln!("Beacons 40m plane + cube, 1280x720, shadowed: GPU median {:.4} ms, mean {:.4} ms; CPU encode median {:.4} ms",spans[120],spans.iter().sum::<f64>()/240.0,encodes[120]);
}
