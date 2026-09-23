//! Row leases: different rows of one component borrow together; the same row
//! refuses, naming the component, the entity, both kinds of lease and both callers.
//! @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
use exact_game::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Default, Component)]
struct Enemy {
    speed: f32,
}
#[derive(Default, Component)]
struct Health(i32);

fn refusal(f: impl FnOnce()) -> String {
    let e = catch_unwind(AssertUnwindSafe(f)).expect_err("must refuse");
    match e.downcast_ref::<String>() {
        Some(text) => text.clone(),
        None => e.downcast_ref::<&str>().unwrap().to_string(),
    }
}
fn at(line: u32) -> String {
    format!("{}:{line}:", file!())
}

/// The chase every game writes, as an author writes it: each enemy reads the
/// player's Transform inside the loop that moves the enemies' Transforms, and a
/// bite holds the player's Health while it changes an enemy's.
struct Chase;
impl Game for Chase {
    const ID: &'static str = "chase";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Transform::at(0.0, 0.0, 0.0), Health(10)));
        for i in 0..6 {
            let angle = i as f32 / 6.0 * std::f32::consts::TAU;
            w.spawn_named(
                format!("enemy-{i}"),
                (
                    Transform::at(math::cos(angle) * 12.0, 0.0, math::sin(angle) * 12.0),
                    Enemy {
                        speed: 3.0 + i as f32 * 0.25,
                    },
                    Health(3),
                ),
            );
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        for (_, (pose, enemy)) in w.query::<(&mut Transform, &Enemy)>().iter() {
            let player = w.require::<Transform>("player").position;
            let to_player = player - pose.position;
            if to_player.length() > 1.0 {
                pose.position += to_player.normalize() * enemy.speed * w.dt();
            }
        }
        let mut player = w.require_mut::<Health>("player");
        for (enemy, _) in w.query::<&Enemy>().iter() {
            let reach = w.local_position(enemy).unwrap() - w.local_position("player").unwrap();
            if reach.length() <= 1.0 && w.require::<Health>(enemy).0 > 0 {
                player.0 -= 1;
                w.require_mut::<Health>(enemy).0 -= 1;
            }
        }
        w.publish("health", player.0);
    }
}

#[test]
fn enemies_chase_the_player_and_bite_inside_their_loops() {
    let mut sim = Sim::<Chase>::new(()).unwrap();
    sim.run(6_000.0);
    let w = sim.world();
    for (_, (pose, _)) in w.query::<(&Transform, &Enemy)>().iter() {
        assert!(pose.position.length() <= 1.0, "{:?}", pose.position);
    }
    assert_eq!(w.count::<Health>(|h| h.0 == 0), 6);
    assert_eq!(w.require::<Health>("player").0, 10 - 6 * 3);
    let mut restored = Sim::<Chase>::new(()).unwrap();
    restored.restore(&sim.save().unwrap()).unwrap();
    assert_eq!(restored.world().hash(), sim.world().hash());
}

#[test]
fn every_accessor_reads_another_row_inside_a_mutable_query() {
    let mut w = World::new(60, 0);
    let root = w.spawn_named("root", Transform::at(1.0, 0.0, 0.0));
    let player = w.spawn_named("player", (Transform::at(2.0, 0.0, 0.0), Parent(root)));
    w.propagate();
    for i in 0..3 {
        w.spawn((Transform::at(10.0 + i as f32, 0.0, 0.0), Enemy::default()));
    }
    let local = Vec3::new(2.0, 0.0, 0.0);
    for (_, pose) in w.query::<&mut Transform>().with::<Enemy>().iter() {
        assert_eq!(w.get::<Transform>(player).unwrap().position, local);
        assert_eq!(w.require::<Transform>("player").position, local);
        assert_eq!(w.local_position("player"), Some(local));
        assert_eq!(w.global_position("player"), Some(Vec3::new(3.0, 0.0, 0.0)));
        pose.position.x -= w.global_position(player).unwrap().x;
    }
    // Consuming iteration yields row guards; other rows stay free meanwhile.
    for (mut pose, _) in w.query::<(&mut Transform, &Enemy)>() {
        pose.position.y = w.require::<Transform>("player").position.x;
    }
    // A shared query lets another row be written, and nested disjoint queries overlap.
    for (_, pose) in w.query::<&Transform>().with::<Enemy>().iter() {
        w.require_mut::<Transform>("root").position.z += pose.position.x;
    }
    for (_, pose) in w.query::<&mut Transform>().with::<Enemy>().iter() {
        for (_, anchor) in w.query::<&Transform>().without::<Enemy>().iter() {
            pose.position.z += anchor.position.x;
        }
    }
    let enemies: Vec<_> = w
        .query::<&Transform>()
        .with::<Enemy>()
        .iter()
        .map(|(_, t)| t.position)
        .collect();
    assert_eq!(
        enemies,
        [7.0, 8.0, 9.0].map(|x| Vec3::new(x, 2.0, 1.0 + 2.0)),
        "each enemy moved by the player's pose and saw root and player"
    );
    assert_eq!(w.require::<Transform>("root").position.z, 7.0 + 8.0 + 9.0);
    // A held row guard admits exclusive borrows of every other row.
    let mut first = w.require_mut::<Transform>("root");
    let mut second = w.require_mut::<Transform>(player);
    std::mem::swap(&mut first.position, &mut second.position);
    drop((first, second));
    assert_eq!(w.local_position("root"), Some(local));
}

#[test]
fn the_same_row_is_refused_naming_component_entity_kinds_and_callers() {
    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::default());
    w.spawn((Transform::default(), Enemy::default()));

    let line = line!() + 2;
    let text = refusal(|| {
        for (_, _pose) in w.query::<&mut Transform>().iter() {
            w.require::<Transform>("player");
        }
    });
    for part in [
        "borrow conflict on Transform of `player` (#0, generation 0)",
        "requested: shared borrow of one row (get/require/position), at ",
        "held by:   exclusive query (&mut Transform), taken at ",
        &at(line),
        &at(line + 1),
    ] {
        assert!(text.contains(part), "{part:?} missing from:\n{text}");
    }

    let guard = w.get_mut::<Transform>(player).unwrap();
    let line = line!() - 1;
    let text = refusal(|| {
        w.global_position(player);
    });
    assert!(
        text.contains("held by:   exclusive borrow of one row"),
        "{text}"
    );
    assert!(
        text.contains(&at(line)) && text.contains(&at(line + 3)),
        "{text}"
    );
    let text = refusal(|| for (_, _pose) in w.query::<&Transform>().iter() {});
    assert!(
        text.contains("requested: shared query (&Transform)") && text.contains("#0"),
        "{text}"
    );
    // Filters narrow a query's lease before it is taken.
    for (_, pose) in w.query::<&Transform>().with::<Enemy>().iter() {
        assert_eq!(pose.position, Vec3::ZERO);
    }
    let text = refusal(|| {
        w.hash();
    });
    assert!(
        text.contains("requested: shared engine read of every row")
            && text.contains("`player` (#0, generation 0)"),
        "{text}"
    );
    drop(guard);

    let pages = w.pages::<Transform>();
    let text = refusal(|| {
        w.get_mut::<Transform>(player);
    });
    assert!(
        text.contains("held by:   shared borrow of every row (pages)"),
        "{text}"
    );
    drop(pages);

    // Overlapping queries name the first row they share.
    let text = refusal(|| {
        for (_, _outer) in w.query::<&mut Transform>().with::<Enemy>().iter() {
            for (_, _inner) in w.query::<&Transform>().iter() {}
        }
    });
    assert!(
        text.contains("Transform of #1 (generation 0)")
            && text.contains("held by:   exclusive query (&mut Transform).with::<Enemy>()"),
        "{text}"
    );

    let rng = w.rng();
    let line = line!() - 1;
    let text = refusal(|| {
        w.rand(0..3);
    });
    assert!(
        text.contains("borrow conflict on resource Rng")
            && text.contains("held by:   exclusive borrow")
            && text.contains(&at(line))
            && text.contains(&at(line + 3)),
        "{text}"
    );
    drop(rng);
    assert!(w.get_mut::<Transform>(player).is_some());
}

#[test]
fn escaped_rows_keep_only_their_rows_and_unwinding_releases_every_lease() {
    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::default());
    let enemy = w.spawn((Transform::default(), Enemy::default()));
    let rows: Vec<_> = w
        .query::<&mut Transform>()
        .with::<Enemy>()
        .into_iter()
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(w.get_mut::<Transform>(player).is_some());
    assert!(refusal(|| {
        w.get::<Transform>(enemy);
    })
    .contains("exclusive query (&mut Transform).with::<Enemy>()"));
    drop(rows);
    assert!(w.get_mut::<Transform>(enemy).is_some());

    let unwound = catch_unwind(AssertUnwindSafe(|| {
        let _held = w.get_mut::<Transform>(player);
        let mut query = w.query::<&mut Transform>().with::<Enemy>();
        let _row = query.iter().next();
        w.get::<Transform>(enemy);
    }));
    assert!(unwound.is_err());
    for (_, pose) in w.query::<&mut Transform>().iter() {
        pose.position.x += 1.0;
    }
    assert_eq!(w.local_position(enemy).unwrap().x, 1.0);
}

#[test]
fn leaked_leases_stay_refused_and_a_structural_edit_retires_their_rows_conservatively() {
    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::default());
    let enemy = w.spawn((Transform::default(), Enemy::default()));
    std::mem::forget(w.get_mut::<Transform>(enemy));
    assert!(w.get::<Transform>(player).is_some());
    assert!(refusal(|| {
        w.get::<Transform>(enemy);
    })
    .contains("exclusive borrow of one row"));

    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::default());
    w.spawn((Transform::default(), Enemy::default()));
    let mut query = w.query::<&mut Transform>().with::<Enemy>();
    query.iter().count();
    std::mem::forget(query);
    assert!(w.get::<Transform>(player).is_some());
    // After the world changes shape the leaked lease's rows are unknowable, so it
    // holds every row of its columns rather than reading storage it cannot trust.
    w.spawn(Enemy::default());
    assert!(refusal(|| {
        w.get::<Transform>(player);
    })
    .contains("exclusive query (&mut Transform).with::<Enemy>()"));
    assert!(w.get::<Enemy>(player).is_none());
}

#[test]
fn exclusive_queries_nest_over_disjoint_rows_and_narrowing_releases_rows() {
    let mut w = World::new(60, 0);
    let player = w.spawn_named("player", Transform::at(1.0, 0.0, 0.0));
    for x in [2.0, 3.0] {
        w.spawn((Transform::at(x, 0.0, 0.0), Enemy::default()));
    }
    for (_, enemy) in w.query::<&mut Transform>().with::<Enemy>().iter() {
        for (_, other) in w.query::<&mut Transform>().without::<Enemy>().iter() {
            other.position.y += enemy.position.x;
            enemy.position.y -= 1.0;
        }
    }
    assert_eq!(w.local_position(player), Some(Vec3::new(1.0, 5.0, 0.0)));

    let mut all = w.query::<&mut Transform>();
    assert_eq!(all.iter().count(), 3);
    assert!(refusal(|| {
        w.get::<Transform>(player);
    })
    .contains("exclusive query (&mut Transform), taken at"));
    let mut enemies = all.with::<Enemy>();
    assert_eq!(w.local_position(player).unwrap().x, 1.0);
    assert_eq!(enemies.iter().count(), 2);
    drop(enemies);
    assert!(w.get_mut::<Transform>(player).is_some());
}
