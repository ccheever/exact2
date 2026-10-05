//! Hostless fights: the training range, duels against the bot, determinism
//! and saves. Input arrives as the hosts deliver it: keys and pointer moves.
use exact_game::{InputEvent, PointerPhase, Sim};
use rivals_logic::fighter::Fighter;
use rivals_logic::round::Round;
use rivals_logic::{Options, Rivals};

fn game(options: Options) -> Sim<Rivals> {
    let mut sim = Sim::<Rivals>::baked(Options { seed: 7, ..options });
    sim.viewport(1280.0, 720.0);
    sim
}
fn range() -> Sim<Rivals> {
    game(Options {
        bots: 3,
        range: true,
        ..Options::default()
    })
}
fn fighter(sim: &Sim<Rivals>, name: &str) -> Fighter {
    sim.world().require::<Fighter>(name).clone()
}
fn round(sim: &Sim<Rivals>) -> Round {
    sim.world().resource::<Round>().clone()
}

fn used_magazine(key: &str) -> Sim<Rivals> {
    let mut sim = range();
    sim.run(100.0);
    sim.tap(key);
    sim.run(300.0);
    sim.tap("KeyF");
    sim.run(300.0);
    sim
}

#[test]
fn a_second_reload_press_in_green_refills_either_weapon_early() {
    for key in ["Digit1", "Digit2"] {
        let mut sim = used_magazine(key);
        sim.tap("KeyR");
        sim.run(20.0);
        let until = fighter(&sim, "player").reload_until;
        let duration = fighter(&sim, "player").weapon.reload_time();
        sim.run(f64::from(duration) * 500.0);
        assert!(fighter(&sim, "player").quick_reload_ready(sim.world().seconds() as f32));
        sim.tap("KeyR");
        sim.run(20.0);
        let f = fighter(&sim, "player");
        assert_eq!(f.reload_until, 0.0);
        assert!(sim.world().seconds() < f64::from(until), "refill is early");
        assert!(f.quick_reload_until > sim.world().seconds() as f32);
        assert_eq!(f.rifle_ammo, 30);
        assert_eq!(f.rocket_ammo, 2);
        sim.run(1000.0);
        sim.tap("KeyF");
        sim.run(20.0);
        let f = fighter(&sim, "player");
        assert_eq!(
            if key == "Digit1" {
                f.rifle_ammo
            } else {
                f.rocket_ammo
            },
            if key == "Digit1" { 29 } else { 1 }
        );
    }
}

#[test]
fn mistimed_attempts_and_a_held_key_keep_the_normal_reload_deadline() {
    for fraction in [0.2, 0.8] {
        let mut sim = used_magazine("Digit1");
        sim.tap("KeyR");
        sim.run(20.0);
        let until = fighter(&sim, "player").reload_until;
        sim.run(1600.0 * fraction);
        sim.tap("KeyR");
        sim.run(20.0);
        assert!(fighter(&sim, "player").reload_missed);
        if fraction < 0.5 {
            sim.run(1600.0 * (0.5 - fraction));
        }
        sim.tap("KeyR");
        sim.run(20.0);
        let f = fighter(&sim, "player");
        assert_eq!(
            f.reload_until, until,
            "a second try cannot recover a missed window"
        );
        assert_eq!(f.rifle_ammo, 29);
        sim.run(1700.0);
        assert_eq!(fighter(&sim, "player").rifle_ammo, 30);
        assert_eq!(fighter(&sim, "player").quick_reload_until, 0.0);
    }
    let mut held = used_magazine("Digit1");
    held.key_down("KeyR");
    held.run(850.0);
    assert!(fighter(&held, "player").quick_reload_ready(held.world().seconds() as f32));
    assert_eq!(
        fighter(&held, "player").rifle_ammo,
        29,
        "holding is not a second press"
    );
    held.run(900.0);
    assert_eq!(fighter(&held, "player").rifle_ammo, 30);
    assert_eq!(fighter(&held, "player").quick_reload_until, 0.0);
}

#[test]
fn reload_window_and_a_missed_attempt_survive_restoration() {
    use exact_game::Paranoid;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        for miss in [false, true] {
            let mut sim = used_magazine("Digit1").paranoid(mode);
            sim.tap("KeyR");
            sim.run(200.0);
            if miss {
                sim.tap("KeyR");
            }
            sim.run(600.0);
            let saved = sim.save().unwrap();
            let mut back = range().paranoid(mode);
            back.restore_bound(&saved).unwrap();
            for s in [&mut sim, &mut back] {
                assert_eq!(
                    fighter(s, "player").quick_reload_ready(s.world().seconds() as f32),
                    !miss
                );
                s.tap("KeyR");
                s.run(100.0);
                assert_eq!(fighter(s, "player").rifle_ammo, if miss { 29 } else { 30 });
                s.run(1200.0);
            }
            assert_eq!(sim.save().unwrap(), back.save().unwrap());
        }
    }
}

#[test]
fn switching_and_respawn_clear_the_reload_attempt() {
    let mut sim = used_magazine("Digit1");
    sim.tap("KeyR");
    sim.run(200.0);
    sim.tap("KeyR");
    sim.run(20.0);
    assert!(fighter(&sim, "player").reload_missed);
    sim.tap("Digit3");
    sim.run(20.0);
    let f = fighter(&sim, "player");
    assert_eq!(f.reload_until, 0.0);
    assert!(!f.reload_missed);
    sim.tap("KeyR");
    sim.run(2000.0);
    assert_eq!(
        fighter(&sim, "player").reload_until,
        0.0,
        "a knife has no magazine"
    );
    sim.tap("Digit1");
    sim.run(20.0);
    assert_eq!(
        fighter(&sim, "player").rifle_ammo,
        29,
        "switching did not refill it"
    );
    sim.tap("KeyR");
    sim.run(200.0);
    sim.tap("KeyR");
    sim.run(20.0);
    let player = sim.world().resolve("player").unwrap();
    rivals_logic::fighter::place(sim.world_mut(), player, [0.0, 18.0]);
    let f = fighter(&sim, "player");
    assert!(!f.reload_missed);
    assert_eq!(f.reload_until, 0.0);
    assert_eq!(f.quick_reload_until, 0.0);
    sim.tap("KeyR");
    sim.run(100.0);
    assert_eq!(
        fighter(&sim, "player").reload_until,
        0.0,
        "a full magazine cannot start one"
    );
}

#[test]
fn a_bandage_requires_a_full_hold_saves_midway_and_is_spent_once() {
    use exact_game::Paranoid;
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = range().paranoid(mode);
        sim.run(100.0);
        sim.hold("KeyQ", 1700.0);
        assert!(!fighter(&sim, "player").bandage_used, "full health is free");
        sim.world().require_mut::<Fighter>("player").hp = 25.0;
        sim.hold("KeyQ", 600.0);
        sim.run(20.0);
        assert_eq!(fighter(&sim, "player").hp, 25.0);
        assert_eq!(fighter(&sim, "player").bandage_until, 0.0);
        assert!(!fighter(&sim, "player").bandage_used);
        sim.key_down("KeyQ");
        sim.run(700.0);
        assert!(fighter(&sim, "player").bandage_until > sim.world().seconds() as f32);
        let saved = sim.save().unwrap();
        let mut back = range().paranoid(mode);
        back.restore_bound(&saved).unwrap();
        for s in [&mut sim, &mut back] {
            s.run(700.0);
            assert_eq!(fighter(s, "player").hp, 25.0, "no early healing");
            s.run(200.0);
            assert_eq!(fighter(s, "player").hp, 65.0);
            assert!(fighter(s, "player").bandage_used);
            s.run(2000.0);
            assert_eq!(fighter(s, "player").hp, 65.0, "holding cannot reuse it");
            s.key_up("KeyQ");
            s.run(20.0);
        }
        assert_eq!(sim.save().unwrap(), back.save().unwrap());
        let player = sim.world().resolve("player").unwrap();
        rivals_logic::fighter::place(sim.world_mut(), player, [0.0, 18.0]);
        assert!(!fighter(&sim, "player").bandage_used, "respawn renews it");
        sim.world().require_mut::<Fighter>("player").hp = 90.0;
        sim.hold("KeyQ", 1700.0);
        assert_eq!(fighter(&sim, "player").hp, 100.0, "healing caps at full");
        assert!(fighter(&sim, "player").bandage_used);
        rivals_logic::round::next_round(sim.world_mut());
        assert!(!fighter(&sim, "player").bandage_used, "new round renews it");
    }
}

#[test]
fn damage_and_combat_interrupt_bandages_without_spending_them() {
    use exact_game::Vec3;
    use rivals_logic::weapons::{damage, Weapon};
    for cancel in ["KeyF", "KeyR", "Digit2", "Space", "ShiftLeft", "KeyC"] {
        let mut sim = range();
        sim.world().require_mut::<Fighter>("player").hp = 30.0;
        sim.key_down("KeyQ");
        sim.run(1000.0);
        if cancel == "ShiftLeft" {
            sim.key_down(cancel);
        } else {
            sim.tap(cancel);
        }
        sim.run(20.0);
        if cancel == "ShiftLeft" {
            sim.key_up(cancel);
        }
        assert_eq!(fighter(&sim, "player").hp, 30.0, "{cancel}");
        let until = fighter(&sim, "player").bandage_until;
        assert!(
            until == 0.0 || until > sim.world().seconds() as f32 + 1.4,
            "{cancel}: {until}"
        );
        assert!(!fighter(&sim, "player").bandage_used, "{cancel}");
    }
    let mut sim = range();
    let player = sim.world().resolve("player").unwrap();
    sim.world().require_mut::<Fighter>(player).hp = 60.0;
    sim.key_down("KeyQ");
    sim.run(500.0);
    // A hit at the completion boundary must win over the delayed heal.
    sim.world().require_mut::<Fighter>(player).bandage_until = sim.world().seconds() as f32;
    let hit = damage(sim.world(), 2, player, 10.0, false, Weapon::Rifle, Vec3::Z).unwrap();
    rivals_logic::round::score(sim.world_mut(), vec![hit]);
    rivals_logic::fighter::bandages(sim.world());
    assert_eq!(fighter(&sim, "player").hp, 50.0);
    assert_eq!(fighter(&sim, "player").bandage_until, 0.0);
    assert!(!fighter(&sim, "player").bandage_used);
    sim.run(1700.0);
    assert_eq!(
        fighter(&sim, "player").hp,
        90.0,
        "held input starts a new attempt"
    );
    let hit = damage(sim.world(), 2, player, 100.0, false, Weapon::Rifle, Vec3::Z).unwrap();
    rivals_logic::round::score(sim.world_mut(), vec![hit]);
    sim.run(1000.0);
    assert_eq!(fighter(&sim, "player").hp, 0.0, "dead fighters cannot heal");
}

#[test]
fn bandaging_slows_movement_and_a_hurt_bot_uses_the_same_action_in_cover() {
    use exact_game::{Transform, Vec3};
    use rivals_logic::{
        bots::{Brain, Plan},
        fighter::Intent,
    };
    let mut sim = range();
    sim.world().require_mut::<Fighter>("player").hp = 20.0;
    sim.key_down("KeyQ");
    sim.hold("KeyD", 700.0);
    let f = fighter(&sim, "player");
    assert!(
        f.bandage_until > 0.0 && f.planar.length() <= rivals_logic::fighter::WALK * 0.35 + 0.001
    );
    assert_eq!(f.shots, 0);
    let bot = sim.world().resolve("bot-1").unwrap();
    let at = sim.world().require::<Transform>(bot).position;
    sim.world().require_mut::<Fighter>(bot).hp = 30.0;
    *sim.world().require_mut::<Brain>(bot) = Brain {
        plan: Plan::Cover,
        goal: at,
        cover_until: 10.0,
        ..Brain::default()
    };
    // No enemies in this local observation: the bot has reached safe cover.
    let seen = rivals_logic::bots::snapshot(sim.world())
        .into_iter()
        .filter(|s| s.entity == bot)
        .collect::<Vec<_>>();
    let intent = rivals_logic::bots::think(sim.world(), bot, &seen, &[]);
    assert!(intent.bandage);
    assert!(!intent.fire && !intent.reload && !intent.sprint && intent.switch.is_none());
    assert_eq!(intent.stick, Vec3::ZERO);
    rivals_logic::fighter::step(sim.world_mut(), bot, &intent);
    assert!(fighter(&sim, "bot-1").bandage_until > 0.0);
    rivals_logic::fighter::step(
        sim.world_mut(),
        bot,
        &Intent {
            fire: true,
            ..Intent::default()
        },
    );
    assert_eq!(fighter(&sim, "bot-1").bandage_until, 0.0);
}
/// A mouse event carrying the device's own motion (a locked pointer: the
/// position stays put), stamped at `at_ms`.
fn mouse(sim: &mut Sim<Rivals>, at_ms: f64, phase: PointerPhase, dx: f32, dy: f32, buttons: u32) {
    sim.input(InputEvent::Pointer {
        id: 1,
        phase,
        x: 640.0,
        y: 360.0,
        dx,
        dy,
        buttons,
        at_ms,
    });
}

#[test]
fn range_headshot_and_body_shot() {
    let mut sim = range();
    sim.run(500.0);
    // The dummies' heads sit at the player's eye height: level aim is a headshot.
    sim.tap("KeyF");
    sim.run(50.0);
    let r = round(&sim);
    assert_eq!(r.log.len(), 1, "{:?}", r.log);
    assert!(r.log[0].head, "{:?}", r.log[0]);
    assert_eq!(r.log[0].victim, 3);
    assert_eq!(fighter(&sim, "bot-2").hp, 64.0);
    // Look down a little with the arrow keys: a body shot.
    sim.hold("ArrowDown", 80.0);
    sim.run(300.0);
    sim.tap("KeyF");
    sim.run(50.0);
    let r = round(&sim);
    assert_eq!(r.log.len(), 2);
    assert!(!r.log[1].head, "{:?}", r.log[1]);
    assert_eq!(fighter(&sim, "bot-2").hp, 44.0);
}

#[test]
fn range_dummy_returns_to_its_lane_after_elimination() {
    let mut sim = range();
    sim.run(500.0);
    let start = sim
        .world()
        .require::<exact_game::Transform>("bot-2")
        .position;
    for _ in 0..3 {
        sim.tap("KeyF");
        sim.run(300.0);
    }
    assert!(!fighter(&sim, "bot-2").alive);
    sim.run(2100.0);
    assert!(fighter(&sim, "bot-2").alive);
    let after = sim
        .world()
        .require::<exact_game::Transform>("bot-2")
        .position;
    assert!(
        (after - start).length() < 0.05,
        "dummy left its training lane: {start} -> {after}"
    );
}

fn shoot_at(sim: &mut Sim<Rivals>, name: &str) {
    let me = fighter(sim, "player");
    let from = sim
        .world()
        .require::<exact_game::Transform>("player")
        .position
        + exact_game::Vec3::Y * me.eye;
    let to =
        sim.world().require::<exact_game::Transform>(name).position + exact_game::Vec3::Y * 0.66;
    let d = to - from;
    let yaw = exact_game::math::atan2(-d.x, -d.z);
    let pitch = exact_game::math::atan2(d.y, exact_game::math::sqrt(d.x * d.x + d.z * d.z));
    sim.input_now(InputEvent::Pointer {
        id: 1,
        phase: PointerPhase::Move,
        x: 640.0,
        y: 360.0,
        dx: -(yaw - me.yaw) / rivals_logic::DEFAULT_SENSITIVITY,
        dy: -(pitch - me.pitch) / rivals_logic::DEFAULT_SENSITIVITY,
        buttons: 0,
        at_ms: 0.0,
    });
    sim.tap("KeyF");
    sim.run(300.0);
}

fn clear_target(sim: &mut Sim<Rivals>) {
    let slot = sim
        .world()
        .resource::<rivals_logic::training::Drill>()
        .target;
    let name = format!("bot-{}", slot - 1);
    if !fighter(sim, &name).alive {
        sim.run(2100.0);
    }
    for _ in 0..5 {
        if !fighter(sim, &name).alive {
            break;
        }
        shoot_at(sim, &name);
    }
    assert!(!fighter(sim, &name).alive, "failed to eliminate {name}");
}

#[test]
fn drill_chains_targets_breaks_on_wrong_hits_and_continues_from_save() {
    use rivals_logic::training::Drill;
    let mut sim = range();
    sim.run(500.0);
    clear_target(&mut sim);
    assert_eq!(sim.world().resource::<Drill>().score, 150);
    assert_eq!(sim.world().resource::<Drill>().target, 4);
    shoot_at(&mut sim, "bot-1");
    assert_eq!(sim.world().resource::<Drill>().combo, 0);
    clear_target(&mut sim);
    let saved = sim.save().unwrap();
    let mut back = range();
    back.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut back] {
        for _ in 0..4 {
            clear_target(s);
        }
        assert_eq!(s.world().resource::<Drill>().cleared, 6);
        assert_eq!(
            round(s).number,
            1,
            "range does not enter a duel round at five kills"
        );
        s.run(30_100.0 - s.world().seconds() * 1000.0);
        let score = s.world().resource::<Drill>().score;
        assert_eq!(
            s.world().published("drill_done").unwrap().as_bool(),
            Some(true)
        );
        s.hold("KeyF", 3000.0);
        assert_eq!(s.world().resource::<Drill>().score, score);
    }
    assert_eq!(sim.save().unwrap(), back.save().unwrap());
}

#[test]
fn opponent_labels_hide_dead_offscreen_and_occluded_heads() {
    use exact_game::{Transform, Vec2};
    let mut sim = range();
    sim.run(500.0);
    let size = Vec2::new(1280.0, 720.0);
    let contacts = rivals_logic::contacts(sim.world(), size, Some(3));
    assert_eq!(contacts.len(), 3);
    assert!(contacts
        .iter()
        .any(|c| c.id == 3 && c.target && c.x == 640.0));
    let bot = sim.world().named("bot-2").unwrap();
    for z in [-18.0, 20.0] {
        sim.world_mut().teleport(bot, Transform::at(0.0, 0.92, z));
        assert!(!rivals_logic::contacts(sim.world(), size, Some(3))
            .iter()
            .any(|c| c.id == 3));
    }
    sim.world_mut()
        .teleport(bot, Transform::at(0.0, 0.92, 12.0));
    sim.world_mut().require_mut::<Fighter>(bot).alive = false;
    assert!(!rivals_logic::contacts(sim.world(), size, Some(3))
        .iter()
        .any(|c| c.id == 3));
}

#[test]
fn crowded_nameplates_stay_readable_without_moving_their_aim_points() {
    use exact_game::{Transform, Vec2, Vec3};
    let mut sim = game(Options {
        bots: 24,
        ..Options::default()
    });
    sim.run(3600.0);
    let size = Vec2::new(1280.0, 720.0);
    let contacts = rivals_logic::contacts(sim.world(), size, None);
    assert!(
        contacts.len() >= 3,
        "crowding must not hide the whole fight"
    );
    for (i, c) in contacts.iter().enumerate() {
        let head = sim.world().require::<Transform>(&c.label).position + Vec3::Y * 0.66;
        let anchor = sim.world().project(head, size).unwrap();
        assert_eq!((c.x, c.y), (anchor.x.round(), anchor.y.round()));
        assert!(c.x >= 68.0 && c.x <= size.x - 68.0 && c.y >= 70.0);
        for other in &contacts[i + 1..] {
            assert!(
                (c.x - other.x).abs() >= 134.0 || (c.y - other.y).abs() >= 54.0,
                "overlapping plates: {} and {}",
                c.label,
                other.label
            );
        }
    }
    let mut drill = range();
    drill.run(500.0);
    // Short screens pack the three lanes together. The requested target wins
    // that space even if another head is nearer to the crosshair.
    let small = Vec2::new(640.0, 360.0);
    let target = rivals_logic::contacts(drill.world(), small, Some(4));
    assert_eq!(target.iter().map(|c| c.id).collect::<Vec<_>>(), [2, 4]);
    let aimed = rivals_logic::contacts(drill.world(), small, None);
    assert_eq!(aimed.len(), 1);
    assert_eq!(aimed[0].id, 3);
    drill.hold("ArrowRight", 100.0);
    let turned = rivals_logic::contacts(drill.world(), small, None);
    assert!(
        turned.iter().any(|c| c.id == 4),
        "turning reveals the newly aimed-at fighter"
    );
    assert!(!turned.iter().any(|c| c.id == 3));
}

#[test]
fn incoming_hit_keeps_its_origin_turns_with_the_player_and_expires_after_restore() {
    use exact_game::{Transform, Vec2, Vec3};
    use rivals_logic::weapons::{damage, Weapon};
    let mut sim = range();
    sim.run(500.0);
    let player = sim.world().named("player").unwrap();
    let origin = sim.world().require::<Transform>(player).position + Vec3::X * 10.0;
    let hit = damage(sim.world(), 2, player, 5.0, false, Weapon::Rifle, origin).unwrap();
    rivals_logic::round::score(sim.world_mut(), vec![hit]);
    sim.run(10.0);
    assert_eq!(
        sim.world().published("incoming").unwrap().as_str(),
        Some("Hit from right")
    );
    assert_eq!(
        sim.world().published("incoming_angle").unwrap().as_number(),
        Some(90.0)
    );
    // The enemy moving does not move the warning; it describes the hit received.
    let bot = sim.world().named("bot-1").unwrap();
    sim.world_mut()
        .teleport(bot, Transform::at(-12.0, 0.92, 12.0));
    let saved = sim.save().unwrap();
    let mut back = range();
    back.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut back] {
        s.hold("ArrowRight", std::f64::consts::FRAC_PI_2 / 2.4 * 1000.0);
        assert_eq!(
            s.world().published("heading").unwrap().as_str(),
            Some("E · 90°")
        );
        assert_eq!(
            s.world().published("incoming").unwrap().as_str(),
            Some("Hit from front")
        );
        assert_eq!(round(s).incoming.unwrap().origin, origin);
        s.world_mut().require_mut::<Fighter>(player).alive = false;
        rivals_logic::publish(
            s.world(),
            &Options {
                range: true,
                bots: 3,
                ..Options::default()
            },
            Vec2::new(1280.0, 720.0),
        );
        assert_eq!(s.world().published("incoming").unwrap().as_str(), Some(""));
        s.world_mut().require_mut::<Fighter>(player).alive = true;
        s.run(2100.0);
        assert_eq!(s.world().published("incoming").unwrap().as_str(), Some(""));
        assert!(round(s).incoming.is_none());
    }
    assert_eq!(sim.save().unwrap(), back.save().unwrap());
}

#[test]
fn incoming_direction_covers_every_quadrant_and_rocket_blasts_use_the_blast_origin() {
    use exact_game::{Transform, Vec3};
    use rivals_logic::round::{heading, Incoming};
    for (point, name, angle) in [
        (Vec3::NEG_Z, "front", 0.0_f32),
        (Vec3::new(1.0, 0.0, -1.0), "front-right", 45.0),
        (Vec3::X, "right", 90.0),
        (Vec3::new(1.0, 0.0, 1.0), "back-right", 135.0),
        (Vec3::Z, "back", 180.0),
        (Vec3::new(-1.0, 0.0, 1.0), "back-left", -135.0),
        (Vec3::NEG_X, "left", -90.0),
        (Vec3::new(-1.0, 0.0, -1.0), "front-left", -45.0),
    ] {
        let result = Incoming {
            origin: point,
            until: 2.0,
        }
        .bearing(Vec3::ZERO, 0.0);
        assert_eq!(result.1, name);
        assert!((result.0 - angle).abs() < 0.01 || (angle == 180.0 && result.0 == -180.0));
    }
    assert_eq!(heading(std::f32::consts::FRAC_PI_2), "W · 270°");
    assert_eq!(heading(std::f32::consts::PI), "S · 180°");
    let mut sim = range();
    sim.run(500.0);
    let origin = sim.world().require::<Transform>("player").position + Vec3::X;
    let hits = rivals_logic::weapons::explode(sim.world_mut(), 2, origin, None);
    let hit = hits.iter().find(|h| h.victim == 1).unwrap();
    assert_eq!(hit.origin, origin);
    rivals_logic::round::score(sim.world_mut(), hits);
    assert_eq!(round(&sim).incoming.unwrap().origin, origin);
}

#[test]
fn mouse_look_turns_by_sensitivity_from_the_first_move() {
    let mut sim = range();
    sim.run(100.0);
    let before = fighter(&sim, "player").yaw;
    // The very first move turns: motion is the device's, not a position change.
    mouse(&mut sim, 100.0, PointerPhase::Move, 100.0, 0.0, 0);
    sim.run(50.0);
    let turned = before - fighter(&sim, "player").yaw;
    assert!(
        (turned - 100.0 * rivals_logic::DEFAULT_SENSITIVITY).abs() < 1e-5,
        "{turned}"
    );
    let camera = sim.world().named("camera").unwrap();
    assert!(sim.world().has::<exact_game::MouseLook>(camera));
}

#[test]
fn right_button_aims_down_sights() {
    let mut sim = range();
    sim.run(100.0);
    mouse(&mut sim, 100.0, PointerPhase::Down, 0.0, 0.0, 2);
    sim.run(50.0);
    assert!(fighter(&sim, "player").aiming);
    let fov = sim
        .world()
        .require::<exact_game::Camera>("camera")
        .fov_y_degrees;
    assert_eq!(fov, 52.0);
    mouse(&mut sim, 150.0, PointerPhase::Up, 0.0, 0.0, 0);
    sim.run(50.0);
    assert!(!fighter(&sim, "player").aiming);
    // Aimed rifle fire is a tighter cone; fire is the left button too.
    mouse(&mut sim, 200.0, PointerPhase::Down, 0.0, 0.0, 1);
    sim.run(50.0);
    mouse(&mut sim, 250.0, PointerPhase::Up, 0.0, 0.0, 0);
    sim.run(50.0);
    assert_eq!(fighter(&sim, "player").shots, 1);
}

#[test]
fn rocket_splash_and_knockback() {
    let mut sim = range();
    sim.run(300.0);
    sim.tap("Digit2");
    sim.run(400.0);
    let before = sim
        .world()
        .require::<exact_game::Transform>("bot-2")
        .position;
    sim.tap("KeyF");
    sim.run(600.0);
    let r = round(&sim);
    assert!(r.log.iter().any(|d| d.victim == 3), "{:?}", r.log);
    // Neighbours four metres away take splash too.
    assert!(
        r.log
            .iter()
            .filter(|d| d.weapon == rivals_logic::weapons::Weapon::Rocket)
            .count()
            >= 2,
        "{:?}",
        r.log
    );
    let after = sim
        .world()
        .require::<exact_game::Transform>("bot-2")
        .position;
    assert!(
        (after - before).length() > 0.5 || !fighter(&sim, "bot-2").alive,
        "{before} {after}"
    );
}

#[test]
fn duel_bot_fights_back() {
    let mut sim = game(Options {
        bots: 1,
        ..Options::default()
    });
    // Stand still for 20 s: the bot should find and kill the player.
    sim.run(20_000.0);
    let player = fighter(&sim, "player");
    assert!(player.deaths >= 1, "bot never killed a passive player");
}

#[test]
fn hunting_bot_escapes_the_side_of_the_central_ramp() {
    use exact_game::Vec3;
    use rivals_logic::{bots::Brain, fighter};
    let mut sim = game(Options {
        bots: 1,
        ..Options::default()
    });
    let player = sim.world().named("player").unwrap();
    let bot = sim.world().named("bot-1").unwrap();
    // The final positions and last sighting from jev-pointer-macos. A fresh
    // encounter isolates the ramp obstruction from score, recoil and respawns.
    fighter::place(sim.world_mut(), player, [-21.639706, 19.99394]);
    fighter::place(sim.world_mut(), bot, [1.6498584, 3.8703682]);
    let start = sim.world().require::<exact_game::Transform>(bot).position;
    {
        let mut brain = sim.world_mut().require_mut::<Brain>(bot);
        brain.target = Some(1);
        brain.last_seen = Vec3::new(-21.639965, 0.91, 8.313441);
        brain.goal = brain.last_seen;
        brain.wander_until = 30.0;
    }
    sim.run(50.0);
    assert!(sim.world().require::<Brain>(bot).detour_until > 0.05);
    let saved = sim.save().unwrap();
    let mut restored = game(Options {
        bots: 1,
        ..Options::default()
    });
    restored.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut restored] {
        s.run(950.0);
        let at = s.world().require::<exact_game::Transform>(bot).position;
        assert!((at - start).length() > 3.0, "bot stayed at the ramp: {at}");
        assert!(
            s.world().require::<Brain>(bot).visible,
            "bot did not reacquire the player"
        );
        s.run(2000.0);
        assert!(
            fighter(s, "bot-1").shots > 0,
            "bot never fired after escaping"
        );
        assert!(
            fighter(s, "player").hp < 100.0 || fighter(s, "player").deaths > 0,
            "escaped bot never hit the player"
        );
    }
    assert_eq!(sim.save().unwrap(), restored.save().unwrap());
}

#[test]
fn hunting_bot_commits_to_a_new_destination_after_searching_a_sighting() {
    use exact_game::{Transform, Vec3};
    use rivals_logic::{bots::Brain, fighter};
    // Arrival and timeout both retire the remembered sighting. Central cover
    // hides the live player, so the bot must choose another search destination.
    for expired in [false, true] {
        let mut sim = game(Options {
            bots: 1,
            ..Options::default()
        });
        let bot = sim.world().named("bot-1").unwrap();
        fighter::place(sim.world_mut(), bot, [0.0, -3.3]);
        {
            let mut brain = sim.world_mut().require_mut::<Brain>(bot);
            brain.target = Some(1);
            brain.last_seen = Vec3::new(0.0, 0.92, if expired { 10.0 } else { -3.3 });
            brain.goal = brain.last_seen;
            brain.wander_until = if expired { 0.0 } else { 30.0 };
        }
        sim.run(10.0);
        let goal = sim.world().require::<Brain>(bot).goal;
        let at = sim.world().require::<Transform>(bot).position;
        assert!((goal - at).length() > 3.0);
        assert_eq!(sim.world().require::<Brain>(bot).target, None);
        sim.run(100.0);
        assert_eq!(
            sim.world().require::<Brain>(bot).goal,
            goal,
            "the chosen search destination was overwritten"
        );
        assert_eq!(sim.world().require::<Brain>(bot).target, None);
    }
}

#[test]
fn a_full_arena_starts_and_restarts_without_overlapping_fighters() {
    let mut sim = game(Options {
        bots: 24,
        ..Options::default()
    });
    for &[x, z] in rivals_logic::arena::SPAWNS {
        let hits = exact_game_physics::overlap(
            sim.world(),
            &exact_game_physics::Shape::Capsule {
                radius: rivals_logic::fighter::RADIUS,
                height: rivals_logic::fighter::HEIGHT,
            },
            exact_game::Transform::at(x, 0.92, z),
            rivals_logic::arena::WORLD,
        );
        assert!(
            hits.is_empty(),
            "spawn [{x}, {z}] overlaps arena geometry: {hits:?}"
        );
    }
    assert_separate_fighters(&sim);
    rivals_logic::round::next_round(sim.world_mut());
    assert_separate_fighters(&sim);
}

#[test]
fn simultaneous_respawns_reserve_different_clear_spawns() {
    let mut sim = game(Options {
        bots: 24,
        ..Options::default()
    });
    // Everyone is due at the same tick. Each placement must become visible
    // when choosing the next one, including the first when nobody is alive.
    for (_, f) in sim.world_mut().query::<&mut Fighter>().iter() {
        f.alive = false;
        f.respawn_at = 0.0;
    }
    let saved = sim.save().unwrap();
    let mut restored = game(Options {
        bots: 24,
        ..Options::default()
    });
    restored.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut restored] {
        rivals_logic::round::respawn(s.world_mut());
        assert!(s.world().query::<&Fighter>().iter().all(|(_, f)| f.alive));
        assert_separate_fighters(s);
        s.run(100.0);
    }
    assert_eq!(sim.save().unwrap(), restored.save().unwrap());
}

fn assert_separate_fighters(sim: &Sim<Rivals>) {
    let fighters: Vec<_> = sim
        .world()
        .query::<(&Fighter, &exact_game::Transform)>()
        .iter()
        .map(|(_, (f, t))| (f.label.clone(), t.position))
        .collect();
    for (i, (name, at)) in fighters.iter().enumerate() {
        for (other, pos) in &fighters[i + 1..] {
            assert!(
                (*pos - *at).length() > 2.0 * rivals_logic::fighter::RADIUS,
                "{name} at {at} overlaps {other} at {pos}"
            );
        }
    }
}

/// A scripted, aggressive player: strafes, jumps, slides, sprays and flicks the
/// mouse. Every input is a key edge or an absolute pointer move at a stamp.
fn scripted(sim: &mut Sim<Rivals>, seconds: u32) {
    for step in 0..seconds * 10 {
        let at = 100.0 * (step + 1) as f64;
        sim.run(100.0);
        let key = ["KeyA", "KeyD", "KeyW"][(step / 7 % 3) as usize];
        sim.key_down(key);
        if step % 7 == 6 {
            sim.key_up(key);
        }
        if step % 13 == 0 {
            sim.tap("Space");
        }
        if step % 17 == 5 {
            sim.key_down("ShiftLeft");
            sim.tap("KeyC");
            sim.key_up("ShiftLeft");
        }
        if step % 4 == 0 {
            sim.key_down("KeyF");
        } else if step % 4 == 2 {
            sim.key_up("KeyF");
        }
        if step % 50 == 25 {
            sim.tap("Digit2");
        } else if step % 50 == 30 {
            sim.tap("Digit1");
        }
        let dx = if step % 20 < 10 { 37.0 } else { -41.0 };
        mouse(sim, at, PointerPhase::Move, dx, 0.0, 0);
    }
}

#[test]
fn replay_gives_the_same_kills() {
    let run = || {
        let mut sim = game(Options {
            bots: 3,
            ..Options::default()
        });
        scripted(&mut sim, 30);
        let kills: Vec<_> = round(&sim)
            .log
            .iter()
            .filter(|d| d.killed)
            .map(|d| (d.attacker, d.victim))
            .collect();
        // Every fighter's totals: they survive the round reset that clears the log.
        let totals: Vec<_> = ["player", "bot-1", "bot-2", "bot-3"]
            .map(|n| {
                let f = fighter(&sim, n);
                (f.shots, f.kills, f.deaths, f.rounds, f.hp.to_bits())
            })
            .to_vec();
        (sim.world().hash(), kills, totals)
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    let shots: u32 = a.2.iter().map(|t| t.0).sum();
    let deaths: u32 = a.2.iter().map(|t| t.2).sum();
    println!(
        "30 s scripted fight: {shots} shots, {deaths} deaths, kills {:?}",
        a.1
    );
    assert!(
        shots > 100 && deaths >= 2,
        "the fight should be a fight: {a:?}"
    );
}

#[test]
fn save_mid_fight_continues_identically() {
    let mut sim = game(Options {
        bots: 3,
        ..Options::default()
    });
    scripted(&mut sim, 12);
    let rockets_or_hits = round(&sim).log.len();
    let saved = sim.save().unwrap();
    let mut copy = game(Options {
        bots: 3,
        ..Options::default()
    });
    copy.restore(&saved).unwrap();
    assert_eq!(copy.world().hash(), sim.world().hash());
    for s in [&mut sim, &mut copy] {
        s.key_down("KeyF");
        s.key_down("KeyW");
        s.run(3000.0);
    }
    assert_eq!(copy.world().hash(), sim.world().hash());
    assert_eq!(copy.save().unwrap(), sim.save().unwrap());
    assert!(round(&sim).log.len() >= rockets_or_hits);
}

#[test]
fn rounds_end_at_five_kills_and_restart() {
    for bots in [1, 7] {
        let mut sim = game(Options {
            bots,
            skill: 1.0,
            ..Options::default()
        });
        // A passive player loses the round to a bot in either smaller mode.
        let mut over = false;
        for _ in 0..240 {
            sim.run(1000.0);
            if sim
                .world()
                .published("over")
                .is_some_and(|v| v.as_bool() == Some(true))
                || round(&sim).over_until > 0.0
            {
                over = true;
                break;
            }
        }
        assert!(
            over,
            "round never ended: {:?}",
            fighter(&sim, "bot-1").kills
        );
        let winner = round(&sim).winner;
        assert!(winner.starts_with("bot-"));
        assert_eq!(fighter(&sim, &winner).kills, 5);
        assert_eq!(
            sim.world().published("to_win").unwrap().as_number(),
            Some(5.0)
        );
        sim.run(4500.0);
        assert_eq!(round(&sim).number, 2);
        assert!(fighter(&sim, &winner).kills < 5);
        assert_eq!(fighter(&sim, &winner).rounds, 1);
    }
}

#[test]
fn mayhem_continues_past_five_and_restores_through_the_round_finish() {
    let mut sim = game(Options {
        bots: 24,
        ..Options::default()
    });
    let leader = |sim: &Sim<Rivals>| {
        sim.world()
            .query::<&Fighter>()
            .iter()
            .map(|(_, f)| f.kills)
            .max()
            .unwrap()
    };
    while leader(&sim) < 5 && sim.world().seconds() < 60.0 {
        sim.run(100.0);
    }
    assert!(
        leader(&sim) >= 5,
        "the crowded fight never scored five kills"
    );
    assert_eq!(
        round(&sim).over_until,
        0.0,
        "Mayhem ended with a duel's score"
    );
    assert_eq!(
        sim.world().published("to_win").unwrap().as_number(),
        Some(25.0)
    );
    let five_at = sim.world().seconds();
    let saved = sim.save().unwrap();
    let mut back = game(Options {
        bots: 24,
        ..Options::default()
    });
    back.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut back] {
        while round(s).over_until == 0.0 && s.world().seconds() < 180.0 {
            s.run(100.0);
        }
        assert!(
            !round(s).winner.is_empty(),
            "Mayhem never reached its target"
        );
        let winner = round(s).winner;
        assert_eq!(fighter(s, &winner).kills, 25);
        assert_eq!(fighter(s, &winner).rounds, 1);
        println!(
            "Mayhem: five kills at {five_at:.1} s, winner {winner} at {:.1} s",
            s.world().seconds()
        );
        s.run(4200.0);
        assert_eq!(round(s).number, 2);
        assert!(round(s).winner.is_empty());
        assert_eq!(
            s.world().published("to_win").unwrap().as_number(),
            Some(25.0)
        );
        assert_eq!(fighter(s, &winner).rounds, 1);
    }
    assert!(sim.save().unwrap() == back.save().unwrap());
}

#[test]
fn shot_feedback_restores_mid_flash_without_repeating_the_shot() {
    use exact_game::{audio::Voices, Paranoid, Visible};
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = range().paranoid(mode);
        sim.run(400.0);
        sim.tap("KeyF");
        sim.run(9.0);
        assert_eq!(fighter(&sim, "player").rifle_ammo, 29);
        assert!(sim.world().require::<Visible>("vm-rifle-flash-core").0);
        let voices = sim.world().resource::<Voices>();
        assert!(voices.voices.iter().any(|v| v.sound == "rifle-shot"));
        assert!(voices.voices.iter().any(|v| v.sound == "head-hit"));
        let next_voice = voices.next_id;
        drop(voices);
        let saved = sim.save().unwrap();
        let mut back = range().paranoid(mode);
        back.restore_bound(&saved).unwrap();
        assert_eq!(saved, back.save().unwrap());
        for s in [&mut sim, &mut back] {
            s.run(400.0);
            assert_eq!(fighter(s, "player").rifle_ammo, 29);
            assert!(!s.world().require::<Visible>("vm-rifle-flash-core").0);
            assert!(s.world().resource::<Voices>().voices.is_empty());
            assert_eq!(s.world().resource::<Voices>().next_id, next_voice);
        }
        assert_eq!(sim.save().unwrap(), back.save().unwrap());
    }
}

#[test]
fn magazine_pose_and_reload_sounds_follow_completion_and_cancellation() {
    use exact_game::{audio::Voices, Transform};
    let mut sim = used_magazine("Digit1");
    let rest = *sim.world().require::<Transform>("vm-rifle-mag");
    sim.tap("KeyR");
    sim.run(9.0);
    assert!(sim
        .world()
        .resource::<Voices>()
        .voices
        .iter()
        .any(|v| v.sound == "reload-out"));
    sim.run(790.0);
    assert!(sim.world().require::<Transform>("vm-rifle-mag").position.y < rest.position.y - 0.1);
    let saved = sim.save().unwrap();
    let mut back = range();
    back.restore_bound(&saved).unwrap();
    for s in [&mut sim, &mut back] {
        s.tap("KeyR");
        s.run(9.0);
        assert_eq!(fighter(s, "player").rifle_ammo, 30);
        assert!(s
            .world()
            .resource::<Voices>()
            .voices
            .iter()
            .any(|v| v.sound == "reload-in"));
        s.run(400.0);
    }
    assert_eq!(sim.save().unwrap(), back.save().unwrap());
    sim.tap("KeyF");
    sim.run(300.0);
    sim.tap("KeyR");
    sim.run(200.0);
    sim.tap("Digit3");
    sim.run(9.0);
    assert!(!sim
        .world()
        .resource::<Voices>()
        .voices
        .iter()
        .any(|v| v.sound == "reload-in"));
}

#[test]
fn ending_the_drill_still_expires_last_shot_feedback() {
    use exact_game::{audio::Voices, Visible};
    let mut sim = range();
    sim.run(29_980.0);
    sim.tap("KeyF");
    sim.run(9.0);
    assert!(!sim.world().resource::<Voices>().voices.is_empty());
    sim.run(400.0);
    assert!(sim.world().resource::<Voices>().voices.is_empty());
    assert!(!sim.world().require::<Visible>("vm-rifle-flash-core").0);
}
