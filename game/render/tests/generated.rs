#![cfg(not(target_arch = "wasm32"))]
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::*;
use exact_game_render::{ModelPresentation, WorldSurface};
use exact_gpu::{fixture, Frame, Surface};

struct Props;
#[derive(Default, Args)]
struct Options {
    color: u32,
    #[restart]
    again: bool,
}
impl Game for Props {
    const ID: &'static str = "shared-generated-props";
    type Args = Options;
    fn setup(w: &mut World, args: &Options) {
        w.insert_resource(Environment {
            background: Some([0.15, 0.22, 0.3]),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        let mesh = asset::MeshData {
            positions: vec![-0.4, -0.4, 0., 0.4, -0.4, 0., 0., 0.4, 0.],
            normals: vec![0., 0., 1., 0., 0., 1., 0., 0., 1.],
            uvs: vec![0.; 6],
            colors: if args.color == 0 {
                [0., 1., 0., 1.].repeat(3)
            } else {
                [1., 0., 0., 1.].repeat(3)
            },
            indices: vec![0, 1, 2],
            bounds: [-0.4, -0.4, 0., 0.4, 0.4, 0.],
            ..Default::default()
        };
        if args.color < 2 {
            let prop = w.generated("prop.model", mesh).unwrap();
            for x in [-1., 0., 1.] {
                w.spawn((Transform::at(x, 0., 0.), prop.clone()));
            }
        }
        w.spawn((
            Transform::at(0., 0., 4.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::default(),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &Options) {}
}
#[test]
fn generated_props_upload_once_share_a_draw_and_keep_vertex_colors() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<Props, ModelPresentation, true>::default();
    surface
        .bind(&[Value::Number(0.), Value::Bool(false)], None)
        .unwrap();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    assert!(surface.assets().requests.is_empty());
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let frame = Frame {
        width: 128.,
        height: 128.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let (pixels, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    assert!(surface.error().is_none(), "{:?}", surface.error());
    let green = pixels.at(64, 64);
    assert!(
        u16::from(green[1]) > u16::from(green[0]) * 2
            && u16::from(green[1]) > u16::from(green[2]) * 2,
        "vertex colour: {green:?}"
    );
    #[derive(Default, Data)]
    struct Reply {
        world: State,
    }
    #[derive(Default, Data)]
    struct State {
        gpu: Work,
        perf: Perf,
    }
    #[allow(non_snake_case)]
    #[derive(Default, Data, PartialEq, Debug)]
    struct Work {
        beforeReady: Counters,
        afterReady: Counters,
    }
    #[allow(non_snake_case)]
    #[derive(Default, Data, PartialEq, Debug)]
    struct Counters {
        meshUploads: u64,
    }
    #[derive(Default, Data)]
    struct Perf {
        draws: u32,
        instances: u32,
    }
    let state = |s: &mut WorldSurface<Props, ModelPresentation, true>| -> State {
        json::from_str::<Reply>(&s.agent(r#"{"op":"state"}"#).unwrap())
            .unwrap()
            .world
    };
    let before = state(&mut surface);
    assert_eq!(before.gpu.beforeReady.meshUploads, 1);
    assert_eq!(before.perf.draws, 2); // one prop group plus tonemapping
    assert_eq!(before.perf.instances, 3);
    for _ in 0..3 {
        surface.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        fixture::render(&gpu, &mut surface, &frame).unwrap();
    }
    let after = state(&mut surface);
    assert_eq!(before.gpu, after.gpu);

    // Unchanged geometry reuses its GPU upload; changed and newly added geometry
    // must reach the pixels even though no host asset delivery occurs.
    for (color, uploads) in [(0., 1), (1., 2), (2., 2), (0., 3)] {
        surface
            .bind(&[Value::Number(color), Value::Bool(true)], None)
            .unwrap();
        assert!(surface.assets().requests.is_empty());
        surface.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        let (pixels, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        let pixel = pixels.at(64, 64);
        if color == 0. {
            assert!(
                u16::from(pixel[1]) > u16::from(pixel[0]) * 2,
                "green: {pixel:?}"
            );
        } else if color == 1. {
            assert!(
                u16::from(pixel[0]) > u16::from(pixel[1]) * 2,
                "red: {pixel:?}"
            );
        } else {
            assert!(
                pixel[2] > pixel[0] && pixel[2] > pixel[1],
                "background: {pixel:?}"
            );
        }
        let work = state(&mut surface).gpu;
        assert_eq!(
            work.beforeReady.meshUploads + work.afterReady.meshUploads,
            uploads
        );
    }
}

struct Grove;
#[derive(Default, Args)]
struct GroveArgs {
    trees: u32,
    primitive: bool,
}
impl Game for Grove {
    const ID: &'static str = "generated-grove";
    type Args = GroveArgs;
    fn setup(w: &mut World, args: &GroveArgs) {
        // A flat-shaded eight-sided cone, like the forest's generated pine.
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        for i in 0..8 {
            let (a, b) = (
                i as f32 * std::f32::consts::FRAC_PI_4,
                (i + 1) as f32 * std::f32::consts::FRAC_PI_4,
            );
            let p = [
                [0., 4., 0.],
                [math::cos(a), 0., math::sin(a)],
                [math::cos(b), 0., math::sin(b)],
            ];
            let n = Vec3::from(p[1]).cross(Vec3::from(p[2])).normalize();
            for v in p {
                positions.extend(v);
                normals.extend(n.to_array());
            }
        }
        let pine = w
            .generated(
                "pine.model",
                asset::MeshData {
                    uvs: vec![0.; positions.len() / 3 * 2],
                    indices: (0..positions.len() as u32 / 3).collect(),
                    positions,
                    normals,
                    bounds: [-1., 0., -1., 1., 4., 1.],
                    ..Default::default()
                },
            )
            .unwrap();
        let side = (args.trees as f32).sqrt().ceil() as u32;
        for i in 0..args.trees {
            let (x, z) = ((i % side) as f32 * 3., (i / side) as f32 * 3.);
            if args.primitive {
                // Two primitives per tree, as the forest's primitive variant.
                w.spawn((Transform::at(x, 1., -z), Mesh::cylinder(0.2, 2.)));
                w.spawn((Transform::at(x, 3., -z), Mesh::sphere(1.)));
            } else {
                w.spawn((Transform::at(x, 0., -z), pine.clone()));
            }
        }
        w.spawn((
            Transform::at(5., 10., 5.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 2., 6.).looking_at(Vec3::new(0., 2., 0.), Vec3::Y),
                Camera::default(),
            ),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &GroveArgs) {
        // Only the camera moves; every tree is static.
        w.require_mut::<Transform>("camera").position.z -= 0.05;
    }
}

/// Frame CPU (feed + encode, GPU excluded) over static model instances while
/// the camera walks, on the live clock (a seekable clock observes the world): `TREES=100000 cargo test --release -p exact-game-render
/// --test generated grove_frame_cpu -- --ignored --nocapture`; `PRIMITIVE=1`
/// draws each tree as two primitives instead.
#[test]
#[ignore = "release CPU measurement over static generated instances; GPU host"]
fn grove_frame_cpu() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let trees: u32 = std::env::var("TREES").map_or(100_000, |n| n.parse().unwrap());
    let primitive = std::env::var("PRIMITIVE").is_ok();
    let format = exact_gpu::wgpu::TextureFormat::Rgba8Unorm;
    let mut surface = WorldSurface::<Grove, ModelPresentation, true>::default();
    surface
        .bind(
            &[Value::Number(f64::from(trees)), Value::Bool(primitive)],
            None,
        )
        .unwrap();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    surface.prepare_assets(&gpu.device, &gpu.queue, format);
    let texture = gpu
        .device
        .create_texture(&exact_gpu::wgpu::TextureDescriptor {
            label: Some("grove"),
            size: exact_gpu::wgpu::Extent3d {
                width: 640,
                height: 360,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: exact_gpu::wgpu::TextureDimension::D2,
            format,
            usage: exact_gpu::wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
    let view = texture.create_view(&Default::default());
    let mut samples = Vec::new();
    for i in 0..400 {
        let frame = Frame {
            width: 640.,
            height: 360.,
            scale: 1.,
            now_ms: f64::from(i) * 1000. / 60.,
            seekable: false,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let start = std::time::Instant::now();
        surface.render(&frame, &gpu.device, &gpu.queue, &mut encoder, &view, format);
        let ms = start.elapsed().as_secs_f64() * 1000.;
        gpu.queue.submit([encoder.finish()]);
        surface.submitted();
        gpu.device
            .poll(exact_gpu::wgpu::PollType::wait_indefinitely())
            .unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        if i >= 100 {
            samples.push(ms);
        }
    }
    samples.sort_by(f64::total_cmp);
    eprintln!(
        "GROVE trees={trees} primitive={primitive} frame CPU p50={:.3} ms p95={:.3} ms",
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100]
    );
}
