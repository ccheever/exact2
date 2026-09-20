use exact_world::{Component, Data, Resource, World};

#[derive(Default, Component)]
struct Health {
    hp: u32,
}
#[derive(Default, Resource)]
struct Score(u64);
#[test]
fn round_trip_preserves_ids_names_rng_resources_and_name_order() {
    let mut w = World::new(144, 8123);
    w.register::<Health>()
        .unwrap()
        .register_resource::<Score>()
        .unwrap();
    let e = w.spawn_named("fox", (Health { hp: 15 },)).unwrap();
    let dead = w.spawn_named("dead", (Health { hp: 4 },)).unwrap();
    w.despawn(dead);
    w.insert_resource(Score(79)).unwrap();
    w.rng().next_u32();
    let bytes = w.save().unwrap();
    let hash = w.hash();
    let mut loaded = World::new(30, 1);
    loaded
        .register_resource::<Score>()
        .unwrap()
        .register::<Health>()
        .unwrap();
    loaded.load(&bytes).unwrap();
    assert_eq!(hash, loaded.hash());
    assert_eq!(bytes, loaded.save().unwrap());
    assert_eq!(loaded.named("fox"), Some(e));
    assert_eq!(loaded.name(e), Some("fox"));
    assert_eq!(loaded.resource::<Score>().0, 79);
    for _ in 0..20 {
        assert_eq!(w.rng().next_u32(), loaded.rng().next_u32());
    }
    assert_eq!(w.spawn(()).unwrap(), loaded.spawn(()).unwrap());
}
#[test]
fn load_failure_is_atomic_and_names_unknown_types() {
    let mut source = World::new(60, 9);
    source.register::<Health>().unwrap();
    source.spawn((Health { hp: 7 },)).unwrap();
    let mut target = World::new(30, 2);
    target.spawn_named("keep", ()).unwrap();
    let before = target.save().unwrap();
    let error = target
        .load(&source.save().unwrap())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Health") && error.contains("unregistered"),
        "{error}"
    );
    assert_eq!(before, target.save().unwrap());
    target.register::<Health>().unwrap();
    let b = source.save().unwrap();
    for i in 0..b.len() {
        assert!(target.load(&b[..i]).is_err());
        assert_eq!(before, target.save().unwrap());
    }
    let mut junk = b.clone();
    junk.push(0);
    assert!(target.load(&junk).is_err());
    let mut bad_version = b;
    bad_version[7] += 1;
    assert!(target.load(&bad_version).is_err());
}
#[test]
fn component_schema_evolves_by_field_name() {
    #[derive(Data)]
    struct Old {
        hp: u32,
        obsolete: Vec<String>,
    }
    impl Default for Old {
        fn default() -> Self {
            Self {
                hp: 50,
                obsolete: vec![],
            }
        }
    }
    impl Component for Old {
        const NAME: &'static str = "Actor";
    }
    #[derive(Data)]
    struct New {
        added: u32,
        hp: u32,
    }
    impl Default for New {
        fn default() -> Self {
            Self { added: 123, hp: 90 }
        }
    }
    impl Component for New {
        const NAME: &'static str = "Actor";
    }
    let mut old = World::new(60, 0);
    old.register::<Old>().unwrap();
    let e = old
        .spawn((Old {
            hp: 7,
            obsolete: vec!["discard".into()],
        },))
        .unwrap();
    let mut new = World::new(60, 0);
    new.register::<New>().unwrap();
    assert!(new.carry(&old.save().unwrap()).unwrap());
    assert_eq!(new.get::<New>(e).unwrap().hp, 7);
    assert_eq!(new.get::<New>(e).unwrap().added, 123);
    assert!(old.carry(&new.save().unwrap()).unwrap());
    assert_eq!(old.get::<Old>(e).unwrap().hp, 7);
    assert!(old.get::<Old>(e).unwrap().obsolete.is_empty());
}
#[test]
fn published_values_and_skipped_fields_are_not_saved() {
    #[derive(Default, Component)]
    struct Cache {
        value: u64,
        #[data(skip)]
        cache: u32,
    }
    let mut w = World::new(60, 0);
    w.register::<Cache>().unwrap();
    let e = w
        .spawn((Cache {
            value: 2,
            cache: 42,
        },))
        .unwrap();
    let hash = w.hash();
    let bytes = w.save().unwrap();
    w.publish("not-in-EXGAME", 42u32).unwrap();
    assert!(w.take_published().is_some());
    w.get_mut::<Cache>(e).unwrap().cache = 17;
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save().unwrap(), bytes);
    w.load(&bytes).unwrap();
    assert_eq!(w.get::<Cache>(e).unwrap().cache, 0);
    assert_eq!(w.hash(), hash);
}

#[test]
fn empty_columns_are_not_continuation_state() {
    let mut clean = World::new(60, 7);
    let mut churned = World::new(60, 7);
    clean.register::<Health>().unwrap();
    churned.register::<Health>().unwrap();
    let e = clean.spawn(()).unwrap();
    assert_eq!(e, churned.spawn(()).unwrap());
    churned.insert(e, Health { hp: 42 }).unwrap();
    churned.remove::<Health>(e);
    assert_eq!(clean.hash(), churned.hash());
    assert_eq!(clean.save().unwrap(), churned.save().unwrap());
    churned.insert(e, Health { hp: 23 }).unwrap();
    assert_ne!(clean.hash(), churned.hash()); // negative control: populated columns count
}

#[test]
fn unregistered_insert_is_refused_without_mutation() {
    let mut w = World::new(60, 0);
    let e = w.spawn(()).unwrap();
    let before = w.save().unwrap();
    assert!(w
        .insert(e, Health { hp: 7 })
        .unwrap_err()
        .to_string()
        .contains("Health"));
    assert_eq!(w.save().unwrap(), before);
}

#[test]
fn registration_errors_name_types_and_panics_poison_partial_hooks() {
    use exact_world::DataError;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    #[derive(Default, Data)]
    struct Hook;
    impl Component for Hook {
        const NAME: &'static str = "Hook";
        fn register(w: &mut World) -> Result<(), DataError> {
            w.register::<Health>()?;
            panic!("partial hook");
        }
    }
    let mut w = World::new(60, 0);
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = w.register::<Hook>();
    }))
    .is_err());
    assert!(w.validate().is_err());
    assert!(w.register::<Hook>().is_err());
    let mut w = World::new(60, 0);
    let before = w.save().unwrap();
    assert!(w
        .spawn(Health::default())
        .unwrap_err()
        .to_string()
        .contains("Health"));
    assert_eq!(w.save().unwrap(), before);
    w.register::<Health>()
        .unwrap()
        .register::<Health>()
        .unwrap();
    let e = w.spawn(Health { hp: 3 }).unwrap();
    assert_eq!(w.get::<Health>(e).unwrap().hp, 3);
    w.register::<exact_world::Parent>().unwrap();
    assert!(w.insert(e, exact_world::Parent::default()).is_err());
}

#[test]
fn exact_load_refuses_incompatible_fields_before_replacing_live_state() {
    #[derive(Default, Data)]
    struct Before {
        points: u32,
    }
    #[derive(Default, Data)]
    struct After {
        score: u32,
    }
    impl Component for Before {
        const NAME: &'static str = "Counter";
    }
    impl Component for After {
        const NAME: &'static str = "Counter";
    }
    let mut a = World::new(60, 0);
    a.register::<Before>().unwrap();
    a.spawn(Before { points: 123 }).unwrap();
    let mut b = World::new(60, 0);
    b.register::<After>().unwrap();
    b.spawn(After { score: 7 }).unwrap();
    let before = b.save().unwrap();
    assert!(b.load(&a.save().unwrap()).is_err());
    assert_eq!(b.save().unwrap(), before);
}
