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
