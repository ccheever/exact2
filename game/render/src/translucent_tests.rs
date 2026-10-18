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
                    Transform::at(0., i as f32 * 0.001, 0.),
                    Mesh::ring(0.5, 1.),
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
    let mut layered = surface(&[Value::Number(0.), Value::Bool(true)]);
    let (pixels, _) = fixture::render(&gpu, &mut layered, &frame()).unwrap();
    let [r, g, b, _] = pixels.at(88, 64);
    assert!(g > b && b > r, "back blue then front green: {r},{g},{b}");
}

#[test]
fn all_translucent_shapes_batch_cull_and_restore_without_shader_variants() {
    let Some(gpu) = fixture::device_or_skip(fixture::device()) else {
        return;
    };
    let mut w = World::new(120, 7);
    w.spawn((
        Transform::at(0., 3., 8.).looking_at(Vec3::ZERO, Vec3::Y),
        Camera::default(),
    ));
    for mesh in [
        Mesh::cube(1.),
        Mesh::sphere(0.5),
        Mesh::capsule(0.3, 1.),
        Mesh::plane(1., 1.),
        Mesh::cylinder(0.5, 1.),
        Mesh::ring(0.2, 0.5),
        Mesh::torus(0.5, 0.1),
        Mesh::cone(0.5, 1.),
    ] {
        w.spawn((
            Transform::default(),
            mesh,
            Material::glow([1.; 3]).alpha(0.3),
        ));
    }
    w.spawn((
        Transform::at(1000., 0., 0.),
        Mesh::ring(0.2, 0.5),
        Material::glow([1.; 3]).alpha(0.3),
    ));
    let mut r = crate::renderer::RendererWithAssets::<false>::new(
        &gpu.device,
        &gpu.queue,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let pipelines = r.quads.work_pipelines();
    let mut feed = crate::Feed::default();
    for restore in [false, true] {
        if restore {
            let saved = w.save();
            w.load(&saved).unwrap();
        }
        feed.feed(&w, &mut r).unwrap();
        let input = feed.frame(&w, 1., 1.);
        r.quads.frame::<false>(&input, &r.models.textures, false);
        r.quads.order::<false>(&gpu.device, &gpu.queue);
        assert_eq!(r.quads.draws.len(), 1);
        let all = r.quads.instances();
        r.quads.frame::<false>(&input, &r.models.textures, true);
        r.quads.order::<false>(&gpu.device, &gpu.queue);
        assert_eq!(r.quads.draws.len(), 1);
        assert!(r.quads.instances() < all, "off-camera primitive is culled");
        assert_eq!(
            r.quads.work_pipelines(),
            pipelines + 2,
            "reuse the two particle blend pipelines"
        );
    }
}
