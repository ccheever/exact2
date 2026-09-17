//! Regression cases from Brief A3; compile-time refusals live beside Data/Resource.
use exact_game::*;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Default, Data)]
struct Arrow {
    #[data(skip)]
    f: Option<fn() -> u32>,
    nested: Vec<Vec<u32>>,
    score: u32,
}
#[test]
fn a01_arrow_and_adjacent_generic_closers_keep_following_fields() {
    let value = Arrow {
        f: Some(|| 1),
        nested: vec![vec![3]],
        score: 42,
    };
    let restored: Arrow = bin::from_slice(&bin::to_vec(&value)).unwrap();
    assert_eq!(restored.score, 42);
    assert_eq!(restored.nested, [vec![3]]);
    assert!(restored.f.is_none());
    let restored: Arrow = json::from_str(&json::to_string(&value).unwrap()).unwrap();
    assert_eq!(restored.score, 42);
    assert_ne!(hash::of(&value), hash::of(&Arrow::default()));
}
#[test]
fn a02_expansions_ignore_shadowed_prelude_names() {
    #[allow(dead_code)]
    type Result<T> = Vec<T>;
    #[allow(dead_code)]
    struct Ok;
    #[allow(dead_code)]
    struct Err;
    #[allow(dead_code)]
    struct Some;
    #[derive(Default, Data)]
    enum Choice {
        #[default]
        Idle,
        Record {
            score: u32,
        },
        Tuple(u32),
    }
    #[derive(Default, Component)]
    struct Record {
        score: u32,
    }
    #[derive(Default, Resource)]
    struct Singleton(u32);
    assert_eq!(
        json::from_str::<Record>(r#"{"score":42}"#).unwrap().score,
        42
    );
    assert_eq!(
        bin::from_slice::<Singleton>(&bin::to_vec(&Singleton(7)))
            .unwrap()
            .0,
        7
    );
    for v in [Choice::Idle, Choice::Record { score: 42 }, Choice::Tuple(7)] {
        assert_eq!(
            hash::of(&v),
            hash::of(&bin::from_slice::<Choice>(&bin::to_vec(&v)).unwrap())
        );
    }
    assert!(json::from_str::<Choice>(r#"{"Unknown":{}}"#).is_err());
}
#[derive(Default, Data)]
struct Health {
    hp: u32,
}
#[test]
fn a03_integral_floats_are_integers_with_exact_range_checks() {
    for (json, hp) in [
        (r#"{"hp":15.0}"#, 15),
        (r#"{"hp":1e2}"#, 100),
        (r#"{"hp":-0}"#, 0),
    ] {
        assert_eq!(json::from_str::<Health>(json).unwrap().hp, hp);
    }
    for bad in ["1.5", "-1.0", "4294967296.0", "1e100"] {
        assert!(json::from_str::<u32>(bad).is_err(), "{bad}");
    }
    assert!(json::from_str::<u64>("18446744073709551616.0").is_err());
    assert!(json::from_str::<i64>("9223372036854775808.0").is_err());
    assert_eq!(
        json::from_str::<i64>("-9223372036854775808.0").unwrap(),
        i64::MIN
    );
    assert_eq!(
        json::from_str::<u64>("18446744073709549568.0").unwrap(),
        u64::MAX - 2047
    );
    assert!(bin::from_slice::<u32>(&bin::to_vec(&f32::NAN)).is_err());
    assert_eq!(bin::from_slice::<u8>(&bin::to_vec(&15.0f32)).unwrap(), 15);
}
#[test]
fn a04_saves_refuse_cycles_and_runtime_breaks_the_highest_cycle_entity_once() {
    let mut w = World::new(60, 0);
    let a = w.spawn(Transform::at(1.0, 0.0, 0.0));
    let b = w.spawn((Parent(a), Transform::at(2.0, 0.0, 0.0)));
    let tail = w.spawn((Parent(a), Transform::at(4.0, 0.0, 0.0)));
    w.insert(a, Parent(b));
    let mut loaded = World::new(60, 0);
    loaded.register_scene();
    let before = loaded.save();
    let err = loaded.load(&w.save()).unwrap_err().to_string();
    assert!(
        err.contains("cycle") && err.contains(&format!("#{}", b.index())),
        "{err}"
    );
    assert_eq!(loaded.save(), before);
    w.propagate();
    assert!(!w.has::<Parent>(b));
    assert!(w.has::<Parent>(a));
    assert_eq!(w.global(a).unwrap().translation.x, 3.0);
    assert_eq!(w.global(tail).unwrap().translation.x, 7.0);
    w.propagate();
    assert_eq!(
        w.journal()
            .iter()
            .filter(|e| e.line.contains("cycle"))
            .count(),
        1
    );
    loaded.load(&w.save()).unwrap();
    // A self-cycle without any Transform must also be rejected and repaired.
    let alone = w.spawn(());
    w.insert(alone, Parent(alone));
    assert!(loaded.load(&w.save()).is_err());
    w.propagate();
    assert!(!w.has::<Parent>(alone));
}
#[test]
fn a05_duplicate_record_and_map_keys_refuse_in_both_decoders() {
    let text = r#"{"hp":1,"hp":999}"#;
    assert!(json::from_str::<Health>(text)
        .err()
        .unwrap()
        .to_string()
        .contains("hp"));
    assert!(json::from_str::<BTreeMap<String, u32>>(text).is_err());
    let mut out = bin::Encoder::default();
    out.begin_struct();
    for n in [1u32, 999] {
        out.field("hp");
        n.write(&mut out);
    }
    out.end_struct();
    let bytes = out.finish();
    assert!(bin::from_slice::<Health>(&bytes).is_err());
    assert!(bin::from_slice::<BTreeMap<String, u32>>(&bytes).is_err());
    // Escapes compare by decoded names; unknown fields are checked too.
    assert!(json::from_str::<Health>(r#"{"hp":1,"h\u0070":2}"#).is_err());
    assert!(json::from_str::<Health>(r#"{"future":{"a":1,"a":2}}"#).is_err());
    assert!(json::from_str::<Vec<Health>>(r#"[{"hp":1},{"hp":2}]"#).is_ok());
}
#[test]
fn a06_containers_replace_while_records_patch() {
    let mut map = BTreeMap::from([("a".into(), 1u32), ("b".into(), 2)]);
    json::read_into(r#"{"a":9}"#, &mut map).unwrap();
    assert_eq!(map, BTreeMap::from([("a".into(), 9)]));
    let mut record = Health { hp: 9 };
    json::read_into("{}", &mut record).unwrap();
    assert_eq!(record.hp, 9);
    let mut option = Some(record);
    json::read_into("[{}]", &mut option).unwrap();
    assert_eq!(option.unwrap().hp, 0);
    let mut vector = vec![9u32, 8];
    json::read_into("[1]", &mut vector).unwrap();
    assert_eq!(vector, [1]);
    let mut array = [9u32, 8];
    json::read_into("[1]", &mut array).unwrap();
    assert_eq!(array, [1, 0]);
    let mut tuple = (9u32, 8u32);
    json::read_into("[1]", &mut tuple).unwrap();
    assert_eq!(tuple, (1, 0));
    #[derive(Default, Data)]
    struct Pair(u32, u32);
    let mut pair = Pair(9, 8);
    json::read_into("[1]", &mut pair).unwrap();
    assert_eq!((pair.0, pair.1), (1, 0));
    let mut map = BTreeMap::from([("stale".into(), 1u32)]);
    bin::read_into(
        &bin::to_vec(&BTreeMap::from([("new".into(), 9u32)])),
        &mut map,
    )
    .unwrap();
    assert_eq!(map, BTreeMap::from([("new".into(), 9)]));
}
#[derive(Default, Component)]
struct ZLater(u32);
#[test]
fn a07_panicking_drop_does_not_retire_or_recycle_a_dirty_slot() {
    #[derive(Default, Component)]
    struct ABomb {
        explode: bool,
    }
    impl Drop for ABomb {
        fn drop(&mut self) {
            assert!(!self.explode, "ABomb destructor");
        }
    }
    let mut w = World::new(60, 0);
    let e = w.spawn((ABomb { explode: true }, ZLater(99)));
    assert!(catch_unwind(AssertUnwindSafe(|| w.despawn(e))).is_err());
    assert!(w.contains(e));
    let other = w.spawn(());
    assert_ne!(other.index(), e.index());
    assert!(!w.has::<ZLater>(other));
    assert!(w.despawn(e));
    let reused = w.spawn(());
    assert_eq!(reused.index(), e.index());
    assert!(!w.has::<ZLater>(reused));
}
#[test]
fn a08_huge_counts_and_strings_refuse_without_allocating_the_claim() {
    use exact_game::data::{MAX_LOAD_BYTES, MAX_LOAD_STRING};
    fn var(mut n: u64) -> Vec<u8> {
        let mut out = vec![];
        while n >= 128 {
            out.push(n as u8 | 128);
            n >>= 7;
        }
        out.push(n as u8);
        out
    }
    let mut seq = vec![7];
    seq.extend(var(MAX_LOAD_BYTES as u64 + 1));
    assert!(bin::from_slice::<Vec<u8>>(&seq).is_err());
    let mut string = vec![6];
    string.extend(var(MAX_LOAD_STRING as u64 + 1));
    let err = bin::from_slice::<String>(&string).unwrap_err().to_string();
    assert!(err.contains("string exceeds"), "{err}");
}
#[test]
fn a09_fifty_thousand_freed_slots_reuse_lowest_first_after_save() {
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..100_000).map(|_| w.spawn(())).collect();
    for &e in entities.iter().step_by(2).rev() {
        w.despawn(e);
    }
    let mut restored = World::new(60, 0);
    restored.load(&w.save()).unwrap();
    assert_eq!(w.save(), restored.save());
    for old in entities.iter().step_by(2) {
        let e = w.spawn(());
        assert_eq!(e, restored.spawn(()));
        assert_eq!(e.index(), old.index());
        assert_eq!(e.generation(), old.generation() + 1);
    }
}
#[test]
fn a10_roots_are_local_parent_chains_reuse_scratch_and_fresh_is_per_tick() {
    struct Moving;
    impl Game for Moving {
        fn setup(w: &mut World, _: &Args) -> Result<(), String> {
            w.spawn_named("root", Transform::at(1.0, 0.0, 0.0));
            Ok(())
        }
        fn tick(w: &mut World, _: &Input) {
            assert!(w.fresh().is_empty());
            if w.tick() == 0 {
                let e = w.named("root").unwrap();
                w.teleport(e, Transform::at(3.0, 0.0, 0.0));
                w.spawn(Transform::default());
            }
        }
    }
    let mut s = Sim::<Moving>::new(&[]).unwrap();
    let root = s.world().named("root").unwrap();
    assert_eq!(s.world().fresh(), [root]);
    s.advance(0.0, Clock::Seekable);
    s.advance(16.667, Clock::Seekable);
    assert_eq!(s.world().fresh().len(), 2);
    assert_eq!(s.world().global(root).unwrap().translation.x, 3.0);
    s.advance(33.334, Clock::Seekable);
    assert!(s.world().fresh().is_empty());
    let w = s.world_mut();
    w.get_mut::<Transform>(root).unwrap().position.x = 9.0;
    assert_eq!(w.global(root).unwrap().translation.x, 9.0);
    let child = w.spawn((Parent(root), Transform::at(2.0, 0.0, 0.0)));
    w.propagate();
    assert_eq!(w.global(child).unwrap().translation.x, 11.0);
    w.remove::<Parent>(child);
    assert_eq!(w.global(child).unwrap().translation.x, 2.0);
    w.despawn(root);
    assert!(w.global(root).is_none());
    w.teleport(root, Transform::default());
    assert!(!w.contains(root));
}
#[test]
fn a11_orphan_chains_leave_after_game_tick_in_entity_order() {
    struct Reaper;
    impl Game for Reaper {
        fn setup(w: &mut World, _: &Args) -> Result<(), String> {
            let leaf = w.spawn_named("leaf", ());
            let middle = w.spawn_named("middle", ());
            let root = w.spawn_named("root", ());
            w.insert(leaf, Parent(middle));
            w.insert(middle, Parent(root));
            Ok(())
        }
        fn tick(w: &mut World, _: &Input) {
            let root = w.named("root").unwrap();
            w.despawn(root);
            assert_eq!(w.len(), 2);
        }
    }
    let mut s = Sim::<Reaper>::new(&[]).unwrap();
    s.advance(0.0, Clock::Seekable);
    s.advance(16.667, Clock::Seekable);
    assert!(s.world().is_empty());
    let lines: Vec<_> = s
        .world()
        .journal()
        .into_iter()
        .filter(|e| e.line.contains("despawn"))
        .map(|e| e.line)
        .collect();
    assert_eq!(
        lines,
        [
            "tick=0 despawn #2",
            "tick=0 despawn #1",
            "tick=0 despawn #0"
        ]
    );
}
#[test]
#[should_panic(expected = "query exceeds 4 filters at ZLater")]
fn a12_fifth_filter_panics_by_name() {
    World::new(60, 0)
        .query::<&ZLater>()
        .without::<ZLater>()
        .without::<ZLater>()
        .without::<ZLater>()
        .without::<ZLater>()
        .without::<ZLater>();
}
#[test]
fn a13_resource_derive_has_its_own_registration_and_storage() {
    #[derive(Default, Resource)]
    struct Score(u32);
    let mut w = World::new(60, 0);
    w.insert_resource(Score(9));
    let mut loaded = World::new(60, 0);
    loaded.register_resource::<Score>();
    loaded.load(&w.save()).unwrap();
    assert_eq!(loaded.resource::<Score>().0, 9);
}
#[test]
fn a14_insert_and_remove_are_lenient_for_dead_incarnations() {
    let mut w = World::new(60, 0);
    let e = w.spawn(());
    w.despawn(e);
    let new = w.spawn(());
    assert!(!w.insert(e, ZLater(1)));
    assert!(w.remove::<ZLater>(e).is_none());
    assert!(!w.has::<ZLater>(new));
    assert!(w.insert(new, ZLater(2)));
    assert_eq!(w.get::<ZLater>(new).unwrap().0, 2);
}
#[test]
fn a15_mutable_guard_locks_the_whole_column_and_options_are_arrays() {
    let mut w = World::new(60, 0);
    let a = w.spawn(ZLater(1));
    let b = w.spawn(ZLater(2));
    let guard = w.get_mut::<ZLater>(a).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| w.get::<ZLater>(b))).is_err());
    drop(guard);
    assert_eq!(w.get::<ZLater>(b).unwrap().0, 2);
    assert_eq!(json::to_string(&None::<u32>).unwrap(), "[]");
    assert_eq!(json::to_string(&Some(7u32)).unwrap(), "[7]");
}
#[test]
fn a16_short_random_draws_work_inside_spawn_and_preserve_the_stream() {
    let mut w = World::new(60, 42);
    let mut rng = Rng::new(42);
    let e = w.spawn(Transform::at(w.rand(3.0..12.0), 0.0, w.rand(-12.0..8.0)));
    assert_eq!(
        w.get::<Transform>(e).unwrap().position,
        Vec3::new(rng.range(3.0..12.0), 0.0, rng.range(-12.0..8.0))
    );
    assert_eq!(w.rand(-5i32..10), rng.range(-5i32..10));
    assert_eq!(w.rand(0u64..u64::MAX), rng.range(0u64..u64::MAX));
    assert_eq!(w.chance(0.5), rng.chance(0.5));
    let items = [1, 2, 3];
    assert_eq!(w.pick(&items), rng.pick(&items));
    assert_eq!(w.pick::<u8>(&[]), None);
    assert_eq!(w.rng().next_u32(), rng.next_u32());
}
#[test]
fn a17_material_builders_preserve_defaults_and_each_other() {
    let m = Material::rgb(0.8, 0.45, 0.15)
        .emissive(1.0, 2.0, 3.0)
        .metallic(0.5)
        .rough(0.3)
        .alpha(0.7);
    assert_eq!(
        m,
        Material {
            color: [0.8, 0.45, 0.15, 0.7],
            emissive: [1.0, 2.0, 3.0],
            metallic: 0.5,
            roughness: 0.3,
            pad: 0.0
        }
    );
    assert_eq!(Material::rgb(1.0, 1.0, 1.0), Material::default());
}
#[test]
fn a18_spawn_named_takes_owned_or_borrowed_text() {
    let mut w = World::new(60, 0);
    let e = w.spawn_named(format!("crate-{}", 1), ());
    assert_eq!(w.named("crate-1"), Some(e));
}
#[test]
fn a19_integer_argument_and_default_bind_refuse_by_name() {
    struct Seeded;
    impl Game for Seeded {
        const ARGS: &'static [&'static str] = &["seed"];
        fn setup(w: &mut World, args: &Args) -> Result<(), String> {
            w.reseed(args.integer(0, "seed")?);
            Ok(())
        }
        fn tick(_: &mut World, _: &Input) {}
    }
    for n in [-1.0, 1.5, 9_007_199_254_740_992.0, f64::INFINITY] {
        assert!(Sim::<Seeded>::new(&[Value::Number(n)])
            .err()
            .unwrap()
            .contains("seed"));
    }
    assert_eq!(
        Sim::<Seeded>::new(&[Value::Number(-0.0)])
            .unwrap()
            .world()
            .seed(),
        0
    );
    assert_eq!(
        Sim::<Seeded>::new(&[Value::Number(9_007_199_254_740_991.0)])
            .unwrap()
            .world()
            .seed(),
        9_007_199_254_740_991
    );
    let mut w = World::new(60, 0);
    assert!(Seeded::bind(&mut w, &Args::default())
        .unwrap_err()
        .contains("seed"));
    let extra: Args = json::from_str(r#"[[{"Number":[1]},{"Number":[2]}]]"#).unwrap();
    assert!(Seeded::bind(&mut w, &extra).unwrap_err().contains("seed"));
}
#[test]
fn a20_one_returns_a_leased_row_and_debug_refuses_ambiguity() {
    let mut w = World::new(60, 0);
    assert!(w.query::<&ZLater>().one().is_none());
    let e = w.spawn(ZLater(1));
    {
        let mut q = w.query::<&mut ZLater>();
        let (id, row) = q.one().unwrap();
        assert_eq!(id, e);
        row.0 = 2;
    }
    assert_eq!(w.get::<ZLater>(e).unwrap().0, 2);
    w.spawn(ZLater(3));
    #[cfg(debug_assertions)]
    {
        let err = catch_unwind(AssertUnwindSafe(|| {
            w.query::<&ZLater>().one();
        }))
        .unwrap_err();
        assert!(err.downcast_ref::<String>().unwrap().contains("ZLater"));
    }
    #[cfg(not(debug_assertions))]
    assert_eq!(w.query::<&ZLater>().one().unwrap().0, e);
}
