use super::*;
#[derive(Default, crate::Component)]
struct Probe {
    value: u32,
    held: u32,
    #[data(skip)]
    skipped: u32,
}
#[test]
fn applied_and_kept_are_observable_and_patch_excludes_skipped_state() {
    let mut base = World::new(60, 0);
    base.spawn_named(
        "probe",
        Probe {
            value: 1,
            held: 2,
            skipped: 44,
        },
    );
    let bytes = base.initializer().unwrap();
    let mut theirs = World::new(60, 0);
    theirs.spawn_named(
        "probe",
        Probe {
            value: 3,
            held: 4,
            skipped: 55,
        },
    );
    base.get_mut::<Probe>("probe").unwrap().held = 9;
    let report = base
        .merge_initializer(&bytes, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert_eq!(report.applied.len(), 1);
    assert_eq!(report.kept.len(), 1);
    let p = base.get::<Probe>("probe").unwrap();
    assert_eq!((p.value, p.held, p.skipped), (3, 9, 44));
}

#[derive(Default, crate::Data)]
struct Nested {
    number: u32,
    #[data(skip)]
    secret: u32,
}
#[derive(Default, crate::Component)]
struct Containers {
    nested: Nested,
    list: Vec<Nested>,
    optional: Option<Nested>,
    array: [Nested; 1],
}
#[test]
fn nested_skipped_fields_are_neither_read_nor_reset_by_patches() {
    let make = |number, secret| Containers {
        nested: Nested { number, secret },
        list: vec![Nested { number, secret }],
        optional: Some(Nested { number, secret }),
        array: [Nested { number, secret }],
    };
    let mut mine = World::new(60, 0);
    mine.spawn_named("nested", make(1, 123));
    let base = mine.initializer().unwrap();
    mine.get_mut::<Containers>("nested").unwrap().nested.secret = 456;
    assert_eq!(base, mine.initializer().unwrap()); // even a changed secret is invisible
    let mut theirs = World::new(60, 0);
    theirs.spawn_named("nested", make(2, 789));
    let report = mine
        .merge_initializer(&base, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert_eq!(report.applied.len(), 4);
    let c = mine.get::<Containers>("nested").unwrap();
    assert_eq!((c.nested.number, c.nested.secret), (2, 456));
    for n in [&c.list[0], c.optional.as_ref().unwrap(), &c.array[0]] {
        assert_eq!((n.number, n.secret), (2, 123));
    }
}

#[test]
fn random_carried_worlds_and_edits_obey_identity_ownership_and_determinism() {
    let mut rng = crate::Rng::new(103);
    for _ in 0..128 {
        let mut mine = World::new(60, 0);
        let mut theirs = World::new(60, 0);
        let mut expected = Vec::new();
        let count = rng.range(1..50u32);
        for i in 0..count {
            let value = rng.range(0..20u32);
            mine.spawn_named(
                format!("e{i}"),
                Probe {
                    value,
                    held: value,
                    skipped: 1,
                },
            );
            let new = rng.range(0..20u32);
            theirs.spawn_named(
                format!("e{i}"),
                Probe {
                    value: new,
                    held: new,
                    skipped: 2,
                },
            );
            expected.push((value, new, rng.range(0..20u32)));
        }
        let base = mine.initializer().unwrap();
        for (i, (_, _, held)) in expected.iter().enumerate() {
            mine.get_mut::<Probe>(format!("e{i}").as_str())
                .unwrap()
                .held = *held;
        }
        let saved = mine.save();
        let identity = mine.merge_initializer(&base, &base, None).unwrap();
        assert!(identity.applied.is_empty() && identity.kept.is_empty());
        assert_eq!(saved, mine.save());
        let mut other = World::new(60, 0);
        other.register::<Probe>();
        other.load(&saved).unwrap();
        let edits = theirs.initializer().unwrap();
        let report = mine.merge_initializer(&base, &edits, None).unwrap();
        let second = other.merge_initializer(&base, &edits, None).unwrap();
        assert_eq!(mine.hash(), other.hash());
        assert_eq!(mine.save(), other.save());
        assert_eq!(report.json(), second.json());
        for (i, (b, t, m)) in expected.iter().enumerate() {
            let p = mine.get::<Probe>(format!("e{i}").as_str()).unwrap();
            assert_eq!(p.value, *t);
            assert_eq!(p.held, if b == m { *t } else { *m });
            assert_eq!(p.skipped, 1);
        }
        for item in &report.applied {
            let p = mine.get::<Probe>(item.entity.as_str()).unwrap();
            let value = if item.field == "value" {
                p.value
            } else {
                p.held
            };
            assert_eq!(value.to_string(), item.new);
        }
        for item in &report.kept {
            let index: usize = item.entity[1..].parse().unwrap();
            assert_eq!(
                mine.get::<Probe>(item.entity.as_str()).unwrap().held,
                expected[index].2
            );
        }
    }
}

#[derive(crate::Kind)]
struct KindProbe {
    probe: Probe,
}
#[derive(Default, crate::Component)]
struct Links {
    entity: Entity,
    typed: crate::Id<KindProbe>,
}
#[test]
fn references_compare_names_across_interleaved_spawn_order_and_remap_when_applied() {
    let mut mine = World::new(60, 0);
    let a = mine.spawn_kind(
        "a",
        KindProbe {
            probe: Probe::default(),
        },
    );
    let b = mine.spawn_kind(
        "b",
        KindProbe {
            probe: Probe::default(),
        },
    );
    mine.spawn_named(
        "link",
        Links {
            entity: a.entity(),
            typed: a,
        },
    );
    let base = mine.initializer().unwrap();
    let mut theirs = World::new(60, 0);
    let bt = theirs.spawn_kind(
        "b",
        KindProbe {
            probe: Probe::default(),
        },
    );
    let at = theirs.spawn_kind(
        "a",
        KindProbe {
            probe: Probe::default(),
        },
    );
    theirs.spawn_named(
        "link",
        Links {
            entity: at.entity(),
            typed: at,
        },
    );
    let unchanged = mine
        .merge_initializer(&base, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert!(unchanged.applied.is_empty() && unchanged.kept.is_empty());
    *theirs.get_mut::<Links>("link").unwrap() = Links {
        entity: bt.entity(),
        typed: bt,
    };
    let report = mine
        .merge_initializer(&base, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert_eq!(report.applied.len(), 2);
    let links = mine.get::<Links>("link").unwrap();
    assert_eq!(links.entity, b.entity());
    assert_eq!(links.typed, b);
    assert_eq!(report.applied[0].old, "\"a\"");
    assert_eq!(report.applied[0].new, "\"b\"");
}

#[test]
fn structural_edits_are_reported_without_spawning_or_guessing_and_report_is_bounded() {
    let mut mine = World::new(60, 0);
    mine.spawn_named("removed", Probe::default());
    mine.spawn_named("component", (Probe::default(), crate::Transform::default()));
    let unnamed = mine.spawn(Probe::default());
    let churn = mine.spawn(Probe::default());
    let base = mine.initializer().unwrap();
    mine.despawn(churn);
    mine.spawn(Probe {
        value: 91,
        ..Default::default()
    });
    let mut theirs = World::new(60, 0);
    theirs.spawn_named("added", Probe::default());
    theirs.spawn_named("component", (Probe::default(), crate::Material::default()));
    theirs.spawn((
        Probe {
            value: 3,
            ..Default::default()
        },
        crate::Transform::default(),
    ));
    theirs.spawn(Probe {
        value: 3,
        ..Default::default()
    });
    let saved = mine.save();
    let report = mine
        .merge_initializer(&base, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert_eq!(report.added.len(), 2);
    assert_eq!(report.removed.len(), 2);
    assert_eq!(report.unmatched.len(), 2);
    assert_eq!(mine.get::<Probe>(unnamed).unwrap().value, 0);
    assert_eq!(mine.save(), saved);
    let base = theirs.initializer().unwrap();
    for i in 0..100 {
        theirs.spawn_named(format!("extra{i}"), Probe::default());
    }
    let report = mine
        .merge_initializer(&base, &theirs.initializer().unwrap(), None)
        .unwrap();
    assert_eq!(report.added.len() + report.unmatched.len(), 64);
    assert!(report.omitted >= 36);
    report.log(&mine);
    assert!(mine.journal().last().unwrap().line.contains("omitted"));
}

#[test]
fn over_budget_initializer_refuses_explicitly() {
    let mut world = World::new(60, 0);
    // Dead slots count as work too: churn cannot circumvent the entity-slot bound.
    world
        .state
        .slots
        .resize_with(MAX_ENTITIES + 1, Slot::default);
    let error = world.initializer().unwrap_err();
    assert!(error.message.contains("250000"));
    let mut work = Work::default();
    work.charge(256 * 1024 * 1024 + 1);
    assert!(work.check().unwrap_err().message.contains("work limit"));
}

#[test]
#[ignore = "isolated worst-case reload timing and RSS diagnostic"]
fn interleaved_200k_churn_reload_cost() {
    let start = std::time::Instant::now();
    let mut mine = World::new(60, 0);
    let mut theirs = World::new(60, 0);
    for i in 0..200_000 {
        mine.spawn_named(
            format!("e{i}"),
            Probe {
                value: 1,
                held: 1,
                skipped: 7,
            },
        );
    }
    let base = mine.initializer().unwrap();
    for i in 0..200_000 {
        // Reverse spawn order and interleave component sets; never align by storage row.
        let e = theirs.spawn_named(
            format!("e{}", 199_999 - i),
            Probe {
                value: 2,
                held: 2,
                skipped: 8,
            },
        );
        if i % 2 == 0 {
            theirs.insert(e, crate::Transform::default());
        }
    }
    for i in (0..200_000).step_by(3) {
        let e = mine.named(&format!("e{i}")).unwrap();
        mine.despawn(e);
        mine.spawn_named(
            format!("e{i}"),
            Probe {
                value: 1,
                held: 9,
                skipped: 7,
            },
        );
    }
    let edits = theirs.initializer().unwrap();
    let prepare = start.elapsed();
    let start = std::time::Instant::now();
    let report = mine.merge_initializer(&base, &edits, None).unwrap();
    let elapsed = start.elapsed();
    for i in 0..200_000 {
        let p = mine.get::<Probe>(format!("e{i}").as_str()).unwrap();
        assert_eq!(p.value, 2);
        assert_eq!(p.held, if i % 3 == 0 { 9 } else { 2 });
        assert_eq!(p.skipped, 7);
    }
    assert_eq!(report.omitted, 500_000 - 64);
    println!(
        "T2 worst 200000: base={} B new={} B prepare={prepare:?} merge={elapsed:?} hash={:016x}",
        base.len(),
        edits.len(),
        mine.hash()
    );
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        println!(
            "{}",
            status.lines().find(|l| l.starts_with("VmHWM:")).unwrap()
        );
    }
}
