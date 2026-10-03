//! Frames of a filled garden rendered offscreen on this machine's GPU, with
//! the renderer's own counters. Ignored by default; run in release:
//! `cargo test --release --manifest-path game/games/garden/.shells/Cargo.toml
//!  -p garden-logic --test render -- --ignored --nocapture --test-threads 1`
use exact_game::{Args, Value};
use exact_game_render::exact_gpu::{fixture, Frame, Surface};
use exact_game_render::WorldSurface;
use garden_logic::{Garden, Options};
use std::time::Instant;

fn bind(surface: &mut WorldSurface<Garden>, cmd: &str, id: u32) {
    let o = Options {
        seed: 1,
        cmd: cmd.into(),
        cmd_id: id,
        smooth: std::env::var("GARDEN_SMOOTH").is_ok(),
        ..Options::default()
    };
    let values: Vec<Value> = o.values();
    surface.bind(&values, None).unwrap();
}

fn field<'a>(state: &'a str, key: &str) -> &'a str {
    let at = state
        .find(&format!("\"{key}\":"))
        .map(|i| i + key.len() + 3);
    at.map(|i| {
        let rest = &state[i..];
        let end = if rest.starts_with('{') {
            rest.find('}').map(|e| e + 1).unwrap_or(rest.len())
        } else {
            rest.find([',', '}']).unwrap_or(rest.len())
        };
        &rest[..end]
    })
    .unwrap_or("?")
}

#[test]
#[ignore]
fn frames_at_scale() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let sizes: Vec<u32> = std::env::var("GARDEN_SIZES")
        .ok()
        .map(|s| s.split(',').map(|n| n.parse().unwrap()).collect())
        .unwrap_or(vec![100, 500, 2_000, 10_000, 20_000, 50_000]);
    println!("| plants | entities | wall ms/frame mean / p95 / max | feed ms | encode ms | draws | instances | triangles | gpu passes |");
    for n in sizes {
        let mut surface = WorldSurface::<Garden>::default();
        bind(&mut surface, "", 0);
        let mut frame = Frame {
            width: 1280.,
            height: 720.,
            scale: 1.,
            now_ms: 0.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        };
        let step = |surface: &mut WorldSurface<Garden>, frame: &mut Frame, ms: f64| {
            frame.now_ms += ms;
            fixture::render(&gpu, surface, frame).unwrap();
        };
        step(&mut surface, &mut frame, 0.);
        bind(&mut surface, "zoom", 1);
        step(&mut surface, &mut frame, 34.);
        bind(&mut surface, &format!("fill {n}"), 2);
        step(&mut surface, &mut frame, 34.);
        // Two and a half minutes on: most plants mature, fruit on the vine.
        // GARDEN_AT=5000 measures while everything is still growing.
        let at = std::env::var("GARDEN_AT").map_or(150_000., |s| s.parse().unwrap());
        step(&mut surface, &mut frame, at);
        let entities = surface.sim().unwrap().world().len();
        surface.agent(r#"{"op":"state","perf":true}"#);
        frame.seekable = false;
        frame.period_ms = 1000. / 60.;
        let mut wall = Vec::new();
        for _ in 0..180 {
            let t = Instant::now();
            step(&mut surface, &mut frame, 1000. / 60.);
            wall.push(t.elapsed().as_secs_f64() * 1000.);
        }
        let state = surface.agent(r#"{"op":"state"}"#).unwrap();
        let mean = wall.iter().sum::<f64>() / wall.len() as f64;
        wall.sort_by(f64::total_cmp);
        let feed = field(&state, "feedMs");
        let encode = field(&state, "encodeMs");
        println!(
            "| {n} | {entities} | {mean:.1} / {:.1} / {:.1} | {} | {} | {} | {} | {} | {} |",
            wall[wall.len() * 95 / 100],
            wall[wall.len() - 1],
            field(feed, "mean"),
            field(encode, "mean"),
            field(&state, "draws"),
            field(&state, "instances"),
            field(&state, "triangles"),
            field(&state, "culled"),
        );
    }
}
