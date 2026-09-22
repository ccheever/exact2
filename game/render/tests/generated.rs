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
    surface.device_ready();
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
