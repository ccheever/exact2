use exact_game::*;

#[derive(Default, Component)]
struct Gait(f32);
#[derive(Default, Args)]
struct Settings {
    count: u32,
    stride: u32,
    #[live]
    dressed: bool,
}
struct Town;
impl Game for Town {
    const ID: &'static str = "node-offsets-proof";
    type Args = Settings;
    fn setup(w: &mut World, a: &Settings) {
        for i in 0..a.count * a.stride {
            let e = w.spawn(Transform::default());
            if i % a.stride == 0 {
                w.insert(e, Gait(i as f32));
            }
        }
    }
    fn tick(_: &mut World, _: &Input, _: &Settings) {}
    fn present(p: &mut Present<'_>, a: &Settings) {
        p.insert_each::<Gait, NodeOffsets>(|reads, e, gait| {
            assert!(reads.has::<Transform>(e));
            assert_eq!(reads.require::<Gait>(e).0, gait.0);
            a.dressed.then(|| {
                NodeOffsets::default().with(
                    NodeName::new("arm"),
                    Transform::at(0., math::sin(gait.0 + reads.tick() as f32), 0.),
                )
            })
        });
    }
}
#[test]
fn offsets_are_write_only_bounded_and_do_not_change_simulation() {
    let args = |dressed| Settings {
        count: 30,
        stride: 1,
        dressed,
    };
    let mut bare = Sim::<Town>::new(args(false)).unwrap();
    let mut dressed = Sim::<Town>::new(args(true)).unwrap();
    bare.run(500.);
    dressed.run(500.);
    assert_eq!(bare.world().hash(), dressed.world().hash());
    // Live arguments travel in the Sim save; World bytes exclude presentation.
    assert_eq!(bare.world().save(), dressed.world().save());
    let mut paranoid = Sim::<Town>::new(args(true))
        .unwrap()
        .paranoid(Paranoid::Save);
    paranoid.run(500.);
    assert_eq!(paranoid.save().unwrap(), dressed.save().unwrap());
    assert_eq!(dressed.world().query::<&NodeOffsets>().iter().count(), 30);
}
#[test]
fn row_replacement_capacity_and_name_data() {
    let mut rows = NodeOffsets::default();
    let names = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
    ];
    for name in names {
        rows = rows.with(NodeName::new(name), Transform::default());
    }
    assert_eq!(rows.rows().len(), 16);
    assert!(rows.overflowed());
    rows = rows.with(NodeName::new("a"), Transform::at(2., 0., 0.));
    assert_eq!(rows.rows()[0].transform.position.x, 2.);
    assert_eq!(json::to_string(&NodeName::new("arm")).unwrap(), "\"arm\"");
    assert!(bin::from_slice::<NodeName>(&bin::to_vec(&NodeName::new("arm"))).is_err());
}
#[test]
fn retained_pages_reclaim_after_sixty_empty_presents() {
    for (count, stride) in [(30, 1), (1000, 1), (30, PAGE as u32), (1000, 17)] {
        let mut sim = Sim::<Town>::new(Settings {
            count,
            stride,
            dressed: true,
        })
        .unwrap();
        sim.run(50.);
        let live = sim.world().presentation_bytes();
        sim.run(50.);
        assert_eq!(sim.world().presentation_bytes(), live);
        let saved = sim.world().save();
        let entities: Vec<_> = sim
            .world()
            .query::<&Gait>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        for &e in &entities {
            sim.world_mut().remove::<Gait>(e);
        }
        // Reappear inside the retention window, then exercise a full empty
        // window to prove that the intervening rows reset the idle count.
        for _ in 0..59 {
            let _ = sim.world_mut();
            sim.run(0.);
        }
        assert_eq!(sim.world().presentation_bytes(), live);
        for &e in &entities {
            sim.world_mut().insert(e, Gait(0.));
        }
        sim.run(0.);
        assert_eq!(sim.world().presentation_bytes(), live);
        sim.run(0.);
        for &e in &entities {
            sim.world_mut().remove::<Gait>(e);
        }
        for _ in 0..60 {
            let _ = sim.world_mut();
            sim.run(0.);
        }
        assert_eq!(sim.world().presentation_bytes(), live);
        let _ = sim.world_mut();
        sim.run(0.);
        let empty = sim.world().presentation_bytes();
        assert!(empty < live / 8, "payload reclaimed: {live} -> {empty}");
        println!("{count} avatars stride {stride}: live {live} bytes; after churn {empty} bytes; row {} bytes", std::mem::size_of::<NodeOffsets>());
        sim.world_mut().load(&saved).unwrap();
        sim.run(50.);
        assert_eq!(sim.world().presentation_bytes(), live);
    }
}

#[test]
fn offsets_refuse_all_decoding_including_partial_records() {
    for input in [r#"{}"#, r#"{"len":17}"#, r#"{"len":255}"#, r#"{"rows":[]}"#] {
        assert!(json::from_str::<NodeOffsets>(input).is_err(), "{input}");
    }
    let rows = NodeOffsets::default().with(NodeName::new("arm"), Transform::default());
    assert!(json::from_str::<NodeOffsets>(&json::to_string(&rows).unwrap()).is_err());
    assert!(bin::from_slice::<NodeOffsets>(&bin::to_vec(&rows)).is_err());
}
