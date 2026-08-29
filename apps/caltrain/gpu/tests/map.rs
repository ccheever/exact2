//! The line map, headless: rendered into a module-owned texture on this
//! machine's GPU and read back (LLP 1009 D4's fixture path). Skips, saying
//! so, when no adapter exists.

use caltrain_gpu::MapSurface;
use exact_gpu::wgpu;
use exact_gpu::{Frame, Surface, Value};

fn station(id: &str) -> Value {
    Value::record(vec![
        Value::str(id),
        Value::str(id),
        Value::Number(1.0),
        Value::Number(100.0),
    ])
}

#[test]
fn the_map_binds_its_inputs_and_lays_out_stations_on_a_line() {
    let mut map = MapSurface::new();
    let line = Value::List(vec![station("sf"), station("mv"), station("sj")].into());
    let board = Value::List(
        vec![Value::record(vec![
            Value::str("d1"),
            Value::Number(131.0),
            Value::str("Local"),
            Value::str("San Francisco"),
            Value::Number(600_000.0),
        ])]
        .into(),
    );
    map.bind(&[line.clone(), Value::str("mv"), board, Value::Number(0.0)])
        .unwrap();
    let v = map.vertices(200.0, 160.0);
    // The line (6), three stations (18), the train (6).
    assert_eq!(v.len(), 30);
    let red = v.iter().filter(|x| x[2] > 0.7 && x[3] < 0.3).count();
    assert_eq!(red, 6, "the selected station is the red dot");
    assert!(
        map.bind(&[
            line,
            Value::Number(1.0),
            Value::List(vec![].into()),
            Value::Number(0.0)
        ])
        .is_err(),
        "a wrong input is refused by name"
    );
}

#[test]
fn the_map_renders_and_reads_back_on_this_machines_gpu() {
    let instance = wgpu::Instance::default();
    let Ok(gpu) = exact_gpu::block_on(exact_gpu::load_gpu(instance, None)) else {
        eprintln!("no GPU adapter here; the readback fixture is skipped");
        return;
    };
    let (w, h) = (200u32, 160u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("fixture"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut map = MapSurface::new();
    let line = Value::List(vec![station("sf"), station("mv"), station("sj")].into());
    map.bind(&[
        line,
        Value::str("mv"),
        Value::List(vec![].into()),
        Value::Number(0.0),
    ])
    .unwrap();
    let frame = Frame {
        width: w as f32,
        height: h as f32,
        scale: 1.0,
        now_ms: 0.0,
    };
    let wants = map.render(&frame, &gpu.device, &gpu.queue, &view, format);
    assert!(!wants, "a static picture wants no more frames");
    // Read back.
    let row = (w * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (row * h) as u64,
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
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = slice.get_mapped_range().unwrap();
    let px = |x: u32, y: u32| {
        let i = (y * row + x * 4) as usize;
        [data[i], data[i + 1], data[i + 2]]
    };
    assert_eq!(
        px(5, 5),
        [247, 247, 247],
        "the background is the clear color"
    );
    let mid = px(100, 80);
    assert!(
        mid[0] > 150 && mid[1] < 100,
        "the selected station's red dot sits mid-line: {mid:?}"
    );
    let on_line = px(100, 40);
    assert!(
        on_line[0] > 170 && on_line[0] < 200 && on_line[0] == on_line[1],
        "the line is gray: {on_line:?}"
    );
}

#[test]
fn the_aurora_renders_a_lit_sky_and_wants_every_frame() {
    use caltrain_gpu::AuroraSurface;
    let instance = wgpu::Instance::default();
    let Ok(gpu) = exact_gpu::block_on(exact_gpu::load_gpu(instance, None)) else {
        eprintln!("no GPU adapter here; skipped");
        return;
    };
    let (w, h) = (64u32, 64u32);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut sky = AuroraSurface::new();
    assert!(
        sky.bind(&[Value::Number(1.0)]).is_err(),
        "the seed is a string"
    );
    sky.bind(&[Value::str("mv")]).unwrap();
    let frame = Frame {
        width: w as f32,
        height: h as f32,
        scale: 1.0,
        now_ms: 1234.0,
    };
    assert!(
        sky.render(&frame, &gpu.device, &gpu.queue, &view, format),
        "lit from the clock: wants every frame"
    );
    let row = 256u32;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
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
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let data = slice.get_mapped_range().unwrap();
    let mut distinct = std::collections::HashSet::new();
    let mut lit = 0;
    for y in 0..h {
        for x in 0..w {
            let i = (y * row + x * 4) as usize;
            let p = [data[i], data[i + 1], data[i + 2]];
            distinct.insert(p);
            if p.iter().any(|c| *c > 60) {
                lit += 1;
            }
        }
    }
    assert!(
        distinct.len() > 200,
        "a field, not a fill: {} colors",
        distinct.len()
    );
    assert!(
        lit > 200,
        "ribbons light the sky: {lit} lit pixels of {}",
        w * h
    );
}
