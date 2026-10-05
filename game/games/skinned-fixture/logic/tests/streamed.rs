//! The fox as a streamed model, animated by `Game::present` (`animation::ShownClips`):
//! it walks once its bytes land, the head's charm follows the drawn rig, and
//! nothing simulated depends on when (or whether) the model arrived.
#[path = "../../../../bake/tests/support/mod.rs"]
mod baked;
#[path = "../../../../render/tests/fixture/device.rs"]
mod gpu_test;
use exact_game::*;

fn assets() -> std::collections::BTreeMap<String, Vec<u8>> {
    baked::assets(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/fox.glb")).unwrap()
}

/// Walked metres: the saved cause the walk cycle is derived from.
#[derive(Default, Resource)]
struct Walked(f32);
struct Streamed;
impl Game for Streamed {
    const ID: &'static str = "skinned-fixture-streamed";
    const STREAMED: &'static [&'static str] = &["fox.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "fox",
            (Transform::at(0., 0., -1.8), Mesh::asset("fox.model")),
        );
        w.spawn_named(
            "charm",
            (
                Transform::default(),
                Mesh::sphere(0.12),
                SocketFollow::new("fox", "b_Head_05").offset(Transform::at(0., 0.2, 0.)),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(6., 3.4, 7.).looking_at(Vec3::new(0., 0.9, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.spawn((
            Transform::at(4., 7., 3.).looking_at(Vec3::ZERO, Vec3::Y),
            DirectionalLight::default(),
        ));
        w.insert_resource(Walked(0.));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let dt = w.dt();
        w.resource_mut::<Walked>().0 += 1.4 * dt;
    }
    fn present(p: &mut Present<'_>, _: &()) {
        // A 1.4 m/s walk with a 1.75 m stride per cycle of "Walk".
        let walked = p.resource::<Walked>().map_or(0., |w| w.0);
        let fox = p.named("fox").unwrap();
        let walk = animation::ShownClips::clip("Walk", walked / 1.75).speed(1.4 / 1.75);
        p.insert(fox, walk.in_place("b_Root_00"));
    }
}

fn status(s: &mut Sim<Streamed>) -> String {
    s.agent_with_inspector(
        r#"{"op":"state","entity":"fox"}"#,
        |_, _| {},
        animation::inspect,
    )
}

#[test]
fn a_streamed_model_animates_and_its_arrival_never_reaches_the_simulation() {
    let bytes = assets();
    let read = |name: &str| bytes.get(name).cloned().ok_or(name.to_owned());
    // `early` has the fox from the start, `late` from tick 30, `never` not at all.
    let mut early = Sim::<Streamed>::new(()).unwrap();
    early.load_assets(read).unwrap();
    let mut late = Sim::<Streamed>::new(()).unwrap();
    let mut never = Sim::<Streamed>::new(()).unwrap();
    assert!(early.world().model("fox.model").is_none(), "streamed: not simulation's");
    assert!(animation::drawn_model(early.world(), "fox.model").is_some());
    assert!(status(&mut never).contains(r#""state":"waiting for its model""#));
    let mut poses = Vec::new();
    for tick in 1..=60 {
        for s in [&mut early, &mut late, &mut never] {
            s.run(1000. / 60. + 0.0001);
        }
        if tick == 30 {
            late.load_assets(read).unwrap();
        }
        assert_eq!(early.world().hash(), late.world().hash(), "tick {tick}");
        assert_eq!(early.world().hash(), never.world().hash(), "tick {tick}");
        // A save waits while a shown model is in flight; once landed, the
        // bytes agree whenever it landed.
        if tick >= 30 {
            assert!(early.save().unwrap() == late.save().unwrap(), "tick {tick}");
        }
        // The charm's simulated pose is its fallback Transform everywhere.
        assert_eq!(
            early.world().global_position("charm"),
            never.world().global_position("charm")
        );
        if tick % 15 == 0 {
            poses.push(early.agent_with_inspector(
                r#"{"op":"state","entity":"fox","pose":true}"#,
                |_, _| {},
                animation::inspect,
            ));
        }
    }
    assert!(status(&mut early).contains(r#""state":"drawn""#), "{}", status(&mut early));
    assert!(status(&mut late).contains(r#""state":"drawn""#));
    assert!(status(&mut never).contains(r#""state":"waiting for its model""#));
    // The inspected joints walk: every sampled tick differs.
    for pair in poses.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
    // A late arrival draws the same pose an early one does at that boundary.
    let pose = |s: &mut Sim<Streamed>| {
        s.agent_with_inspector(
            r#"{"op":"state","entity":"fox","pose":true}"#,
            |_, _| {},
            animation::inspect,
        )
    };
    assert_eq!(pose(&mut early), pose(&mut late));
}

#[test]
fn the_streamed_walk_is_drawn_and_carries_the_socketed_charm() {
    use exact_game_render::{
        exact_gpu::{fixture, wgpu, Frame, Surface},
        WorldSurface,
    };
    let Some(gpu) = gpu_test::device_or_skip(exact_game_render::exact_gpu::fixture::device())
    else {
        return;
    };
    let bytes = assets();
    // The charm sits on the drawn head: the walk's sampled joint, not the
    // simulation's fallback.
    let mut s = Sim::<Streamed>::new(()).unwrap();
    s.load_assets(|name: &str| bytes.get(name).cloned().ok_or(name.to_owned()))
        .unwrap();
    s.run(500.);
    let model = animation::drawn_model(s.world(), "fox.model").unwrap();
    let mut renderer = exact_game_render::Renderer::new(
        &gpu.device,
        &gpu.queue,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    renderer.prepare_model("fox.model", model).unwrap();
    let mut feed = exact_game_render::Feed::default();
    feed.feed(s.world(), &mut renderer).unwrap();
    let fox = s.world().named("fox").unwrap();
    let mut shown = animation::ShownPose::default();
    assert!(shown.sample(s.world(), fox, &animation::bind_pose(model)).unwrap());
    let head = animation::socket_node(s.world(), "fox", "b_Head_05").unwrap();
    let t = *s.get::<Transform>("fox").unwrap();
    let offset = s.get::<SocketFollow>("charm").unwrap().offset;
    let expected = (Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
        * animation::joint_matrix(model, &shown.local, head)
        * Mat4::from_scale_rotation_translation(offset.scale, offset.rotation, offset.position))
    .w_axis
    .truncate();
    let bind = (Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.position)
        * animation::joint_matrix(model, &animation::bind_pose(model), head))
    .w_axis
    .truncate();
    let frame = feed.frame(s.world(), 1., 16. / 9.);
    let charm = (frame.attachments.iter())
        .find(|a| a.entity == s.world().named("charm").unwrap())
        .expect("the charm is drawn on the streamed fox")
        .pose;
    assert!(
        charm.position.distance(expected) < 1e-4,
        "{charm:?}, expected {expected:?}"
    );
    assert!(expected.distance(bind) > 1e-3, "the head moved off its bind pose");

    // Pixels: the fox never moves in the simulation, so two frames differ
    // only by the walk drawn from the streamed model.
    let mut surface = WorldSurface::<Streamed, exact_game_render::ModelExecutor, true>::default();
    surface.device_ready(wgpu::Features::empty());
    surface.bind(&[], None).unwrap();
    for _ in 0..16 {
        for n in surface.assets().requests {
            surface.asset(&n, Ok(&bytes[&n]));
        }
        surface.prepare_assets(&gpu.device, &gpu.queue, wgpu::TextureFormat::Rgba8Unorm);
    }
    let mut frame = Frame {
        width: 640.,
        height: 360.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let (start, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    frame.now_ms = 400.;
    let (stride, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    let changed = (start.data.chunks_exact(4))
        .zip(stride.data.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 4))
        .count();
    start.save("streamed-fox-start");
    stride.save("streamed-fox-stride");
    assert!(changed > 200, "the walk changes the drawn fox: {changed} pixels");
    assert!(surface.take_error().is_none());
}
