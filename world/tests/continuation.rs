//! Frozen bytes produced by core/world 428436d, before K1c storage/input changes.
use exact_world::*;
#[derive(Default, Component)]
struct Position {
    x: i64,
    label: String,
}
#[derive(Default, Component)]
struct Tag;
#[derive(Default, Resource)]
struct Ledger {
    values: Vec<u16>,
}
struct Fixture;
impl Game for Fixture {
    const ID: &'static str = "k1c-continuation";
    const HZ: u32 = 1000;
    const ACTIONS: &'static [Action] = &[
        Action::button("move", &["KeyW"]),
        Action::axis("drive", "KeyA", "KeyD"),
    ];
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<Position>()?
            .register::<Tag>()?
            .register_resource::<Ledger>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &()) {
        for i in 0..137 {
            w.spawn_named(format!("slot{i}"), ()).unwrap();
        }
        for i in (0..137).rev() {
            w.insert(
                w.entity_at(i).unwrap(),
                Position {
                    x: i as i64,
                    label: format!("value{i}"),
                },
            )
            .unwrap();
            if i % 7 == 0 {
                w.insert(w.entity_at(i).unwrap(), Tag).unwrap();
            }
        }
        for i in (0..137).step_by(11) {
            w.despawn(w.entity_at(i).unwrap()).unwrap();
            w.spawn(Position {
                x: -1,
                label: "recycled".into(),
            })
            .unwrap();
        }
        w.insert_resource(Ledger {
            values: vec![0, 17, u16::MAX],
        })
        .unwrap();
        w.set_parent(w.entity_at(136).unwrap(), Some(w.entity_at(65).unwrap()))
            .unwrap();
        w.work("assets", Work::Pending).unwrap();
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        for (_, (p, tag)) in &mut w.query::<(&mut Position, Option<&Tag>)>() {
            p.x += i64::from(input.held("move")) + i64::from(tag.is_some());
        }
        w.resource_mut::<Ledger>().values[1] += 1;
        w.publish("drive", input.axis("drive")).unwrap();
        w.publish("tick", w.tick() as u32).unwrap();
        w.rng().next_u32();
    }
}
// Length framing is only the test inventory; each payload is an unchanged save.
pub fn inventory() -> Vec<u8> {
    let mut sim = Sim::<Fixture>::new(()).unwrap();
    for event in [
        InputEvent::Key {
            code: "KeyW".into(),
            down: true,
            at_ms: 0.,
        },
        InputEvent::Axis {
            name: "drive".into(),
            value: 0.5,
            at_ms: 0.5,
        },
        InputEvent::Key {
            code: "KeyW".into(),
            down: false,
            at_ms: 2.,
        },
        InputEvent::Blur { at_ms: 3.5 },
    ] {
        sim.input(event).unwrap();
    }
    let mut out = Vec::new();
    for _ in 0..5 {
        let world = sim.world().save().unwrap();
        let mut driver = sim.save().unwrap();
        assert!(world.starts_with(b"EXGAME\0\x04"));
        assert!(driver.starts_with(b"EXSIM\0\x0a"));
        // v10 adds only the pending bit: normalize the independently frozen v9
        // continuation inventory while explicitly checking that new bit.
        assert_eq!(driver.pop(), Some(u8::from(sim.world().tick() != 0)));
        driver[6] = 9;
        assert_eq!(driver[8], 10);
        driver[8] = 9;
        out.extend_from_slice(&sim.world().hash().unwrap().to_le_bytes());
        for bytes in [world, driver] {
            out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        let saved = sim.save().unwrap();
        sim = Sim::from_save(&saved).unwrap();
        sim.run(1.).unwrap();
    }
    out
}
#[test]
fn k1b_v4_v9_bytes_and_hashes_remain_identical_after_churn_and_input() {
    assert_eq!(inventory(), include_bytes!("fixtures/k1b-continuation.bin"));
}
