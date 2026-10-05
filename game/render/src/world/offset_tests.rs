use super::*;
use exact_game::Offset;

#[test]
fn rewriting_the_same_offsets_is_not_a_pose_change() {
    // Game::present rewrites every Offset row each tick; only content counts.
    let mut w = World::new(60, 1);
    let e = w.spawn(Transform::default());
    let mut offsets = super::offsets::Offsets::default();
    w.insert(e, Offset(Transform::at(1., 0., 0.)));
    assert_eq!(offsets.diff(&w), super::offsets::Change::Rows);
    w.remove::<Offset>(e);
    w.insert(e, Offset(Transform::at(1., 0., 0.)));
    assert_eq!(offsets.diff(&w), super::offsets::Change::None);
    w.insert(e, Offset(Transform::at(1.5, 0., 0.)));
    assert_eq!(offsets.diff(&w), super::offsets::Change::Values);
    assert_eq!(offsets.roots(), [e]);
    w.remove::<Offset>(e);
    assert_eq!(offsets.diff(&w), super::offsets::Change::Rows);
    assert_eq!(
        offsets.roots(),
        [e],
        "a removed offset moves its entity too"
    );
}

// Present rewrites every Offset row each tick; with nothing moving, the feed
// writes no transform page after the first, parented ones included.
struct Bobbing;
impl Game for Bobbing {
    type Args = ();
    const ID: &'static str = "feed-offset";
    fn setup(w: &mut World, _: &Self::Args) {
        let root = w.spawn_named("root", (Transform::default(), Mesh::cube(1.0)));
        w.spawn((
            Transform::at(0., 2., 0.),
            exact_game::Parent(root),
            Mesh::cube(1.0),
        ));
    }
    fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
    fn present(w: &mut exact_game::Present<'_>, _: &Self::Args) {
        let root = w.named("root").unwrap();
        w.insert(root, Offset(Transform::at(0., 0.5, 0.)));
    }
}
#[test]
fn a_static_offset_writes_no_pages_per_tick() {
    let mut sim = Sim::<Bobbing>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    sim.advance(0., Clock::Seekable);
    f.feed_to(sim.world(), &mut r).unwrap();
    let root = sim.world().named("root").unwrap();
    assert_eq!(r.position(root, false).y, 0.5, "the offset is drawn");
    // Two ticks settle the history buffers; later ticks write nothing.
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    r.calls.clear();
    sim.advance_with(500., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(
        !r.calls.iter().any(|c| matches!(c, Call::Transform(..))),
        "{:?}",
        r.calls
    );
}

// A walker (with a child two pages on) steps once its offset at tick 3; a
// parented crowd and a second, still offset stand by.
struct Step;
impl Game for Step {
    type Args = ();
    const ID: &'static str = "feed-offset-step";
    fn setup(w: &mut World, _: &Self::Args) {
        let walker = w.spawn_named("walker", (Transform::default(), Mesh::cube(1.0)));
        w.spawn_named("still", (Transform::at(5., 0., 0.), Mesh::cube(1.0)));
        for i in 0..2 * PAGE {
            let tree = w.spawn((Transform::at(i as f32, 0., 9.), Mesh::cube(1.0)));
            w.spawn((Transform::at(0., 1., 0.), Parent(tree), Mesh::cube(1.0)));
        }
        w.spawn_named(
            "hand",
            (Transform::at(0., 2., 0.), Parent(walker), Mesh::cube(1.0)),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
    fn present(p: &mut exact_game::Present<'_>, _: &Self::Args) {
        let y = if p.tick() >= 3 { 1. } else { 0. };
        let walker = p.named("walker").unwrap();
        p.insert(walker, Offset(Transform::at(0., y, 0.)));
        let still = p.named("still").unwrap();
        p.insert(still, Offset(Transform::at(0., 0., 1.)));
    }
}
#[test]
fn an_offset_change_moves_only_its_subtree_and_both_history_buffers() {
    let mut sim = Sim::<Step>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    let w = |sim: &Sim<Step>, name: &str| sim.world().named(name).unwrap();
    let (walker, hand) = (w(&sim, "walker"), w(&sim, "hand"));
    assert_eq!(hand.index() as usize / PAGE, 4 * PAGE / PAGE);
    f.feed_to(sim.world(), &mut r).unwrap();
    let tick = |sim: &mut Sim<Step>, f: &mut Feed, r: &mut Recording| {
        r.calls.clear();
        sim.run(1000. / 60.);
        f.feed_to(sim.world(), r).unwrap();
    };
    for _ in 0..2 {
        tick(&mut sim, &mut f, &mut r);
    }
    assert!(f.offsets.moved.is_empty(), "still offsets move nothing");
    tick(&mut sim, &mut f, &mut r);
    assert_eq!(sim.world().tick(), 3);
    // The step moves the walker and its hand: nothing else is re-posed, and
    // only their two pages are written.
    assert_eq!(f.offsets.moved, [walker, hand]);
    let written: Vec<_> = (r.calls.iter())
        .filter_map(|c| match c {
            Call::Transform(first, ..) => Some(*first),
            _ => None,
        })
        .collect();
    assert_eq!(written, [0, (4 * PAGE) as u32], "{:?}", r.calls);
    assert_eq!(r.position(walker, false).y, 1.);
    assert_eq!(r.position(hand, false).y, 3.);
    assert_eq!(r.position(hand, true).y, 2., "the step interpolates");
    // The next tick's buffer was last written before the step: it takes the
    // stepped poses too, rather than drawing the walker back down.
    tick(&mut sim, &mut f, &mut r);
    assert!(f.offsets.moved.is_empty());
    for previous in [false, true] {
        assert_eq!(r.position(walker, previous).y, 1.);
        assert_eq!(r.position(hand, previous).y, 3.);
    }
    tick(&mut sim, &mut f, &mut r);
    assert!(
        !r.calls.iter().any(|c| matches!(c, Call::Transform(..))),
        "{:?}",
        r.calls
    );
}

// A walking unit among a crowd of model instances: its offset re-poses its own
// page of instances, never the crowd (it used to pass `Moved::All`).
#[test]
fn an_animated_offset_steps_only_its_pages_model_instances() {
    let Some(gpu) = crate::test_device::device_or_skip(exact_gpu::fixture::device()) else {
        return;
    };
    let mut renderer = crate::Renderer::new(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let model: exact_game::asset::Model =
        exact_game::bin::from_slice(include_bytes!("../../../bake/tests/fixtures/crate.model"))
            .unwrap();
    renderer.prepare_model("crate.model", &model).unwrap();
    struct Crowd;
    impl Game for Crowd {
        type Args = ();
        const ID: &'static str = "offset-model-poses";
        fn setup(w: &mut World, _: &()) {
            for i in 0..4 * PAGE {
                w.spawn((Transform::at(i as f32, 0., 0.), Mesh::asset("crate.model")));
            }
            let walker =
                w.spawn_named("walker", (Transform::default(), Mesh::asset("crate.model")));
            w.spawn((
                Transform::at(0., 1., 0.),
                Parent(walker),
                Mesh::asset("crate.model"),
            ));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut exact_game::Present<'_>, _: &()) {
            let bob = (p.tick() % 8) as f32 * 0.1;
            let walker = p.named("walker").unwrap();
            p.insert(walker, Offset(Transform::at(0., bob, 0.)));
        }
    }
    let steps = crate::models::pose_step_count;
    let mut sim = Sim::<Crowd>::new(()).unwrap();
    let mut feed = Feed::default();
    feed.feed(sim.world(), &mut renderer).unwrap();
    let tick = 1000. / 60.;
    for _ in 0..2 {
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
    }
    let instances = renderer.models.poses.len();
    assert!(instances > 4 * PAGE);
    for _ in 0..4 {
        let before = steps();
        sim.run(tick);
        feed.feed(sim.world(), &mut renderer).unwrap();
        // The walker's page: the walker, its child and the crowd sharing it.
        let stepped = steps() - before;
        assert!((2..=PAGE).contains(&stepped), "{stepped} of {instances}");
        let bobbing = (renderer.models.poses.iter())
            .filter(|p| p[0] != p[1])
            .count();
        assert_eq!(bobbing, 2, "the walker and its child interpolate");
    }
}
