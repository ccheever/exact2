//! Hostless fights: the training range, duels against the bot, determinism
//! and saves. Input arrives as the hosts deliver it: keys and pointer moves.
use exact_game::{InputEvent, PointerPhase, Sim};
use rivals_logic::fighter::Fighter;
use rivals_logic::round::Round;
use rivals_logic::{Options, Rivals};

fn game(options: Options) -> Sim<Rivals> {
    let mut sim = Sim::<Rivals>::new(Options { seed: 7, ..options }).unwrap();
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
    let mut sim = game(Options {
        bots: 1,
        skill: 1.0,
        ..Options::default()
    });
    // A passive player loses the round to the bot.
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
    assert_eq!(fighter(&sim, "bot-1").kills, 5);
    assert_eq!(round(&sim).winner, "bot-1");
    sim.run(4500.0);
    assert_eq!(round(&sim).number, 2);
    assert_eq!(fighter(&sim, "bot-1").kills, 0);
    assert_eq!(fighter(&sim, "bot-1").rounds, 1);
}
