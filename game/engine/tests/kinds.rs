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
    assert_eq!(w.with_row(id, |r| r.transform.position.x), 0.0);
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
