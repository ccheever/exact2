use exact_game::*;

#[derive(Kind)]
struct Lamp {
    #[read]
    transform: Transform,
    material: Material,
    light: Option<PointLight>,
}
#[derive(Kind)]
struct Bulb {
    light: PointLight,
}

fn lamp(x: f32) -> Lamp {
    Lamp {
        transform: Transform::at(x, 0.0, 0.0),
        material: Material::default(),
        light: None,
    }
}
fn generations<C: Component>(w: &World) -> Vec<u64> {
    w.pages::<C>().iter().map(|p| p.generation).collect()
}

#[test]
fn reads_preserve_epoch_every_page_and_wire_state() {
    let mut w = World::new(60, 7);
    for x in 0..(2 * PAGE + 3) {
        let id = w.spawn_kind(format!("lamp-{x}"), lamp(x as f32));
        if x % 2 == 0 {
            w.insert(id.entity(), PointLight::default());
        }
    }
    let before = (
        w.mutation_epoch(),
        generations::<Transform>(&w),
        generations::<Material>(&w),
        generations::<PointLight>(&w),
        w.save(),
        w.hash(),
    );
    let id = w.bind::<Lamp>("lamp-0").unwrap();
    let a = w.row(id).unwrap();
    let b = w.row(id).unwrap();
    assert_eq!(a.transform.position, b.transform.position);
    assert!(a.light.is_some());
    assert_eq!(w.row(id).unwrap().transform.position.x, 0.0);
    for (i, row) in w.rows::<Lamp>().enumerate() {
        assert_eq!(row.id.entity().index() as usize, i);
        assert_eq!(row.light.is_some(), i % 2 == 0);
    }
    assert_eq!(
        before,
        (
            w.mutation_epoch(),
            generations::<Transform>(&w),
            generations::<Material>(&w),
            generations::<PointLight>(&w),
            w.save(),
            w.hash()
        )
    );
}

#[test]
fn spawn_and_typed_ids_have_exactly_the_old_representation() {
    let mut typed = World::new(60, 7);
    let id = typed.spawn_kind("lamp", lamp(2.0));
    let mut raw = World::new(60, 7);
    let entity = raw.spawn_named("lamp", (Transform::at(2.0, 0.0, 0.0), Material::default()));
    assert_eq!(typed.save(), raw.save());
    assert_eq!(typed.hash(), raw.hash());
    assert_eq!(
        json::to_string(&id).unwrap(),
        json::to_string(&entity).unwrap()
    );
    assert_eq!(bin::to_vec(&id), bin::to_vec(&entity));
    let decoded: Id<Lamp> = bin::from_slice(&bin::to_vec(&entity)).unwrap();
    assert_eq!(decoded, id);
    let epoch = typed.mutation_epoch();
    assert_eq!(typed.the::<Lamp>(), id);
    assert_eq!(typed.mutation_epoch(), epoch);
}

#[test]
fn mutable_rows_share_one_column_lease_and_can_escape_the_iterator() {
    let mut w = World::new(60, 0);
    let first = w.spawn_kind("a", lamp(0.0));
    let second = w.spawn_kind("b", lamp(1.0));
    let pose_pages = generations::<Transform>(&w);
    let revision = w.revision::<Material>();
    let mut rows = w.rows_mut::<Lamp>();
    let mut a = rows.next().unwrap();
    let mut b = rows.next().unwrap();
    drop(rows);
    a.material.emissive = [1.0; 3];
    b.material.emissive = [2.0; 3];
    assert_eq!(w.revision::<Material>(), revision + 1);
    assert_eq!(generations::<Transform>(&w), pose_pages);
    drop((a, b));
    w.edit(first, |row| row.material.roughness = 0.7);
    w.edit(second, |row| row.material.roughness = 0.3);
    assert_eq!(w.row(first).unwrap().material.emissive, [1.0; 3]);
}

#[test]
fn checked_binding_rejects_missing_components_and_recycled_handles() {
    let mut w = World::new(60, 0);
    let id = w.spawn_kind("lamp", lamp(0.0));
    w.remove::<Material>(id.entity());
    let error = w.row(id).err().unwrap().to_string();
    assert!(
        error.contains("Lamp") && error.contains("Material"),
        "{error}"
    );
    let before = (w.mutation_epoch(), generations::<Transform>(&w));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.edit(id, |_| ()))).is_err());
    assert_eq!(before, (w.mutation_epoch(), generations::<Transform>(&w)));
    w.despawn(id.entity());
    let replacement = w.spawn_kind("lamp", lamp(1.0));
    assert_eq!(id.entity().index(), replacement.entity().index());
    assert!(w.row(id).is_err());
    assert!(w.row(replacement).is_ok());
}

#[test]
fn children_validate_membership_and_parent_without_mutation() {
    let mut w = World::new(60, 0);
    let parent = w.spawn_kind("lamp", lamp(0.0));
    let bulb = w.spawn_kind(
        "lamp/bulb",
        Bulb {
            light: PointLight::default(),
        },
    );
    assert!(w.child::<Bulb>(parent, "bulb").is_err());
    w.insert(bulb.entity(), Parent(parent.entity()));
    let epoch = w.mutation_epoch();
    assert_eq!(w.child::<Bulb>(parent, "bulb").unwrap(), bulb);
    assert_eq!(epoch, w.mutation_epoch());
    w.remove::<PointLight>(bulb.entity());
    let error = w.child::<Bulb>(parent, "bulb").unwrap_err().to_string();
    assert!(error.contains("lamp/bulb") && error.contains("PointLight"));
}

#[test]
#[should_panic(expected = "expected exactly one kinds::Lamp, found 0")]
fn exactly_one_names_the_missing_kind() {
    World::new(60, 0).the::<Lamp>();
}

#[test]
#[should_panic(expected = "expected exactly one kinds::Lamp, found 2")]
fn exactly_one_refuses_ambiguity() {
    let mut w = World::new(60, 0);
    w.spawn_kind("a", lamp(0.0));
    w.spawn_kind("b", lamp(1.0));
    w.the::<Lamp>();
}

#[derive(Default, Component)]
struct Wick {
    bulb: Id<Bulb>,
}
#[derive(Kind)]
struct BoundLamp {
    #[child("bulb", bulb)]
    wick: Wick,
}
fn panic_text(f: impl FnOnce()) -> String {
    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
    if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        e.downcast_ref::<&str>().unwrap().to_string()
    }
}
#[test]
fn declared_child_is_saved_once_and_stale_live_and_loaded_errors_match() {
    let mut w = World::new(60, 0);
    let parent = w.spawn_named("lamp", Wick::default());
    let bulb = w.spawn_named("lamp/bulb", (PointLight::default(), Parent(parent)));
    let id = w.bind::<BoundLamp>(parent).unwrap();
    assert_eq!(w.row(id).unwrap().wick.bulb.entity(), bulb);
    let before = (
        w.save(),
        w.hash(),
        w.mutation_epoch(),
        generations::<Wick>(&w),
    );
    w.bind::<BoundLamp>(parent).unwrap();
    w.row(id).unwrap();
    assert_eq!(
        before,
        (
            w.save(),
            w.hash(),
            w.mutation_epoch(),
            generations::<Wick>(&w)
        )
    );
    w.despawn(bulb);
    let new = w.spawn_named("lamp/bulb", (PointLight::default(), Parent(parent)));
    assert_eq!(new.index(), bulb.index());
    let bytes = w.save();
    let error = w.row(id).err().unwrap().to_string();
    for expected in [
        "BoundLamp",
        "lamp",
        "wick.bulb",
        "Bulb",
        "generation",
        "row",
    ] {
        assert!(error.contains(expected), "{error}");
    }
    w.load(&bytes).unwrap();
    assert_eq!(error, w.row(id).err().unwrap().to_string());
    assert!(
        w.bind::<BoundLamp>(parent).is_err(),
        "setup must not silently retarget saved state"
    );
    assert_eq!(bytes, w.save());
}
#[test]
fn newly_added_parent_requires_explicit_binding_and_checks_parent() {
    let mut w = World::new(60, 0);
    let p = w.spawn_named("new", Wick::default());
    let b = w.spawn_named("new/bulb", PointLight::default());
    assert!(w
        .bind::<BoundLamp>(p)
        .unwrap_err()
        .to_string()
        .contains("Parent"));
    w.insert(b, Parent(p));
    let id = w.bind::<BoundLamp>(p).unwrap();
    assert_eq!(w.row(id).unwrap().wick.bulb.entity(), b);
    w.remove::<PointLight>(b);
    let error = w.row(id).err().unwrap().to_string();
    assert!(
        error.contains("PointLight") && error.contains("wick.bulb"),
        "{error}"
    );
}

type PositionAlias = Transform;
#[derive(Kind)]
struct AliasDuplicate {
    a: Transform,
    b: PositionAlias,
}
#[test]
fn alias_duplicate_preflight_has_no_spawn_or_lease_side_effect() {
    let mut w = World::new(60, 0);
    w.spawn_named("anchor", Transform::default());
    let before = (
        w.save(),
        w.mutation_epoch(),
        generations::<Transform>(&w),
        w.entities_revision(),
    );
    let error = panic_text(|| {
        w.spawn_kind(
            "bad",
            AliasDuplicate {
                a: Transform::default(),
                b: Transform::default(),
            },
        );
    });
    assert!(
        error.contains("duplicate component") && error.contains("a and b"),
        "{error}"
    );
    for mutable in [false, true] {
        let error = panic_text(|| {
            if mutable {
                w.rows_mut::<AliasDuplicate>();
            } else {
                w.rows::<AliasDuplicate>();
            }
        });
        assert!(error.contains("duplicate component"));
    }
    let id: Id<AliasDuplicate> =
        bin::from_slice(&bin::to_vec(&w.named("anchor").unwrap())).unwrap();
    assert!(w
        .row(id)
        .err()
        .unwrap()
        .to_string()
        .contains("duplicate component"));
    assert!(panic_text(|| w.edit(id, |_| ())).contains("duplicate component"));
    assert_eq!(
        before,
        (
            w.save(),
            w.mutation_epoch(),
            generations::<Transform>(&w),
            w.entities_revision()
        )
    );
}
#[test]
fn overlapping_edit_diagnostic_names_inner_outer_kind_entity_component_and_operation() {
    let mut w = World::new(60, 0);
    let id = w.spawn_kind("desk", lamp(0.0));
    w.edit(id, |_| {
        let before = (w.mutation_epoch(), w.revision::<Material>());
        let error = panic_text(|| w.edit(id, |_| ()));
        for expected in [
            "edit", "Lamp", "desk", "Entity", "Material", "inside", "mutably",
        ] {
            assert!(error.contains(expected), "{error}");
        }
        assert_eq!(before, (w.mutation_epoch(), w.revision::<Material>()));
    });
}

// Generated views are scoped in the derive's anonymous const, not this namespace.
struct BulbRef;
struct BulbMut;
#[test]
fn user_view_names_do_not_collide() {
    let _ = (BulbRef, BulbMut);
}

#[test]
fn interleaved_200k_with_churn_visits_every_match_and_refuses_above_the_work_bound() {
    let mut w = World::new(60, 0);
    for i in 0..200_000 {
        let e = w.spawn(());
        if i % 3 == 0 {
            w.insert(e, PointLight::default());
        }
        if i % 5 == 0 {
            w.despawn(e);
            w.spawn(());
        }
    }
    let expected = (0..200_000).filter(|i| i % 3 == 0 && i % 5 != 0).count();
    assert_eq!(w.rows::<Bulb>().count(), expected);
    for mut row in w.rows_mut::<Bulb>() {
        row.light.intensity = 17.0;
    }
    assert_eq!(
        w.rows::<Bulb>()
            .filter(|r| r.light.intensity == 17.0)
            .count(),
        expected
    );
    assert!(expected > 50_000); // Negative control: empty iteration cannot pass.
    w.spawn(());
    let epoch = w.mutation_epoch();
    assert!(panic_text(|| {
        w.rows_mut::<Bulb>();
    })
    .contains("limit is 200000 entity slots"));
    assert_eq!(epoch, w.mutation_epoch());
}

#[test]
fn spawn_kind_resolves_and_attaches_declared_child_before_use() {
    let mut w = World::new(60, 0);
    let before = (w.save(), w.mutation_epoch(), w.entities_revision());
    assert!(panic_text(|| {
        w.spawn_kind(
            "lamp",
            BoundLamp {
                wick: Wick::default(),
            },
        );
    })
    .contains("lamp/bulb"));
    assert_eq!(
        before,
        (w.save(), w.mutation_epoch(), w.entities_revision())
    );
    let child = w.spawn_kind(
        "lamp/bulb",
        Bulb {
            light: PointLight::default(),
        },
    );
    let parent = w.spawn_kind(
        "lamp",
        BoundLamp {
            wick: Wick::default(),
        },
    );
    assert_eq!(w.row(parent).unwrap().wick.bulb, child);
    assert_eq!(w.get::<Parent>(child).unwrap().0, parent.entity());
    let saved = w.save();
    w.load(&saved).unwrap();
    assert_eq!(w.row(parent).unwrap().wick.bulb, child);
}

#[derive(Default, Component)]
struct HiddenWick {
    #[data(skip)]
    bulb: Id<Bulb>,
}
#[derive(Kind)]
struct HiddenLamp {
    #[child("bulb", bulb)]
    wick: HiddenWick,
}
#[test]
fn a_declared_child_cannot_hide_in_a_skipped_field() {
    let mut w = World::new(60, 0);
    let before = (w.save(), w.mutation_epoch());
    let error = panic_text(|| {
        w.spawn_kind(
            "hidden",
            HiddenLamp {
                wick: HiddenWick::default(),
            },
        );
    });
    assert!(
        error.contains("HiddenLamp")
            && error.contains("HiddenWick")
            && error.contains("bulb")
            && error.contains("data(skip) is forbidden"),
        "{error}"
    );
    assert_eq!(before, (w.save(), w.mutation_epoch()));
}

#[test]
fn decoded_and_cross_world_ids_recheck_membership_without_changing_wire_state() {
    let mut a = World::new(60, 1);
    let id = a.spawn_kind("lamp", lamp(0.0));
    let decoded: Id<Lamp> = bin::from_slice(&bin::to_vec(&id)).unwrap();
    let mut b = World::new(60, 1);
    let entity = b.spawn_named("incomplete", Transform::default());
    assert_eq!(entity, id.entity());
    assert!(b.row(id).err().unwrap().to_string().contains("Material"));
    assert!(b.row(decoded).is_err());
    b.insert(entity, Material::default());
    let before = b.save();
    assert!(b.row(decoded).is_ok());
    assert_eq!(before, b.save());
    b.load(&before).unwrap();
    assert!(b.row(decoded).is_ok());
    b.remove::<Material>(entity);
    let incomplete = b.save();
    b.load(&incomplete).unwrap();
    assert!(b.row(decoded).is_err());
    assert!(a.row(id).is_ok());
}

#[derive(Kind)]
struct SharedPose {
    #[read]
    transform: Transform,
}
#[test]
fn edit_context_has_a_bound_and_unwinds_without_poisoning_the_world() {
    fn nested(w: &World, id: Id<SharedPose>, depth: usize) {
        if depth > 0 {
            w.edit(id, |r| {
                assert_eq!(r.transform.position, Vec3::ZERO);
                nested(w, id, depth - 1);
            });
        }
    }
    let mut w = World::new(60, 1);
    let id = w.spawn_kind(
        "pose",
        SharedPose {
            transform: Transform::default(),
        },
    );
    nested(&w, id, 32);
    let before = (w.save(), w.mutation_epoch());
    assert!(panic_text(|| nested(&w, id, 33)).contains("nesting exceeds 32"));
    nested(&w, id, 32);
    assert_eq!(before, (w.save(), w.mutation_epoch()));
}
