//! Offscreen diagnostic, not vsync/FPS: cargo run --release -p exact-game-render
//! --example cubes -- [N] [frames]. With no N, run 10k, 100k, 200k and 500k.
use exact_game::{
    math, Camera, Clock, Component, DirectionalLight, Entity, Environment, Game, Input, Material,
    Mesh, Quat, Resource, Sim, Transform, Vec3, World,
};
use exact_game_render::{Feed, Renderer};
use exact_gpu::{fixture, wgpu};
use std::time::Instant;

#[derive(Default, Component)]
struct Spin {
    step: Quat,
}
#[derive(Default, Resource)]
struct Orbit {
    camera: Entity,
    radius: f32,
}
struct Cubes;
#[derive(Default, Resource)]
struct Still {
    moving: u64,
}
#[derive(Default, exact_game::Args)]
struct CubesArgs {
    /// Canvas setup argument.
    pub n: u64,
}
impl Game for Cubes {
    const ID: &'static str = "cubes-bench";
    type Args = CubesArgs;
    fn setup(w: &mut World, a: &Self::Args) {
        let n = a.n as usize;
        let side = (n as f64).cbrt().ceil() as usize;
        for i in 0..n {
            let axis = Vec3::new(
                frac(i as f32 * 0.3719) - 0.5,
                frac(i as f32 * 0.7331) - 0.5,
                frac(i as f32 * 0.1913) - 0.5,
            )
            .normalize_or(Vec3::Y);
            let step = Quat::from_axis_angle(axis, (0.5 + (i % 7) as f32 * 0.25) / Self::HZ as f32);
            let position = (Vec3::new(
                (i % side) as f32,
                (i / side % side) as f32,
                (i / (side * side)) as f32,
            ) - Vec3::splat((side - 1) as f32 * 0.5))
                * 2.;
            let color = hue(frac(i as f32 * 0.61803));
            w.spawn((
                Transform {
                    position,
                    ..Default::default()
                },
                Mesh::cube(1.0),
                Material::rgb(color[0], color[1], color[2]),
                Spin { step },
            ));
        }
        let radius = 1.8 * side as f32 + 6.;
        let camera = w.spawn((
            camera(radius, 0.),
            Camera {
                far: radius * 5.,
                ..Default::default()
            },
        ));
        w.insert_resource(Orbit { camera, radius });
        w.spawn((
            Transform::at(5., 10., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight {
                shadows: false,
                ..Default::default()
            },
        ));
        w.insert_resource(Environment {
            zenith: [0.25; 3],
            horizon: [0.25; 3],
            ground: [0.25; 3],
            ambient: 1.,
            sun_disc: 0.,
            bloom: None,
            ..Default::default()
        });
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        let moving = w
            .try_resource::<Still>()
            .map_or(usize::MAX, |s| s.moving as usize);
        {
            for (_, (spin, t)) in w.query::<(&Spin, &mut Transform)>().iter().take(moving) {
                t.rotation = (spin.step * t.rotation).normalize();
            }
        }
        let orbit = w.resource::<Orbit>();
        *w.get_mut::<Transform>(orbit.camera).unwrap() =
            camera(orbit.radius, (w.tick() + 1) as f32 / Self::HZ as f32);
    }
}
fn frac(x: f32) -> f32 {
    x - math::floor(x)
}
fn hue(h: f32) -> [f32; 3] {
    let x = h * 6.;
    let f = frac(x);
    match x as u32 % 6 {
        0 => [1., f, 0.],
        1 => [1. - f, 1., 0.],
        2 => [0., 1., f],
        3 => [0., 1. - f, 1.],
        4 => [f, 0., 1.],
        _ => [1., 0., 1. - f],
    }
}
fn camera(radius: f32, seconds: f32) -> Transform {
    let theta = seconds * std::f32::consts::TAU / 20.;
    Transform::at(
        math::sin(theta) * radius,
        radius * 0.35,
        math::cos(theta) * radius,
    )
    .looking_at(Vec3::ZERO, Vec3::Y)
}
fn summary(values: &mut [f64]) -> String {
    if values.is_empty() {
        return "unavailable".into();
    }
    values.sort_unstable_by(f64::total_cmp);
    format!(
        "p50={:.4} p95={:.4}",
        values[(values.len() * 50).div_ceil(100) - 1],
        values[(values.len() * 95).div_ceil(100) - 1]
    )
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let counts = args
        .first()
        .map(|s| vec![s.parse::<usize>().expect("N is an integer")])
        .unwrap_or_else(|| vec![10_000, 100_000, 200_000, 500_000]);
    let frames: usize = args
        .get(1)
        .map_or(240, |s| s.parse().expect("frames is an integer"));
    assert!(frames > 0);
    let gpu = match fixture::device() {
        Ok(gpu) => {
            eprintln!("adapter: {:?}", gpu.adapter.get_info());
            Some(gpu)
        }
        Err(reason) => {
            eprintln!("SKIP GPU feed/encode timings: {reason}; simulation still runs");
            None
        }
    };
    for n in counts {
        assert!(n > 0);
        let mut sim = Sim::<Cubes>::new(CubesArgs { n: n as u64 }).unwrap();
        let mode = args.get(2).map_or("all", String::as_str);
        if mode != "all" {
            sim.world_mut().insert_resource(Still {
                moving: if mode == "one-percent" {
                    (n / 100) as u64
                } else {
                    0
                },
            });
        }
        let mut feed = Feed::default();
        feed.filter_same_values(mode != "all");
        let mut renderer = gpu
            .as_ref()
            .map(|g| Renderer::new(&g.device, &g.queue, wgpu::TextureFormat::Rgba8Unorm));
        let target = gpu.as_ref().map(|g| {
            g.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("cubes 2560x1440"),
                    size: wgpu::Extent3d {
                        width: 2560,
                        height: 1440,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });
        if let Some(r) = &mut renderer {
            feed.feed(sim.world(), r).unwrap();
        }
        sim.advance(0., Clock::Live);
        let mut ticks = Vec::with_capacity(frames);
        let mut feeds = Vec::with_capacity(frames);
        let mut encodes = Vec::with_capacity(frames);
        for i in 0..frames + 60 {
            let now = ((i + 1) as f64 * 1000. / 60.) + 0.001;
            let start = Instant::now();
            let mut tick_ms = 0.;
            let mut feed_ms = 0.;
            sim.advance_with(now, Clock::Live, |w, left| {
                tick_ms = start.elapsed().as_secs_f64() * 1000.;
                if left < 2 {
                    if let Some(r) = &mut renderer {
                        let start = Instant::now();
                        feed.feed(w, r).unwrap();
                        feed_ms = start.elapsed().as_secs_f64() * 1000.;
                    }
                }
            });
            if i >= 60 {
                ticks.push(tick_ms);
                if gpu.is_some() {
                    feeds.push(feed_ms);
                }
            }
            if let (Some(g), Some(r), Some(target)) = (&gpu, &mut renderer, &target) {
                let start = Instant::now();
                let frame = feed.frame(sim.world(), sim.alpha(), 2560. / 1440.);
                r.draw(target, (2560, 1440), &frame);
                if i >= 60 {
                    encodes.push(start.elapsed().as_secs_f64() * 1000.);
                }
                // Bound work in flight; the wait is explicitly outside CPU timings.
                g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
        println!("mode={mode} N={n} 2560x1440 4xMSAA frames={frames} | sim/tick ms {} | feed/tick ms {} | encode/frame ms {}",summary(&mut ticks),summary(&mut feeds),summary(&mut encodes));
    }
}
