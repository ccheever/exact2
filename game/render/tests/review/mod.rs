use super::*;
use exact_game_render::{Bloom, RenderError, Shadows};

fn cube_scene(gpu: &Gpu, format: wgpu::TextureFormat) -> Renderer {
    let mut r = Renderer::new(&gpu.device, &gpu.queue, format);
    let (v, i) = shapes::cube();
    let mesh = r.add_mesh(&v, &i);
    r.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
        .unwrap();
    r.write_materials(0, &material([0.4; 3], 1.0)).unwrap();
    r.set_batches(&[Batch::new(mesh, 0..1)], &[0]).unwrap();
    r
}

// Read the tonemap into half floats, before a UNORM conversion can hide NaNs.
fn assert_finite_half_output(gpu: &Gpu, texture: &wgpu::Texture) {
    let row = (texture.width() * 8).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("finite HDR readback"),
        size: u64::from(row) * u64::from(texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    for row in data.chunks_exact(row as usize) {
        for pixel in row[..texture.width() as usize * 8].chunks_exact(8) {
            for channel in pixel[..6].chunks_exact(2) {
                let half = u16::from_le_bytes(channel.try_into().unwrap());
                assert_ne!(half & 0x7c00, 0x7c00, "non-finite tonemapped RGB: {half:x}");
                assert!(half <= 0x3c00, "RGB outside [0, 1]: {half:x}");
            }
        }
    }
}

#[test]
fn overrange_emissive_cannot_poison_bloom_or_tonemap() {
    let Some(gpu) = gpu() else { return };
    let mut f = frame();
    f.environment.bloom = Some(Bloom::default());
    f.environment.horizon = [0.03; 3];
    f.environment.zenith = [0.03; 3];
    f.environment.ground = [0.03; 3];
    let mut r = cube_scene(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let texture = target(&gpu, (640, 360), wgpu::TextureFormat::Rgba8Unorm);
    r.write_materials(0, &material([0.0; 3], 0.0)).unwrap();
    let baseline = render(&gpu, &mut r, &texture, &f);
    r.write_materials(0, &material([0.0; 3], 1e8)).unwrap();
    let bright = render(&gpu, &mut r, &texture, &f);
    bright.save("overrange-emissive");
    assert!(
        bright.at(320, 180)[..3]
            .iter()
            .all(|&c| (245..255).contains(&c)),
        "HDR shoulder retains highlight headroom: {:?}",
        bright.at(320, 180)
    );
    for (a, b) in bright.at(0, 0)[..3].iter().zip(&baseline.at(0, 0)[..3]) {
        assert!(a.abs_diff(*b) <= 2, "far corner contaminated: {a} vs {b}");
    }
    let mut floating = cube_scene(&gpu, wgpu::TextureFormat::Rgba16Float);
    let float_target = target(&gpu, (640, 360), wgpu::TextureFormat::Rgba16Float);
    // Over-range, NaN and both infinities, with and without bloom.
    for emission in [1e8, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        floating
            .write_materials(0, &material([0.0; 3], emission))
            .unwrap();
        for bloom in [None, Some(Bloom::default())] {
            f.environment.bloom = bloom;
            floating.draw(
                &float_target.create_view(&Default::default()),
                (640, 360),
                &f,
            );
            assert_finite_half_output(&gpu, &float_target);
        }
    }
}

#[test]
fn zero_quaternion_sparse_hole_and_negative_scale() {
    let Some(gpu) = gpu() else { return };
    let mut r = cube_scene(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let texture = target(&gpu, (256, 160), wgpu::TextureFormat::Rgba8Unorm);
    let mut f = frame();
    f.environment = Environment {
        sun_disc: 0.0,
        bloom: f.environment.bloom,
        exposure: f.environment.exposure,
        ..Environment::default()
    };
    f.sun = Some(Sun {
        shadows: None,
        ..Sun::default()
    });
    let positive = transform(
        Vec3::ZERO,
        Quat::from_rotation_y(0.4),
        Vec3::new(1.5, 0.7, 1.0),
    );
    r.write_transforms_both(0, &positive).unwrap();
    let reference = render(&gpu, &mut r, &texture, &f);
    for axis in 7..10 {
        let mut negative = positive;
        negative[axis] = -negative[axis];
        r.write_transforms_both(0, &positive).unwrap();
        r.begin_tick();
        r.write_transforms(0, &negative).unwrap();
        for alpha in [0.0, 0.5, 1.0] {
            f.alpha = alpha;
            assert_eq!(reference.data, render(&gpu, &mut r, &texture, &f).data);
        }
    }
    r.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
        .unwrap();
    let identity = render(&gpu, &mut r, &texture, &f);
    let mut zero_q = transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    zero_q[3..7].fill(0.0);
    r.write_transforms_both(0, &zero_q).unwrap();
    assert_eq!(identity.data, render(&gpu, &mut r, &texture, &f).data);
    // A genuinely absent slot, never written, is all-zero GPU memory.
    let mut sparse = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba16Float);
    let (v, i) = shapes::cube();
    let mesh = sparse.add_mesh(&v, &i);
    sparse.write_transforms_both(1_000_000, &positive).unwrap();
    sparse
        .write_materials(1_000_000, &material([0.4; 3], 1.0))
        .unwrap();
    sparse.set_batches(&[Batch::new(mesh, 0..1)], &[0]).unwrap();
    let floating = target(&gpu, (256, 160), wgpu::TextureFormat::Rgba16Float);
    sparse.draw(&floating.create_view(&Default::default()), (256, 160), &f);
    assert_finite_half_output(&gpu, &floating);
}

#[test]
fn arena_capacity_is_a_named_atomic_refusal() {
    let Some(gpu) = gpu() else { return };
    let mut r = cube_scene(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let limit = r.max_slots();
    assert_eq!(
        u64::from(limit),
        gpu.device
            .limits()
            .max_storage_buffer_binding_size
            .min(gpu.device.limits().max_buffer_size)
            / 64 // Full affine attachment overrides are the widest per-entity arena.
    );
    let texture = target(&gpu, (64, 64), wgpu::TextureFormat::Rgba8Unorm);
    let f = frame();
    let before = render(&gpu, &mut r, &texture, &f);
    let t = transform(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE);
    for first in [limit, u32::MAX] {
        let expected = RenderError::Capacity {
            arena: "transforms",
            slot: u64::from(first),
            limit: u64::from(limit),
        };
        assert_eq!(r.write_transforms(first, &t), Err(expected.clone()));
        assert_eq!(r.write_transforms_both(first, &t), Err(expected.clone()));
        assert_eq!(
            r.write_materials(first, &material([0.0; 3], 0.0)),
            Err(RenderError::Capacity {
                arena: "materials",
                slot: u64::from(first),
                limit: u64::from(limit),
            })
        );
        assert_eq!(r.set_batches(&[], &[first]), Err(expected.clone()));
    }
    let long = vec![0; limit as usize + 1];
    assert_eq!(
        r.set_batches(&[], &long),
        Err(RenderError::Capacity {
            arena: "slots",
            slot: u64::from(limit),
            limit: u64::from(limit)
        })
    );
    assert_eq!(before.data, render(&gpu, &mut r, &texture, &f).data);
    // Last valid slot, including the non-power-of-two growth cap.
    r.write_transforms_both(limit - 1, &t).unwrap();
    r.write_materials(limit - 1, &material([0.0; 3], 0.0))
        .unwrap();
    assert_eq!(before.data, render(&gpu, &mut r, &texture, &f).data);
}

#[test]
fn resize_buckets_preserve_logical_pixels_and_never_shrink() {
    let Some(gpu) = gpu() else { return };
    let mut r = cube_scene(&gpu, wgpu::TextureFormat::Rgba8Unorm);
    let mut f = frame();
    f.environment.bloom = Some(Bloom::default());
    f.environment = Environment::default();
    r.write_materials(0, &material([0.0; 3], 20.0)).unwrap();
    let mut first = None;
    let mut previous_creations = 0;
    for (index, size) in [
        (640, 360),
        (650, 366),
        (640, 360),
        (639, 359),
        (650, 366),
        (17, 11),
        (1, 1),
    ]
    .into_iter()
    .enumerate()
    {
        let texture = target(&gpu, size, wgpu::TextureFormat::Rgba8Unorm);
        let stats = r.draw(&texture.create_view(&Default::default()), size, &f);
        let pixels = fixture::read(&gpu, &texture).unwrap();
        if index == 0 {
            first = Some(pixels.data.clone());
        }
        if index == 1 {
            assert_eq!(stats.texture_creations, previous_creations + 9);
        }
        if index >= 2 {
            assert_eq!(stats.texture_creations, previous_creations);
        }
        if index == 2 {
            assert_eq!(first.as_ref().unwrap(), &pixels.data);
        }
        // Also compare each size against a renderer with no resize history.
        let mut fresh = cube_scene(&gpu, texture.format());
        fresh.write_materials(0, &material([0.0; 3], 20.0)).unwrap();
        assert_eq!(
            pixels.data,
            render(&gpu, &mut fresh, &texture, &f).data,
            "size {size:?}"
        );
        previous_creations = stats.texture_creations;
    }
}

#[test]
fn vertical_sun_sweep_keeps_shadow_edge_within_one_texel() {
    let Some(gpu) = gpu() else { return };
    let mut r = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    let (v, i) = shapes::plane();
    let plane = r.add_mesh(&v, &i);
    let (v, i) = shapes::cube();
    let cube = r.add_mesh(&v, &i);
    r.write_transforms_both(0, &transform(Vec3::ZERO, Quat::IDENTITY, Vec3::splat(20.0)))
        .unwrap();
    r.write_transforms_both(
        1,
        &transform(Vec3::Y, Quat::from_rotation_y(0.3), Vec3::ONE),
    )
    .unwrap();
    r.write_materials(0, &material([0.5; 3], 0.0).repeat(2))
        .unwrap();
    r.set_batches(&[Batch::new(plane, 0..1), Batch::new(cube, 1..2)], &[0, 1])
        .unwrap();
    let mut f = frame();
    f.camera_position = Vec3::new(0.0, 4.0, 8.0);
    f.view = view::look_at_mat4(f.camera_position, Vec3::ZERO, Vec3::Y);
    f.proj = directx::orthographic(-2.0, 2.0, -1.0, 1.0, 0.1, 60.0);
    f.environment = Environment {
        background: None,
        zenith: [0.2; 3],
        horizon: [0.2; 3],
        ground: [0.2; 3],
        ambient: 0.5,
        sun_disc: 0.0,
        fog: None,
        ..f.environment
    };
    let settings = Shadows {
        cascades: 1,
        distance: 60.0,
        softness: 1.5,
    };
    f.sun = Some(Sun {
        direction: -Vec3::Y,
        shadows: Some(settings),
        illuminance: 2.0,
        color: Vec3::ONE,
    });
    let texture = target(&gpu, (2048, 512), wgpu::TextureFormat::Rgba8Unorm);
    // Orthographic slice sphere, including the renderer's PCF pad/quantization.
    let radius = ((Vec3::new(2.0, 1.0, (60.0 - 0.1) * 0.5).length() * 1.005 * 16.0).ceil()) / 16.0;
    let texel = radius * 2.0 / 2048.0;
    let mut previous: Option<f32> = None;
    let mut largest = 0.0_f32;
    // Include 8.25 degrees to actually cross the old acos(0.99) = 8.11° switch.
    for step in -33..=33 {
        let angle = (step as f32 * 0.25).to_radians();
        f.sun.as_mut().unwrap().direction = Vec3::new(angle.sin(), -angle.cos(), 0.0);
        let pixels = render(&gpu, &mut r, &texture, &f);
        // Recover the slanted straight edge over a patch. A single hard-raster
        // scanline can jump a full projected texel even with a continuous basis.
        let mut edge = 0.0;
        for row in 0..21 {
            let (_, y) = project(&f, Vec3::new(0.0, 0.0, -0.2 + row as f32 * 0.02), &pixels);
            let dark = pixels.at(1024, y)[0];
            let lit = pixels.at(1536, y)[0];
            assert!(lit > dark + 50);
            let threshold = (f32::from(dark) + f32::from(lit)) * 0.5;
            let x = (1025..1536)
                .find(|&x| f32::from(pixels.at(x, y)[0]) >= threshold)
                .unwrap();
            let a = f32::from(pixels.at(x - 1, y)[0]);
            let b = f32::from(pixels.at(x, y)[0]);
            edge += ((x - 1) as f32 + (threshold - a) / (b - a) + 0.5) / 2048.0 * 4.0 - 2.0;
        }
        edge /= 21.0;
        if let Some(last) = previous {
            largest = largest.max((edge - last).abs());
            assert!(
                (edge - last).abs() < texel,
                "sun {}°: edge {last} -> {edge}, texel {texel}",
                step as f32 * 0.25
            );
        }
        previous = Some(edge);
    }
    eprintln!("vertical sun: maximum world edge step {largest:.6} m; shadow texel {texel:.6} m");
}
