//! Actions on world time: a button's edges, pads, triggers and their lifecycle.
//! At 60 Hz tick `k` covers world [k/60 s, (k+1)/60 s): an event at 105 ms
//! lands in tick 6, and a hold that matures at 305 ms fires in tick 18.
use exact_game::*;

#[derive(Default, Resource)]
struct Fired {
    shots: u32,
    held_ticks: u32,
}
struct Fire;
impl Game for Fire {
    const ID: &'static str = "input-fire";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().button("fire", &["KeyF"])
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Fired::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut fired = w.resource_mut::<Fired>();
        // The recipe: a fire button is a `button` read with `pressed`.
        if input.pressed("fire") {
            fired.shots += 1;
        }
        if input.held("fire") {
            fired.held_ticks += 1;
        }
    }
}

/// The Rivals case: both edges of a tap land in one tick. `held` never sees it;
/// `pressed` does, with no trigger declared.
#[test]
fn a_same_tick_tap_is_pressed_but_never_held() {
    let mut sim = Sim::<Fire>::new(()).unwrap();
    sim.run(100.);
    sim.tap("KeyF");
    sim.run(100.);
    let fired = sim.world().resource::<Fired>();
    assert_eq!((fired.shots, fired.held_ticks), (1, 0));
}

const NAMES: [&str; 9] = [
    "jump", "sprint", "roll", "special", "dash", "brake", "combo", "z", "a",
];
/// Every edge the tick saw, as `tick name+` and `tick name-`.
#[derive(Default, Resource)]
struct Seen {
    lines: Vec<String>,
}
struct Pads;
impl Game for Pads {
    const ID: &'static str = "input-pads";
    type Args = ();
    fn actions() -> Actions {
        Actions::new()
            .stick("move", Stick::wasd().or_pad(Pad::Left))
            .button("jump", &["Space", "GamepadSouth"])
            .hold("sprint", &["ShiftLeft", "GamepadL3"], 0.2)
            .double("roll", &["KeyR"], 0.25)
            .chord("special", &["ControlLeft", "KeyE"])
            .tap("dash", &["KeyQ", "GamepadEast"])
            .context("driving", |a| a.button("brake", &["KeyB", "GamepadWest"]))
            .chord("combo", &["GamepadL1", "GamepadSouth"])
            // Declared out of name order, on one key: their sorted states share an event.
            .hold("z", &["KeyH"], 0.2)
            .hold("a", &["KeyH"], 0.2)
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
        w.insert_resource(Seen::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let tick = w.tick();
        {
            let mut seen = w.resource_mut::<Seen>();
            for name in NAMES {
                if input.pressed(name) {
                    seen.lines.push(format!("{tick} {name}+"));
                }
                if input.released(name) {
                    seen.lines.push(format!("{tick} {name}-"));
                }
            }
        }
        let step = input.stick_xz("move") * 4.0 * w.dt();
        w.require_mut::<Transform>("player").position += step;
        for message in input.messages() {
            match message.as_str() {
                "drive" => w.input_context("driving", true),
                "walk" => w.input_context("driving", false),
                m => w.rebind_text(m.strip_prefix("bind ").unwrap()).unwrap(),
            }
        }
    }
}
/// A sim whose host clock starts at world zero.
fn pads() -> Sim<Pads> {
    started(Sim::new(()).unwrap())
}
fn started<G: Game>(mut sim: Sim<G>) -> Sim<G> {
    sim.advance(0., Clock::Seekable);
    sim
}
/// Advance to an absolute host time in milliseconds.
fn at<G: Game>(sim: &mut Sim<G>, ms: f64) {
    sim.advance(ms, Clock::Seekable);
}
fn seen(sim: &Sim<Pads>) -> Vec<String> {
    sim.world().resource::<Seen>().lines.clone()
}
/// Journal lines without their `t=… tick=…` prefix.
fn journal<G: Game>(sim: &Sim<G>) -> Vec<String> {
    let lines = sim.world().journal().into_iter();
    lines
        .map(|e| e.line.splitn(3, ' ').nth(2).unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_hold_matures_on_world_time_and_across_a_save() {
    for mode in [Paranoid::Off, Paranoid::Save, Paranoid::FreshGame] {
        let mut sim = started(Sim::<Pads>::new(()).unwrap().paranoid(mode));
        at(&mut sim, 105.);
        sim.key_down("ShiftLeft");
        at(&mut sim, 205.);
        // Mid-hold: a restored world continues the hold and matures it alike.
        let mut restored = Sim::<Pads>::new(()).unwrap().paranoid(mode);
        restored.restore(&sim.save().unwrap()).unwrap();
        restored.advance(0., Clock::Seekable);
        for (game, offset) in [(&mut sim, 0.), (&mut restored, -205.)] {
            at(game, 316. + offset);
            assert_eq!(seen(game), [] as [&str; 0]);
            at(game, 317. + offset);
            assert_eq!(seen(game), ["18 sprint+"]);
            game.key_up("ShiftLeft");
            at(game, 420. + offset);
            assert_eq!(seen(game), ["18 sprint+", "19 sprint-"]);
            // A press shorter than the window fires nothing.
            game.key_down("ShiftLeft");
            at(game, 500. + offset);
            game.key_up("ShiftLeft");
            at(game, 900. + offset);
            assert_eq!(seen(game).len(), 2);
        }
        assert_eq!(sim.world().hash(), restored.world().hash());
    }
}

#[test]
fn double_chord_and_tap_fire_on_their_world_stamps() {
    let mut sim = pads();
    // A double whose presses straddle a tick: 110 ms in tick 6, 120 ms in tick 7.
    for (ms, down) in [(110., true), (112., false), (120., true), (125., false)] {
        at(&mut sim, ms);
        if down {
            sim.key_down("KeyR");
        } else {
            sim.key_up("KeyR");
        }
    }
    // Two presses 300 ms apart are not a double.
    at(&mut sim, 400.);
    sim.tap("KeyR");
    at(&mut sim, 700.);
    sim.tap("KeyR");
    at(&mut sim, 1000.);
    assert_eq!(seen(&sim), ["7 roll+", "7 roll-"]);

    // A chord fires when its last key goes down while the others are held,
    // in the tick that key lands; the other order fires nothing.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.key_down("ControlLeft");
    at(&mut sim, 110.);
    sim.key_down("KeyE");
    at(&mut sim, 155.);
    sim.key_up("KeyE");
    at(&mut sim, 205.);
    sim.key_up("ControlLeft");
    sim.key_down("KeyE");
    at(&mut sim, 210.);
    sim.key_down("ControlLeft");
    at(&mut sim, 400.);
    assert_eq!(seen(&sim), ["6 special+", "9 special-"]);

    // A tap fires pressed and released together at a release within 200 ms,
    // on a key or a pad button; a long press fires nothing.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.key_down("KeyQ");
    at(&mut sim, 155.);
    sim.key_up("KeyQ");
    at(&mut sim, 300.);
    sim.key_down("KeyQ");
    at(&mut sim, 550.);
    sim.key_up("KeyQ");
    at(&mut sim, 610.);
    sim.pad_button(0, "GamepadEast", 1.);
    at(&mut sim, 620.);
    sim.pad_button(0, "GamepadEast", 0.);
    at(&mut sim, 700.);
    assert_eq!(seen(&sim), ["9 dash+", "9 dash-", "37 dash+", "37 dash-"]);
}

/// Appendix E c2's test: a full stick and a held key move the world alike,
/// compared on the world hash, not on the input.
#[test]
fn a_full_stick_and_a_held_key_leave_the_same_world() {
    let drive = |pad: Option<f32>| {
        let mut sim = pads();
        at(&mut sim, 100.);
        match pad {
            Some(x) => sim.pad_stick(0, Pad::Left, x, 0.),
            None => sim.key_down("KeyD"),
        }
        at(&mut sim, 1600.);
        match pad {
            Some(_) => sim.pad_stick(0, Pad::Left, 0., 0.),
            None => sim.key_up("KeyD"),
        }
        at(&mut sim, 1700.);
        (sim.world().hash(), sim.local_position("player").unwrap())
    };
    let (key, moved) = drive(None);
    assert!(moved.x > 5.9, "{moved}");
    assert_eq!(drive(Some(1.)).0, key);
    // The negative control: a partial stick is a different world.
    assert_ne!(drive(Some(0.7)).0, key);
}

#[test]
fn pad_lifecycle_releases_and_starts_fresh() {
    // Disconnect mid-hold releases; reconnect at the same index starts released.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.pad_button(0, "GamepadSouth", 1.);
    at(&mut sim, 155.);
    sim.pad_disconnect(0);
    at(&mut sim, 205.);
    sim.pad_stick(0, Pad::Left, 0.5, 0.);
    at(&mut sim, 300.);
    assert_eq!(seen(&sim), ["6 jump+", "9 jump-"]);
    let lines = journal(&sim);
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("pad 0"))
            .collect::<Vec<_>>(),
        [
            "pad 0 connected standard",
            "pad 0 disconnected",
            "pad 0 connected standard"
        ]
    );

    // Focus loss releases a pad's buttons and forgets the pad.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.pad_button(1, "GamepadSouth", 1.);
    at(&mut sim, 155.);
    sim.input(InputEvent::Blur {
        at_ms: 155.,
        player: 0,
    });
    at(&mut sim, 205.);
    assert_eq!(seen(&sim), ["6 jump+", "9 jump-"]);
    assert!(sim.pad(1).is_none());

    // Keyboard and pad are one action: switching device mid-hold neither
    // presses again nor drops.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.key_down("Space");
    at(&mut sim, 120.);
    sim.pad_button(0, "GamepadSouth", 1.);
    at(&mut sim, 130.);
    sim.key_up("Space");
    at(&mut sim, 190.);
    sim.pad_button(0, "GamepadSouth", 0.);
    at(&mut sim, 300.);
    assert_eq!(seen(&sim), ["6 jump+", "11 jump-"]);

    // A pad repeating its state is one event; a non-standard pad delivers nothing.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.pad_stick(2, Pad::Left, 0.5, 0.);
    sim.pad_stick(2, Pad::Left, 0.5, 0.);
    let mut unmapped = [0.; 17];
    unmapped[0] = 1.;
    sim.input(InputEvent::Pad {
        index: 3,
        connected: true,
        standard: false,
        buttons: unmapped,
        axes: [0.; 4],
        at_ms: 105.,
        player: 0,
    });
    at(&mut sim, 200.);
    assert_eq!(seen(&sim), [] as [&str; 0]);
    assert!(journal(&sim)
        .contains(&"pad 3 connected unmapped: no standard mapping, delivers nothing".into()));
}

#[test]
fn contexts_and_rebinding_release_and_wait_for_a_fresh_press() {
    let mut sim = pads();
    at(&mut sim, 100.);
    // Out of its context, a held key reads nothing.
    sim.key_down("KeyB");
    at(&mut sim, 150.);
    sim.post("drive");
    // Entering the context with KeyB down does not press brake.
    at(&mut sim, 250.);
    sim.key_up("KeyB");
    at(&mut sim, 300.);
    sim.key_down("KeyB");
    at(&mut sim, 350.);
    // Leaving the context mid-hold releases on the next tick, by name.
    sim.post("walk");
    at(&mut sim, 400.);
    assert_eq!(seen(&sim), ["18 brake+", "22 brake-"]);
    assert!(journal(&sim).contains(&"context driving off: released brake".into()));
    assert!(!sim.world().input_context_active("driving"));

    // Rebinding a held action releases it; a new key already down waits for
    // its next press.
    let mut sim = pads();
    at(&mut sim, 105.);
    sim.key_down("Space");
    sim.pad_button(0, "GamepadNorth", 1.);
    at(&mut sim, 150.);
    sim.post("bind jump GamepadNorth");
    at(&mut sim, 250.);
    sim.pad_button(0, "GamepadNorth", 0.);
    at(&mut sim, 300.);
    sim.pad_button(0, "GamepadNorth", 1.);
    at(&mut sim, 350.);
    assert_eq!(seen(&sim), ["6 jump+", "10 jump-", "18 jump+"]);
    let lines = journal(&sim);
    assert!(
        lines.contains(&"bind jump GamepadNorth".into()),
        "{lines:?}"
    );
    assert!(lines.contains(&"bind jump: released, rebound while held".into()));
    // The binding is world state: saved, hashed and restored.
    let saved = sim.save().unwrap();
    let mut restored = Sim::<Pads>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(
        restored.world().resource::<Bindings>().actions,
        [Binding {
            action: "jump".into(),
            keys: vec!["GamepadNorth".into()]
        }]
    );
    assert_eq!(restored.world().hash(), sim.world().hash());
}

#[derive(Default, Args)]
struct Stored {
    #[live]
    bindings: String,
}
struct Seeded;
impl Game for Seeded {
    const ID: &'static str = "input-seeded";
    type Args = Stored;
    fn actions() -> Actions {
        Actions::new().button("jump", &["Space"])
    }
    fn bindings(args: &Stored) -> String {
        args.bindings.clone()
    }
    fn setup(w: &mut World, _: &Stored) {
        w.insert_resource(Seen::default());
    }
    fn tick(w: &mut World, input: &Input, _: &Stored) {
        let tick = w.tick();
        if input.pressed("jump") {
            w.resource_mut::<Seen>().lines.push(format!("{tick} jump+"));
        }
    }
}

/// A new game copies the player's stored bindings into its world.
#[test]
fn a_new_game_seeds_the_stored_bindings() {
    let mut sim = started(
        Sim::<Seeded>::new(Stored {
            bindings: "jump GamepadNorth; ghost KeyG".into(),
        })
        .unwrap(),
    );
    at(&mut sim, 105.);
    sim.key_down("Space");
    sim.pad_button(0, "GamepadNorth", 1.);
    at(&mut sim, 200.);
    assert_eq!(sim.world().resource::<Seen>().lines, ["6 jump+"]);
    let lines = journal(&sim);
    assert!(
        lines.contains(&"bind jump GamepadNorth".into()),
        "{lines:?}"
    );
    assert!(lines
        .iter()
        .any(|l| l.starts_with("bind ghost: not a declared button")));
    // Without stored bindings the world has no Bindings resource, so its
    // hash is what it was before rebinding existed.
    let plain = Sim::<Seeded>::new(Stored::default()).unwrap();
    assert!(plain.world().try_resource::<Bindings>().is_none());
}

#[test]
fn the_agent_drives_a_pad_and_reads_what_fired() {
    let mut sim = pads();
    at(&mut sim, 100.);
    let typed = sim.agent(r#"{"op":"type","key":"GamepadSouth","phase":"down","now":105}"#);
    assert!(typed.contains(r#""delivery":"synthesized""#), "{typed}");
    let tapped = sim.agent(r#"{"op":"tap","stick":"left","value":[1,0],"now":105}"#);
    assert!(
        tapped.contains(r#""stick":"left","value":[1,0]"#),
        "{tapped}"
    );
    let state = sim.agent(r#"{"op":"state","now":120}"#);
    assert!(
        state.contains(r#""heldActions":["move","jump"]"#),
        "{state}"
    );
    assert!(
        // The press keeps its own event in the tick, before the stick's.
        state.contains(r#""fired":{"pressed":["jump","move"],"released":[]}"#),
        "{state}"
    );
    assert!(
        state.contains(r#""pads":[{"index":0,"standard":true,"buttons":[1,"#),
        "{state}"
    );
    for refused in [
        r#"{"op":"type","key":"Space"}"#,
        r#"{"op":"type","key":"GamepadA"}"#,
        r#"{"op":"tap","stick":"left","value":[2,0]}"#,
        r#"{"op":"tap","stick":"middle","value":[0,0]}"#,
    ] {
        assert!(sim.agent(refused).contains("\"error\""), "{refused}");
    }
}

/// A resting world: it changes only on the sprint hold's edges.
#[derive(Default, Resource)]
struct Sprints(u32);
struct Rest;
impl Game for Rest {
    const ID: &'static str = "input-rest";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().hold("sprint", &["ShiftLeft"], 0.2)
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Sprints::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        if input.pressed("sprint") {
            w.resource_mut::<Sprints>().0 += 1;
        }
    }
}

/// LLP 1128 I2: a pending hold is work `clock settle` waits for, and a world
/// whose hold started earlier is a different world (I1).
#[test]
fn a_pending_hold_keeps_the_world_unsettled_and_is_hashed() {
    let mut sim = started(Sim::<Rest>::new(()).unwrap());
    at(&mut sim, 105.);
    sim.key_down("ShiftLeft");
    at(&mut sim, 150.);
    let clock = sim.agent(r#"{"op":"clock"}"#);
    assert!(
        clock.contains(r#""changing":["input trigger sprint"]"#),
        "{clock}"
    );
    assert!(sim.settle());
    assert_eq!(sim.world().resource::<Sprints>().0, 1);
    assert!(
        sim.world().tick() >= 19,
        "settled at {}",
        sim.world().tick()
    );

    let pending = |start: f64| {
        let mut sim = started(Sim::<Rest>::new(()).unwrap());
        at(&mut sim, start);
        sim.key_down("ShiftLeft");
        at(&mut sim, 150.);
        sim
    };
    let (early, late) = (pending(105.), pending(110.));
    assert_eq!(early.world().tick(), late.world().tick());
    assert_ne!(early.world().hash(), late.world().hash());
    assert_eq!(early.world().hash(), pending(105.).world().hash());
}

#[test]
fn edge_bearing_pad_states_keep_their_stamps() {
    // Press at 101 ms, a stick change at 115 ms in the same tick, release at
    // 310 ms: a 209 ms press, not a 195 ms tap.
    let mut sim = pads();
    at(&mut sim, 101.);
    sim.pad_button(0, "GamepadEast", 1.);
    at(&mut sim, 115.);
    sim.pad_stick(0, Pad::Left, 0.5, 0.);
    at(&mut sim, 310.);
    sim.pad_button(0, "GamepadEast", 0.);
    at(&mut sim, 400.);
    assert!(
        !seen(&sim).iter().any(|l| l.contains("dash")),
        "{:?}",
        seen(&sim)
    );

    // A chord across two pads keeps its order when a stick moves after it.
    let mut sim = pads();
    at(&mut sim, 101.);
    sim.pad_button(1, "GamepadL1", 1.);
    at(&mut sim, 105.);
    sim.pad_button(0, "GamepadSouth", 1.);
    at(&mut sim, 110.);
    sim.pad_stick(1, Pad::Left, 0.5, 0.);
    at(&mut sim, 200.);
    assert!(seen(&sim).contains(&"6 combo+".into()), "{:?}", seen(&sim));
}

#[test]
fn holds_sharing_a_key_release_together() {
    for blur in [false, true] {
        let mut sim = pads();
        at(&mut sim, 105.);
        sim.key_down("KeyH");
        at(&mut sim, 400.);
        if blur {
            sim.input(InputEvent::Blur {
                at_ms: 400.,
                player: 0,
            });
        } else {
            sim.key_up("KeyH");
        }
        at(&mut sim, 500.);
        let lines = seen(&sim);
        assert_eq!(lines, ["18 z+", "18 a+", "24 z-", "24 a-"], "blur {blur}");
        let state = sim.agent(r#"{"op":"state"}"#);
        assert!(state.contains(r#""heldActions":[]"#), "{state}");
    }
}

#[test]
fn the_pad_pushed_furthest_steers() {
    let run = |pads: &[(u32, f32)]| {
        let mut sim = self::pads();
        at(&mut sim, 100.);
        for &(index, x) in pads {
            sim.pad_stick(index, Pad::Left, x, 0.);
        }
        at(&mut sim, 1600.);
        sim.local_position("player").unwrap().x
    };
    // Opposite sticks do not cancel: pad 0 at full deflection wins.
    assert_eq!(run(&[(0, 1.), (1, -0.5)]), run(&[(0, 1.)]));
    assert_eq!(run(&[(0, -0.5), (1, 1.)]), run(&[(0, 1.)]));
}
