//! The declared look (`motion.look`) against the Rust it replaced (`look.rs`):
//! the same rows, bit for bit, through a day and a night; and a declaration
//! edit reaching a running game without a build or a change to its world.
use exact_game::{Clock, Offset, Opacity, Sim, Transform, Vec3};
use forest_logic::camp::{Cycle, DAY};
use forest_logic::creatures::{Pack, Wolf};
use forest_logic::forest::Grove;
use forest_logic::player::{Child, Fate, Item, Kind};
use forest_logic::{motion, Forest, Options};

fn looked(trees: u32) -> Sim<Forest> {
    Sim::<Forest>::baked(Options {
        trees,
        art: "pass".into(),
        ..Options::default()
    })
}

fn place(sim: &mut Sim<Forest>, name: &str, at: Vec3) {
    let w = sim.world_mut();
    let e = w.resolve(name).unwrap();
    let t = *w.require::<Transform>(e);
    w.teleport(e, Transform { position: at, ..t });
}

fn bits(t: &Transform) -> [u32; 10] {
    let mut out = [0; 10];
    for (o, v) in out.iter_mut().zip(
        t.position
            .to_array()
            .into_iter()
            .chain(t.rotation.to_array())
            .chain(t.scale.to_array()),
    ) {
        *o = v.to_bits();
    }
    out
}

/// Every presentation row the look wrote, by entity.
fn rows(sim: &Sim<Forest>) -> Vec<String> {
    let w = sim.world();
    let mut out: Vec<String> = w
        .query::<&Offset>()
        .iter()
        .map(|(e, o)| format!("#{} offset {:?}", e.index(), bits(&o.0)))
        .chain(
            w.query::<&Opacity>()
                .iter()
                .map(|(e, o)| format!("#{} opacity {:08x}", e.index(), o.0.to_bits())),
        )
        .collect();
    out.sort();
    out
}

/// A day and a night, sampled: walking, chopping and carrying, a child
/// following, the flashlight, the Deer and the wolves, and the player among
/// the trees with the camera behind them.
fn script(rust: bool) -> Vec<Vec<String>> {
    motion::use_rust(rust);
    let mut sim = looked(800);
    let mut seen = Vec::new();
    sim.run(300.0);
    seen.push(rows(&sim));
    let tree = {
        let g = sim.world().resource::<Grove>();
        (0..g.hp.len())
            .filter(|&c| g.hp[c] > 0)
            .map(|c| g.at(c as u32))
            .min_by(|a, b| a.length().total_cmp(&b.length()))
            .unwrap()
    };
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
    seen.push(rows(&sim));
    let logs: Vec<Vec3> = sim
        .world()
        .query::<(&Transform, &Item)>()
        .iter()
        .filter(|(_, (t, i))| i.kind == Kind::Log && (t.position - tree).length() < 3.0)
        .map(|(_, (t, _))| t.position)
        .collect();
    for log in logs {
        place(&mut sim, "player", log + Vec3::Y * 0.8);
        sim.run(50.0);
        sim.tap("KeyE");
        sim.run(100.0);
    }
    seen.push(rows(&sim));
    {
        let w = sim.world_mut();
        w.require_mut::<Child>("child-1").fate = Fate::Following;
    }
    let at = sim.world().require::<Transform>("player").position;
    place(&mut sim, "child-1", at + Vec3::new(6.0, 0.0, 0.0));
    sim.hold("KeyW", 600.0);
    seen.push(rows(&sim));
    sim.tap("KeyF");
    sim.run(200.0);
    seen.push(rows(&sim));
    sim.world_mut().resource_mut::<Cycle>().t = DAY + 1.0;
    sim.run(3000.0);
    seen.push(rows(&sim));
    {
        let w = sim.world_mut();
        let wolves: Vec<_> = w.query::<&Wolf>().iter().map(|(e, _)| e).collect();
        for (k, e) in wolves.into_iter().enumerate() {
            w.require_mut::<Wolf>(e).mode = [Pack::Wander, Pack::Chase, Pack::Flee][k % 3];
        }
    }
    sim.run(500.0);
    seen.push(rows(&sim));
    for key in ["KeyS", "KeyA", "KeyW", "KeyD"] {
        sim.hold(key, 900.0);
        seen.push(rows(&sim));
    }
    let errors = motion::errors();
    assert!(errors.is_empty(), "the look failed: {errors:?}");
    motion::use_rust(false);
    seen
}

#[test]
fn the_declared_look_writes_the_rust_looks_rows_bit_for_bit() {
    let rust = script(true);
    let declared = script(false);
    assert_eq!(rust.len(), declared.len());
    for (k, (a, b)) in rust.iter().zip(&declared).enumerate() {
        let only_rust: Vec<_> = a.iter().filter(|r| !b.contains(r)).take(4).collect();
        let only_declared: Vec<_> = b.iter().filter(|r| !a.contains(r)).take(4).collect();
        assert!(
            only_rust.is_empty() && only_declared.is_empty(),
            "sample {k}: Rust alone {only_rust:?}, declared alone {only_declared:?}"
        );
    }
    // The script reached every rule: limbs and bodies, carried and fallen
    // supplies, faded crowns or undergrowth, and the beam both ways.
    let all: Vec<&String> = declared.iter().flatten().collect();
    let offsets = all.iter().filter(|r| r.contains("offset")).count();
    let faded = all
        .iter()
        .filter(|r| r.ends_with(&format!("{:08x}", 0.3f32.to_bits())))
        .count();
    assert!(offsets > 100, "{offsets} offsets");
    assert!(faded > 0, "nothing faded");
    assert!(all
        .iter()
        .any(|r| r.ends_with(&format!("opacity {:08x}", 1f32.to_bits()))));
    assert!(all
        .iter()
        .any(|r| r.ends_with(&format!("opacity {:08x}", 0f32.to_bits()))));
}

#[test]
fn a_declaration_edit_reaches_the_running_world_without_touching_it() {
    motion::use_rust(false);
    motion::reload(motion::SOURCE);
    let mut sim = looked(800);
    let mut control = looked(800);
    let mut now = 0.0;
    for s in [&mut sim, &mut control] {
        // Among the trees, walking: crowns and undergrowth fade around them.
        s.advance(0.0, Clock::Live);
        let g = s.world().resource::<Grove>();
        let c = (0..g.hp.len()).filter(|&c| g.hp[c] > 0).nth(200).unwrap();
        let tree = g.at(c as u32);
        drop(g);
        place(s, "player", Vec3::new(tree.x, tree.y + 0.95, tree.z + 1.5));
        s.key_down("KeyD");
    }
    for _ in 0..120 {
        now += 1000.0 / 60.0;
        sim.advance(now, Clock::Live);
        control.advance(now, Clock::Live);
    }
    let faded = |sim: &Sim<Forest>| -> Vec<u32> {
        sim.world()
            .query::<&Opacity>()
            .iter()
            .map(|(_, o)| o.0.to_bits())
            .filter(|b| *b != 0 && *b != 1f32.to_bits())
            .collect()
    };
    let before = faded(&sim);
    let tick = sim.world().tick();
    let edited = motion::SOURCE.replace("const FADED = 0.3", "const FADED = 0.55");
    let started = std::time::Instant::now();
    motion::reload(&edited);
    now += 1000.0 / 60.0;
    sim.advance(now, Clock::Live);
    let reload_ms = started.elapsed().as_secs_f64() * 1000.0;
    let (stats, compile_us) = motion::stats();
    control.advance(now, Clock::Live);
    assert!(motion::errors().is_empty(), "{:?}", motion::errors());
    assert_eq!(sim.world().tick(), tick + 1, "the world continued");
    assert_eq!(
        sim.world().hash(),
        control.world().hash(),
        "the world is untouched"
    );
    let after = faded(&sim);
    assert!(
        before.iter().all(|b| *b == 0.3f32.to_bits()) && !before.is_empty(),
        "{before:?}"
    );
    assert!(
        after.iter().all(|b| *b == 0.55f32.to_bits()) && !after.is_empty(),
        "{after:?}"
    );
    // A refused edit names its line and leaves the running look in place.
    motion::reload(&edited.replace("you.flashlight", "you.flashlite"));
    now += 1000.0 / 60.0;
    sim.advance(now, Clock::Live);
    let errors = motion::errors();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("flashlite") && e.contains("flashlight")),
        "{errors:?}"
    );
    assert!(faded(&sim).iter().all(|b| *b == 0.55f32.to_bits()));
    println!(
        "reload: compile {compile_us:.0} µs, edit to presented {reload_ms:.2} ms (one tick and present), {stats:?}"
    );
    motion::reload(motion::SOURCE);
}

#[test]
fn the_checker_reports_what_each_rule_reads() {
    motion::use_rust(false);
    motion::reload(motion::SOURCE);
    let mut sim = looked(300);
    sim.run(100.0);
    let report = motion::report();
    println!("{report}");
    assert!(report.contains("each … in Part"), "{report}");
    assert!(report.contains("GPU-lowerable"), "{report}");
}
