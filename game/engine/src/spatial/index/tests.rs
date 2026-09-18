use super::*;
use std::time::Instant;

struct Quiet;
impl crate::Game for Quiet {
    type Args = ();
    const ID: &'static str = "sight-cost";
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &crate::Input, _: &()) {}
}

#[test]
fn mesh_bvh_200k_deep_far_read_and_dense_refusal() {
    let mut w = World::new(60, 0);
    let subject = w.spawn_named("subject", (Transform::at(0., 0., -10_000.), Mesh::cube(2.)));
    let camera = w.spawn((
        Transform::at(0., 0., 10.),
        Camera {
            far: 20_000.,
            ..Default::default()
        },
    ));
    // Reverse/interleaved spatial order, a 100k-deep parent chain, unrelated roots,
    // and slot churn. Character configuration is present on the queried object.
    #[derive(Default, crate::Component)]
    struct Actor {
        character: crate::character::Character,
    }
    w.insert(subject, Actor::default());
    let mut parent = w.spawn((Transform::at(1000., 0., -10_000.), Mesh::cube(1.)));
    for i in 0..99_998 {
        parent = w.spawn((Transform::default(), Parent(parent), Mesh::cube(1.)));
        let other = w.spawn((
            Transform::at(2000. + (100_000 - i) as f32, 0., -5000.),
            Mesh::cube(1.),
        ));
        if i % 97 == 0 {
            w.despawn(other);
            w.spawn((Transform::at(-2000. - i as f32, 0., -5000.), Mesh::cube(1.)));
        }
    }
    let target = parent;
    let blocker = w.spawn_named("wall", (Transform::at(0., 0., -5000.), Mesh::cube(4.)));
    let loading = w.spawn((Transform::at(0., 0., -100.), Mesh::asset("loading.model")));
    let _ = loading;
    w.propagate();
    assert_eq!(w.query::<&Mesh>().iter().count(), 200_000);
    let points = corners(w.global(subject).unwrap(), Vec3::ONE, Vec3::ZERO);
    let from = Vec3::new(0., 0., 10.);
    let hash = w.hash();
    let epoch = w.mutation_epoch();
    let saved = w.save();
    let read = |w: &World| {
        let mut sight = Sight::new(w, subject, Some(target), Some(camera)).unwrap();
        let blocked = sight
            .segment(
                Vec3::new(0., 0., -10_000.),
                w.global(target).unwrap().translation.into(),
                1,
                |_, _| true,
            )
            .unwrap();
        assert!(
            !blocked,
            "target's deep ancestors must not block its endpoint"
        );
        assert_eq!(
            occlusion(&mut sight, from, &points).unwrap(),
            (1., vec![blocker])
        );
        VISIT_LIMIT - sight.remaining
    };
    let start = Instant::now();
    let visits = read(&w);
    let cold = start.elapsed();
    let mut warm = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        assert_eq!(read(&w), visits);
        warm.push(start.elapsed());
    }
    warm.sort();
    assert!(visits < 400_000, "{visits}");
    assert_eq!(w.mutation_epoch(), epoch);
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save(), saved);
    eprintln!("sight 200k meshes, 99999-deep chain, 10km subject: cold={cold:?}, warm median={:?}, max={:?}, visits={visits}", warm[4], warm[8]);
    let mut sim = crate::Sim::<Quiet>::new(()).unwrap();
    *sim.world_mut() = w;
    sim.world().sight.0.borrow_mut().take();
    let request = format!(
        r##"{{"op":"layout","entity":"subject","to":"#{}","width":800,"height":600}}"##,
        target.index()
    );
    let start = Instant::now();
    let expected = sim.agent(&request);
    let cold = start.elapsed();
    assert!(
        expected.contains(r#""occluded":1,"occluders":["wall"]"#),
        "{expected}"
    );
    assert!(expected.contains(r#""lineOfSight":true"#), "{expected}");
    let mut samples = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        assert_eq!(sim.agent(&request), expected);
        samples.push(start.elapsed());
    }
    samples.sort();
    eprintln!(
        "full layout 200k: cold={cold:?}, warm median={:?}, max={:?}",
        samples[4], samples[8]
    );
    let mut w = std::mem::replace(sim.world_mut(), World::new(60, 0));
    // Negative control: real blocker removal changes the 15-ray answer.
    w.despawn(blocker);
    let mut sight = Sight::new(&w, subject, Some(target), Some(camera)).unwrap();
    assert_eq!(occlusion(&mut sight, from, &points).unwrap(), (0., vec![]));
    drop(sight);
    drop(w);

    // The broad phase cannot prune 200k overlapping on-ray boxes. Refusal must
    // be explicit and identical cold/warm, never an apparently clear result.
    let mut w = World::new(60, 0);
    let subject = w.spawn((Transform::default(), Mesh::cube(2.)));
    for _ in 0..200_000 {
        w.spawn((Transform::at(0., 0., 5.), Mesh::cube(4.)));
    }
    let points = corners(Affine3A::IDENTITY, Vec3::ONE, Vec3::ZERO);
    for pass in ["cold", "warm"] {
        let start = Instant::now();
        let mut sight = Sight::new(&w, subject, None, None).unwrap();
        assert!(occlusion(&mut sight, from, &points)
            .unwrap_err()
            .contains("work budget exceeded"));
        assert_eq!(sight.remaining, 0);
        eprintln!(
            "sight overlapping 200k {pass} refusal={:?}, visits={VISIT_LIMIT}",
            start.elapsed()
        );
    }
    let mut sight = Sight::new(&w, subject, None, None).unwrap();
    assert!(sight.segment(from, Vec3::ZERO, 1, |_, _| true).unwrap());
    assert!(
        VISIT_LIMIT - sight.remaining < 64,
        "LOS must early-exit at one blocker"
    );
}

#[test]
fn sight_limit_includes_dead_slots_and_is_an_agent_error() {
    struct Empty;
    impl crate::Game for Empty {
        type Args = ();
        const ID: &'static str = "sight-limit";
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("subject", Transform::default());
        }
        fn tick(_: &mut World, _: &crate::Input, _: &()) {}
    }
    let mut s = crate::Sim::<Empty>::new(()).unwrap();
    let mut dead = Vec::new();
    for _ in 0..SLOT_LIMIT {
        dead.push(s.world_mut().spawn(()));
    }
    for e in dead {
        s.world_mut().despawn(e);
    }
    assert_eq!(s.world().len(), 1);
    let result = s.agent(r#"{"op":"layout","entity":"subject"}"#);
    assert_eq!(
        result,
        r#"{"tick":0,"error":"layout visibility index limit exceeded (262144 entity slots, including dead slots)"}"#
    );
}

#[test]
fn route_diagnostic_200k_interleaved_churn_returns_nearest_and_clear_side() {
    let mut sim = crate::Sim::<Quiet>::new(()).unwrap();
    let w = sim.world_mut();
    w.spawn_named("subject", Transform::at(0., 0., 0.));
    let far = w.spawn_named("far", (Transform::at(0., 0., 8.), Mesh::cube(2.)));
    let near = w.spawn_named("nearest", (Transform::at(0., 0., 4.), Mesh::cube(2.)));
    for i in 3..200_000 {
        let x = if i % 2 == 0 { 100. } else { -100. };
        let e = w.spawn((Transform::at(x, 0., i as f32), Mesh::cube(1.)));
        if i % 3 == 0 {
            w.despawn(e);
            w.spawn((Transform::at(x, 0., i as f32), Mesh::cube(1.)));
        }
    }
    let before = sim.world().hash();
    let report = sim
        .route_diagnostic("subject", Vec3::ZERO, Vec3::new(0., 0., 10.))
        .unwrap();
    assert!(report.contains("\"name\":\"nearest\""), "{report}");
    assert!(
        report.contains("\"bounds\":{\"min\":[-1.0,-1.0,3.0],\"max\":[1.0,1.0,5.0]}"),
        "{report}"
    );
    assert!(!report.contains("\"nearestClearSide\":null"), "{report}");
    assert_eq!(before, sim.world().hash());
    sim.world_mut().despawn(near);
    let report = sim
        .route_diagnostic("subject", Vec3::ZERO, Vec3::new(0., 0., 10.))
        .unwrap();
    assert!(report.contains("\"name\":\"far\""), "{report}");
    sim.world_mut().despawn(far);
    assert_eq!(
        sim.route_diagnostic("subject", Vec3::ZERO, Vec3::new(0., 0., 10.))
            .unwrap(),
        r#"{"blocker":null,"nearestClearSide":null}"#
    );
    // Retain entities to cross the same explicit layout slot ceiling.
    while sim.world().alive_mask.len() * 64 <= SLOT_LIMIT {
        sim.world_mut().spawn(());
    }
    assert!(sim
        .route_diagnostic("subject", Vec3::ZERO, Vec3::Z)
        .unwrap_err()
        .contains("262144"));
}
