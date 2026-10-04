//! Hostless days and nights: chopping, carrying, feeding, the Deer and saves.
use exact_game::{Sim, Transform, Vec3, Visible};
use forest_logic::camp::{Cycle, Fire, DAY, PERIOD};
use forest_logic::creatures::{Deer, Mind, Wolf};
use forest_logic::forest::{Grove, CLEARING};
use forest_logic::player::{self, Child, Fate, Item, Kind, Player, Trail, TrailKind};
use forest_logic::{Forest, Options};

const TICK: f64 = 1000.0 / 60.0;

fn game(trees: u32, lite: bool) -> Sim<Forest> {
    Sim::<Forest>::new(Options {
        trees,
        lite,
        ..Options::default()
    })
    .unwrap()
}

fn place(sim: &mut Sim<Forest>, name: &str, at: Vec3) {
    let w = sim.world_mut();
    let e = w.resolve(name).unwrap();
    let t = *w.require::<Transform>(e);
    w.teleport(e, Transform { position: at, ..t });
}

fn player(sim: &Sim<Forest>) -> Player {
    sim.world().require::<Player>("player").clone()
}

/// The standing tree closest to the camp, as (cell, position).
fn nearest_tree(sim: &Sim<Forest>) -> (u32, Vec3) {
    let g = sim.world().resource::<Grove>();
    (0..g.hp.len())
        .filter(|&c| g.hp[c] > 0)
        .map(|c| (c as u32, g.at(c as u32)))
        .min_by(|a, b| a.1.length().total_cmp(&b.1.length()))
        .unwrap()
}

#[test]
fn the_forest_has_exactly_the_trees_asked_for_outside_the_clearing() {
    for trees in [500, 3000] {
        let sim = game(trees, false);
        let g = sim.world().resource::<Grove>();
        assert_eq!(g.standing, trees);
        assert_eq!(g.hp.iter().filter(|&&h| h > 0).count() as u32, trees);
        for c in 0..g.hp.len() {
            if g.hp[c] > 0 {
                let p = g.at(c as u32);
                assert!(
                    Vec3::new(p.x, 0.0, p.z).length() > CLEARING,
                    "tree in the clearing"
                );
                assert!(p.x.abs() < g.half && p.z.abs() < g.half);
            }
        }
    }
}

#[test]
fn trunks_stop_the_player_with_and_without_rapier() {
    for lite in [false, true] {
        let mut sim = game(800, lite);
        sim.run(200.0);
        let (_, tree) = nearest_tree(&sim);
        // Stand 3 m south of the trunk and walk north into it.
        place(
            &mut sim,
            "player",
            Vec3::new(tree.x, tree.y + 0.95, tree.z + 3.0),
        );
        sim.hold("KeyW", 1500.0);
        let p = sim.local_position("player").unwrap();
        let gap = Vec3::new(p.x - tree.x, 0.0, p.z - tree.z).length();
        assert!(gap > 0.5, "lite={lite}: walked into the trunk, gap {gap}");
        assert!(gap < 1.6, "lite={lite}: never reached the trunk, gap {gap}");
    }
}

#[test]
fn chop_carry_and_feed_the_fire() {
    let mut sim = game(800, false);
    sim.run(100.0);
    let (cell, tree) = nearest_tree(&sim);
    place(
        &mut sim,
        "player",
        Vec3::new(tree.x, tree.y + 0.95, tree.z + 1.0),
    );
    sim.run(100.0);
    for _ in 0..3 {
        sim.tap("KeyE");
        sim.run(500.0);
    }
    let g = sim.world().resource::<Grove>().clone();
    assert_eq!(g.hp[cell as usize], 0, "three blows fell a tree");
    assert_eq!(g.standing, 799);
    assert_eq!(player(&sim).chopped, 1);
    let logs: Vec<Vec3> = sim
        .world()
        .query::<(&Transform, &Item)>()
        .iter()
        .filter(|(_, (t, i))| {
            i.kind == Kind::Log && !i.carried && (t.position - tree).length() < 3.0
        })
        .map(|(_, (t, _))| t.position)
        .collect();
    assert_eq!(logs.len(), 2, "a felled tree drops two logs");
    // Walking through the stump is now possible: its collider is gone.
    for log in &logs {
        place(&mut sim, "player", *log + Vec3::Y * 0.8);
        sim.run(50.0);
        sim.tap("KeyE");
        sim.run(100.0);
    }
    assert_eq!(player(&sim).pack.len(), 2, "both logs carried");
    let before = sim.world().resource::<Fire>().fuel;
    place(&mut sim, "player", Vec3::new(0.0, 0.95, 2.5));
    sim.run(100.0);
    sim.tap("KeyE");
    sim.run(100.0);
    let fire = *sim.world().resource::<Fire>();
    assert!(player(&sim).pack.is_empty());
    assert_eq!(fire.fed, 2);
    assert!(fire.fuel > before + 20.0, "fuel {before} → {}", fire.fuel);
}

#[test]
fn hunger_drains_and_food_restores_it() {
    let mut sim = game(500, true);
    sim.run(20_000.0);
    let hungry = player(&sim).hunger;
    assert!(hungry < 92.0, "hunger {hungry}");
    sim.world_mut().require_mut::<Player>("player").hunger = 40.0;
    let hungry = 40.0;
    let food = sim
        .world()
        .query::<(&Transform, &Item)>()
        .iter()
        .find(|(_, (_, i))| i.kind == Kind::Food)
        .map(|(_, (t, _))| t.position)
        .unwrap();
    place(&mut sim, "player", food + Vec3::Y * 0.7);
    sim.run(50.0);
    sim.tap("KeyE");
    sim.run(50.0);
    sim.tap("KeyQ");
    sim.run(50.0);
    assert!(player(&sim).hunger > hungry + 30.0);
}

#[test]
fn rescue_compass_supplies_and_restore_follow_the_childs_fate() {
    let mut sim = game(500, true);
    place(&mut sim, "child-1", Vec3::new(0.0, 0.6, -8.0));
    place(&mut sim, "child-2", Vec3::new(50.0, 0.6, 0.0));
    assert_eq!(
        player::guidance(sim.world()),
        "Find a lost child · N · 11 m"
    );
    place(&mut sim, "player", Vec3::new(0.0, 0.9, -7.0));
    let before = sim.world().resource::<Fire>().fuel;
    sim.tap("KeyE");
    sim.run(TICK);
    // This child is already inside the lit camp, so the interaction rescues it.
    assert_eq!(sim.world().require::<Child>("child-1").fate, Fate::Rescued);
    assert!(sim
        .take_messages()
        .iter()
        .any(|message| message == "rescued"));
    let food = sim.world().count::<Item>(|item| item.kind == Kind::Food);
    let fuel = sim.world().resource::<Fire>().fuel;
    assert!(
        fuel > before + 19.0,
        "rescue adds 20 fuel: {before} → {fuel}"
    );
    let save = sim.save().unwrap();
    let mut restored = game(500, true);
    restored.restore(&save).unwrap();
    for game in [&mut sim, &mut restored] {
        game.run(1000.0);
        assert_eq!(
            game.world().count::<Item>(|item| item.kind == Kind::Food),
            food
        );
        assert!(
            game.world().resource::<Fire>().fuel < fuel,
            "supplies awarded only once"
        );
        assert!(player::guidance(game.world()).starts_with("Find a lost child · E"));
    }
    assert_eq!(sim.save().unwrap(), restored.save().unwrap());
}

#[test]
fn a_child_needs_a_living_escort_and_a_lit_fire_before_supplies_arrive() {
    let mut sim = game(500, true);
    place(&mut sim, "child-1", Vec3::new(0.0, 0.6, 2.5));
    sim.world_mut().require_mut::<Child>("child-1").fate = Fate::Following;
    sim.world_mut().resource_mut::<Fire>().fuel = 0.0;
    let food = sim.world().count::<Item>(|item| item.kind == Kind::Food);
    sim.run(TICK);
    assert_eq!(
        sim.world().require::<Child>("child-1").fate,
        Fate::Following
    );
    assert!(player::guidance(sim.world()).starts_with("Escort 1 to the fire · N"));
    sim.world_mut().resource_mut::<Fire>().fuel = 50.0;
    sim.world_mut().require_mut::<Player>("player").dead = true;
    sim.run(TICK);
    assert_eq!(
        sim.world().require::<Child>("child-1").fate,
        Fate::Following
    );
    sim.world_mut().require_mut::<Player>("player").dead = false;
    sim.run(TICK);
    assert_eq!(sim.world().require::<Child>("child-1").fate, Fate::Rescued);
    assert_eq!(
        sim.world().count::<Item>(|item| item.kind == Kind::Food),
        food + 2
    );
    sim.world_mut().require_mut::<Child>("child-2").fate = Fate::Rescued;
    assert_eq!(
        player::guidance(sim.world()),
        "All children safe · Keep the fire burning"
    );
}

#[test]
fn the_deer_comes_at_night_and_the_flashlight_stuns_it() {
    let mut sim = game(800, true);
    assert!(!sim.world().require::<Visible>("deer").0);
    // The first day starts after dawn, at t = 8 s.
    sim.run((DAY as f64 - 8.0 - 3.0) * 1000.0);
    assert_eq!(sim.world().require::<Deer>("deer").mind, Mind::Hidden);
    sim.run(5000.0);
    assert!(sim.world().resource::<Cycle>().night());
    let deer = *sim.world().require::<Deer>("deer");
    assert_eq!(deer.mind, Mind::Stalk);
    assert!(sim.world().require::<Visible>("deer").0);
    // Inside the light the Deer circles at its edge and never enters.
    let safe = sim.world().resource::<Fire>().radius();
    for _ in 0..20 {
        sim.run(250.0);
        let d = sim.local_position("deer").unwrap();
        assert!(
            Vec3::new(d.x, 0.0, d.z).length() >= safe,
            "the Deer entered the light"
        );
    }
    // Step out of the light, face the Deer and switch on the flashlight.
    let d = sim.local_position("deer").unwrap();
    // A quarter turn round the fire from it, so it has to come for us.
    let radial = Vec3::new(d.x, 0.0, d.z).normalize();
    let out = Vec3::new(-radial.z, 0.0, radial.x) * (safe + 3.0);
    place(&mut sim, "player", Vec3::new(out.x, 0.95, out.z));
    {
        let w = sim.world_mut();
        let to = (d - out).with_y(0.0).normalize();
        w.require_mut::<Player>("player").facing = to;
    }
    sim.tap("KeyF");
    let mut stunned = false;
    for _ in 0..(8.0 * 60.0) as u32 {
        sim.run(TICK);
        stunned |= sim.world().require::<Deer>("deer").mind == Mind::Stunned;
        if stunned {
            break;
        }
        // Keep facing it while it closes in.
        let d = sim.local_position("deer").unwrap();
        let p = sim.local_position("player").unwrap();
        let w = sim.world_mut();
        w.require_mut::<Player>("player").facing = (d - p).with_y(0.0).normalize();
    }
    let deer = *sim.world().require::<Deer>("deer");
    assert!(stunned && deer.strikes == 0, "{deer:?}");
    assert!(player(&sim).battery < 100.0);
}

#[test]
fn wolves_wander_and_dawn_counts_a_night() {
    let mut sim = game(800, true);
    let start: Vec<Vec3> = sim
        .world()
        .query::<(&Transform, &Wolf)>()
        .iter()
        .map(|(_, (t, _))| t.position)
        .collect();
    assert_eq!(start.len(), 8);
    sim.run(PERIOD as f64 * 1000.0);
    let moved = sim
        .world()
        .query::<(&Transform, &Wolf)>()
        .iter()
        .zip(&start)
        .filter(|((_, (t, _)), s)| (t.position - **s).length() > 2.0)
        .count();
    assert!(moved >= 6, "only {moved} wolves moved");
    let c = *sim.world().resource::<Cycle>();
    assert_eq!((c.day, c.survived), (2, 1));
    assert_eq!(sim.world().require::<Deer>("deer").mind, Mind::Hidden);
}

#[test]
fn a_mid_night_save_restores_and_continues_identically() {
    for lite in [true, false] {
        let mut a = game(1500, lite);
        a.key_down("KeyD");
        a.run((DAY as f64 + 6.0) * 1000.0);
        a.key_up("KeyD");
        assert!(a.world().resource::<Cycle>().night());
        let saved = a.save().unwrap();
        let mut b = game(1500, lite);
        b.restore(&saved).unwrap();
        assert_eq!(a.world().hash(), b.world().hash());
        for sim in [&mut a, &mut b] {
            sim.key_down("KeyS");
            sim.run(4000.0);
            sim.key_up("KeyS");
            sim.tap("KeyF");
            sim.run(4000.0);
        }
        assert_eq!(a.world().hash(), b.world().hash(), "lite={lite}");
        assert_eq!(a.save().unwrap(), b.save().unwrap());
    }
}

#[test]
fn starving_alone_in_the_dark_ends_the_run() {
    let mut sim = game(500, true);
    sim.run(100.0);
    {
        let w = sim.world_mut();
        let mut p = w.require_mut::<Player>("player");
        p.hunger = 0.0;
        p.health = 3.0;
    }
    sim.run(2000.0);
    let p = player(&sim);
    assert!(p.dead && p.health == 0.0);
    let hud = sim.world().published("dead").unwrap();
    assert!(matches!(hud, exact_game::Value::Bool(true)), "{hud:?}");
    assert!(sim.take_messages().iter().any(|m| m == "died"));
    // The dead do not walk.
    let before = sim.local_position("player").unwrap();
    sim.hold("KeyD", 500.0);
    assert_eq!(sim.local_position("player").unwrap(), before);
}

#[test]
fn death_freezes_the_survived_nights_count() {
    let mut sim = game(500, true);
    sim.world_mut().resource_mut::<Cycle>().t = PERIOD - 0.1;
    {
        let mut p = sim.world_mut().require_mut::<Player>("player");
        p.dead = true;
        p.health = 0.0;
    }
    sim.run(200.0);
    assert_eq!(sim.world().resource::<Cycle>().day, 2);
    assert_eq!(sim.world().resource::<Cycle>().survived, 0);
    assert_eq!(
        sim.world().published("survived"),
        Some(exact_game::Value::Number(0.0))
    );
}

#[test]
fn supply_compass_tracks_uncollected_food_and_restores_the_chosen_landmark() {
    let mut sim = game(500, true);
    sim.post("track food");
    sim.run(TICK);
    let target = sim.world().resource::<Trail>().target.unwrap();
    assert_eq!(sim.world().require::<Item>(target).kind, Kind::Food);
    let at = sim.world().require::<Transform>(target).position;
    place(&mut sim, "player", at + Vec3::new(0.0, 0.7, 1.75));
    // Finish the arranged teleport's camera follow before this gameplay checkpoint.
    sim.run(TICK);
    assert!(
        player::guidance(sim.world()).contains(" · N · 2 m"),
        "a rounded two metres must still offer a direction"
    );
    let save = sim.save().unwrap();
    let mut restored = game(500, true);
    restored.restore(&save).unwrap();
    assert!(
        save == restored.save().unwrap(),
        "immediate save roundtrip differs"
    );
    for sim in [&mut sim, &mut restored] {
        assert_eq!(sim.world().resource::<Trail>().target, Some(target));
        sim.tap("KeyE");
        sim.run(100.0);
        assert!(sim.world().require::<Item>(target).carried);
        assert_ne!(sim.world().resource::<Trail>().target, Some(target));
        sim.post("track camp");
        sim.run(TICK);
        assert!(player::guidance(sim.world()).starts_with("Return to camp · "));
        sim.post("track unknown");
        sim.run(TICK);
        assert_eq!(sim.world().resource::<Trail>().kind, TrailKind::Camp);
    }
    let (a, b) = (sim.save().unwrap(), restored.save().unwrap());
    assert!(
        a == b,
        "save continuation differs: worlds {:x}/{:x}; first differing byte {:?}; lengths {}/{}",
        sim.world().hash(),
        restored.world().hash(),
        a.iter().zip(&b).position(|(a, b)| a != b),
        a.len(),
        b.len()
    );
}

#[test]
fn fuel_compass_falls_back_to_a_tree_then_tracks_the_dropped_logs() {
    let mut sim = game(500, true);
    let loose: Vec<_> = sim
        .world()
        .query::<&Item>()
        .iter()
        .filter(|(_, item)| item.kind != Kind::Food)
        .map(|(e, _)| e)
        .collect();
    for item in loose {
        sim.world_mut().despawn(item);
    }
    sim.post("track fuel");
    sim.run(TICK);
    let target = sim.world().resource::<Trail>().target.unwrap();
    assert!(sim
        .world()
        .get::<forest_logic::forest::Tree>(target)
        .is_some());
    let at = sim.world().require::<Transform>(target).position;
    place(&mut sim, "player", at + Vec3::new(0.0, 0.95, 1.0));
    for _ in 0..3 {
        sim.tap("KeyE");
        sim.run(400.0);
    }
    let target = sim.world().resource::<Trail>().target.unwrap();
    assert_eq!(sim.world().require::<Item>(target).kind, Kind::Log);
    assert!(player::guidance(sim.world()).starts_with("Gather log · "));
}

#[test]
fn food_compass_refreshes_an_empty_search_when_supplies_arrive() {
    for rescue in [false, true] {
        let mut sim = game(500, true);
        let foods: Vec<_> = sim
            .world()
            .query::<&Item>()
            .iter()
            .filter(|(_, item)| item.kind == Kind::Food)
            .map(|(e, _)| e)
            .collect();
        for item in foods {
            sim.world_mut().despawn(item);
        }
        sim.post("track food");
        sim.run(TICK);
        assert!(sim.world().resource::<Trail>().target.is_none());
        assert_eq!(
            player::guidance(sim.world()),
            "No food found · More arrives at dawn"
        );
        if rescue {
            place(&mut sim, "child-1", Vec3::new(0.0, 0.6, 2.5));
            sim.world_mut().require_mut::<Child>("child-1").fate = Fate::Following;
        } else {
            sim.world_mut().resource_mut::<Cycle>().t = PERIOD - 0.1;
        }
        sim.run(200.0);
        assert!(
            sim.world().resource::<Trail>().target.is_some(),
            "rescue={rescue}"
        );
        assert!(player::guidance(sim.world()).starts_with("Gather food · "));
    }
}

#[test]
fn wolves_hunt_a_player_outside_the_light_and_bite() {
    let mut sim = game(800, true);
    sim.run(100.0);
    let wolf = sim
        .world()
        .query::<(&Transform, &Wolf)>()
        .iter()
        .map(|(_, (t, _))| t.position)
        .next()
        .unwrap();
    place(&mut sim, "player", wolf + Vec3::new(6.0, 0.5, 0.0));
    sim.run(3000.0);
    assert!(player(&sim).health < 100.0, "no wolf bit");
}
