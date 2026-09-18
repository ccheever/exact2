//! A tiny placement game alongside greybox: existing proof hashes stay intact.
use exact_game::{
    place::{self, Side},
    *,
};

struct Placement;
impl Game for Placement {
    const ID: &'static str = "placement-test";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        place::blockout("P.\n.B", 4.0, Vec3::ZERO, |ch, at| {
            w.spawn_named(
                ch.to_string(),
                (
                    Transform {
                        position: at,
                        ..Transform::default()
                    },
                    Mesh::cube(2.0),
                ),
            );
        })
        .unwrap();
        let top = place::on_top_of(w, "P", 1.0);
        let beside = place::next_to(w, "B", Side::Front, 0.5, Vec3::ONE);
        w.spawn_named(
            "top",
            (
                Transform {
                    position: top,
                    rotation: place::facing(top, beside),
                    ..Transform::default()
                },
                Mesh::cube(1.0),
            ),
        );
        for at in place::ring(4, 8.0)
            .chain(place::ring_jittered(7, 5, 10.0, 12.0))
            .chain(place::grid(2, 3, 2.0))
            .chain(place::line(top, beside, 3))
            .chain(place::scatter(
                8,
                Vec3::splat(-5.0),
                Vec3::splat(5.0),
                6,
                1.0,
            ))
        {
            w.spawn(Transform {
                position: at,
                ..Transform::default()
            });
        }
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}

#[test]
fn placement_game_replays_and_restores() {
    let mut a = Sim::<Placement>::new(()).unwrap();
    let mut b = Sim::<Placement>::new(()).unwrap();
    assert_eq!(a.world().len(), 27);
    assert_eq!(
        a.get::<Transform>("top").unwrap().position,
        Vec3::new(0.0, 1.5, 0.0)
    );
    a.run(1000.0);
    for _ in 0..10 {
        b.run(100.0);
    }
    assert_eq!(a.world().hash(), b.world().hash());
    b.restore(&a.save().unwrap()).unwrap();
    assert_eq!(a.world().save(), b.world().save());
}

#[test]
fn bounds_include_parent_rotation_scale_and_plane_slab() {
    let mut w = World::new(60, 0);
    let parent = w.spawn(Transform::at(10.0, 2.0, 3.0).with_scale(2.0));
    let child = w.spawn_named(
        "plane",
        (Parent(parent), Transform::default(), Mesh::plane(4.0, 6.0)),
    );
    assert_eq!(place::on_top_of(&w, child, 2.0), Vec3::new(10.0, 3.0, 3.0));
    assert_eq!(
        place::next_to(&w, child, Side::Right, 1.0, Vec3::splat(2.0)),
        Vec3::new(16.0, 1.99, 3.0)
    );
    w.get_mut::<Transform>(parent).unwrap().rotation =
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    assert!((place::next_to(&w, child, Side::Right, 1.0, Vec3::splat(2.0)).x - 18.0).abs() < 1e-5);
}

#[test]
fn edge_cases_and_scatter_spacing() {
    assert_eq!(place::ring(0, 1.0).count(), 0);
    assert_eq!(place::grid(0, 4, 1.0).count(), 0);
    assert_eq!(place::grid(4, 0, 1.0).count(), 0);
    assert_eq!(
        place::line(Vec3::X, Vec3::Y, 1).collect::<Vec<_>>(),
        [Vec3::X]
    );
    assert_eq!(place::line(Vec3::X, Vec3::Y, 0).count(), 0);
    assert_eq!(place::facing(Vec3::X, Vec3::X), Quat::IDENTITY);
    for target in [Vec3::Y, -Vec3::Y, -Vec3::Z, Vec3::ONE] {
        assert!(
            (place::facing(Vec3::ZERO, target) * -Vec3::Z - target.normalize()).length() < 1e-6
        );
    }
    let points = place::scatter(77, -Vec3::ONE, Vec3::ONE, 20, 0.4);
    assert_eq!(points.len(), 20);
    assert_eq!(points, place::scatter(77, -Vec3::ONE, Vec3::ONE, 20, 0.4));
    for (i, p) in points.iter().enumerate() {
        assert!(p.abs().cmple(Vec3::ONE).all());
        assert!(points[..i].iter().all(|q| p.distance(*q) >= 0.4));
    }
    for map in ["AB\nA", "A\nAB", "AB\né."] {
        let error =
            place::blockout(map, 1.0, Vec3::ZERO, |_, _| panic!("partial map")).unwrap_err();
        assert_eq!(error.row, 2);
        assert!(error.to_string().contains("column"));
    }
    let mut count = 0;
    place::blockout("A.\r\n B\r\n", 1.0, Vec3::ZERO, |_, _| count += 1).unwrap();
    assert_eq!(count, 2);
}
