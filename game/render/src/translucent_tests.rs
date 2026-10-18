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
    eprintln!("halo draws: 1 halo = {one_draws}, 100 halos = {hundred_draws}");
    // Draw equality qualifies batching with an honest counter; the occluder below
    // checks that the counter responds to an actual additional draw.
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
fn viewmodels_ignore_walls_and_share_draws() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
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
        "reflection retains ring coverage"
    );
}

struct SignedShapes;
impl Game for SignedShapes {
    const ID: &'static str = "signed-shapes";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        Halos::setup(
            w,
            &Options {
                count: 1,
                ..Default::default()
            },
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let mut t = w.require_mut::<Transform>("halo-0");
        t.rotation = Quat::from_rotation_z(0.6);
        t.scale *= Vec3::new(-1.4, -0.7, 1.2);
        t.position.x += 0.2;
    }
}
#[test]
fn signed_axes_match_opaque_silhouettes_at_endpoints_and_between_ticks() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mask = |p: &fixture::Pixels| {
        p.data
            .chunks_exact(4)
            .map(|p| p[0] > 10)
            .collect::<Vec<_>>()
    };
    for mesh in [Mesh::ring(0.5, 1.), Mesh::cone(1., 2.)] {
        for scale in [
            Vec3::new(-1., 1., 1.),
            Vec3::new(1., -1., 1.),
            Vec3::new(1., 1., -1.),
            Vec3::new(-1., -1., 1.),
            Vec3::new(-1., 1., -1.),
            Vec3::new(1., -1., -1.),
        ] {
            let make = |alpha| {
                let mut s = WorldSurface::<SignedShapes>::default();
                s.bind(&[], None).unwrap();
                let w = s.world_for_test();
                *w.require_mut::<Mesh>("halo-0") = mesh.clone();
                w.require_mut::<Transform>("halo-0").scale = scale;
                w.require_mut::<Material>("halo-0").color[3] = alpha;
                s
            };
            let (mut opaque, mut transparent) = (make(1.), make(0.5));
            for now_ms in [0., 17., 25.] {
                let f = Frame { now_ms, ..frame() };
                let a = fixture::render(&gpu, &mut opaque, &f).unwrap().0;
                let b = fixture::render(&gpu, &mut transparent, &f).unwrap().0;
                assert!(a.count(|p| p[0] > 10) > 100);
                assert_eq!(mask(&a), mask(&b), "{mesh:?}, {scale:?}, {now_ms}ms");
                if now_ms == 25. {
                    assert!((opaque.sim().unwrap().alpha() - 0.5).abs() < 0.001);
                }
            }
        }
    }
}

#[test]
fn transparent_world_composites_before_viewmodels_in_both_depth_orders() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    for (world_y, view_y) in [(2., 0.), (0., 2.)] {
        let mut s = surface(&[Value::Number(1.)]);
        let w = s.world_for_test();
        w.require_mut::<Transform>("halo-0").position.y = view_y;
        w.insert(w.named("halo-0").unwrap(), ViewModel);
        w.spawn((
            Transform::at(0., world_y, 0.),
            Mesh::ring(0.5, 1.),
            Material::glow([0., 1., 0.]).alpha(0.5),
        ));
        let pixels = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        let [r, g, b, _] = pixels.at(88, 64);
        assert!(r > g && g > b, "viewmodel red composites last: {r},{g},{b}");
        assert_eq!(
            s.renderer_for_test().unwrap().depth_split(),
            crate::VIEWMODEL_DEPTH,
            "transparent-only viewmodels reserve depth"
        );
        assert_eq!(s.renderer_for_test().unwrap().quads.draws.len(), 2);
        // The opaque viewmodel must occlude the physically farther translucent
        // viewmodel within their reserved slice, independent of world depth.
        s.world_for_test().spawn((
            Transform::at(0., view_y + 1., 0.),
            Mesh::ring(0.5, 1.),
            Material::glow([0., 0., 1.]),
            ViewModel,
        ));
        let mixed = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        let mut reference = surface(&[Value::Number(0.)]);
        reference.world_for_test().spawn((
            Transform::at(0., view_y + 1., 0.),
            Mesh::ring(0.5, 1.),
            Material::glow([0., 0., 1.]),
            ViewModel,
        ));
        let blue_only = fixture::render(&gpu, &mut reference, &frame()).unwrap().0;
        assert_eq!(
            mixed.data, blue_only.data,
            "opaque viewmodel hides both transparent layers"
        );
    }
}

#[test]
fn reflected_socket_normals_light_a_non_emissive_surface() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut s = WorldSurface::<SocketShape, crate::ModelExecutor, true>::default();
    s.bind(&[], None).unwrap();
    let w = s.world_for_test();
    *w.require_mut::<Transform>("owner") = Transform {
        scale: Vec3::new(-1., 1., 1.),
        ..Default::default()
    };
    w.require_mut::<SocketFollow>("shape").offset.rotation =
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    *w.require_mut::<Mesh>("shape") = Mesh::ring(0.5, 1.);
    *w.require_mut::<Material>("shape") = Material::rgb(0.4, 0.2, 0.1).alpha(0.5);
    w.resource_mut::<Environment>().ambient = 0.;
    w.spawn((
        Transform::at(2., 3., 8.).looking_at(Vec3::ZERO, Vec3::Y),
        DirectionalLight {
            shadows: false,
            ..Default::default()
        },
    ));
    let reflected = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert!(reflected.count(|p| p[0] > 100) > 500);
    s.world_for_test().require_mut::<Transform>("owner").scale = Vec3::ONE;
    let plain = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert_eq!(
        reflected.data, plain.data,
        "reflection retains outward diffuse normals"
    );
    crate::quads::primitives::wrong_normals(&mut s.renderer_for_test().unwrap().quads, &gpu.device);
    s.world_for_test();
    let wrong = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
    assert_ne!(
        plain.data, wrong.data,
        "inverted lighting normals fail the pixel oracle"
    );
    assert_eq!(wrong.count(|p| p[0] > 100), 0);
}

struct SoftDepth;
impl Game for SoftDepth {
    const ID: &'static str = "soft-current-depth";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.; 3]),
            bloom: None,
            fog: None,
            tone_mapping: ToneMapping::None,
            ..Default::default()
        });
        w.spawn((Transform::at(0., 0., 5.), Camera::default()));
        w.spawn((
            Transform::at(0., 0., -2.),
            Mesh::cube(4.),
            Material::rgb(0., 0., 0.),
        ));
        w.spawn((
            Transform::at(0., 0., 0.1),
            Emitter {
                rate: 0.,
                lifetime: 10.,
                speed: 0.,
                spread: 0.,
                gravity: Vec3::ZERO,
                size: [1., 1.],
                color: [[1.; 4], [1.; 4]],
                additive: false,
                ..Default::default()
            }
            .burst(1),
            ParticleLook {
                soft: 1.,
                ..Default::default()
            },
        ));
        // Seed a live particle before the renderer's first frame.
        emitter::step(w);
        w.spawn_named(
            "viewmodel",
            (
                Transform::at(1.5, 0., 0.),
                Mesh::sphere(0.1),
                Material::glow([1., 0., 0.]).alpha(0.5),
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn soft_particles_use_current_depth_on_first_frame_and_viewmodel_transitions() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let make = || {
        let mut s = WorldSurface::<SoftDepth>::default();
        s.bind(&[], None).unwrap();
        s
    };
    let mut baseline = make();
    let reference = fixture::render(&gpu, &mut baseline, &frame())
        .unwrap()
        .0
        .at(64, 64)[0];
    assert!(
        reference > 10 && reference < 150,
        "intersection fade: {reference}"
    );
    for starts_attached in [true, false] {
        let mut s = make();
        for member in [starts_attached, !starts_attached, starts_attached] {
            let w = s.world_for_test();
            let e = w.named("viewmodel").unwrap();
            if member {
                w.insert(e, ViewModel);
            } else {
                w.remove::<ViewModel>(e);
            }
            let pixels = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
            let actual = pixels.at(64, 64)[0];
            assert!(
                actual.abs_diff(reference) <= 2,
                "starts={starts_attached}, member={member}: {actual} != {reference}"
            );
            assert_eq!(
                s.renderer_for_test().unwrap().depth_split(),
                if member { crate::VIEWMODEL_DEPTH } else { 0. }
            );
        }
    }
}

#[test]
fn cone_picking_matches_rendered_resolved_follower_and_parented_socket_child() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut masks = Vec::new();
    for child in [false, true] {
        let mut s = WorldSurface::<SocketShape, crate::ModelExecutor, true>::default();
        s.bind(&[], None).unwrap();
        let w = s.world_for_test();
        w.require_mut::<Transform>("owner").scale = Vec3::new(-3., 1., 1.);
        let shape = w.named("shape").unwrap();
        let signed = Transform {
            scale: Vec3::new(1., -1., 1.),
            ..Default::default()
        };
        let cone = if child {
            w.remove::<Mesh>(shape);
            w.spawn((
                signed,
                Parent(shape),
                Mesh::cone(0.6, 1.6),
                Material::glow([1., 0., 0.]).alpha(0.5),
            ))
        } else {
            *w.get_mut::<Transform>(shape).unwrap() = signed;
            w.get_mut::<SocketFollow>(shape).unwrap().offset = signed;
            *w.get_mut::<Mesh>(shape).unwrap() = Mesh::cone(0.6, 1.6);
            shape
        };
        let pixels = fixture::render(&gpu, &mut s, &frame()).unwrap().0;
        let visible = |x, y| pixels.at(x, y)[0] > 10;
        let (mut hits, mut misses) = (0, 0);
        // Interior pixels avoid the polygonal silhouette's raster edge. The
        // analytic picker must hit exactly the same projected surface as the GPU.
        for y in 3..125 {
            for x in 3..125 {
                let red = visible(x, y);
                if !(y - 2..=y + 2).all(|v| (x - 2..=x + 2).all(|u| visible(u, v) == red)) {
                    continue;
                }
                let hit = s
                    .sim()
                    .unwrap()
                    .pick(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                assert_eq!(
                    hit.map(|h| h.entity),
                    red.then_some(cone),
                    "child={child}, {x},{y}"
                );
                if red {
                    hits += 1;
                } else {
                    misses += 1;
                }
            }
        }
        assert!(hits > 300 && misses > 300, "hits={hits}, misses={misses}");
        masks.push(pixels.data);
    }
    assert_ne!(
        masks[0], masks[1],
        "full socket affine differs from decomposed child pose"
    );
}
