use super::*;
use exact_game::{Camera, Clock, DirectionalLight, Game, Input, PointLight, Quat, Sim, Vec3};

use crate::fixture::{Call, Recording};
struct Moving;
impl Game for Moving {
    type Args = ();
    const ID: &'static str = "feed-test";
    fn setup(w: &mut World, _: &Self::Args) {
        w.spawn((Transform::default(), Mesh::cube(1.0), Material::default()));
    }
    fn tick(w: &mut World, _: &Input, _: &Self::Args) {
        for (_, t) in w.query::<&mut Transform>().iter() {
            t.position.x += 1.0;
        }
    }
}
struct Stop;
impl Game for Stop {
    type Args = ();
    const ID: &'static str = "feed-stop";
    fn setup(w: &mut World, a: &Self::Args) {
        Moving::setup(w, a)
    }
    fn tick(w: &mut World, i: &Input, args: &Self::Args) {
        if w.tick() == 0 {
            Moving::tick(w, i, args);
        }
    }
}
#[test]
fn pages_are_whole_ordered_and_holes_are_zero() {
    let mut w = World::new(60, 0);
    for i in 0..PAGE * 3 {
        let e = w.spawn(());
        if i == 7 || i == 2 * PAGE + 10 {
            w.insert(e, Transform::at(i as f32, 1., 2.));
        }
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let writes: Vec<_> = r
        .calls
        .iter()
        .filter(|c| matches!(c, Call::Transform(..)))
        .collect();
    assert_eq!(
        writes,
        [
            &Call::Transform(0, PAGE * 10, true),
            &Call::Transform((PAGE * 2) as u32, PAGE * 10, true)
        ]
    );
    assert!(r.current[..70].iter().all(|v| *v == 0.0));
    r.calls.clear();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.is_empty());
}
#[test]
fn moved_then_two_still_ticks_stop_all_history_work() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    r.calls.clear();
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(r.calls.contains(&Call::Begin));
    assert_ne!(r.previous, r.current); // Includes the setup-to-first-tick stamp collision.
    assert!(!r.calls.contains(&Call::Batches));
    r.calls.clear();
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert_eq!(r.calls, [Call::Begin, Call::Transform(0, PAGE * 10, false)]);
    assert_eq!(r.previous, r.current);
    r.calls.clear();
    sim.advance_with(51., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert!(r.calls.is_empty());
    assert_eq!(r.previous, r.current);
}
#[test]
fn propagated_chain_overlays_page_and_fresh_teleport_writes_both() {
    let mut w = World::new(60, 0);
    let root = w.spawn(Transform {
        rotation: Quat::from_rotation_y(0.7),
        ..Transform::at(2., 3., 4.).with_scale(2.)
    });
    let a = w.spawn((Transform::at(1., 0., 0.).with_scale(3.), Parent(root)));
    let b = w.spawn((Transform::at(0., 2., 0.), Parent(a), Mesh::cube(1.0)));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let expected = Vec3::new(2., 3., 4.) + Quat::from_rotation_y(0.7) * Vec3::new(2., 12., 0.);
    assert!(r.position(b, false).distance(expected) < 1e-5);
    assert!(r.current[b.index() as usize * 10 + 7..][..3]
        .iter()
        .all(|s| (*s - 6.).abs() < 1e-5));
    r.calls.clear();
    w.teleport(b, Transform::at(10., 0., 0.));
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Previous(b.index(), 10)));
    assert_eq!(r.position(b, true), r.position(b, false));
    let e = w.spawn((Transform::at(50., 0., 0.), Mesh::cube(1.0)));
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(e, true), Vec3::new(50., 0., 0.));
}
#[test]
fn structure_and_visibility_rebuild_but_movement_does_not() {
    let mut w = World::new(60, 0);
    let a = w.spawn((Transform::default(), Mesh::cube(1.0)));
    let b = w.spawn((Transform::default(), Mesh::sphere(1.0), Visible(false)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [a.index()]);
    r.calls.clear();
    w.get_mut::<Transform>(a).unwrap().position.x = 1.;
    f.feed_to(&w, &mut r).unwrap();
    assert!(!r.calls.contains(&Call::Batches));
    w.insert(b, Visible(true));
    w.despawn(a);
    let c = w.spawn((Transform::at(7., 0., 0.), Mesh::cube(1.0)));
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.contains(&c.index()) && r.slots.contains(&b.index()));
    assert_eq!(r.position(c, true).x, 7.);
    w.despawn(c);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.slots, [b.index()]);
    w.remove::<Transform>(b);
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.slots.is_empty());
}
#[test]
fn materials_repack_pages_only_on_revision_and_default_missing_values() {
    let mut w = World::new(60, 0);
    let a = w.spawn((
        Transform::default(),
        Material::grid([0.2, 0.3, 0.4], 2.0).emissive(2., 3., 4.),
    ));
    let b = w.spawn((Transform::default(), Mesh::cube(1.0)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Material(0, PAGE * 12)));
    assert_eq!(
        &r.materials[..9],
        &[0.2, 0.3, 0.4, -2., 0., 0.5, 2., 3., 4.]
    );
    assert_eq!(
        &r.materials[b.index() as usize * 12..][..12],
        &material_floats(Material::default())
    );
    r.calls.clear();
    w.get_mut::<Transform>(a).unwrap().position.x = 2.;
    f.feed_to(&w, &mut r).unwrap();
    assert!(!r.calls.iter().any(|c| matches!(c, Call::Material(..))));
    w.remove::<Material>(a);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(&r.materials[..12], &material_floats(Material::default()));
}
#[test]
fn long_advance_feeds_only_last_two_ticks() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    let mut ticks = Vec::new();
    sim.advance_with(60_000., Clock::Seekable, |w, left| {
        if left < 2 {
            ticks.push(w.tick());
            f.feed_to(w, &mut r).unwrap();
        }
    });
    assert_eq!(ticks, [3599, 3600]);
    assert_eq!(r.previous[0], 3599.);
    assert_eq!(r.current[0], 3600.);
}
#[test]
fn capacity_refuses_before_history_and_allows_partial_last_page() {
    let mut w = World::new(60, 0);
    for _ in 0..=PAGE {
        w.spawn(Transform::default());
    }
    let mut f = Feed::default();
    let mut r = Recording {
        limit: PAGE as u32,
        ..Default::default()
    };
    let e = f.feed_to(&w, &mut r).unwrap_err();
    assert_eq!(
        e,
        RenderError::Capacity {
            arena: "transforms",
            slot: PAGE as u64,
            limit: PAGE as u64
        }
    );
    assert!(r.calls.is_empty());
    r.limit += 1;
    f.feed_to(&w, &mut r).unwrap();
    assert!(r.calls.contains(&Call::Transform(0, (PAGE + 1) * 10, true)));
}
#[test]
fn primitive_dimensions_are_instance_data_and_capsules_deform_exactly() {
    let mut w = World::new(60, 0);
    for mesh in [
        Mesh::sphere(1.0),
        Mesh::cylinder(1.0, 1.0),
        Mesh::plane(40., 40.),
        Mesh::Capsule {
            radius: 0.4,
            height: 1.,
        },
        Mesh::Capsule {
            radius: 0.4,
            height: 1.,
        },
        Mesh::Capsule {
            radius: 0.5,
            height: 1.,
        },
        Mesh::cube(1.0),
    ] {
        w.spawn((Transform::default(), mesh));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.meshes.len(), 5);
    for (i, expected) in [
        (0, Vec3::ONE),
        (1, Vec3::new(1., 0.5, 1.)),
        (2, Vec3::new(20., 0., 20.)),
        (3, Vec3::new(0.4, 0.5, 0.4)),
    ] {
        let extent = r.meshes[i]
            .0
            .iter()
            .map(|v| {
                let dims = Vec3::from_slice(&r.materials[i * 12 + 9..i * 12 + 12]);
                let scale = if v.uv[1] == 1.0 {
                    Vec3::splat(dims.x)
                } else {
                    dims
                };
                (Vec3::from_array(v.position) * scale + Vec3::Y * v.uv[0] * dims.y).abs()
            })
            .fold(Vec3::ZERO, Vec3::max);
        assert!(
            extent.distance(expected) < 1e-5,
            "{extent:?} != {expected:?}"
        );
    }
}
#[test]
fn camera_slerps_and_nearest_lights_interpolate_without_frame_scans() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
    let w = sim.world_mut();
    let camera = w.spawn((Transform::default(), Camera::default()));
    w.spawn((Transform::default(), DirectionalLight::default()));
    let mut lights = Vec::new();
    for i in 0..20 {
        lights.push(w.spawn((Transform::at(i as f32 + 1., 0., 0.), PointLight::default())));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    // Every pose advanced +X by one; even the one camera and selected lights blend.
    let input = f.frame(sim.world(), 0.25, 2.);
    assert_eq!(input.camera_position.x, 0.25);
    assert_eq!(input.points.len(), 16);
    assert_eq!(input.points[0].position.x, 1.25);
    assert_eq!(input.points[15].position.x, 16.25);
    assert!(input.sun.unwrap().shadows.is_some());
    assert!(input.environment.bloom.is_some());
    assert_eq!(
        input.environment.fog,
        exact_game::Environment::default().fog
    );
    sim.world().get_mut::<Transform>(camera).unwrap().rotation = Quat::from_rotation_y(1.0);
    f.feed_to(sim.world(), &mut r).unwrap();
    let input = f.frame(sim.world(), 0.5, 2.);
    let forward = input.view.inverse().transform_vector3(-Vec3::Z);
    assert!(forward.distance(Quat::from_rotation_y(0.5) * -Vec3::Z) < 1e-5);
    sim.world_mut()
        .teleport(lights[19], Transform::at(0.9, 0., 0.));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(
        f.frame(sim.world(), 0.5, 2.).points[0].position,
        Vec3::new(0.9, 0., 0.)
    );
}

#[allow(unsafe_code)]
pub(crate) mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
    struct Counter;
    #[global_allocator]
    static ALLOCATOR: Counter = Counter;
    // SAFETY: all allocation operations delegate unchanged to the system allocator.
    unsafe impl GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            COUNT.with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            COUNT.with(|c| {
                if let Some(n) = c.get() {
                    c.set(Some(n + 1));
                }
            });
            unsafe { System.realloc(ptr, layout, size) }
        }
    }
    pub fn count(f: impl FnOnce()) -> usize {
        COUNT.with(|c| c.set(Some(0)));
        f();
        COUNT.with(|c| c.replace(None).unwrap())
    }
}
#[test]
fn steady_sim_feed_and_frame_inputs_allocate_nothing() {
    let mut sim = Sim::<Moving>::new(()).unwrap();
    let camera = sim
        .world_mut()
        .spawn((Transform::at(0., 0., 10.), Camera::default()));
    sim.world_mut()
        .spawn((Transform::at(1., 1., 1.), PointLight::default()));
    let mut f = Feed::default();
    let mut r = Recording {
        record: false,
        ..Default::default()
    };
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Live);
    sim.advance_with(17., Clock::Live, |w, _| f.feed_to(w, &mut r).unwrap());
    let mut trace = crate::trace::Trace::new(sim.world(), "#0", 256).unwrap();
    let count = allocations::count(|| {
        for i in 2..240 {
            sim.advance_with(i as f64 * 1000. / 60. + 0.001, Clock::Live, |w, left| {
                if left < 2 {
                    f.feed_to(w, &mut r).unwrap();
                    trace.feed(w);
                }
            });
            let frame = f.frame(sim.world(), 0.5, 16. / 9.);
            std::hint::black_box(frame.camera_position);
            trace.frame(i as f64, 0.5, 1, f.trace_camera(0.5), [0.; 3]);
        }
    });
    assert_eq!(count, 0);
    assert!(sim.world().contains(camera));
}

#[test]
fn first_transform_on_an_older_entity_initializes_history_and_reset_reuses_meshes() {
    struct Later;
    impl Game for Later {
        type Args = ();
        const ID: &'static str = "late-pose";
        fn setup(w: &mut World, _: &Self::Args) {
            w.spawn(Mesh::cube(1.0));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            let e = w.query::<&Mesh>().iter().next().unwrap().0;
            if !w.has::<Transform>(e) {
                w.insert(e, Transform::at(9., 0., 0.));
            }
        }
    }
    let mut sim = Sim::<Later>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    assert_eq!(r.current[0], 9.);
    assert_eq!(r.previous[0], 9.);
    assert_eq!(r.slots, [0]);
    f.reset();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.meshes.len(), 1);
}

#[test]
fn ancestor_teleports_and_parent_edits_snap_mesh_camera_and_lights() {
    struct Empty;
    impl Game for Empty {
        type Args = ();
        const ID: &'static str = "parent-snap";
        fn setup(_: &mut World, _: &Self::Args) {}
        fn tick(_: &mut World, _: &Input, _: &Self::Args) {}
    }
    let mut sim = Sim::<Empty>::new(()).unwrap();
    let w = sim.world_mut();
    let root = w.spawn(Transform::default());
    let middle = w.spawn((Transform::at(1., 0., 0.), Parent(root)));
    let mesh = w.spawn((Transform::at(1., 0., 0.), Parent(middle), Mesh::cube(1.0)));
    let camera = w.spawn((Transform::at(0., 0., 8.), Parent(middle), Camera::default()));
    let light = w.spawn((Transform::default(), Parent(middle), PointLight::default()));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world_mut().teleport(root, Transform::at(20., 0., 0.));
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(mesh, true).x, 22.);
    assert_eq!(r.position(mesh, false).x, 22.);
    let frame = f.frame(sim.world(), 0.5, 1.);
    assert_eq!(frame.camera_position.x, 21.);
    assert_eq!(frame.points[0].position.x, 21.);
    // Clear freshness, then remove/reinsert Parent with no fresh entity involved.
    sim.advance_with(34., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world_mut().remove::<Parent>(middle);
    sim.world_mut().propagate();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(middle, true).x, 1.);
    assert_eq!(r.position(mesh, true).x, 2.);
    assert_eq!(f.frame(sim.world(), 0.5, 1.).camera_position.x, 1.);
    sim.world_mut().insert(middle, Parent(root));
    sim.world_mut().propagate();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(r.position(mesh, true).x, 22.);
    assert_eq!(r.position(camera, true).x, 21.);
    assert_eq!(r.position(light, true).x, 21.);
}

#[test]
fn load_invalidates_equal_revisions_and_does_not_change_save_or_hash() {
    let mut w = World::new(60, 0);
    let e = w.spawn((
        Transform::at(1., 0., 0.),
        Mesh::cube(1.0),
        Camera::default(),
    ));
    let a = w.save();
    w.get_mut::<Transform>(e).unwrap().position.x = 8.;
    let b = w.save();
    w.load(&a).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    let revision = w.revision::<Transform>();
    let generation = w.presentation_generation();
    w.load(&b).unwrap();
    assert_eq!(w.revision::<Transform>(), revision);
    assert!(w.presentation_generation() > generation);
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.position(e, true).x, 8.);
    assert_eq!(r.position(e, false).x, 8.);
    assert_eq!(f.frame(&w, 0.5, 1.).camera_position.x, 8.);
    let hash = w.hash();
    w.load(&b).unwrap();
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save(), b);
}

#[test]
fn sun_skips_transformless_light_and_slerps_each_tick() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    sim.world_mut().spawn(DirectionalLight::default());
    let sun = sim
        .world_mut()
        .spawn((Transform::default(), DirectionalLight::default()));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    assert!(f.frame(sim.world(), 0., 1.).sun.is_some());
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    sim.world().get_mut::<Transform>(sun).unwrap().rotation = Quat::from_rotation_y(1.);
    f.feed_to(sim.world(), &mut r).unwrap();
    let direction = f.frame(sim.world(), 0.5, 1.).sun.unwrap().direction;
    assert!(direction.distance(Quat::from_rotation_y(0.5) * -Vec3::Z) < 1e-5);
}

#[test]
fn zero_quaternion_cpu_pose_is_finite_identity() {
    let mut w = World::new(60, 0);
    let e = w.spawn(Transform {
        rotation: Quat::from_xyzw(0., 0., 0., 0.),
        ..Default::default()
    });
    assert_eq!(scene::pose(&w, e).unwrap().rotation, Quat::IDENTITY);
    // Degenerate decomposition must not normalize zero/non-finite into NaN either.
    w.get_mut::<Transform>(e).unwrap().scale = Vec3::ZERO;
    assert_eq!(scene::pose(&w, e).unwrap().rotation, Quat::IDENTITY);
}

#[test]
fn light_membership_uses_current_distance_then_entity_order() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let w = sim.world_mut();
    let camera = w.spawn((Transform::default(), Camera::default()));
    for _ in 0..15 {
        w.spawn((Transform::at(0., 1., 0.), PointLight::default()));
    }
    let old = w.spawn((Transform::at(-10., 0., 0.), PointLight::default()));
    let challenger = w.spawn((Transform::at(10.1, 0., 0.), PointLight::default()));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    let selected = |f: &Feed, e| f.scene.lights_for_test().contains(&e);
    assert!(selected(&f, old));
    for x in [0.2, 0., 0.2, 0.] {
        sim.world().get_mut::<Transform>(camera).unwrap().position.x = x;
        f.feed_to(sim.world(), &mut r).unwrap();
        assert!(selected(&f, old));
        assert!(!selected(&f, challenger));
        assert_eq!(f.scene.lights_for_test().len(), 16);
    }
    // Stop's first tick translates every light by one metre. Cross their
    // current midpoint to prove selection has no retained-membership bias.
    for x in [1.2, 1., 1.2, 1.] {
        sim.world().get_mut::<Transform>(camera).unwrap().position.x = x;
        f.feed_to(sim.world(), &mut r).unwrap();
        assert_eq!(selected(&f, old), x == 1.);
        assert_eq!(selected(&f, challenger), x != 1.);
        assert_eq!(f.scene.lights_for_test().len(), 16);
    }
    sim.world().get_mut::<Transform>(camera).unwrap().position.x = 2.;
    f.feed_to(sim.world(), &mut r).unwrap();
    assert!(!selected(&f, old));
    assert!(selected(&f, challenger));
    let tied_position = sim.world().get::<Transform>(old).unwrap().position;
    sim.world()
        .get_mut::<Transform>(challenger)
        .unwrap()
        .position = tied_position;
    f.feed_to(sim.world(), &mut r).unwrap();
    assert!(
        selected(&f, old),
        "equal distances retain the lower entity index"
    );
    assert!(!selected(&f, challenger));
}

#[test]
fn light_selection_matches_restored_current_state_with_twenty_lights() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let w = sim.world_mut();
    let camera = w.spawn((Transform::default(), Camera::default()));
    for i in 0..20 {
        w.spawn((
            Transform::at(if i < 16 { -10. } else { 10.1 }, i as f32 * 0.001, 0.),
            PointLight::default(),
        ));
    }
    let mut running = Feed::default();
    let mut r = Recording::default();
    running.feed_to(w, &mut r).unwrap();
    assert_eq!(
        running.scene.lights_for_test().len(),
        16,
        "empty selection cannot pass"
    );
    sim.world().get_mut::<Transform>(camera).unwrap().position.x = 0.2;
    running.feed_to(sim.world(), &mut r).unwrap();
    let save = sim.save().unwrap();
    sim.restore(&save).unwrap();
    let mut restored = Feed::default();
    restored.feed_to(sim.world(), &mut r).unwrap();
    assert_eq!(
        running.scene.lights_for_test(),
        restored.scene.lights_for_test()
    );
}

#[test]
fn fixture_writes_one_or_two_pages_while_moving_and_none_at_rest() {
    let mut sim = Sim::<crate::test_game::Fixture>::from_values(&[
        exact_game::Value::Number(7.),
        exact_game::Value::Bool(false),
        exact_game::Value::Bool(false),
    ])
    .unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.input(exact_game::InputEvent::Key {
        code: "KeyW".into(),
        down: true,
        at_ms: 0.,
    });
    let writes = |r: &Recording| {
        r.calls
            .iter()
            .filter(|c| matches!(c, Call::Transform(..) | Call::Material(..)))
            .count()
    };
    for tick in 1..=96 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        assert_eq!(writes(&r), 1);
        assert!(r.calls.contains(&Call::Transform(0, PAGE * 10, false)));
    }
    sim.input(exact_game::InputEvent::Key {
        code: "KeyW".into(),
        down: false,
        at_ms: 1600.001,
    });
    sim.input(exact_game::InputEvent::Key {
        code: "KeyE".into(),
        down: true,
        at_ms: 1600.001,
    });
    let mut glowing = 0;
    let mut previous_emissive = 0.;
    for tick in 97..=360 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        assert!(writes(&r) <= 1);
        assert!(!r.calls.iter().any(|c| matches!(c, Call::Material(..))));
        let frame = f.frame(sim.world(), 1., 1.);
        let emissive = frame.glows[0].material_at(frame.seconds)[6];
        glowing += usize::from(emissive > previous_emissive);
        previous_emissive = emissive;
        if tick > 300 {
            assert_eq!(writes(&r), 0);
        }
    }
    assert!(glowing > 1, "the fixture sphere must have glowed");
    eprintln!("fixture writes: moving=1 transform page/tick; glow sampled without material-page writes; settled=0");
}

#[test]
fn identical_parent_writes_stop_uploading_settled_transform_pages() {
    let mut sim = Sim::<Stop>::new(()).unwrap();
    let w = sim.world_mut();
    let root = w.spawn(Transform::default());
    let mut entities = Vec::new();
    for i in 2..PAGE * 40 {
        entities.push(w.spawn((Transform::at(i as f32, 0., 0.), Material::default())));
    }
    let child = entities[PAGE];
    w.insert(child, Parent(root));
    w.propagate();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(w, &mut r).unwrap();
    assert_eq!(
        r.calls
            .iter()
            .filter(|c| matches!(c, Call::Transform(..)))
            .count(),
        1
    );
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| f.feed_to(w, &mut r).unwrap());
    // Repeated writes of identical values still bump the column revision.
    for tick in 2..100 {
        sim.world().get_mut::<Transform>(root).unwrap().position.x = 1.;
        sim.world_mut().propagate();
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        if tick > 40 {
            assert!(!r.calls.iter().any(|c| matches!(c, Call::Transform(..))));
        }
    }
    assert_eq!(r.position(child, true), r.position(child, false));
}

#[test]
fn several_pages_of_still_transforms_with_moving_camera_write_only_its_page() {
    struct CameraOnly;
    impl Game for CameraOnly {
        type Args = ();
        const ID: &'static str = "large-still-writes";
        fn setup(w: &mut World, _: &Self::Args) {
            for _ in 0..(PAGE * 3 + 17) {
                w.spawn(Transform::default());
            }
            w.spawn((Transform::default(), Camera::default()));
        }
        fn tick(w: &mut World, _: &Input, _: &Self::Args) {
            for (_, (_, t)) in w.query::<(&Camera, &mut Transform)>().iter() {
                t.position.x += 1.;
            }
        }
    }
    let mut sim = Sim::<CameraOnly>::new(()).unwrap();
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(sim.world(), &mut r).unwrap();
    sim.advance(0., Clock::Seekable);
    for tick in 1..=40 {
        r.calls.clear();
        sim.advance_with(
            tick as f64 * 1000. / 60. + 0.001,
            Clock::Seekable,
            |w, _| f.feed_to(w, &mut r).unwrap(),
        );
        if tick >= 35 {
            assert_eq!(
                r.calls,
                [
                    Call::Begin,
                    Call::Transform((PAGE * 3) as u32, PAGE * 10, false)
                ]
            );
        }
    }
    eprintln!(
        "3 full pages plus a partial page of still entities + moving camera: 1 transform page/write per tick, 0 material writes"
    );
}

#[test]
fn animated_dimensions_never_grow_geometry_and_all_spheres_batch_together() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named("pulse", (Transform::default(), Mesh::sphere(0.5)));
    for i in 1..7 {
        w.spawn((Transform::default(), Mesh::sphere(i as f32)));
    }
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(r.meshes.len(), 1);
    assert_eq!(f.batches.len(), 1);
    assert_eq!(f.slots.len(), 7);
    for radius in [0.01, 0.5, 4.599, 2.0, 0.5] {
        *w.get_mut::<Mesh>(e).unwrap() = Mesh::sphere(radius);
        f.feed_to(&w, &mut r).unwrap();
        assert_eq!(r.meshes.len(), 1);
    }
    assert_eq!(f.batches.len(), 1);
    assert_eq!(r.materials[9], 1.0);
}
#[test]
fn asset_mesh_waits_without_inventing_geometry() {
    let mut w = World::new(60, 0);
    w.spawn((Transform::default(), Mesh::asset("castle")));
    let mut feed = Feed::default();
    let mut r = Recording::default();
    feed.feed_to(&w, &mut r).unwrap();
    assert!(feed.batches.is_empty());
}

#[test]
fn revealing_a_hidden_primitive_keeps_its_dimensions() {
    let mut w = World::new(60, 0);
    let e = w.spawn((Transform::default(), Mesh::sphere(2.0), Visible(false)));
    let mut f = Feed::default();
    let mut r = Recording::default();
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(&r.materials[9..12], &[4.0; 3]);
    assert!(f.batches.is_empty());
    w.get_mut::<Visible>(e).unwrap().0 = true;
    f.feed_to(&w, &mut r).unwrap();
    assert_eq!(f.batches.len(), 1);
    assert_eq!(&r.materials[9..12], &[4.0; 3]);
}

#[test]
#[ignore = "release CPU feed diagnostic; recording backend, no GPU"]
fn feed_cpu_cost() {
    use std::time::Instant;
    for n in [200_000, 500_000] {
        for moving in [n, n / 100, 0] {
            let mut w = World::new(60, 0);
            for _ in 0..n {
                w.spawn((Transform::default(), Mesh::cube(1.0), Material::default()));
            }
            let camera = w.spawn((Transform::default(), Camera::default()));
            w.load(&w.save()).unwrap(); // Clear setup-only fresh entities before steady measurements.
            let mut f = Feed::default();
            let mut r = Recording {
                record: false,
                ..Recording::default()
            };
            f.feed_to(&w, &mut r).unwrap();
            let mut samples = Vec::new();
            for tick in 0..300 {
                for (_, t) in w.query::<&mut Transform>().iter().take(moving) {
                    t.position.x += 1.;
                }
                w.get_mut::<Transform>(camera).unwrap().position.x += 1.;
                let start = Instant::now();
                f.feed_to(&w, &mut r).unwrap();
                if tick >= 60 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            eprintln!(
                "CPU_FEED n={n} moving={moving} p50={:.4} p95={:.4}",
                samples[119], samples[227]
            );
        }
    }
}

#[test]
fn renderer_defaults_match_world_and_negative_alpha_does_not_enable_grid() {
    assert_eq!(
        crate::Environment::default().fog,
        exact_game::Environment::default().fog
    );
    let mut material = Material::default();
    material.color[3] = -2.0;
    assert!(material_floats(material)[3] >= 0.0);
    material.grid_spacing = 2.0;
    assert_eq!(material_floats(material)[3], -2.0);
}

#[test]
fn aspect_only_feed_preserves_authored_integer_camera_height() {
    let mut w = World::new(60, 0);
    w.spawn((
        Transform::default(),
        Camera::orthographic(180.).integer_scale(),
    ));
    w.propagate();
    let mut f = Feed::default();
    f.feed_to(&w, &mut Recording::default()).unwrap();
    assert!((f.frame(&w, 1., 2.).proj.y_axis.y - 2. / 180.).abs() < 1e-7);
    assert!((f.frame_pixels(&w, 1., (800., 400.)).proj.y_axis.y - 2. / 200.).abs() < 1e-7);
}

#[test]
fn e10_glow_samples_frame_time_without_writing_saved_materials() {
    use exact_game::{Glow, Now, Tween};
    struct Empty;
    impl Game for Empty {
        const ID: &'static str = "glow-frame";
        type Args = ();
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Empty>::new(()).unwrap();
    sim.run(250.);
    let w = sim.world_mut();
    let e = w.spawn((
        Transform::default(),
        Mesh::sphere(0.5),
        Material::glow([3., 2., 1.]),
        Glow(Tween::new(0.)),
    ));
    w.get_mut::<Glow>(e)
        .unwrap()
        .0
        .to(Now { tick: 0, hz: 60 }, 1., 0.5);
    let mut feed = Feed::default();
    let mut r = Recording::default();
    feed.feed_to(w, &mut r).unwrap();
    let frame = feed.frame(w, 1., 1.);
    assert_eq!(frame.glows.len(), 1);
    let glow = &frame.glows[0];
    assert_eq!(&glow.material_at(0.)[6..9], &[0., 0., 0.]);
    assert_eq!(&glow.material_at(0.25)[6..9], &[1.5, 1., 0.5]);
    assert_eq!(&glow.material_at(0.5)[6..9], &[2. + 1. / 1.5, 2., 1.]);
    let saved = w.hash();
    let frame = feed.frame(w, 0.5, 1.);
    let expected = w.get::<Glow>(e).unwrap().0.value_at(frame.seconds, w.hz());
    assert!((frame.seconds - 14.5 / 60.).abs() < 1e-9);
    assert_eq!(frame.glows[0].material_at(frame.seconds)[8], expected);
    assert!(
        expected
            < w.get::<Glow>(e).unwrap().0.value(Now {
                tick: w.tick(),
                hz: w.hz()
            })
    );
    assert_eq!(w.hash(), saved);
    assert_eq!(w.get::<Material>(e).unwrap().emissive, [3., 2., 1.]);
    w.remove::<Glow>(e);
    feed.feed_to(w, &mut r).unwrap();
    assert!(feed.frame(w, 1., 1.).glows.is_empty());
    assert_eq!(&r.materials[6..9], &[3., 2., 1.]);
}
