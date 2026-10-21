#![cfg(not(target_arch = "wasm32"))]
// A procedurally rigged humanoid (holding a socketed sword) and a quadruped walk in
// place; offscreen frames show both skinned bodies and their limbs moving.
// EXACT_GPU_OUT=<dir> writes rig-walk-a.ppm and rig-walk-b.ppm for review.
#[path = "fixture/device.rs"]
mod test_device;
use exact_game::animation;
use exact_game::rig::{self, Gait, Rig};
use exact_game::*;
use exact_game_render::{ModelExecutor, WorldSurface};
use exact_gpu::{fixture, Frame, Surface};

struct Walkers;
impl Game for Walkers {
    const ID: &'static str = "rig-walkers";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.55, 0.7, 0.85].into()),
            bloom: None,
            fog: None,
            ..Default::default()
        });
        let hero = Rig::humanoid(1.8);
        let hero = w
            .generated_model(
                "hero.model",
                hero.model([hero.idle("idle"), hero.walk("walk", Gait::walk(1.4))]),
            )
            .unwrap();
        let dog = Rig::quadruped(1.4);
        let dog = w
            .generated_model(
                "dog.model",
                dog.model([dog.idle("idle"), dog.walk("walk", Gait::walk(1.2))]),
            )
            .unwrap();
        // Side by side along their walking direction (+Z), seen in profile.
        for (name, mesh, z, speed) in [("hero", hero, -1.1, 1.4), ("dog", dog, 1.1, 1.2)] {
            let a = Animator::new([rig::locomotion("move", "idle", [(speed, "walk")])]);
            w.spawn_named(name, (Transform::at(0., 0., z), mesh, a));
            rig::drive(w, name, "move", speed);
        }
        w.spawn((
            Transform::default(),
            Mesh::cuboid(Vec3::new(0.035, 0.035, 0.8)),
            Material::rgb(0.85, 0.85, 0.9),
            SocketFollow::new("hero", "hand_r").offset(Transform::at(0., -0.06, 0.38)),
        ));
        w.spawn((
            Transform::default(),
            Mesh::plane(12., 12.),
            Material::rgb(0.3, 0.45, 0.25),
        ));
        w.spawn((
            Transform::at(4.2, 1.1, 0.).looking_at(Vec3::new(0., 0.8, 0.), Vec3::Y),
            Camera::default(),
        ));
        w.spawn((
            Transform::at(4., 6., 2.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        animation::step(w);
    }
}

#[test]
fn procedural_rigs_render_skinned_and_their_limbs_move() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut surface = WorldSurface::<Walkers, ModelExecutor, true>::default();
    surface.bind(&[], None).unwrap();
    surface.device_ready(exact_gpu::wgpu::Features::empty());
    assert!(
        surface.assets().requests.is_empty(),
        "generated models need no delivery"
    );
    let mut frame = Frame {
        width: 480.,
        height: 270.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.0,
    };
    let mut shots = Vec::new();
    for (ms, name) in [(1000., "rig-walk-a"), (1300., "rig-walk-b")] {
        frame.now_ms = ms;
        surface.prepare_assets(
            &gpu.device,
            &gpu.queue,
            exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
        );
        let (pixels, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        pixels.save(name);
        shots.push(pixels);
    }
    // Skin and cloth colours, not sky or grass, cover each figure's half.
    let figure = |p: [u8; 4]| {
        let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
        !(b > r + 25 && b > g) && !(g > r + 20 && g > b + 10)
    };
    let (w, h) = (480, 270);
    // Looking along -X, screen right is -Z: the humanoid is right, the dog left.
    for (left, label) in [(false, "humanoid"), (true, "quadruped")] {
        let mut count = 0;
        for y in 0..h {
            for x in 0..w {
                if (x < w / 2) == left && figure(shots[0].at(x, y)) {
                    count += 1;
                }
            }
        }
        assert!(count > 400, "{label} half has {count} figure pixels");
    }
    let changed = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| shots[0].at(x, y) != shots[1].at(x, y))
        .count();
    assert!(
        changed > 300,
        "walking must move limbs: {changed} pixels changed"
    );
}

struct DepthWalkers<const PREPASS: bool>;
impl<const PREPASS: bool> Game for DepthWalkers<PREPASS> {
    const ID: &'static str = "depth-walkers";
    const SAMPLES: SceneSamples = SceneSamples::One;
    const DEPTH_PREPASS: bool = PREPASS;
    type Args = ();
    fn setup(w: &mut World, a: &()) {
        Walkers::setup(w, a);
    }
    fn tick(w: &mut World, i: &Input, a: &()) {
        Walkers::tick(w, i, a);
    }
}
#[test]
fn camera_depth_matches_perspective_skinning_and_socket_poses() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    let mut plain = WorldSurface::<DepthWalkers<false>, ModelExecutor, true>::default();
    let mut depth = WorldSurface::<DepthWalkers<true>, ModelExecutor, true>::default();
    plain.bind(&[], None).unwrap();
    depth.bind(&[], None).unwrap();
    plain.device_ready(exact_gpu::wgpu::Features::empty());
    depth.device_ready(exact_gpu::wgpu::Features::empty());
    let mut frame = Frame {
        width: 240.,
        height: 135.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
        headroom: 1.,
    };
    for ms in [1000., 1300.] {
        frame.now_ms = ms;
        let (a, _) = fixture::render(&gpu, &mut plain, &frame).unwrap();
        let (b, _) = fixture::render(&gpu, &mut depth, &frame).unwrap();
        assert!(plain.error().is_none() && depth.error().is_none());
        let changed = (0..135)
            .flat_map(|y| (0..240).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                a.at(x, y)
                    .into_iter()
                    .zip(b.at(x, y))
                    .any(|(a, b)| a.abs_diff(b) > 2)
            })
            .count();
        assert_eq!(changed, 0, "animated frame {ms}");
    }
}

#[derive(Default, Args)]
struct PoseArgs {
    x: f32,
}
struct Posed<const CPU: bool>;
impl<const CPU: bool> Game for Posed<CPU> {
    const ID: &'static str = "posed-pixels";
    const SAMPLES: SceneSamples = SceneSamples::One;
    type Args = PoseArgs;
    fn setup(w: &mut World, args: &PoseArgs) {
        use asset::{MaterialData, MeshData, Model, Node};
        let rest = Mat4::from_rotation_z(0.3);
        let model = Model {
            meshes: vec![MeshData {
                positions: vec![-0.5, -0.5, 0., 0.5, -0.5, 0., 0.5, 0.5, 0., -0.5, 0.5, 0.],
                normals: [0., 0., 1.].repeat(4),
                uvs: vec![0.; 8],
                indices: vec![0, 1, 2, 0, 2, 3],
                bounds: [-0.5, -0.5, 0., 0.5, 0.5, 0.],
                ..Default::default()
            }],
            materials: vec![MaterialData {
                base_color: [1., 0., 0., 1.],
                emissive: [1., 0., 0.],
                metallic: 0.,
                ..Default::default()
            }],
            nodes: vec![
                Node {
                    transform: Mat4::from_translation(Vec3::new(0., 0.2, 0.)).to_cols_array(),
                    ..Default::default()
                },
                Node {
                    name: "arm".into(),
                    parent: Some(0),
                    posable: !CPU,
                    transform: (rest
                        * if CPU {
                            Mat4::from_translation(Vec3::X * args.x)
                        } else {
                            Mat4::IDENTITY
                        })
                    .to_cols_array(),
                    ..Default::default()
                },
                Node {
                    parent: Some(1),
                    mesh: Some(0),
                    ..Default::default()
                },
            ],
            bounds: [-2., -2., -1., 2., 2., 1.],
            ..Default::default()
        };
        let mesh = w.generated_model("posed.model", model).unwrap();
        w.spawn_named("actor", (Transform::default(), mesh));
        w.spawn((Transform::at(0., 0., 4.), Camera::default()));
        w.insert_resource(Environment {
            background: Some([0., 0., 0.].into()),
            bloom: None,
            fog: None,
            ..Default::default()
        });
    }
    fn tick(_: &mut World, _: &Input, _: &PoseArgs) {}
    fn present(p: &mut Present<'_>, args: &PoseArgs) {
        if !CPU {
            let actor = p.named("actor").unwrap();
            p.insert(
                actor,
                NodeOffsets::default().with(NodeName::new("arm"), Transform::at(args.x, 0., 0.)),
            );
        }
    }
}

#[test]
fn posed_model_pixels_match_cpu_hierarchy_and_move() {
    let Some(gpu) = test_device::device_or_skip(fixture::device()) else {
        return;
    };
    fn shot<const CPU: bool>(gpu: &exact_gpu::Gpu, x: f64) -> fixture::Pixels {
        let mut surface = WorldSurface::<Posed<CPU>, ModelExecutor, true>::default();
        surface.bind(&[exact_gpu::Value::Number(x)], None).unwrap();
        surface.device_ready(exact_gpu::wgpu::Features::empty());
        let frame = Frame {
            width: 160.,
            height: 96.,
            scale: 1.,
            now_ms: 1000.,
            seekable: true,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
            headroom: 1.,
        };
        let (pixels, _) = fixture::render(gpu, &mut surface, &frame).unwrap();
        assert!(surface.error().is_none(), "{:?}", surface.error());
        pixels.save(&format!("posed-{CPU}-{x}"));
        pixels
    }
    let mut previous: Option<fixture::Pixels> = None;
    for x in [-0.7, 0.7] {
        let actual = shot::<false>(&gpu, x);
        let expected = shot::<true>(&gpu, x);
        let red = |p: [u8; 4]| p[0] > 100 && p[1] < 30 && p[2] < 30;
        assert!(
            actual.count(red) > 400,
            "posed model must cover its own pixels"
        );
        let differences = actual
            .data
            .iter()
            .zip(&expected.data)
            .filter(|(a, b)| a.abs_diff(**b) > 2)
            .count();
        assert!(
            differences < 20,
            "GPU pose differs from CPU hierarchy in {differences} channels at x={x}"
        );
        if let Some(before) = previous {
            let moved = actual
                .data
                .chunks_exact(4)
                .zip(before.data.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert!(moved > 400, "pose must move visible geometry: {moved}");
        }
        previous = Some(actual);
    }
}
