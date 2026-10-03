//! Frame cost on this machine's GPU, offscreen: the real `WorldSurface` the
//! canvas runs, a live clock at 120 Hz, a seven-bot free-for-all at 1920×1080
//! (device pixels), every frame submitted and waited for. Prints wall time per
//! frame (simulation + feed + encode + GPU) and the surface's own GPU pass times.
//! `cargo test -p rivals-logic --release --test frame -- --ignored --nocapture`
use exact_game::Value;
use exact_game_render::exact_gpu::{fixture, wgpu, Frame, Surface};
use exact_game_render::WorldSurface;
use rivals_logic::Rivals;
use std::time::Instant;

type Canvas = WorldSurface<Rivals>;

#[test]
#[ignore]
fn frame_time_ffa_1080p() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = Canvas::default();
    surface.device_ready(gpu.device.features());
    surface.bind(&[Value::Number(7.0), Value::Number(7.0)], None).unwrap();
    // Deliver every requested asset from the bake's output, then prepare them.
    for _ in 0..4 {
        let requests = surface.assets().requests;
        if requests.is_empty() {
            break;
        }
        for name in requests {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets").join(&name);
            surface.asset(&name, Ok(&std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}"))));
        }
        surface.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let (width, height) = (960.0, 540.0);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("frame bench"),
        size: wgpu::Extent3d { width: 1920, height: 1080, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let period = 1000.0 / 120.0;
    let mut walls = Vec::new();
    let armed = surface.agent(r#"{"op":"state","perf":true}"#).is_some();
    for i in 0..900 {
        let frame = Frame {
            width,
            height,
            scale: 2.0,
            now_ms: i as f64 * period,
            seekable: false,
            period_ms: period,
            children_generation: 0,
            shader_generation: 0,
        };
        let start = Instant::now();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        surface.render(&frame, &gpu.device, &gpu.queue, &mut encoder, &view, format);
        gpu.queue.submit([encoder.finish()]);
        surface.submitted();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        if i >= 300 {
            walls.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
    assert!(surface.error().is_none(), "{:?}", surface.error());
    walls.sort_by(f64::total_cmp);
    let q = |p: f64| walls[((walls.len() - 1) as f64 * p) as usize];
    println!(
        "frame wall ms (sim+feed+encode+GPU, 1920x1080, 4xMSAA): p50 {:.2} p95 {:.2} p99 {:.2} max {:.2}",
        q(0.5),
        q(0.95),
        q(0.99),
        walls[walls.len() - 1]
    );
    let state = surface.agent(r#"{"op":"state","perf":true}"#).unwrap_or_default();
    let perf = state.find("\"perf\"").map(|i| &state[i..]).unwrap_or("");
    println!("armed {armed}; perf: {}", &perf[..perf.len().min(1800)]);
}
