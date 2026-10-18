use crate::{
    exact_gpu::{fixture, wgpu, Frame, Surface},
    WorldSurface,
};
use exact_game::*;

#[derive(Default, Args)]
struct Options {
    count: u32,
    layers: bool,
    occluder: bool,
    spheres: bool,
}
struct Halos;
impl Game for Halos {
    const ID: &'static str = "halo-batch";
    type Args = Options;
    fn setup(w: &mut World, args: &Options) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Environment::default()
        });
        w.spawn((
            Transform::at(0., 8., 0.).looking_at(Vec3::ZERO, Vec3::Z),
            Camera::orthographic(4.),
        ));
        for i in 0..args.count {
            w.spawn_named(
                format!("halo-{i}"),
                (
                    if args.count > 1 {
                        Transform::at((i % 10) as f32 * 0.4 - 1.8, 0., (i / 10) as f32 * 0.4 - 1.8)
                    } else {
                        Transform::default()
                    },
                    if args.spheres {
                        Mesh::sphere(if args.count > 1 { 0.17 } else { 1. })
                    } else if args.count > 1 {
                        Mesh::ring(0.06, 0.17)
                    } else {
                        Mesh::ring(0.5, 1.)
                    },
                    Material::glow([1., 0., 0.]).alpha(0.5),
                ),
            );
        }
        if args.count > 0 {
            w.spawn((
                Transform::at(1000., 0., 0.),
                Mesh::ring(0.5, 1.),
                Material::glow([1., 0., 0.]).alpha(0.5),
            ));
        }
        if args.layers {
            // Spawn the nearer green first: depth must override entity order.
            w.spawn((
                Transform::at(0., 2., 0.),
                Mesh::ring(0.5, 1.),
                Material::glow([0., 1., 0.]).alpha(0.5),
            ));
            w.spawn((
                Transform::at(0., 1., 0.),
                Mesh::ring(0.5, 1.),
                Material::glow([0., 0., 1.]).alpha(0.5),
            ));
        }
        if args.occluder {
            w.spawn((
                Transform::at(0., 3., 0.),
                Mesh::cube(2.2),
                Material::glow([0., 1., 0.]),
            ));
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Options) {}
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
        headroom: 1.,
    }
}
fn surface(args: &[Value]) -> WorldSurface<Halos> {
    let mut s = WorldSurface::default();
    s.device_ready(wgpu::Features::empty());
    s.bind(args, None).unwrap();
    s
}
fn draws(s: &mut WorldSurface<Halos>) -> u32 {
    #[derive(Default, Data)]
    struct Perf {
        draws: u32,
    }
    #[derive(Default, Data)]
    struct WorldState {
        perf: Perf,
    }
    #[derive(Default, Data)]
    struct State {
        world: WorldState,
    }
    json::from_str::<State>(&s.agent(r#"{"op":"state"}"#).unwrap())
        .unwrap()
        .world
        .perf
        .draws
}
#[test]
fn halos_share_one_draw_and_sort_depth_without_filling_the_hole() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut one = surface(&[Value::Number(1.)]);
    let before = one.sim().unwrap().world().hash();
    let (pixels, _) = fixture::render(&gpu, &mut one, &frame()).unwrap();
    assert_eq!(one.sim().unwrap().world().hash(), before);
    assert_eq!(
        pixels.at(64, 64),
        [0, 0, 0, 255],
        "annulus hole stays empty"
    );
    let [r, g, b, _] = pixels.at(88, 64);
    assert!(r > 80 && g < 5 && b < 5, "ring alpha coverage: {r},{g},{b}");
    pixels.save("halo-1");
    one.renderer_for_test().unwrap().cull.keep_all = true;
    let (unculled, _) = fixture::render(&gpu, &mut one, &frame()).unwrap();
    assert_eq!(pixels.data, unculled.data, "culling changes no pixels");
    one.renderer_for_test().unwrap().cull.keep_all = false;
    let one_draws = draws(&mut one);
    let mut hundred = surface(&[Value::Number(100.)]);
    let (pixels, _) = fixture::render(&gpu, &mut hundred, &frame()).unwrap();
    pixels.save("halo-100");
    let hundred_draws = draws(&mut hundred);
    let instances = hundred.renderer_for_test().unwrap().quads.instances();
    assert_eq!(
        instances,
        100 * 96,
        "every ring contributes all 96 triangles"
    );
    assert!(hundred_oracle(&pixels, instances, hundred_draws, one_draws));
    let saved = hundred.carry().unwrap().unwrap();
    let mut restored = surface(&[Value::Number(100.)]);
    restored
        .restore(&saved, crate::exact_gpu::Restore::Open)
        .unwrap();
    let restored_pixels = fixture::render(&gpu, &mut restored, &frame()).unwrap().0;
    assert_eq!(
        pixels.data, restored_pixels.data,
        "restore reproduces actual pixels"
    );
    // Negative controls exercise the same oracle used above.
    let mut dropped = surface(&[Value::Number(99.)]);
    let missing = fixture::render(&gpu, &mut dropped, &frame()).unwrap().0;
    assert!(
        !hundred_oracle(&missing, instances, hundred_draws, one_draws),
        "coverage detects a dropped instance even with a forged count"
    );
    for (_, material) in hundred.world_for_test().query::<&mut Material>().iter() {
        material.color[3] = 0.25;
    }
    let wrong_alpha = fixture::render(&gpu, &mut hundred, &frame()).unwrap().0;
    assert!(!hundred_oracle(
        &wrong_alpha,
        instances,
        hundred_draws,
        one_draws
    ));
    assert!(
        !hundred_oracle(&pixels, instances, one_draws + 99, one_draws),
        "a per-object draw fails"
    );
    assert!(!hundred_oracle(
        &pixels,
        instances - 96,
        hundred_draws,
        one_draws
    ));
    eprintln!("halo draws: 1 halo = {one_draws}, 100 halos = {hundred_draws}");
    assert_eq!(hundred_draws, one_draws, "entity count must not add draws");
    // Negative control: an opaque occluder really adds another compatible batch.
    let mut occluded = surface(&[Value::Number(1.), Value::Bool(false), Value::Bool(true)]);
    let (pixels, _) = fixture::render(&gpu, &mut occluded, &frame()).unwrap();
    let [r, g, b, _] = pixels.at(88, 64);
    assert!(g > r && g > b, "green occluder: {r},{g},{b}");
    let mut reference = surface(&[Value::Number(0.), Value::Bool(false), Value::Bool(true)]);
    let (reference_pixels, _) = fixture::render(&gpu, &mut reference, &frame()).unwrap();
    assert_eq!(
        pixels.data, reference_pixels.data,
        "opaque depth hides every halo pixel"
    );
    assert!(draws(&mut occluded) > one_draws);
    let sphere_args = |n| {
        [
            Value::Number(n),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(true),
        ]
    };
    let mut sphere = surface(&sphere_args(1.));
    fixture::render(&gpu, &mut sphere, &frame()).unwrap();
    let sphere_instances = sphere.renderer_for_test().unwrap().quads.instances();
    let sphere_draws = draws(&mut sphere);
    let mut spheres = surface(&sphere_args(100.));
    let sphere_pixels = fixture::render(&gpu, &mut spheres, &frame()).unwrap().0;
    assert_eq!(
        spheres.renderer_for_test().unwrap().quads.instances(),
        sphere_instances * 100
    );
    assert_eq!(draws(&mut spheres), sphere_draws);
    assert!(
        hundred_coverage(&sphere_pixels),
        "all spherical halos have alpha coverage"
    );
    for i in 0..100 {
        let x = (((i % 10) as f32 * 0.4 - 1.8 + 2.) * 32.) as u32;
        let y = (((i / 10) as f32 * 0.4 - 1.8 + 2.) * 32.) as u32;
        assert!(
            (186..=190).contains(&sphere_pixels.at(x, y)[0]),
            "sphere {i} has a filled centre, unlike a ring"
        );
    }
    let mut layered = surface(&[Value::Number(0.), Value::Bool(true)]);
    let (pixels, _) = fixture::render(&gpu, &mut layered, &frame()).unwrap();
    let [r, g, b, _] = pixels.at(88, 64);
    assert!(g > b && b > r, "back blue then front green: {r},{g},{b}");
}

fn hundred_oracle(
    pixels: &fixture::Pixels,
    instances: u64,
    draws: u32,
    expected_draws: u32,
) -> bool {
    instances == 100 * 96 && draws == expected_draws && hundred_coverage(pixels)
}
fn hundred_coverage(pixels: &fixture::Pixels) -> bool {
    (0..100).all(|i| {
        let x = (((i % 10) as f32 * 0.4 - 1.8 + 0.12 + 2.) * 32.) as u32;
        let y = (((i / 10) as f32 * 0.4 - 1.8 + 2.) * 32.) as u32;
        let [r, g, b, _] = pixels.at(x, y);
        (186..=190).contains(&r) && g == 0 && b == 0
    })
}

struct Shapes;
impl Game for Shapes {
    const ID: &'static str = "translucent-shapes";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        w.spawn((
            Transform::at(0., 5., 8.).looking_at(Vec3::ZERO, Vec3::Y),
            Camera::orthographic(6.),
        ));
        for (i, mesh) in [
            Mesh::cube(0.8),
            Mesh::sphere(0.5),
            Mesh::capsule(0.3, 1.),
            Mesh::plane(0.8, 0.8),
            Mesh::cylinder(0.5, 1.),
            Mesh::ring(0.2, 0.5),
            Mesh::torus(0.4, 0.1),
            Mesh::cone(0.5, 1.),
        ]
        .into_iter()
        .enumerate()
        {
            w.spawn_named(
                format!("shape-{i}"),
                (
                    Transform::at((i % 4) as f32 * 1.4 - 2.1, 0., (i / 4) as f32 * 2. - 1.),
                    mesh,
                    Material::glow([1.; 3]).alpha(0.3),
                ),
            );
        }
        w.spawn((
            Transform::at(1000., 0., 0.),
            Mesh::ring(0.2, 0.5),
            Material::glow([1.; 3]).alpha(0.3),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn all_translucent_shapes_batch_cull_and_restore_pixels() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<Shapes>::default();
    s.bind(&[], None).unwrap();
    let pixels = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert_eq!(s.renderer_for_test().unwrap().quads.draws.len(), 1);
    let culled = s.renderer_for_test().unwrap().quads.instances();
    for i in 0..8 {
        let rect = s
            .sim()
            .unwrap()
            .layout(format!("shape-{i}").as_str())
            .unwrap()
            .screen;
        let covered = (rect.y.max(0.) as u32..(rect.y + rect.h).min(128.) as u32).any(|y| {
            (rect.x.max(0.) as u32..(rect.x + rect.w).min(128.) as u32)
                .any(|x| pixels.at(x, y)[0] > 50)
        });
        assert!(covered, "shape {i} has individual coverage");
    }
    s.renderer_for_test().unwrap().cull.keep_all = true;
    let unculled = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert_eq!(pixels.data, unculled.data);
    assert!(s.renderer_for_test().unwrap().quads.instances() > culled);
    let saved = s.carry().unwrap().unwrap();
    let mut restored = WorldSurface::<Shapes>::default();
    restored.bind(&[], None).unwrap();
    restored
        .restore(&saved, crate::exact_gpu::Restore::Open)
        .unwrap();
    assert_eq!(
        pixels.data,
        fixture::render(&gpu, &mut restored, &frame())
            .unwrap()
            .0
            .data
    );
}

#[test]
fn alpha_membership_changes_match_a_freshly_restored_renderer() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = surface(&[Value::Number(1.)]);
    fixture::render(&gpu, &mut s, &frame()).unwrap();
    for alpha in [
        Some(1.),
        Some(0.5),
        Some(1.),
        None,
        Some(0.5),
        None,
        Some(1.),
    ] {
        let w = s.world_for_test();
        let e = w.named("halo-0").unwrap();
        match alpha {
            Some(a) => {
                w.insert(e, Material::glow([1., 0., 0.]).alpha(a));
            }
            None => {
                w.remove::<Material>(e);
            }
        }
        let live = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        assert!(
            live.count(|p| p[0] > 50) > 50,
            "membership never loses the object: {alpha:?}"
        );
        let saved = s.carry().unwrap().unwrap();
        let mut fresh = surface(&[Value::Number(1.)]);
        fresh
            .restore(&saved, crate::exact_gpu::Restore::Open)
            .unwrap();
        let restored = fixture::render(&gpu, &mut fresh, &frame()).unwrap().0;
        assert!(live.data == restored.data, "live/restored alpha {alpha:?}");
    }
}

#[test]
fn negative_local_scales_preserve_opaque_silhouette_and_viewmodels_ignore_walls() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = surface(&[Value::Number(1.)]);
    let w = s.world_for_test();
    let e = w.named("halo-0").unwrap();
    *w.require_mut::<Mesh>(e) = Mesh::cone(1., 2.);
    let mut pose = w.require_mut::<Transform>(e);
    pose.rotation = Quat::from_rotation_z(0.6);
    pose.scale = Vec3::new(-1.4, 0.7, 1.);
    drop(pose);
    w.require_mut::<Material>(e).color[3] = 1.;
    let opaque = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    s.world_for_test().require_mut::<Material>(e).color[3] = 0.5;
    let alpha = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    let mask = |pixels: &fixture::Pixels| {
        pixels
            .data
            .chunks_exact(4)
            .map(|p| p[0] > 10)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        mask(&opaque),
        mask(&alpha),
        "alpha only changes opacity, including signed local scale"
    );
    let mut wall = surface(&[Value::Number(1.), Value::Bool(false), Value::Bool(true)]);
    let hidden = fixture::render(&gpu, &mut wall, &frame()).unwrap().0;
    let mut only_wall = surface(&[Value::Number(0.), Value::Bool(false), Value::Bool(true)]);
    let reference = fixture::render(&gpu, &mut only_wall, &frame()).unwrap().0;
    assert!(
        hidden.data == reference.data,
        "world depth completely occludes the ordinary translucent ring"
    );
    let w = wall.world_for_test();
    let e = w.named("halo-0").unwrap();
    w.insert(e, ViewModel);
    let visible = fixture::render(&gpu, &mut wall, &frame()).unwrap().0;
    assert!(
        visible.at(88, 64)[0] > 150,
        "viewmodel stays in front of the world wall"
    );
    let w = wall.world_for_test();
    for _ in 1..100 {
        w.spawn((
            Transform::default(),
            Mesh::ring(0.5, 1.),
            Material::glow([1., 0., 0.]).alpha(0.5),
            ViewModel,
        ));
    }
    let before = draws(&mut wall);
    fixture::render(&gpu, &mut wall, &frame()).unwrap();
    assert_eq!(
        draws(&mut wall),
        before,
        "viewmodels also batch by depth membership"
    );
}

struct SocketShape;
impl Game for SocketShape {
    const ID: &'static str = "affine-translucency";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        let model = asset::Model {
            nodes: vec![asset::Node {
                name: "socket".into(),
                transform: Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4).to_cols_array(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mesh = w.generated_model("socket.model", model).unwrap();
        w.spawn_named("owner", (Transform::default(), mesh));
        w.spawn_named(
            "shape",
            (
                Transform::default(),
                SocketFollow::new("owner", "socket"),
                Mesh::sphere(1.),
                Material::glow([1., 0., 0.]).alpha(0.5),
            ),
        );
        let mut camera = Camera::orthographic(4.);
        camera.near = 1.;
        camera.far = 20.;
        w.spawn((Transform::at(0., 0., 8.), camera));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn sheared_socket_culling_matches_every_frustum_edge_and_reflection_keeps_the_front() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<SocketShape, crate::ModelExecutor, true>::default();
    s.bind(&[], None).unwrap();
    for (position, scale) in [
        (Vec3::new(-5.5, 0., 0.), Vec3::new(4., 1., 1.)),
        (Vec3::new(5.5, 0., 0.), Vec3::new(4., 1., 1.)),
        (Vec3::new(0., -5.5, 0.), Vec3::new(1., 4., 1.)),
        (Vec3::new(0., 5.5, 0.), Vec3::new(1., 4., 1.)),
        (Vec3::new(0., 0., 9.), Vec3::new(1., 1., 4.)),
        (Vec3::new(0., 0., -15.5), Vec3::new(1., 1., 4.)),
    ] {
        {
            let mut owner = s.world_for_test().require_mut::<Transform>("owner");
            owner.position = position;
            owner.scale = scale;
        }
        {
            let w = s.world_for_test();
            let near = position.z > 0.;
            *w.require_mut::<Mesh>("shape") = if near {
                Mesh::plane(2., 2.)
            } else {
                Mesh::sphere(1.)
            };
            w.require_mut::<SocketFollow>("shape").offset.rotation = if near {
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_4)
            } else {
                Quat::IDENTITY
            };
        }
        if let Some(r) = s.renderer_for_test() {
            r.cull.keep_all = false;
        }
        let culled = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        assert!(
            culled.count(|p| p[0] > 20) > 0,
            "shape crosses edge at {position:?}"
        );
        s.renderer_for_test().unwrap().cull.keep_all = true;
        let all = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        assert_eq!(culled.data, all.data, "affine culling at {position:?}");
    }
    let w = s.world_for_test();
    *w.require_mut::<Transform>("owner") = Transform {
        scale: Vec3::new(-1., 1., 1.),
        ..Default::default()
    };
    w.require_mut::<SocketFollow>("shape").offset.rotation =
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    *w.require_mut::<Mesh>("shape") = Mesh::ring(0.5, 1.);
    let reflected = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert!(
        reflected.count(|p| p[0] > 100) > 500,
        "reflected ring retains its front face and normal"
    );
    s.world_for_test().require_mut::<Transform>("owner").scale = Vec3::ONE;
    let plain = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert_eq!(
        reflected.data, plain.data,
        "reflection retains ring lighting and coverage"
    );
}
