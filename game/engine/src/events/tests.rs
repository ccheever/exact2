use crate::*;
#[derive(Default, Event)]
#[event(capacity = 2)]
struct Hit {
    target: Entity,
    damage: f32,
}
#[derive(Default, Resource)]
struct Damage(f32);
struct Combat;
impl Game for Combat {
    const ID: &'static str = "event-save-oracle";
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        w.register_event::<Hit>();
    }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("wolf-3", Transform::default());
        w.insert_resource(Damage::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        for hit in input.events::<Hit>() {
            assert_eq!(w.name(hit.target), Some("wolf-3"));
            w.resource_mut::<Damage>().0 += hit.damage;
        }
        if w.tick() == 0 {
            w.emit_event(Hit {
                target: w.named("wolf-3").unwrap(),
                damage: 12.,
            });
        }
    }
}
#[test]
fn next_tick_events_hash_save_restore_drain_and_overflow_refuses_atomically() {
    let mut sim = Sim::<Combat>::new(()).unwrap();
    sim.advance(0., Clock::Seekable);
    sim.advance(17., Clock::Seekable);
    assert_eq!(sim.world().resource::<Damage>().0, 0.);
    let before = sim.world().hash();
    assert!(sim.world().emit_event(Hit {
        target: sim.world().named("wolf-3").unwrap(),
        damage: 0.
    }));
    let full = sim.world().hash();
    assert_ne!(before, full);
    for _ in 0..3 {
        assert!(!sim.world().emit_event(Hit::default()));
        assert_eq!(sim.world().hash(), full);
    }
    assert_eq!(
        sim.world()
            .journal()
            .iter()
            .filter(|j| j.line.contains("event Hit refused"))
            .count(),
        1
    );
    let save = sim.save().unwrap();
    let mut restored = Sim::<Combat>::new(()).unwrap();
    restored.restore(&save).unwrap();
    assert_eq!(sim.world().hash(), restored.world().hash());
    for s in [&mut sim, &mut restored] {
        s.advance(17., Clock::Seekable);
        s.advance(34., Clock::Seekable);
    }
    assert_eq!(sim.world().resource::<Damage>().0, 12.);
    assert_eq!(sim.world().hash(), restored.world().hash());
    let events = sim.agent(r#"{"op":"state","events":true,"event_type":"Hit"}"#);
    assert!(
        events.contains("\"damage\":12") && events.contains("\"tick\":2"),
        "{events}"
    );
    println!("state world events Hit: {events}");
    assert!(sim
        .agent(r#"{"op":"state","events":true,"event_type":"Missing"}"#)
        .contains("unknown event type Missing"));
    sim.advance(51., Clock::Seekable);
    sim.advance(68., Clock::Seekable);
    assert_eq!(sim.world().resource::<Damage>().0, 12.);
    assert!(sim.quiescent(), "an empty event queue must rest");
}
