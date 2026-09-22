use super::*;
use exact_game::{Game, Input, Sim};
struct Lights;
impl Game for Lights {
    const ID: &'static str = "current-scene-lights";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("camera", (Transform::default(), Camera::default()));
        for i in 0..17 {
            w.spawn_named(
                format!("light-{i:02}"),
                (Transform::at(i as f32 + 1., 0., 0.), PointLight::default()),
            );
        }
        w.spawn_named(
            "off",
            (
                Transform::default(),
                PointLight {
                    intensity: 0.,
                    ..Default::default()
                },
            ),
        );
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.get_mut::<Transform>("camera").unwrap().position.x = w.tick_end().tick as f32 / 60.;
    }
}
fn feed(scene: &mut Scene, w: &World) {
    scene.feed(w, true, true, true, false);
}
fn names(scene: &Scene, w: &World) -> Vec<String> {
    scene
        .lights_for_test()
        .iter()
        .map(|&e| w.name(e).unwrap().to_owned())
        .collect()
}
#[test]
fn seventeen_lights_exclude_zero_and_match_continuous_seek_restore() {
    let mut sim = Sim::<Lights>::new(()).unwrap();
    let mut continuous = Scene::default();
    for _ in 0..600 {
        sim.run(1000. / 60.);
        feed(&mut continuous, sim.world());
    }
    let mut seek = Sim::<Lights>::new(()).unwrap();
    seek.run(10_000.);
    let mut direct = Scene::default();
    feed(&mut direct, seek.world());
    let saved = sim.save().unwrap();
    sim.restore(&saved).unwrap();
    let mut restored = Scene::default();
    feed(&mut restored, sim.world());
    let expected: Vec<_> = [9, 8, 10, 7, 11, 6, 12, 5, 13, 4, 14, 3, 15, 2, 16, 1]
        .into_iter()
        .map(|i| format!("light-{i:02}"))
        .collect();
    eprintln!(
        "continuous {:?}; seek {:?}; restore {:?}",
        names(&continuous, sim.world()),
        names(&direct, seek.world()),
        names(&restored, sim.world())
    );
    assert_eq!(names(&continuous, sim.world()), expected);
    assert_eq!(names(&direct, seek.world()), expected);
    assert_eq!(names(&restored, sim.world()), expected);
    // A zero light at the camera must not displace any eligible light.
    sim.world().get_mut::<Transform>("off").unwrap().position.x = 10.;
    feed(&mut restored, sim.world());
    assert_eq!(names(&restored, sim.world()), expected);
}
#[test]
fn twelve_lights_all_contribute_and_zero_intensity_edits_reselect() {
    let mut sim = Sim::<Lights>::new(()).unwrap();
    for i in 12..17 {
        let e = sim.world().named(&format!("light-{i:02}")).unwrap();
        sim.world_mut().despawn(e);
    }
    let mut scene = Scene::default();
    feed(&mut scene, sim.world());
    assert_eq!(scene.lights_for_test().len(), 12);
    let f = scene.frame(sim.world(), 1., glam::Vec2::ONE, false);
    assert_eq!(f.points.len(), 12);
    assert!(f.points.iter().all(|p| p.intensity > 0.));
    sim.world().require_mut::<PointLight>("light-00").intensity = 0.;
    scene.feed(sim.world(), false, false, false, false);
    assert_eq!(scene.lights_for_test().len(), 11);
    sim.world().require_mut::<PointLight>("off").intensity = 1.;
    scene.feed(sim.world(), false, false, false, false);
    assert_eq!(scene.lights_for_test().len(), 12);
    assert_eq!(names(&scene, sim.world())[0], "off");
}

#[test]
fn saved_light_spring_samples_frame_time_and_clamps_negative_overshoot() {
    use exact_game::{Lit, Now, Spring};
    let mut sim = Sim::<Lights>::new(()).unwrap();
    let e = sim.world().named("light-09").unwrap();
    let mut lit = Lit(Spring::new(0.));
    lit.to(Now { tick: 0, hz: 60 }, 1.);
    sim.world_mut().insert(e, lit);
    sim.run(100.);
    let mut scene = Scene::default();
    feed(&mut scene, sim.world());
    let before = sim.save().unwrap();
    let actual = scene
        .frame(sim.world(), 0.5, glam::Vec2::ONE, false)
        .points
        .iter()
        .find(|p| p.position.x == 10.)
        .unwrap()
        .intensity;
    let expected = sim.world().require::<Lit>(e).0.value_at(5.5 / 60., 60)
        * sim.world().require::<PointLight>(e).intensity;
    assert_eq!(actual, expected);
    assert_eq!(before, sim.save().unwrap());
    sim.restore(&before).unwrap();
    scene.reset();
    feed(&mut scene, sim.world());
    assert_eq!(
        scene
            .frame(sim.world(), 0.5, glam::Vec2::ONE, false)
            .points
            .iter()
            .find(|p| p.position.x == 10.)
            .unwrap()
            .intensity,
        actual
    );
    // Current tick is positive but the previous frame sample crosses below zero.
    let now = sim.world().now();
    let mut spring = Spring::new(1.);
    spring.start_tick = now.tick - 1;
    spring.start_value = -1.;
    spring.start_velocity = 200.;
    sim.world_mut().insert(e, Lit(spring));
    assert!(sim.world().require::<Lit>(e).0.value(now) > 0.);
    scene.feed(sim.world(), false, false, false, false);
    assert_eq!(
        scene
            .frame(sim.world(), 0., glam::Vec2::ONE, false)
            .points
            .iter()
            .find(|p| p.position.x == 10.)
            .unwrap()
            .intensity,
        0.
    );
}

#[test]
fn e11_boundary_ties_and_invalid_lights_never_displace_useful_lights() {
    let mut sim = Sim::<Lights>::new(()).unwrap();
    for i in 0..17 {
        sim.world()
            .require_mut::<Transform>(format!("light-{i:02}").as_str())
            .position = Vec3::X;
    }
    sim.world().require_mut::<PointLight>("off").intensity = 1.;
    sim.world().require_mut::<PointLight>("off").range = 0.;
    let expected: Vec<_> = (0..16).map(|i| format!("light-{i:02}")).collect();
    let mut scene = Scene::default();
    feed(&mut scene, sim.world());
    assert_eq!(names(&scene, sim.world()), expected);
    let saved = sim.save().unwrap();
    sim.restore(&saved).unwrap();
    scene.reset();
    feed(&mut scene, sim.world());
    assert_eq!(names(&scene, sim.world()), expected);
    for (intensity, range) in [
        (0., 10.),
        (-1., 10.),
        (f32::NAN, 10.),
        (f32::INFINITY, 10.),
        (1., f32::NAN),
        (1., f32::INFINITY),
        (1., -1.),
    ] {
        {
            let mut light = sim.world().require_mut::<PointLight>("off");
            light.intensity = intensity;
            light.range = range;
        }
        feed(&mut scene, sim.world());
        assert_eq!(names(&scene, sim.world()), expected);
    }
    for i in 0..4 {
        let e = sim.world().named(&format!("light-{i:02}")).unwrap();
        sim.world_mut().remove::<PointLight>(e);
    }
    feed(&mut scene, sim.world());
    assert_eq!(
        names(&scene, sim.world()),
        (4..17).map(|i| format!("light-{i:02}")).collect::<Vec<_>>()
    );
    for i in 0..4 {
        let e = sim.world().named(&format!("light-{i:02}")).unwrap();
        sim.world_mut().insert(e, PointLight::default());
    }
    feed(&mut scene, sim.world());
    assert_eq!(names(&scene, sim.world()), expected);
}

#[test]
fn light_churn_preserves_survivor_interpolation_and_snaps_new_arrivals() {
    fn points(scene: &mut Scene, w: &World, alpha: f32) -> Vec<(f32, f32)> {
        scene
            .frame(w, alpha, glam::Vec2::ONE, false)
            .points
            .iter()
            .map(|p| (p.color.x, p.position.x))
            .collect()
    }
    let mut w = World::new(60, 0);
    let light = |id: u32| PointLight {
        color: [id as f32, 0., 0.],
        ..Default::default()
    };
    let entities: Vec<_> = (0..6)
        .map(|i| w.spawn((Transform::at(i as f32 * 2., 0., 0.), light(i))))
        .collect();
    w.load(&w.save()).unwrap();
    let mut scene = Scene::default();
    feed(&mut scene, &w);
    for &e in &entities {
        w.get_mut::<Transform>(e).unwrap().position.x += 1.;
    }
    scene.feed(&w, true, true, false, false);
    w.remove::<PointLight>(entities[0]);
    w.remove::<PointLight>(entities[2]);
    w.remove::<Transform>(entities[4]);
    w.despawn(entities[1]);
    let replacement = w.spawn((Transform::at(-3., 0., 0.), light(99)));
    assert_eq!(replacement.index(), entities[1].index());
    assert_ne!(replacement, entities[1]);
    scene.feed(&w, false, true, true, false);
    assert_eq!(
        points(&mut scene, &w, 0.5),
        [(99., -3.), (3., 6.5), (5., 10.5)]
    );

    w.insert(entities[0], light(0));
    w.insert(entities[2], light(2));
    w.insert(entities[4], Transform::at(9., 0., 0.));
    scene.feed(&w, false, true, true, false);
    assert_eq!(
        points(&mut scene, &w, 0.),
        [
            (0., 1.),
            (99., -3.),
            (2., 5.),
            (3., 6.),
            (4., 9.),
            (5., 10.)
        ]
    );
    assert_eq!(
        points(&mut scene, &w, 0.5),
        [
            (0., 1.),
            (99., -3.),
            (2., 5.),
            (3., 6.5),
            (4., 9.),
            (5., 10.5)
        ]
    );
    let current = points(&mut scene, &w, 1.);
    let saved = w.save();
    let mut restored = Scene::default();
    feed(&mut restored, &w);
    assert_eq!(points(&mut restored, &w, 1.), current);
    assert_eq!(w.save(), saved);
}
