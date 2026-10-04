//! The art pass's renderer features: per-instance opacity and tint on models,
//! the visible sky, textured generated meshes.
#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::{ModelPresentation, WorldSurface};
use exact_gpu::{fixture, Frame, Gpu, Surface, Value};

fn gpu() -> Option<Gpu> {
    test_device::device_or_skip(fixture::device())
}
fn frame() -> Frame {
    Frame {
        width: 128.,
        height: 128.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    }
}
fn quad(colors: [f32; 4]) -> asset::MeshData {
    asset::MeshData {
        positions: vec![-1., -1., 0., 1., -1., 0., 1., 1., 0., -1., 1., 0.],
        normals: [0., 0., 1.].repeat(4),
        uvs: vec![0., 1., 1., 1., 1., 0., 0., 0.],
        colors: colors.repeat(4),
        indices: vec![0, 1, 2, 0, 2, 3],
        bounds: [-1., -1., 0., 1., 1., 0.],
        ..Default::default()
    }
}

#[derive(Default, Args)]
struct FadeArgs {
    opacity: f32,
    tint: bool,
    primitive: bool,
}
struct Fade;
impl Game for Fade {
    const ID: &'static str = "look-fade";
    type Args = FadeArgs;
    fn setup(w: &mut World, args: &FadeArgs) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            ambient: 1.,
            zenith: [1.; 3],
            horizon: [1.; 3],
            ground: [1.; 3],
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 3.), Camera::default()));
        let e = if args.primitive {
            w.spawn((Transform::default(), Mesh::cube(2.)))
        } else {
            let panel = w.generated("panel.model", quad([1.; 4])).unwrap();
            w.spawn((Transform::default(), panel))
        };
        if args.opacity < 1. {
            w.insert(e, Opacity(args.opacity));
        }
        if args.tint {
            w.insert(e, Material::rgb(1., 0.2, 0.2));
        }
    }
    fn tick(_: &mut World, _: &Input, _: &FadeArgs) {}
}
fn fade(gpu: &Gpu, opacity: f32, tint: bool, primitive: bool) -> fixture::Pixels {
    let mut s = WorldSurface::<Fade, ModelPresentation, true>::default();
    let args = [
        Value::Number(f64::from(opacity)),
        Value::Bool(tint),
        Value::Bool(primitive),
    ];
    s.bind(&args, None).unwrap();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let (pixels, _) = fixture::render(gpu, &mut s, &frame()).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    pixels
}
// Lit panel pixels in the centre 32 x 32.
fn lit(p: &fixture::Pixels) -> usize {
    (48..80)
        .flat_map(|y| (48..80).map(move |x| (x, y)))
        .filter(|&(x, y)| p.at(x, y)[1] > 40 || p.at(x, y)[0] > 40)
        .count()
}

#[test]
fn model_opacity_dithers_and_tint_multiplies() {
    let Some(gpu) = gpu() else { return };
    let counts = [1., 0.5, 0.25, 0.].map(|o| lit(&fade(&gpu, o, false, false)));
    eprintln!("lit centre pixels at opacity 1, 0.5, 0.25, 0: {counts:?}");
    assert_eq!(counts[0], 32 * 32);
    // Screen-door coverage, within the MSAA resolve of the dither's edges.
    assert!((counts[1] as i32 - 512).abs() < 80, "{counts:?}");
    assert!((counts[2] as i32 - 256).abs() < 80, "{counts:?}");
    assert_eq!(counts[3], 0);
    let (plain, tinted) = (
        fade(&gpu, 1., false, false).at(64, 64),
        fade(&gpu, 1., true, false).at(64, 64),
    );
    assert!(
        tinted[0] >= plain[0].saturating_sub(10) && u16::from(tinted[1]) + 50 < u16::from(plain[1]),
        "a tinted model instance: {plain:?} -> {tinted:?}"
    );
    // Primitives fade the same way.
    let cube = [1., 0.5].map(|o| lit(&fade(&gpu, o, false, true)));
    assert_eq!(cube[0], 32 * 32);
    assert!((cube[1] as i32 - 512).abs() < 80, "{cube:?}");
}

// A 16 x 8 equirect: the -Z half of the sky red, the +Z half green (columns
// 4..12 face -Z), the lower half blue.
fn sky_texture() -> asset::TextureData {
    let (w, h) = (16u32, 8u32);
    let mut level = Vec::new();
    for y in 0..h {
        for x in 0..w {
            level.extend(if y >= h / 2 {
                [0, 0, 255, 255]
            } else if (4..12).contains(&x) {
                [255, 0, 0, 255]
            } else {
                [0, 255, 0, 255]
            });
        }
    }
    let mut mips = vec![level];
    for (mw, mh) in [(8, 4), (4, 2), (2, 1), (1, 1)] {
        mips.push([128u8; 4].repeat(mw * mh));
    }
    asset::TextureData {
        width: w,
        height: h,
        mips,
        ..Default::default()
    }
}
fn sky(gpu: &Gpu, look: glam::Vec3, visible: bool, rotation: f32) -> [u8; 4] {
    use exact_game_render::{EnvironmentMapInput, FrameInput, Renderer};
    let format = exact_gpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::new(&gpu.device, &gpu.queue, format);
    r.add_texture("sky.tex", &sky_texture()).unwrap();
    let texture = gpu
        .device
        .create_texture(&exact_gpu::wgpu::TextureDescriptor {
            label: None,
            size: exact_gpu::wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: exact_gpu::wgpu::TextureDimension::D2,
            format,
            usage: exact_gpu::wgpu::TextureUsages::RENDER_ATTACHMENT
                | exact_gpu::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
    let mut f = FrameInput {
        view: glam::camera::rh::view::look_to_mat4(glam::Vec3::ZERO, look, glam::Vec3::X),
        environment_map: Some(EnvironmentMapInput {
            texture: "sky.tex",
            intensity: 1.,
            rgbm: 0.,
            visible,
            rotation,
        }),
        sun: None,
        ..Default::default()
    };
    f.environment.fog = None;
    f.environment.bloom = None;
    f.environment.sun_disc = 0.;
    f.environment.background = Some([0.; 3]);
    r.draw(&texture.create_view(&Default::default()), (32, 32), &f);
    fixture::read(gpu, &texture).unwrap().at(16, 16)
}

#[test]
fn an_environment_map_can_be_the_visible_sky_turned_by_its_yaw() {
    let Some(gpu) = gpu() else { return };
    let ahead = sky(&gpu, -glam::Vec3::Z, true, 0.);
    let behind = sky(&gpu, glam::Vec3::Z, true, 0.);
    let below = sky(&gpu, -glam::Vec3::Y + glam::Vec3::Z * 0.01, true, 0.);
    let turned = sky(&gpu, -glam::Vec3::Z, true, std::f32::consts::PI);
    let hidden = sky(&gpu, -glam::Vec3::Z, false, 0.);
    eprintln!(
        "ahead {ahead:?} behind {behind:?} below {below:?} turned {turned:?} hidden {hidden:?}"
    );
    assert!(ahead[0] > 150 && ahead[1] < 60, "{ahead:?}");
    assert!(behind[1] > 150 && behind[0] < 60, "{behind:?}");
    assert!(below[2] > 150, "{below:?}");
    assert!(
        turned[1] > 150 && turned[0] < 60,
        "half a turn shows the other half: {turned:?}"
    );
    assert_eq!(hidden, [0, 0, 0, 255], "not visible: the background stays");
}

struct Terrain;
impl Game for Terrain {
    const ID: &'static str = "look-terrain";
    const ASSETS: &'static [&'static str] = &["halves.tex"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            ambient: 1.,
            zenith: [1.; 3],
            horizon: [1.; 3],
            ground: [1.; 3],
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 3.), Camera::default()));
        let terrain = w
            .generated_model(
                "terrain.model",
                asset::Model {
                    bounds: [-1., -1., 0., 1., 1., 0.],
                    meshes: vec![quad([1.; 4])],
                    materials: vec![asset::MaterialData {
                        metallic: 0.,
                        base_color_texture: Some(0),
                        ..Default::default()
                    }],
                    textures: vec!["halves.tex".into()],
                    nodes: vec![asset::Node {
                        mesh: Some(0),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
        w.spawn((Transform::default(), terrain));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
// 8 x 8, linear, filtered: the left half red, the right half blue.
fn halves() -> asset::TextureData {
    let mut mips = Vec::new();
    for size in [8usize, 4, 2, 1] {
        let mut level = Vec::new();
        for _ in 0..size {
            for x in 0..size {
                level.extend(match (size, x < size / 2) {
                    (1, _) => [128, 0, 128, 255],
                    (_, true) => [255, 0, 0, 255],
                    _ => [0, 0, 255, 255],
                });
            }
        }
        mips.push(level);
    }
    asset::TextureData {
        width: 8,
        height: 8,
        mips,
        filter: [asset::Filter::Linear; 3],
        ..Default::default()
    }
}

#[test]
fn a_generated_model_samples_a_shared_texture_through_its_uvs() {
    let Some(gpu) = gpu() else { return };
    let mut s = WorldSurface::<Terrain, ModelPresentation, true>::default();
    s.bind(&[], None).unwrap();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    assert_eq!(s.assets().requests, vec!["halves.tex".to_owned()]);
    s.asset("halves.tex", Ok(&bin::to_vec(&halves())));
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let (pixels, _) = fixture::render(&gpu, &mut s, &frame()).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    let (left, right) = (pixels.at(48, 64), pixels.at(80, 64));
    assert!(left[0] > 150 && left[2] < 60, "left half red: {left:?}");
    assert!(
        right[2] > 150 && right[0] < 60,
        "right half blue: {right:?}"
    );
}

#[derive(Default, Args)]
struct SparkArgs {
    stretch: f32,
    textured: bool,
}
struct Sparks;
impl Game for Sparks {
    const ID: &'static str = "look-sparks";
    const ASSETS: &'static [&'static str] = &["flip.tex"];
    type Args = SparkArgs;
    fn setup(w: &mut World, args: &SparkArgs) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 10.), Camera::orthographic(4.)));
        // One spark a burst, flying right at 4 m/s, white and opaque, 0.1 m.
        let e = w.spawn((
            Transform {
                position: Vec3::new(-1., 0., 0.),
                rotation: Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
                ..Default::default()
            },
            Emitter {
                rate: 0.,
                lifetime: 1.,
                speed: 4.,
                spread: 0.,
                gravity: Vec3::ZERO,
                size: [0.1, 0.1],
                color: [[1.; 4], [1.; 4]],
                additive: false,
                ..Default::default()
            }
            .burst(1),
        ));
        let texture = if args.textured { "flip.tex" } else { "" };
        w.insert(
            e,
            ParticleLook {
                texture: texture.into(),
                atlas: [2, 1],
                stretch: args.stretch,
                ..Default::default()
            },
        );
    }
    fn tick(w: &mut World, _: &Input, _: &SparkArgs) {
        emitter::step(w);
    }
}
// A 2 x 1 atlas: frame 0 red, frame 1 green.
fn flip() -> asset::TextureData {
    asset::TextureData {
        width: 2,
        height: 1,
        mips: vec![vec![255, 0, 0, 255, 0, 255, 0, 255], vec![128, 128, 0, 255]],
        filter: [asset::Filter::Nearest; 3],
        ..Default::default()
    }
}
fn sparks(gpu: &Gpu, stretch: f32, textured: bool, now_ms: f64) -> fixture::Pixels {
    let mut s = WorldSurface::<Sparks, ModelPresentation, true>::default();
    s.bind(
        &[Value::Number(f64::from(stretch)), Value::Bool(textured)],
        None,
    )
    .unwrap();
    s.device_ready(exact_gpu::wgpu::Features::empty());
    s.asset("flip.tex", Ok(&bin::to_vec(&flip())));
    s.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    // The first frame starts the clock; the second runs to `now_ms`.
    fixture::render(gpu, &mut s, &frame()).unwrap();
    let frame = Frame { now_ms, ..frame() };
    let (pixels, _) = fixture::render(gpu, &mut s, &frame).unwrap();
    assert!(s.error().is_none(), "{:?}", s.error());
    pixels
}
// The lit extent of the spark: columns and rows with any bright pixel.
fn extent(p: &fixture::Pixels) -> (usize, usize) {
    let lit = |x: u32, y: u32| p.at(x, y).iter().take(3).any(|&c| c > 60);
    let cols = (0..128).filter(|&x| (0..128).any(|y| lit(x, y))).count();
    let rows = (0..128).filter(|&y| (0..128).any(|x| lit(x, y))).count();
    (cols, rows)
}

#[test]
fn particles_stretch_along_their_motion_and_flip_through_an_atlas() {
    let Some(gpu) = gpu() else { return };
    // 4 m tall view over 128 px: 32 px per metre; a 0.1 m spark is ~3 px.
    let round = extent(&sparks(&gpu, 0., false, 200.));
    let streak = extent(&sparks(&gpu, 0.1, false, 200.));
    eprintln!("spark extent (cols, rows): round {round:?}, stretched {streak:?}");
    assert!(round.0 <= 5 && round.1 <= 5, "{round:?}");
    // 0.1 s of 4 m/s adds 0.4 m: about 13 px along X, still thin in Y.
    assert!(streak.0 >= 12 && streak.1 <= 5, "{streak:?}");
    let colour = |p: &fixture::Pixels| {
        (0..128)
            .flat_map(|y| (0..128).map(move |x| (x, y)))
            .map(|(x, y)| p.at(x, y))
            .max_by_key(|c| u16::from(c[0]) + u16::from(c[1]))
            .unwrap()
    };
    let (early, late) = (
        colour(&sparks(&gpu, 0., true, 200.)),
        colour(&sparks(&gpu, 0., true, 600.)),
    );
    eprintln!("flipbook early {early:?} late {late:?}");
    assert!(early[0] > 150 && early[1] < 40, "frame 0 red: {early:?}");
    assert!(late[1] > 150 && late[0] < 40, "frame 1 green: {late:?}");
}

#[derive(Default, Args)]
struct TimedArgs {
    quality: u32,
}
struct Timed;
impl Game for Timed {
    const ID: &'static str = "look-timed";
    type Args = TimedArgs;
    fn setup(w: &mut World, args: &TimedArgs) {
        w.insert_resource(AmbientOcclusion {
            quality: [AoQuality::Medium, AoQuality::Low, AoQuality::High]
                [args.quality as usize % 3],
            ..Default::default()
        });
        w.spawn((
            Transform::at(0., 3., 6.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
        w.spawn((Transform::default(), Mesh::plane(20., 20.)));
        w.spawn((Transform::at(0., 0.5, 0.), Mesh::cube(1.)));
        w.spawn((
            Transform::at(5., 10., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.spawn((
            Transform::at(0., 4., 0.).looking_at(Vec3::ZERO, Vec3::Z),
            SpotLight::default(),
            LightShadows,
        ));
        w.spawn((Transform::at(0., 1., 0.), Emitter::sparks()));
    }
    fn tick(w: &mut World, _: &Input, _: &TimedArgs) {
        emitter::step(w);
    }
}

#[test]
fn every_pass_reports_gpu_time() {
    let Some(gpu) = gpu() else { return };
    if !gpu
        .device
        .features()
        .contains(exact_gpu::wgpu::Features::TIMESTAMP_QUERY)
    {
        eprintln!("SKIP: no TIMESTAMP_QUERY");
        return;
    }
    let mut s = WorldSurface::<Timed>::default();
    s.bind(&[], None).unwrap();
    s.agent(r#"{"op":"state","perf":true}"#);
    // The agent's virtual clock (seekable frames) times passes too.
    let mut f = frame();
    for _ in 0..12 {
        f.now_ms += 16.;
        fixture::render(&gpu, &mut s, &f).unwrap();
    }
    let state = s.agent(r#"{"op":"state"}"#).unwrap();
    let gpu_ms = &state[state.find("\"gpuMs\"").expect("gpuMs")..];
    eprintln!("{}", &gpu_ms[..gpu_ms.len().min(3000)]);
    for pass in [
        "forward + sky",
        "shadow 0",
        "local shadows",
        "ssao occlusion",
        "ssao upsample",
        "bloom",
        "tonemap",
        "cull",
    ] {
        let at = gpu_ms
            .find(&format!("\"{pass}\":{{"))
            .unwrap_or_else(|| panic!("{pass} missing"));
        let ring = &gpu_ms[at..at + gpu_ms[at..].find('}').unwrap()];
        assert!(!ring.contains("\"count\":0"), "{pass} untimed: {ring}");
    }
}

/// SSAO's GPU time at 1920 x 1080: `cargo test --release -p exact-game-render
/// --test look ssao_cost_at_1080p -- --ignored --nocapture`.
#[test]
#[ignore = "GPU timing measurement; GPU host"]
fn ssao_cost_at_1080p() {
    let Some(gpu) = gpu() else { return };
    // Per-pass timestamps overlap on tiled GPUs, so measure what SSAO adds to a
    // whole GPU-completed 1080p frame: the median frame with it on minus off.
    let median = |quality: u32| {
        let mut s = WorldSurface::<Timed>::default();
        s.bind(&[Value::Number(f64::from(quality))], None).unwrap();
        let mut f = Frame {
            width: 1920.,
            height: 1080.,
            ..frame()
        };
        let format = exact_gpu::wgpu::TextureFormat::Rgba8Unorm;
        let target = gpu
            .device
            .create_texture(&exact_gpu::wgpu::TextureDescriptor {
                label: None,
                size: exact_gpu::wgpu::Extent3d {
                    width: 1920,
                    height: 1080,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: exact_gpu::wgpu::TextureDimension::D2,
                format,
                usage: exact_gpu::wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let mut ms = Vec::new();
        for i in 0..80 {
            f.now_ms += 16.;
            let start = std::time::Instant::now();
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            s.render(&f, &gpu.device, &gpu.queue, &mut encoder, &target, format);
            gpu.queue.submit([encoder.finish()]);
            s.submitted();
            gpu.device
                .poll(exact_gpu::wgpu::PollType::wait_indefinitely())
                .unwrap();
            if i >= 20 {
                ms.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        ms.sort_by(f64::total_cmp);
        ms[ms.len() / 2]
    };
    let mut best = [f64::MAX; 4];
    for round in 0..5 {
        let frame = [3, 1, 0, 2].map(median);
        eprintln!(
            "SSAO 1080p round {round}: frame off {:.3}, low {:.3}, medium {:.3}, high {:.3} ms",
            frame[0], frame[1], frame[2], frame[3]
        );
        for (b, v) in best.iter_mut().zip(frame) {
            *b = b.min(v);
        }
    }
    eprintln!(
        "SSAO 1080p added (best medians): low {:.3} medium {:.3} high {:.3} ms",
        best[1] - best[0],
        best[2] - best[0],
        best[3] - best[0]
    );
}

#[derive(Default, Args)]
struct LodArgs {
    distance: f32,
}
struct Lod;
impl Game for Lod {
    const ID: &'static str = "look-lod";
    type Args = LodArgs;
    fn setup(w: &mut World, args: &LodArgs) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            ambient: 1.,
            zenith: [1.; 3],
            horizon: [1.; 3],
            ground: [1.; 3],
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., args.distance), Camera::default()));
        let near = w.generated("near.model", quad([1., 0., 0., 1.])).unwrap();
        w.generated("far.model", quad([0., 1., 0., 1.])).unwrap();
        w.spawn((
            Transform::default(),
            near,
            ModelLod {
                levels: vec![LodLevel {
                    distance: 6.,
                    model: "far.model".into(),
                }],
                hide: Some(20.),
            },
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &LodArgs) {}
}

#[test]
fn a_model_lod_swaps_by_camera_distance_and_hides_beyond_its_limit() {
    let Some(gpu) = gpu() else { return };
    let centre = |distance: f64| {
        let mut s = WorldSurface::<Lod, ModelPresentation, true>::default();
        s.bind(&[Value::Number(distance)], None).unwrap();
        s.device_ready(exact_gpu::wgpu::Features::empty());
        s.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        let (pixels, _) = fixture::render(&gpu, &mut s, &frame()).unwrap();
        assert!(s.error().is_none(), "{:?}", s.error());
        pixels.at(64, 64)
    };
    let near = centre(3.);
    assert!(near[0] > 100 && near[1] < 40, "near level: {near:?}");
    let far = centre(10.);
    assert!(far[1] > 100 && far[0] < 40, "far level: {far:?}");
    let gone = centre(30.);
    assert!(gone[0] < 10 && gone[1] < 10, "beyond hide: {gone:?}");
}
