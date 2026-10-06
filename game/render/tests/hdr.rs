//! LLP 1100 D12b: the tone curve reaches the frame's headroom.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;

use exact_game_render::{Environment, FrameInput, Renderer};
use exact_gpu::{wgpu, Gpu};
use glam::camera::rh::{proj::directx, view};
use glam::Vec3;

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(exact_gpu::fixture::device())
}

/// IEEE half to f32 (normal and zero values: what a tone curve writes).
fn half(bits: u16) -> f32 {
    let exp = i32::from((bits >> 10) & 0x1f);
    let mant = f32::from(bits & 0x3ff);
    let v = if exp == 0 {
        mant / 1024.0 * 2f32.powi(-14)
    } else {
        (1.0 + mant / 1024.0) * 2f32.powi(exp - 15)
    };
    if bits & 0x8000 != 0 {
        -v
    } else {
        v
    }
}

/// The centre pixel of an 8×8 `Rgba16Float` target after one frame.
fn centre(gpu: &Gpu, renderer: &mut Renderer, frame: &FrameInput<'_>) -> [f32; 3] {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hdr test"),
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.draw(&texture.create_view(&Default::default()), (8, 8), frame);
    let row = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row) * 8,
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
        wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let bytes = buffer.slice(..).get_mapped_range().unwrap();
    let at = (4 * row as usize) + 4 * 8;
    let px: Vec<f32> = (0..3)
        .map(|c| {
            half(u16::from_le_bytes([
                bytes[at + c * 2],
                bytes[at + c * 2 + 1],
            ]))
        })
        .collect();
    [px[0], px[1], px[2]]
}

#[test]
fn the_tone_curve_reaches_the_frames_headroom() {
    let Some(gpu) = gpu() else {
        return;
    };
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba16Float);
    let mut frame = FrameInput {
        view: view::look_at_mat4(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y),
        proj: directx::perspective(1.0, 1.0, 0.1, 100.0),
        camera_position: Vec3::new(0.0, 0.0, 10.0),
        sun: None,
        environment: Environment {
            background: None,
            zenith: [9.0; 3],
            ground: [8.0; 3],
            ambient: 0.0,
            horizon: [8.0; 3],
            sun_disc: 0.0,
            fog: None,
            exposure: 1.0,
            bloom: None,
        },
        ..Default::default()
    };
    let sdr = centre(&gpu, &mut renderer, &frame);
    assert!(
        sdr.iter().all(|c| *c > 0.9 && *c <= 1.0),
        "headroom 1: SDR white at most, {sdr:?}"
    );
    frame.headroom = 4.0;
    let hdr = centre(&gpu, &mut renderer, &frame);
    // 4·f(8/4) = 3.66 linear, sRGB-encoded (extended) ≈ 1.756.
    assert!(
        hdr.iter().all(|c| (*c - 1.756).abs() < 0.02),
        "headroom 4: past SDR white, {hdr:?}"
    );
}
