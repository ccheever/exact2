//! Offscreen diagnostic, not vsync/FPS: cargo run --release -p exact-game-render
//! --example cubes -- [N] [frames] [all|one-percent|still|field]. With no N, run
//! 10k, 100k, 200k and 500k. `field` puts the camera at eye level inside N pillars
//! with sun shadows (most of them off screen, tall casters all around); `timed` is
//! the all-visible orbit. Both report GPU pass times and each view's drawn instances;
//! CUBES_TIMELINE=1 prints each frame's pass start/duration (the GPU may be shared).
//! `cpu-cull` times the rejected alternative: the same swept-sphere test on the CPU.
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
            fog: None,
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
/// Pillars (boxes, cylinders) and balls on a 4 m grid around an eye-level camera
/// turning once per 20 s.
struct Field;
#[derive(Default, Component)]
struct Look;
impl Game for Field {
    const ID: &'static str = "cubes-field";
    type Args = CubesArgs;
    fn setup(w: &mut World, a: &Self::Args) {
        let n = a.n as usize;
        let side = (n as f64).sqrt().ceil() as usize;
        let half = (side as f32 - 1.) * 2.;
        w.spawn((
            Transform::default(),
            Mesh::plane(side as f32 * 4. + 8., side as f32 * 4. + 8.),
            Material::rgb(0.3, 0.32, 0.3),
        ));
        for i in 0..n {
            let height = 1. + 11. * frac(i as f32 * 0.61803);
            let color = hue(frac(i as f32 * 0.3719));
            let mesh = match i % 3 {
                0 => Mesh::cuboid(Vec3::new(1., height, 1.)),
                1 => Mesh::cylinder(0.5, height),
                _ => Mesh::sphere(0.8),
            };
            w.spawn((
                Transform::at(
                    (i % side) as f32 * 4. - half + 2.,
                    if i % 3 == 2 { 0.8 } else { height * 0.5 },
                    (i / side) as f32 * 4. - half + 2.,
                ),
                mesh,
                Material::rgb(color[0], color[1], color[2]),
            ));
        }
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 1.7, 0.),
                Camera {
                    far: 300.,
                    ..Default::default()
                },
                Look,
            ),
        );
        w.spawn((
            Transform::at(-6., 4., 3.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.insert_resource(Environment {
            bloom: None,
            ..Default::default()
        });
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        let seconds = (w.tick() + 1) as f32 / Self::HZ as f32;
        w.require_mut::<Transform>("camera").rotation =
            Quat::from_rotation_y(seconds * std::f32::consts::TAU / 20.);
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
    let mode = args.get(2).map_or("all", String::as_str);
    if mode == "cpu-cull" {
        for n in counts {
            cpu_cull(n, frames);
        }
        return;
    }
    if mode == "field" || mode == "timed" {
        for n in counts {
            if mode == "field" {
                measure::<Field>(gpu.as_ref(), n, frames, "field");
            } else {
                measure::<Cubes>(gpu.as_ref(), n, frames, "all-visible");
            }
        }
        return;
    }
    for n in counts {
        assert!(n > 0);
        let mut sim = Sim::<Cubes>::new(CubesArgs { n: n as u64 }).unwrap();
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

// Timestamps resolved and read after every frame; this measures, it does not pace.
struct Queries {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
}
impl Queries {
    fn new(g: &exact_gpu::Gpu) -> Option<Self> {
        if !g
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let bytes = u64::from(exact_game_render::GPU_PASS_COUNT) * 16;
        let buffer = |usage| {
            g.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: bytes,
                usage,
                mapped_at_creation: false,
            })
        };
        Some(Self {
            set: g.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: exact_game_render::GPU_PASS_COUNT * 2,
            }),
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC),
            read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ),
        })
    }
    /// Per-pass milliseconds, then the whole frame's first-to-last envelope.
    fn read(&self, g: &exact_gpu::Gpu) -> Vec<f64> {
        let pairs = exact_game_render::GPU_PASS_COUNT;
        g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let mut encoder = g.device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(&self.set, 0..pairs * 2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, u64::from(pairs) * 16);
        g.queue.submit([encoder.finish()]);
        self.read
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = self.read.slice(..).get_mapped_range().unwrap();
        let stamps: Vec<u64> = data
            .chunks_exact(8)
            .map(|b| u64::from_ne_bytes(b.try_into().unwrap()))
            .collect();
        drop(data);
        self.read.unmap();
        let period = f64::from(g.queue.get_timestamp_period()) / 1e6;
        if std::env::var_os("CUBES_TIMELINE").is_some() {
            let first = stamps.iter().copied().filter(|&s| s > 0).min().unwrap_or(0);
            let names = exact_game_render::GPU_PASS_NAMES;
            let line: Vec<String> = stamps
                .chunks_exact(2)
                .enumerate()
                .filter(|(_, p)| p[0] > 0)
                .map(|(i, p)| {
                    format!(
                        "{}@{:.3}+{:.3}",
                        names[i],
                        (p[0] - first) as f64 * period,
                        p[1].saturating_sub(p[0]) as f64 * period
                    )
                })
                .collect();
            eprintln!("timeline {}", line.join(" "));
        }
        let mut ms: Vec<f64> = stamps
            .chunks_exact(2)
            .map(|p| {
                if p[0] > 0 && p[1] >= p[0] {
                    (p[1] - p[0]) as f64 * period
                } else {
                    0.
                }
            })
            .collect();
        let first = stamps.iter().copied().filter(|&s| s > 0).min().unwrap_or(0);
        let last = stamps.iter().copied().max().unwrap_or(0);
        ms.push(last.saturating_sub(first) as f64 * period);
        ms
    }
}

fn measure<G: Game<Args = CubesArgs>>(
    gpu: Option<&exact_gpu::Gpu>,
    n: usize,
    frames: usize,
    label: &str,
) {
    let Some(g) = gpu else {
        eprintln!("SKIP {label}: no GPU");
        return;
    };
    let (width, height) = (2560, 1440);
    let mut sim = Sim::<G>::new(CubesArgs { n: n as u64 }).unwrap();
    let mut feed = Feed::default();
    let mut r = Renderer::new(&g.device, &g.queue, wgpu::TextureFormat::Rgba8Unorm);
    let queries = Queries::new(g);
    let target = g
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    feed.feed(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Live);
    let (mut encodes, mut draws, mut views) = (Vec::new(), 0, [0u64; 4]);
    // Per pass (then the envelope), every frame: medians resist shared-GPU stalls.
    let mut gpu_ms = vec![Vec::new(); exact_game_render::GPU_PASS_COUNT as usize + 1];
    for i in 0..frames + 30 {
        let now = (i + 1) as f64 * 1000. / 60. + 0.001;
        sim.advance_with(now, Clock::Live, |w, left| {
            if left < 2 {
                feed.feed(w, &mut r).unwrap();
            }
        });
        // The per-view readback is diagnostic work: only the last frames pay for it.
        r.count_culled(i + 5 >= frames + 30);
        let start = Instant::now();
        let mut frame = feed.frame(sim.world(), sim.alpha(), width as f32 / height as f32);
        frame.timestamps = queries.as_ref().map(|q| &q.set);
        let stats = r.draw(&target, (width, height), &frame);
        let encode = start.elapsed().as_secs_f64() * 1000.;
        let times = queries.as_ref().map(|q| q.read(g));
        g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        if i >= 30 {
            encodes.push(encode);
            draws = stats.draws;
            if let Some(times) = times {
                for (column, t) in gpu_ms.iter_mut().zip(times) {
                    column.push(t);
                }
            }
            if let Some(v) = r.culled() {
                views = v;
            }
        }
    }
    let names = exact_game_render::GPU_PASS_NAMES;
    let median = |column: &mut Vec<f64>| {
        column.sort_unstable_by(f64::total_cmp);
        column.get(column.len() / 2).copied().unwrap_or(0.)
    };
    let envelope = median(gpu_ms.last_mut().unwrap());
    let mut pass = |name: &str| median(&mut gpu_ms[names.iter().position(|n| *n == name).unwrap()]);
    println!(
        "{label} N={n} {width}x{height} 4xMSAA frames={frames} | draws {draws} | instances camera {} shadows {:?} | encode/frame ms {} | GPU ms p50: envelope {:.3} shadow0 {:.3} shadow1 {:.3} shadow2 {:.3} forward {:.3} cull {:.3}",
        views[0],
        &views[1..],
        summary(&mut encodes),
        envelope,
        pass("shadow 0"),
        pass("shadow 1"),
        pass("shadow 2"),
        pass("forward + sky"),
        pass("cull"),
    );
}

// What CPU culling would cost per frame: both tick poses per item (the renderer keeps
// no CPU copy of either history), cull.wgsl's swept sphere, four views, and stable
// compaction into preallocated per-view lists (upload not included).
fn cpu_cull(n: usize, frames: usize) {
    let mut seed = 1u64;
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 40) as f32 / 16777216.
    };
    let poses: Vec<[Transform; 2]> = (0..n)
        .map(|_| {
            let t = Transform {
                position: Vec3::new(next(), next(), next()) * 1000. - 500.,
                rotation: Quat::from_rotation_y(next() * 6.),
                scale: Vec3::ONE,
            };
            [
                t,
                Transform {
                    position: t.position + Vec3::X * 0.1,
                    ..t
                },
            ]
        })
        .collect();
    let camera = glam::camera::rh::proj::directx::perspective(1., 16. / 9., 0.1, 300.)
        * glam::camera::rh::view::look_at_mat4(Vec3::ZERO, Vec3::X, Vec3::Y);
    let views: Vec<[glam::Vec4; 6]> = (0..4)
        .map(|v| {
            let m = if v == 0 {
                camera
            } else {
                glam::camera::rh::proj::directx::orthographic(-40., 40., -40., 40., -200., 200.)
            };
            let [r0, r1, r2, r3] = [m.row(0), m.row(1), m.row(2), m.row(3)];
            [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(|p| p / p.truncate().length())
        })
        .collect();
    let mut lists: Vec<Vec<u32>> = (0..4).map(|_| Vec::with_capacity(n)).collect();
    let mut times = Vec::with_capacity(frames);
    for _ in 0..frames {
        let start = Instant::now();
        for list in &mut lists {
            list.clear();
        }
        for (i, [a, b]) in poses.iter().enumerate() {
            let (u0, u1) = (a.scale * 0.5, b.scale * 0.5);
            let mid = (u0 + u1) * 0.5;
            let (e0, e1) = (a.rotation * mid, b.rotation * mid);
            let center = (a.position + b.position + e0 + e1) * 0.5;
            let radius = a.position.distance(b.position) * 0.5
                + e0.distance(e1) * 0.5
                + u0.distance(u1) * 0.5
                + a.scale.max_element().max(b.scale.max_element()) * 0.87;
            for (list, planes) in lists.iter_mut().zip(&views) {
                if planes
                    .iter()
                    .all(|p| p.truncate().dot(center) + p.w >= -radius)
                {
                    list.push(i as u32);
                }
            }
        }
        times.push(start.elapsed().as_secs_f64() * 1000.);
        std::hint::black_box(&lists);
    }
    println!(
        "cpu-cull N={n} 4 views frames={frames} | cull+compact ms {} | kept {:?} | upload per frame {} bytes",
        summary(&mut times),
        lists.iter().map(Vec::len).collect::<Vec<_>>(),
        lists.iter().map(|l| l.len() * 4).sum::<usize>()
    );
}
