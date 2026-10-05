//! The art pass is a look, not a game: the same inputs give the same fight in
//! every look, the pass saves and restores, and what it draws is derived.
use exact_game::{InputEvent, MaterialOverrides, Offset, Opacity, PointerPhase, Sim, Transform};
use rivals_logic::art::{Held, Limb, Part, Soldier};
use rivals_logic::fighter::Fighter;
use rivals_logic::round::Round;
use rivals_logic::weapons::{Rocket, Weapon};
use rivals_logic::{Options, Rivals};

fn game(bots: u32, art: &str) -> Sim<Rivals> {
    let mut sim = Sim::<Rivals>::new(Options {
        seed: 7,
        bots,
        art: art.into(),
        ..Options::default()
    })
    .unwrap();
    sim.viewport(1280.0, 720.0);
    sim
}
/// The baked models and textures, for saves (which wait for what is shown).
fn loaded(mut sim: Sim<Rivals>) -> Sim<Rivals> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/");
    sim.load_assets(|name| std::fs::read(format!("{dir}{name}")))
        .expect("bake the art first: bun game/app/shells.mjs game/games/rivals --test");
    sim
}

fn mouse(sim: &mut Sim<Rivals>, at_ms: f64, dx: f32) {
    sim.input(InputEvent::Pointer {
        id: 1,
        phase: PointerPhase::Move,
        x: 640.0,
        y: 360.0,
        dx,
        dy: 0.0,
        buttons: 0,
        at_ms,
    });
}
/// An aggressive scripted player: strafes, jumps, slides, sprays, fires
/// rockets, stabs and flicks the mouse.
fn scripted(sim: &mut Sim<Rivals>, seconds: u32) {
    for step in 0..seconds * 10 {
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
        match step % 60 {
            20 => sim.tap("Digit2"),
            32 => sim.tap("Digit3"),
            40 => sim.tap("Digit1"),
            45 => sim.tap("KeyT"),
            _ => {}
        }
        let dx = if step % 20 < 10 { 37.0 } else { -41.0 };
        mouse(sim, 100.0 * f64::from(step + 1), dx);
    }
}

/// Everything the fight is made of, without entity indices (the pass's scenery
/// shifts them): every fighter and its pose, the rockets in flight, the round.
fn fight(sim: &Sim<Rivals>) -> String {
    let w = sim.world();
    let fighters: Vec<String> = w
        .query::<(&Fighter, &Transform)>()
        .iter()
        .map(|(_, (f, t))| format!("{f:?} {:?}", t.position))
        .collect();
    let rockets: Vec<String> = w
        .query::<(&Rocket, &Transform)>()
        .iter()
        .map(|(_, (r, t))| format!("{r:?} {:?}", t.position))
        .collect();
    format!("{fighters:#?}\n{rockets:#?}\n{:?}", *w.resource::<Round>())
}

#[test]
fn every_look_fights_the_same_fight() {
    for bots in [1, 7] {
        let mut classic = game(bots, "");
        let mut dusk = game(bots, "pass");
        let mut night = game(bots, "night");
        for second in 0..24 {
            for sim in [&mut classic, &mut dusk, &mut night] {
                scripted(sim, 1);
            }
            let expected = fight(&classic);
            assert_eq!(fight(&dusk), expected, "{bots} bots, second {second}: pass");
            assert_eq!(
                fight(&night),
                expected,
                "{bots} bots, second {second}: night"
            );
        }
        let r = classic.world().resource::<Round>().clone();
        assert!(
            r.log.len() + r.feed.len() > 3 || r.number > 1,
            "the script should fight: {r:?}"
        );
    }
}

#[test]
fn an_unknown_look_is_refused() {
    let refused = Sim::<Rivals>::new(Options {
        art: "neon".into(),
        ..Options::default()
    });
    assert!(refused.is_err_and(|e| e.contains("art")));
}

#[test]
fn the_pass_saves_and_continues_identically() {
    let mut sim = loaded(game(3, "pass"));
    scripted(&mut sim, 6);
    let saved = sim.save().unwrap();
    let mut copy = loaded(game(3, "pass"));
    copy.restore(&saved).unwrap();
    assert_eq!(copy.world().hash(), sim.world().hash());
    // Key edges only: they are stamped on each sim's own clock.
    for s in [&mut sim, &mut copy] {
        s.key_down("KeyF");
        s.key_down("KeyW");
        s.run(1200.0);
        s.tap("Digit2");
        s.run(1500.0);
        s.tap("Digit1");
        s.tap("KeyT");
        s.run(800.0);
    }
    assert_eq!(copy.world().hash(), sim.world().hash());
    assert_eq!(copy.save().unwrap(), sim.save().unwrap());
}

#[test]
fn the_pass_draws_from_saved_causes() {
    let mut sim = game(1, "pass");
    sim.run(200.0);
    {
        // One soldier model, in its team's armour, facing its bot's aim.
        let w = sim.world();
        let soldiers: Vec<_> = w.query::<&Soldier>().iter().map(|(e, _)| e).collect();
        assert_eq!(soldiers.len(), 1);
        assert!(w.get::<Offset>(soldiers[0]).is_some());
        let (limb, _) = w.query::<&Limb>().iter().next().expect("soldier parts");
        let looks = w.get::<MaterialOverrides>(limb).expect("team colours");
        assert_eq!(looks.0[0].color, Some([0.86, 0.22, 0.24, 1.0]));
    }
    // Only the held weapon shows; no flash before a shot.
    let shown = |sim: &Sim<Rivals>, part: Part| {
        let w = sim.world();
        let parts: Vec<_> = w
            .query::<&Held>()
            .iter()
            .filter(|(_, h)| h.part == part)
            .map(|(e, h)| (h.weapon, w.get::<Opacity>(e).map_or(1.0, |o| o.0)))
            .collect();
        parts
    };
    for (weapon, opacity) in shown(&sim, Part::Body) {
        assert_eq!(opacity, if weapon == Weapon::Rifle { 1.0 } else { 0.0 });
    }
    assert!(shown(&sim, Part::Flash).iter().all(|(_, o)| *o == 0.0));
    sim.tap("KeyF");
    sim.run(16.0);
    assert!(shown(&sim, Part::Flash).iter().any(|(_, o)| *o == 1.0));
    sim.run(200.0);
    assert!(shown(&sim, Part::Flash).iter().all(|(_, o)| *o == 0.0));
}
