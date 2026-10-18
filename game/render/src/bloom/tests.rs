use super::*;
use exact_gpu::{fixture, Gpu};

fn texture(gpu: &Gpu, size: (u32, u32)) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bloom equivalence"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn read(gpu: &Gpu, texture: &wgpu::Texture) -> Vec<u16> {
    let row = (texture.width() * 8).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloom half-float readback"),
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
    let mapped = buffer.slice(..).get_mapped_range().unwrap();
    mapped
        .chunks_exact(row as usize)
        .flat_map(|r| {
            r[..texture.width() as usize * 8]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes(c.try_into().unwrap()))
        })
        .collect()
}

#[test]
fn tiled_bright_matches_fragment_at_edges_odd_sizes_and_nonfinite_inputs() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let pipelines = Pipelines::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("original 13-tap bloom oracle"),
            source: wgpu::ShaderSource::Wgsl(crate::pipeline::shader_sources()[4].clone().into()),
        });
    let reference = crate::pipeline::make_pipeline(
        &gpu.device,
        None,
        &shader,
        "bloom oracle",
        "vs",
        Some("bright_reference"),
        &[Some(&pipelines.bloom_layout)],
        &[],
        None,
        1,
        wgpu::TextureFormat::Rgba16Float,
        None,
        &[],
        false,
        true,
    );
    let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bloom test frame"),
        size: (crate::frame::FLOATS * 4) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut frame = [0.; crate::frame::FLOATS];
    frame[77] = 1.;
    gpu.queue
        .write_buffer(&uniform, 0, crate::buffers::bytes(&frame));
    // The physical target deliberately exceeds the logical image. Padding is
    // bright poison: reading it instead of clamping the logical edge is visible.
    let physical = (280, 180);
    let source = texture(&gpu, physical);
    let source_view = source.create_view(&Default::default());
    let targets = BloomTargets::new(&gpu.device, physical, &pipelines, &uniform, &source_view);
    let cases = (1..=17).flat_map(|w| (1..=17).map(move |h| (w, h))).chain([
        (31, 32),
        (32, 31),
        (33, 33),
        (127, 65),
        (256, 128),
        (257, 129),
    ]);
    let values = [
        0x0000_u16, 0x3800, 0x3bff, 0x3c00, 0x3c01, 0x4000, 0x4c00, 0x7bfe, 0x7c00, 0xfc00, 0x7e00,
    ];
    // Alternate bounded HDR and pathological HDR so bright neighbors cannot
    // hide threshold/soft-knee differences in every test image.
    for (case, (width, height)) in cases.enumerate() {
        let values = if case % 2 == 0 {
            &values[..7]
        } else {
            &values[..]
        };
        let mut bytes = Vec::with_capacity((physical.0 * physical.1 * 8) as usize);
        for y in 0..physical.1 {
            for x in 0..physical.0 {
                for channel in 0..4 {
                    let half = if x >= width || y >= height {
                        0x7bfe
                    } else {
                        values[((x * 17 + y * 31 + channel * 7) as usize) % values.len()]
                    };
                    bytes.extend_from_slice(&half.to_le_bytes());
                }
            }
        }
        gpu.queue.write_texture(
            source.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(physical.0 * 8),
                rows_per_image: None,
            },
            source.size(),
        );
        gpu.queue.write_buffer(
            &targets.sampling,
            0,
            crate::buffers::bytes(&[width as f32, height as f32, 0., 0.]),
        );
        let size = ((width / 2).max(1), (height / 2).max(1));
        let actual = texture(&gpu, size);
        let expected = texture(&gpu, size);
        let actual_view = actual.create_view(&Default::default());
        let expected_view = expected.create_view(&Default::default());
        let destination = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom test destination"),
            layout: &pipelines.bright_target_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&actual_view),
            }],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipelines.bright);
            pass.set_bind_group(0, &targets.source, &[0]);
            pass.set_bind_group(1, &destination, &[]);
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
        }
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &expected_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(&reference);
            pass.set_bind_group(0, &targets.source, &[0]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        for (index, (a, b)) in read(&gpu, &actual)
            .into_iter()
            .zip(read(&gpu, &expected))
            .enumerate()
        {
            assert_ne!(
                a & 0x7c00,
                0x7c00,
                "nonfinite output at {width}x{height} channel {index}"
            );
            // Compare before tonemapping, allowing at most two representable
            // half-float steps for GPU arithmetic contraction.
            assert!(
                a.abs_diff(b) <= 2,
                "{width}x{height} channel {index}: {a:04x} != {b:04x}"
            );
        }
    }
}
