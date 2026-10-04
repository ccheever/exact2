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
