use super::*;

#[test]
fn teleport_from_preserves_post_landing_motion_for_mesh_camera_and_light() {
    struct Empty;
    impl Game for Empty {
        type Args = ();
        const ID: &'static str = "landing-motion-history";
        fn setup(_: &mut World, _: &()) {}
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let mut sim = Sim::<Empty>::new(()).unwrap();
    let w = sim.world_mut();
    let root = w.spawn((Transform::default(), Mesh::cube(1.)));
    let child = w.spawn((Transform::at(2., 0., 0.), Parent(root), Mesh::cube(1.)));
    w.spawn((Transform::at(0., 0., 8.), Parent(root), Camera::default()));
    w.spawn((Transform::default(), Parent(root), PointLight::default()));
    w.propagate();
    let mut feed = Feed::default();
    let mut recording = Recording::default();
    feed.feed_to(w, &mut recording).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance_with(17., Clock::Seekable, |w, _| {
        feed.feed_to(w, &mut recording).unwrap()
    });
    sim.world_mut()
        .teleport_from(root, Transform::at(20., 0., 0.), Transform::at(24., 0., 0.));
    feed.feed_to(sim.world(), &mut recording).unwrap();
    assert_eq!(
        (
            recording.position(root, true).x,
            recording.position(root, false).x
        ),
        (20., 24.)
    );
    assert_eq!(
        (
            recording.position(child, true).x,
            recording.position(child, false).x
        ),
        (22., 26.)
    );
    let frame = feed.frame(sim.world(), 0.25, 1.);
    assert_eq!(frame.camera_position.x, 21.);
    assert_eq!(frame.lights[0].position.x, 21.);
    let saved = sim.world().save();
    sim.advance_with(34., Clock::Seekable, |w, _| {
        feed.feed_to(w, &mut recording).unwrap()
    });
    assert!(sim.world().interpolation_origin(root).is_none());
    let w = sim.world_mut();
    w.load(&saved).unwrap();
    assert!(w.interpolation_origin(root).is_none());
    w.teleport_from(root, Transform::at(40., 0., 0.), Transform::at(44., 0., 0.));
    w.teleport(root, Transform::at(50., 0., 0.));
    assert!(w.interpolation_origin(root).is_none());
    feed.feed_to(w, &mut recording).unwrap();
    assert_eq!(
        (
            recording.position(root, true).x,
            recording.position(root, false).x
        ),
        (50., 50.)
    );
}
