use exact_game::{Component, Data, Resource, Transform, World};

#[derive(Default, Component)]
struct Health {
    hp: u32,
}
#[derive(Default, Resource)]
struct Score(u64);
#[test]
fn round_trip_preserves_ids_names_rng_resources_and_registration_order() {
    let mut w = World::new(144, 8123);
    w.register::<Health>()
        .register::<Transform>()
        .register_resource::<Score>();
    let e = w.spawn_named("fox", (Transform::at(1.0, 2.0, 3.0), Health { hp: 15 }));
    let dead = w.spawn_named("dead", (Health { hp: 4 },));
    w.despawn(dead);
    w.insert_resource(Score(79));
    w.rng().next_u32();
    w.propagate();
    let bytes = w.save();
    let hash = w.hash();
    let mut loaded = World::new(30, 1);
    loaded
        .register_resource::<Score>()
        .register::<Transform>()
        .register::<Health>();
    loaded.load(&bytes).unwrap();
    assert_eq!(hash, loaded.hash());
    assert_eq!(bytes, loaded.save());
    assert_eq!(loaded.named("fox"), Some(e));
    assert_eq!(loaded.name(e), Some("fox"));
    assert_eq!(loaded.resource::<Score>().0, 79);
    assert_eq!(
        loaded.global(e).unwrap().translation,
        exact_game::Vec3::new(1.0, 2.0, 3.0).into()
    );
    for _ in 0..20 {
        assert_eq!(w.rng().next_u32(), loaded.rng().next_u32());
    }
    assert_eq!(w.spawn(()), loaded.spawn(()));
}
#[test]
fn load_failure_is_atomic_and_names_unknown_types() {
    let mut source = World::new(60, 9);
    source.spawn((Health { hp: 7 },));
    let mut target = World::new(30, 2);
    target.spawn_named("keep", ());
    let before = target.save();
    let error = target.load(&source.save()).unwrap_err().to_string();
    assert!(
        error.contains("Health") && error.contains("unregistered"),
        "{error}"
    );
    assert_eq!(before, target.save());
    target.register::<Health>();
    let b = source.save();
    for i in 0..b.len() {
        assert!(target.load(&b[..i]).is_err());
        assert_eq!(before, target.save());
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
    let e = old.spawn((Old {
        hp: 7,
        obsolete: vec!["discard".into()],
    },));
    let mut new = World::new(60, 0);
    new.register::<New>();
    new.load(&old.save()).unwrap();
    assert_eq!(new.get::<New>(e).unwrap().hp, 7);
    assert_eq!(new.get::<New>(e).unwrap().added, 123);
    old.load(&new.save()).unwrap();
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
    let e = w.spawn((Cache {
        value: 2,
        cache: 42,
    },));
    let hash = w.hash();
    let bytes = w.save();
    w.get_mut::<Cache>(e).unwrap().cache = 17;
    assert_eq!(w.hash(), hash);
    assert_eq!(w.save(), bytes);
    w.load(&bytes).unwrap();
    assert_eq!(w.get::<Cache>(e).unwrap().cache, 0);
    assert_eq!(w.hash(), hash);
}
#[test]
fn engine_scene_components_first_spawned_mid_game_restore_unregistered() {
    // Grow a Garden's restore refused `unregistered component Parent` until the
    // game registered the engine's own component (garden diary, friction 3).
    let mut w = World::new(60, 3);
    let root = w.spawn(Transform::at(1.0, 0.0, 0.0));
    let fruit = w.spawn((
        Transform::at(0.0, 2.0, 0.0),
        exact_game::Parent(root),
        exact_game::Mesh::Sphere { radius: 0.2 },
        exact_game::Material::default(),
        exact_game::Visible(true),
        exact_game::Ambient,
    ));
    // The lights, the viewmodel and mouse look later lanes added restore the same way.
    w.spawn((
        Transform::default(),
        exact_game::SpotLight::default(),
        exact_game::LightShadows,
        exact_game::ViewModel,
        exact_game::Opacity(0.5),
        exact_game::MouseLook::default(),
    ));
    w.propagate();
    let mut fresh = World::new(60, 0);
    fresh.load(&w.save()).unwrap();
    assert_eq!(fresh.hash(), w.hash());
    assert_eq!(
        fresh.global(fruit).unwrap().translation,
        exact_game::Vec3::new(1.0, 2.0, 0.0).into()
    );
}
