//! Presentation that persists between presents: rows a present writes again
//! unchanged are not changes, rows it stops writing go, and rows derived per
//! entity (`Present::each`) are derived again only where their keys changed.
use exact_game::*;
use std::cell::Cell;

#[derive(Default, Component)]
struct Crop {
    stage: u8,
}

thread_local! {
    static DERIVED: Cell<u32> = const { Cell::new(0) };
}
fn derived() -> u32 {
    DERIVED.with(Cell::get)
}

/// A hundred crops; one grows every tick, crop 7 loses its `Crop` at tick 5
/// and crop 8 is despawned at tick 6.
fn grow(w: &mut World) {
    let crops: Vec<Entity> = w.query::<&Crop>().iter().map(|(e, _)| e).collect();
    let tick = w.tick() as usize;
    w.require_mut::<Crop>(crops[tick % crops.len()]).stage += 1;
    if tick == 5 {
        let e = w.named("crop-7").unwrap();
        w.remove::<Crop>(e);
    }
    if tick == 6 {
        let e = w.named("crop-8").unwrap();
        w.despawn(e);
    }
}
fn plant(w: &mut World) {
    for i in 0..100 {
        w.spawn_named(
            format!("crop-{i}"),
            (Transform::at(i as f32, 0., 0.), Crop::default()),
        );
    }
}
/// Each crop's tint by its stage, and odd stages half faded.
fn look(p: &mut Present<'_>, e: Entity) {
    DERIVED.with(|d| d.set(d.get() + 1));
    let stage = p.require::<Crop>(e).stage;
    p.insert(
        e,
        Tint {
            color: [stage as f32, 1., 1., 1.],
            emissive: [0.; 3],
        },
    );
    if stage % 2 == 1 {
        p.insert(e, Opacity(0.5));
    }
}

struct Field;
impl Game for Field {
    const ID: &'static str = "derived";
    const HZ: u32 = 10;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        plant(w);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        grow(w);
    }
    fn present(p: &mut Present<'_>, _: &()) {
        p.each::<Crop>(|p, e| {
            look(p, e);
            Derived::Kept
        });
    }
}

fn rows(w: &World) -> Vec<(u32, u32, Option<u32>)> {
    w.query::<&Tint>()
        .iter()
        .map(|(e, t)| {
            let faded = w.get::<Opacity>(e).map(|o| o.0.to_bits());
            (e.index(), t.color[0].to_bits(), faded)
        })
        .collect()
}

#[test]
fn a_derivation_runs_again_only_for_what_changed() {
    DERIVED.with(|d| d.set(0));
    let mut sim = Sim::<Field>::new(()).unwrap();
    assert_eq!(derived(), 100, "setup's present derives every crop");
    let tint = sim.world().revision::<Tint>();
    for tick in 1..=4u32 {
        assert_eq!(sim.run(100.), 1);
        assert_eq!(derived(), 100 + tick, "one crop grew");
    }
    // Four crops changed their tint: four rows written, nothing else.
    assert_eq!(sim.world().changed::<Tint>(tint).count(), 4);
    let e = sim.world().named("crop-1").unwrap();
    assert_eq!(sim.world().require::<Tint>(e).color[0], 1.);
    assert!(sim.world().has::<Opacity>(e));
    for _ in 0..200 {
        sim.run(100.);
    }
    // Every crop's rows follow its stage: an even one's old Opacity is gone.
    let w = sim.world();
    for (e, crop) in w.query::<&Crop>().iter() {
        assert_eq!(w.require::<Tint>(e).color[0], crop.stage as f32);
        assert_eq!(w.has::<Opacity>(e), crop.stage % 2 == 1);
    }
    assert!(w.query::<&Crop>().iter().any(|(_, c)| c.stage == 2));
    // Crop 7 lost its key, and its rows; crop 8 is gone.
    let seven = sim.world().named("crop-7").unwrap();
    assert!(!sim.world().has::<Tint>(seven));
    assert!(sim.world().named("crop-8").is_none());
}

#[test]
fn kept_rows_equal_a_fresh_present_in_every_paranoid_mode() {
    let mut plain = Sim::<Field>::new(()).unwrap();
    plain.run(2_500.);
    for mode in [Paranoid::Save, Paranoid::FreshGame] {
        let mut checked = Sim::<Field>::new(()).unwrap().paranoid(mode);
        checked.run(2_500.);
        assert_eq!(rows(checked.world()), rows(plain.world()), "{mode:?}");
    }
    // A restored world presents in full: the same rows.
    let mut back = Sim::<Field>::new(()).unwrap();
    back.restore(&plain.save().unwrap()).unwrap();
    assert_eq!(rows(back.world()), rows(plain.world()));
}

#[test]
#[should_panic(expected = "differs from a fresh present")]
fn paranoid_refuses_a_derivation_that_reads_the_time() {
    struct Clocked;
    impl Game for Clocked {
        const ID: &'static str = "derived";
        const HZ: u32 = 10;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            p.each::<Crop>(|p, e| {
                let pulse = p.tick() as f32;
                p.insert(
                    e,
                    Tint {
                        color: [pulse, 1., 1., 1.],
                        emissive: [0.; 3],
                    },
                );
                Derived::Kept
            });
        }
    }
    Sim::<Clocked>::new(())
        .unwrap()
        .paranoid(Paranoid::Save)
        .run(500.);
}

#[test]
fn animated_rows_are_derived_at_every_present() {
    struct Pulsing;
    impl Game for Pulsing {
        const ID: &'static str = "derived";
        const HZ: u32 = 10;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(w: &mut World, _: &Input, _: &()) {
            grow(w);
        }
        fn present(p: &mut Present<'_>, _: &()) {
            p.each::<Crop>(|p, e| {
                look(p, e);
                // Ripe crops pulse with the time; the rest keep.
                if p.require::<Crop>(e).stage < 2 {
                    return Derived::Kept;
                }
                let pulse = p.tick() as f32;
                p.insert(e, Opacity(pulse));
                Derived::Animated
            });
        }
    }
    let mut checked = Sim::<Pulsing>::new(()).unwrap().paranoid(Paranoid::Save);
    checked.run(25_000.);
    let mut plain = Sim::<Pulsing>::new(()).unwrap();
    plain.run(25_000.);
    assert_eq!(rows(checked.world()), rows(plain.world()));
    let ripe = plain.world().named("crop-0").unwrap();
    assert_eq!(
        plain.world().require::<Opacity>(ripe).0,
        plain.world().tick() as f32
    );
}

#[test]
fn a_rewrite_with_the_same_value_is_not_a_change_and_an_unwritten_row_goes() {
    struct Blinking;
    impl Game for Blinking {
        const ID: &'static str = "derived";
        const HZ: u32 = 10;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            let one = p.named("crop-1").unwrap();
            p.insert(one, Tint::default());
            if p.tick().is_multiple_of(2) {
                let two = p.named("crop-2").unwrap();
                p.insert(two, Opacity(0.5));
            }
        }
    }
    let mut sim = Sim::<Blinking>::new(()).unwrap();
    let tint = sim.world().revision::<Tint>();
    sim.run(100.);
    assert_eq!(sim.world().revision::<Tint>(), tint, "the same tint again");
    let two = sim.world().named("crop-2").unwrap();
    assert!(!sim.world().has::<Opacity>(two), "tick 1 did not write it");
    sim.run(100.);
    assert!(sim.world().has::<Opacity>(two));
}

#[test]
#[should_panic(expected = "one writer per present")]
fn a_derived_row_has_one_writer() {
    struct Twice;
    impl Game for Twice {
        const ID: &'static str = "derived";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            p.each::<Crop>(|p, e| {
                look(p, e);
                Derived::Kept
            });
            let one = p.named("crop-1").unwrap();
            p.insert(one, Tint::default());
        }
    }
    Sim::<Twice>::new(()).unwrap();
}

#[test]
#[should_panic(expected = "a derivation writes only its own entity's rows")]
fn a_derivation_writes_only_its_entity() {
    struct Reaching;
    impl Game for Reaching {
        const ID: &'static str = "derived";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            p.each::<Crop>(|p, _| {
                let other = p.named("crop-1").unwrap();
                p.insert(other, Opacity(0.5));
                Derived::Kept
            });
        }
    }
    Sim::<Reaching>::new(()).unwrap();
}

#[test]
fn a_row_is_the_same_only_bit_for_bit() {
    // Negative zero equals zero, but draws, saves and digests tell them apart:
    // a present that writes one after the other changes the row.
    struct Signed;
    impl Game for Signed {
        const ID: &'static str = "derived";
        const HZ: u32 = 10;
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            plant(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
        fn present(p: &mut Present<'_>, _: &()) {
            let one = p.named("crop-1").unwrap();
            let x = if p.tick().is_multiple_of(2) {
                0.0
            } else {
                -0.0
            };
            p.insert(one, Offset(Transform::at(x, 0., 0.)));
        }
    }
    let mut sim = Sim::<Signed>::new(()).unwrap().paranoid(Paranoid::Save);
    sim.run(100.);
    let one = sim.world().named("crop-1").unwrap();
    let x = sim.world().require::<Offset>(one).0.position.x;
    assert!(x == 0.0 && x.is_sign_negative());
}
