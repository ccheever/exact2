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
fn chopping_reports_progress_and_recovery_across_a_checkpoint() {
    for lite in [false, true] {
        let mut sim = game(800, lite);
        sim.run(100.0);
        let (cell, tree) = nearest_tree(&sim);
        place(
            &mut sim,
            "player",
            Vec3::new(tree.x, tree.y + 0.95, tree.z + 1.0),
        );
        sim.run(100.0);
        sim.tap("KeyE");
        sim.run(100.0);
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(sim.world().resource::<Grove>().hp[cell as usize], 2);
        assert!(player(&sim).cooldown > 0.0);
        let prompt = sim.world().published("prompt").unwrap().text().to_owned();
        assert!(prompt.starts_with("Axe recovering"), "{prompt}");
        assert!(prompt.ends_with("2 hits left"), "{prompt}");
        let saved = sim.save().unwrap();
        let mut restored = game(800, lite);
        restored.restore(&saved).unwrap();
        for game in [&mut sim, &mut restored] {
            game.run(300.0);
            assert_eq!(
                game.world().published("prompt").unwrap().text(),
                "Hold E: chop · 2 hits left"
            );
            game.tap("KeyE");
            game.run(400.0);
            assert_eq!(
                game.world().published("prompt").unwrap().text(),
                "Hold E: chop · 1 hit left"
            );
            game.tap("KeyE");
            game.run(100.0);
            assert_eq!(game.world().resource::<Grove>().hp[cell as usize], 0);
            assert!(game
                .world()
                .published("prompt")
                .unwrap()
                .text()
                .starts_with("Axe recovering"));
            game.run(300.0);
            assert_eq!(
                game.world().published("prompt").unwrap().text(),
                "E: pick up log"
            );
        }
        assert!(sim.save().unwrap() == restored.save().unwrap());
    }
}

#[test]
fn a_held_axe_resumes_its_cadence_without_collecting_the_logs() {
    for lite in [false, true] {
        let mut sim = game(800, lite);
        sim.run(100.0);
        let (cell, tree) = nearest_tree(&sim);
        place(
            &mut sim,
            "player",
            Vec3::new(tree.x, tree.y + 0.95, tree.z + 1.0),
        );
        sim.run(100.0);
        sim.key_down("KeyE");
        sim.run(100.0);
        assert_eq!(sim.world().resource::<Grove>().hp[cell as usize], 2);
        let saved = sim.save().unwrap();
        let mut restored = game(800, lite);
        restored.restore(&saved).unwrap();
        for game in [&mut sim, &mut restored] {
            // Save while the key is down, then let two more swings finish.
            game.run(900.0);
            game.key_up("KeyE");
            game.run(100.0);
            assert_eq!(game.world().resource::<Grove>().hp[cell as usize], 0);
            assert_eq!(player(game).chopped, 1);
            assert!(player(game).pack.is_empty(), "holding E must not collect");
            assert_eq!(
                game.world().published("prompt").unwrap().text(),
                "E: pick up log"
            );
        }
        assert!(sim.save().unwrap() == restored.save().unwrap());
    }
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

#[test]
fn a_prepared_camp_shelters_through_dawn_in_both_collision_modes() {
    for lite in [false, true] {
        for time in [8.0, DAY - 1.0, DAY + 1.0, PERIOD - 0.25] {
            let mut sim = game(500, lite);
            sim.world_mut().resource_mut::<Cycle>().t = time;
            let cycle = *sim.world().resource::<Cycle>();
            sim.world_mut().resource_mut::<Fire>().fuel = cycle.dawn_fuel(Fire::default()) + 0.25;
            sim.world_mut().require_mut::<Player>("player").hunger =
                cycle.until_dawn() * 0.45 + 11.0;
            for child in ["child-1", "child-2"] {
                sim.world_mut().require_mut::<Child>(child).fate = Fate::Rescued;
            }
            sim.run(TICK);
            assert!(sim
                .world()
                .published("night_plan")
                .unwrap()
                .text()
                .starts_with("Shelter by the fire until dawn"));
            assert_eq!(
                sim.world().published("night_supplies").unwrap().text(),
                "To dawn: fire ready · food ready"
            );
            let saved = sim.save().unwrap();
            let mut restored = game(500, lite);
            restored.restore(&saved).unwrap();
            for sim in [&mut sim, &mut restored] {
                sim.run(cycle.until_dawn() as f64 * 1000.0);
                assert_eq!(sim.world().resource::<Cycle>().survived, 1);
                assert!(player(sim).health == 100.0 && !player(sim).dead);
                assert!(sim.world().resource::<Fire>().fuel >= 10.0);
                assert!(player(sim).hunger >= 10.0);
            }
            assert!(sim.save().unwrap() == restored.save().unwrap());
        }
    }
}

#[test]
fn preparation_requires_fuel_in_the_fire_and_keeps_the_chosen_compass() {
    let mut sim = game(500, true);
    for child in ["child-1", "child-2"] {
        sim.world_mut().require_mut::<Child>(child).fate = Fate::Rescued;
    }
    sim.post("track fuel");
    sim.run(TICK);
    let target = sim.world().resource::<Trail>().target;
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Gather fuel for the next dawn"
    );
    let log = target.unwrap();
    assert_eq!(sim.world().require::<Item>(log).kind, Kind::Log);
    player::interact(sim.world_mut(), player::Action::Take(log, Kind::Log), false);
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Feed your carried fuel into the campfire"
    );
    assert!(player::preparation(sim.world(), true)
        .1
        .contains("+9 fire fuel"));
    player::interact(sim.world_mut(), player::Action::Feed, false);
    assert!(player::preparation(sim.world(), true)
        .0
        .starts_with("Shelter by the fire"));
    assert_eq!(sim.world().resource::<Trail>().target, target);
    place(&mut sim, "player", Vec3::new(5.0, 0.95, 0.0));
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Supplies ready · Return to camp to shelter"
    );
    // A new dawn brings a whole new night to budget for.
    sim.world_mut().resource_mut::<Cycle>().t = 0.0;
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Gather fuel for the next dawn"
    );
}

#[test]
fn food_readiness_counts_carried_meals_and_prompts_eating_before_starvation() {
    let mut sim = game(500, true);
    sim.world_mut().resource_mut::<Fire>().fuel = 100.0;
    sim.world_mut().require_mut::<Player>("player").hunger = 5.0;
    assert_eq!(
        player::preparation(sim.world(), true).1,
        "To dawn: fire ready · 2 food needed"
    );
    let foods: Vec<_> = sim
        .world()
        .query::<&Item>()
        .iter()
        .filter(|(_, i)| i.kind == Kind::Food)
        .take(2)
        .map(|(e, _)| e)
        .collect();
    for food in foods {
        player::interact(
            sim.world_mut(),
            player::Action::Take(food, Kind::Food),
            false,
        );
    }
    assert_eq!(
        player::preparation(sim.world(), true).1,
        "To dawn: fire ready · food ready"
    );
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Q: eat a carried meal before sheltering"
    );
    sim.tap("KeyQ");
    sim.run(TICK);
    assert!(player::preparation(sim.world(), true)
        .0
        .starts_with("Shelter by the fire"));
    sim.run(12_000.0);
    assert_eq!(
        player::preparation(sim.world(), true).0,
        "Q: eat a carried meal before sheltering"
    );
    sim.tap("KeyQ");
    sim.run(TICK);
    assert_eq!(
        player::preparation(sim.world(), true).1,
        "To dawn: fire ready · food ready"
    );
    assert!(player(&sim).pack.is_empty());
    let remaining = sim.world().resource::<Cycle>().until_dawn();
    sim.run(remaining as f64 * 1000.0 + TICK);
    assert_eq!(sim.world().resource::<Cycle>().survived, 1);
    assert!(!player(&sim).dead && player(&sim).hunger > 10.0);
}

fn carry(sim: &mut Sim<Forest>, kind: Kind) -> exact_game::Entity {
    let e = player::drop_item(sim.world_mut(), kind, 8.0, 0.0);
    player::interact(sim.world_mut(), player::Action::Take(e, kind), false);
    e
}

#[test]
fn windbreak_build_is_atomic_one_time_and_saved_with_its_appearance() {
    for lite in [false, true] {
        let mut sim = game(500, lite);
        let food = carry(&mut sim, Kind::Food);
        carry(&mut sim, Kind::Log);
        carry(&mut sim, Kind::Log);
        let before = sim.save().unwrap();
        player::build_windbreak(sim.world_mut());
        assert!(
            sim.save().unwrap() == before,
            "missing scrap must not spend logs"
        );
        carry(&mut sim, Kind::Scrap);
        let spare = carry(&mut sim, Kind::Scrap);
        place(&mut sim, "player", Vec3::new(8.0, 0.95, 0.0));
        sim.tap("KeyR");
        sim.run(TICK);
        assert!(!sim.world().resource::<Fire>().windbreak);
        assert_eq!(
            player(&sim).pack.len(),
            5,
            "building away from camp refuses"
        );
        place(&mut sim, "player", Vec3::new(0.0, 0.95, 3.0));
        sim.world_mut().require_mut::<Player>("player").dead = true;
        let before = sim.save().unwrap();
        player::build_windbreak(sim.world_mut());
        assert!(sim.save().unwrap() == before, "dead players cannot build");
        sim.world_mut().require_mut::<Player>("player").dead = false;
        sim.run(TICK);
        assert!(matches!(
            sim.world().published("build_ready"),
            Some(exact_game::Value::Bool(true))
        ));
        let fuel = sim.world().resource::<Fire>().fuel;
        sim.tap("KeyR");
        sim.run(TICK);
        assert!(sim.world().resource::<Fire>().windbreak);
        assert_eq!(player(&sim).pack, vec![food, spare]);
        assert!(
            (sim.world().resource::<Fire>().fuel - fuel).abs() < 0.02,
            "building must not also feed the fire"
        );
        let screen = sim.world().resolve("windbreak").unwrap();
        assert!(sim.world().has::<exact_game::Mesh>(screen));
        assert_eq!(
            sim.world()
                .query::<&exact_game::Parent>()
                .iter()
                .filter(|(_, p)| p.0 == screen)
                .count(),
            2,
            "both wooden posts exist"
        );
        assert_eq!(
            sim.world().published("build_hint").unwrap().text(),
            "Built · Fire uses half the fuel"
        );
        let saved = sim.save().unwrap();
        let mut restored = game(500, lite);
        restored.restore(&saved).unwrap();
        let count = sim.world().len();
        for sim in [&mut sim, &mut restored] {
            sim.post("build windbreak");
            sim.run(1000.0);
            assert_eq!(sim.world().len(), count, "repeated build adds nothing");
            assert_eq!(player(sim).pack, vec![food, spare]);
            assert!(!player::can_build(sim.world()));
        }
        assert!(sim.save().unwrap() == restored.save().unwrap());
    }
}

#[test]
fn windbreak_halves_burn_but_preserves_the_dawn_reserve() {
    for lite in [false, true] {
        for time in [8.0, DAY - 1.0, DAY + 1.0, PERIOD - 0.25] {
            let mut sim = game(500, lite);
            for kind in [Kind::Log, Kind::Log, Kind::Scrap] {
                carry(&mut sim, kind);
            }
            sim.post("build windbreak");
            sim.run(TICK);
            assert!(sim.world().resource::<Fire>().windbreak);
            sim.world_mut().resource_mut::<Cycle>().t = time;
            let cycle = *sim.world().resource::<Cycle>();
            let need = cycle.dawn_fuel(*sim.world().resource::<Fire>());
            assert!((need - 10.0 - (cycle.dawn_fuel(Fire::default()) - 10.0) * 0.5).abs() < 0.001);
            sim.world_mut().resource_mut::<Fire>().fuel = need + 0.25;
            for child in ["child-1", "child-2"] {
                sim.world_mut().require_mut::<Child>(child).fate = Fate::Rescued;
            }
            sim.run(TICK);
            assert_eq!(
                player::preparation(sim.world(), true).1,
                "To dawn: fire ready · food ready"
            );
            let saved = sim.save().unwrap();
            let mut restored = game(500, lite);
            restored.restore(&saved).unwrap();
            for sim in [&mut sim, &mut restored] {
                sim.run(cycle.until_dawn() as f64 * 1000.0);
                let fuel = sim.world().resource::<Fire>().fuel;
                assert!(
                    (10.0..10.3).contains(&fuel),
                    "lite={lite}, time={time}: fuel={fuel}"
                );
                assert_eq!(sim.world().resource::<Cycle>().survived, 1);
                assert!(!player(sim).dead && player(sim).health == 100.0);
            }
            assert!(sim.save().unwrap() == restored.save().unwrap());
        }
    }
}

#[test]
fn material_compasses_choose_the_requested_kind_and_restore_the_landmark() {
    let mut sim = game(500, true);
    for (kind, command) in [(Kind::Log, "track logs"), (Kind::Scrap, "track scrap")] {
        sim.post(command);
        sim.run(TICK);
        let trail = *sim.world().resource::<Trail>();
        let target = trail.target.unwrap();
        assert_eq!(sim.world().require::<Item>(target).kind, kind);
        let saved = sim.save().unwrap();
        let mut restored = game(500, true);
        restored.restore(&saved).unwrap();
        assert_eq!(*restored.world().resource::<Trail>(), trail);
        player::interact(sim.world_mut(), player::Action::Take(target, kind), false);
        player::update_trail(sim.world_mut(), true);
        let next = sim.world().resource::<Trail>().target.unwrap();
        assert_ne!(target, next);
        assert_eq!(sim.world().require::<Item>(next).kind, kind);
    }
    let logs: Vec<_> = sim
        .world()
        .query::<&Item>()
        .iter()
        .filter(|(_, i)| i.kind == Kind::Log && !i.carried)
        .map(|(e, _)| e)
        .collect();
    for e in logs {
        sim.world_mut().despawn(e);
    }
    sim.post("track logs");
    sim.run(TICK);
    let target = sim.world().resource::<Trail>().target.unwrap();
    assert!(
        sim.world().has::<forest_logic::forest::Tree>(target),
        "logs fall back to a standing tree"
    );
}
