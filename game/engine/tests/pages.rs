use exact_game::{Material, Quat, Transform, Vec3, World, PAGE};
use std::mem::{align_of, offset_of, size_of};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn renderer_layouts_are_ten_contiguous_floats() {
    assert_eq!(size_of::<Transform>(), 40);
    assert_eq!(align_of::<Transform>(), 4);
    assert_eq!(offset_of!(Transform, position), 0);
    assert_eq!(offset_of!(Transform, rotation), 12);
    assert_eq!(offset_of!(Transform, scale), 28);
    assert_eq!(size_of::<Material>(), 40);
    assert_eq!(align_of::<Material>(), 4);
    assert_eq!(offset_of!(Material, color), 0);
    assert_eq!(offset_of!(Material, metallic), 16);
    assert_eq!(offset_of!(Material, roughness), 20);
    assert_eq!(offset_of!(Material, emissive), 24);
    assert_eq!(offset_of!(Material, grid_spacing), 36);
    let mut world = World::new(60, 0);
    world.spawn((
        Transform {
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::from_xyzw(4.0, 5.0, 6.0, 7.0),
            scale: Vec3::new(8.0, 9.0, 10.0),
        },
        Material {
            color: [1.0, 2.0, 3.0, 4.0],
            metallic: 5.0,
            roughness: 6.0,
            emissive: [7.0, 8.0, 9.0],
            grid_spacing: 10.0,
        },
    ));
    let expected: Vec<u8> = (1..=10).flat_map(|i| (i as f32).to_ne_bytes()).collect();
    assert_eq!(
        &world.pages::<Transform>().iter().next().unwrap().bytes()[..40],
        expected
    );
    assert_eq!(
        &world.pages::<Material>().iter().next().unwrap().bytes()[..40],
        expected
    );
}

#[test]
fn page_masks_zero_bytes_leases_and_reallocation() {
    let mut world = World::new(60, 0);
    assert_eq!(world.pages::<Transform>().iter().count(), 0);
    let entities: Vec<_> = (0..3 * PAGE).map(|_| world.spawn(())).collect();
    for &i in &[1, 63, 64, PAGE - 1, PAGE, 2 * PAGE + 7] {
        world.insert(entities[i], Transform::at(i as f32, 2.0, 3.0));
    }
    world.remove::<Transform>(entities[PAGE]); // whole middle page is freed
    let pages = world.pages::<Transform>();
    let views: Vec<_> = pages.iter().collect();
    assert_eq!(
        views.iter().map(|v| v.first).collect::<Vec<_>>(),
        [0, (2 * PAGE) as u32]
    );
    assert_eq!(views[0].mask().len(), PAGE / 64);
    assert_eq!(views[0].mask()[0], (1 << 1) | (1 << 63));
    assert_eq!(views[0].mask()[1], 1);
    assert_eq!(views[0].mask()[PAGE / 64 - 1], 1 << 63);
    assert_eq!(views[1].mask()[0], 1 << 7);
    assert_eq!(views[0].as_ptr() as usize % 4, 0);
    for view in &views {
        assert_eq!(view.bytes().len(), PAGE * 40);
        for slot in 0..PAGE {
            if view.mask()[slot / 64] & (1 << (slot % 64)) == 0 {
                assert!(view.bytes()[slot * 40..(slot + 1) * 40]
                    .iter()
                    .all(|&b| b == 0));
            }
        }
    }
    assert!(catch_unwind(AssertUnwindSafe(|| world.get_mut::<Transform>(entities[1]))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| {
        world.query::<&mut Transform>().iter().count();
    }))
    .is_err());
    assert_eq!(world.query::<&Transform>().iter().count(), 5);
    drop(views);
    drop(pages);
    world.remove::<Transform>(entities[63]);
    assert!(
        world.pages::<Transform>().iter().next().unwrap().bytes()[63 * 40..64 * 40]
            .iter()
            .all(|&b| b == 0)
    );
    world.insert(entities[PAGE], Transform::default());
    assert_eq!(world.pages::<Transform>().iter().count(), 3);
    let lease = world.get_mut::<Transform>(entities[1]).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| world.pages::<Transform>())).is_err());
    drop(lease);
    world.get_mut::<Transform>(entities[1]).unwrap().position.x = 81.0;
    assert_eq!(
        &world.pages::<Transform>().iter().next().unwrap().bytes()[40..44],
        &81.0f32.to_ne_bytes()
    );
}

#[test]
fn write_generations_cover_mutable_rows_without_marking_other_pages() {
    use exact_game::Component;
    #[derive(Default, Component)]
    struct Selected;
    let mut w = World::new(60, 0);
    let entities: Vec<_> = (0..PAGE * 3)
        .map(|_| w.spawn(Transform::default()))
        .collect();
    let generations = |w: &World| {
        w.pages::<Transform>()
            .iter()
            .map(|p| p.generation)
            .collect::<Vec<_>>()
    };
    let initial = generations(&w);
    let epoch = w.mutation_epoch();
    let lease = w.get_mut::<Transform>(entities[PAGE + 7]).unwrap();
    drop(lease); // Handing out a row marks even a same-value/no assignment lease.
    assert_ne!(w.mutation_epoch(), epoch);
    let after = generations(&w);
    assert_eq!(after[0], initial[0]);
    assert_ne!(after[1], initial[1]);
    assert_eq!(after[2], initial[2]);
    w.insert(entities[PAGE * 2 + 90], Selected);
    for (_, t) in w.query::<&mut Transform>().with::<Selected>().iter() {
        t.position.x = 1.;
    }
    let filtered = generations(&w);
    assert_eq!(&filtered[..2], &after[..2]);
    assert_ne!(filtered[2], after[2]);
    for (_, mut t) in w.query::<(&Selected, Option<&mut Transform>)>() {
        t.as_mut().unwrap().position.x = 2.;
    }
    let owned = generations(&w);
    assert_eq!(&owned[..2], &filtered[..2]);
    assert_ne!(owned[2], filtered[2]);
    let mut query = w.query::<&mut Transform>();
    query.iter().next().unwrap().1.position.x = 3.;
    drop(query);
    let partial = generations(&w);
    assert_ne!(partial[0], owned[0]);
    assert_eq!(&partial[1..], &owned[1..]);
    w.remove::<Transform>(entities[PAGE + 7]);
    assert_ne!(generations(&w)[1], partial[1]);
    let saved = w.save();
    let hash = w.hash();
    let presentation = w.presentation_generation();
    w.load(&saved).unwrap();
    assert_ne!(w.presentation_generation(), presentation);
    assert_eq!(w.hash(), hash);
    assert!(generations(&w).iter().all(|g| *g != 0));
}

#[test]
fn membership_union_is_ordered_and_does_not_require_both_columns() {
    use exact_game::Component;
    #[derive(Default, Component)]
    struct A;
    #[derive(Default, Component)]
    struct B;
    let mut w = World::new(60, 0);
    let es: Vec<_> = (0..PAGE * 3).map(|_| w.spawn(())).collect();
    for i in [0, PAGE + 9, PAGE * 2] {
        w.insert(es[i], A);
    }
    for i in [17, PAGE + 9] {
        w.insert(es[i], B);
    }
    let found: Vec<_> = w
        .query::<(Option<&A>, Option<&B>)>()
        .with_any::<A, B>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    assert_eq!(found, [es[0], es[17], es[PAGE + 9], es[PAGE * 2]]);
}

#[test]
fn sparse_owned_components_expose_only_initialized_runs() {
    use exact_game::Component;
    #[derive(Default, Component)]
    struct Owned {
        label: String,
    }
    let mut world = World::new(60, 0);
    let entities: Vec<_> = (0..PAGE + 3).map(|_| world.spawn(())).collect();
    for i in [1, 2, 63, 64, PAGE + 2] {
        world.insert(
            entities[i],
            Owned {
                label: format!("value {i}"),
            },
        );
    }
    world.remove::<Owned>(entities[63]);
    let pages = world.pages::<Owned>();
    let values: Vec<_> = pages
        .iter()
        .flat_map(|page| {
            page.runs()
                .flat_map(|(first, values)| {
                    values
                        .iter()
                        .enumerate()
                        .map(move |(i, value)| (first + i as u32, value.label.clone()))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        values,
        [1, 2, 64, PAGE + 2].map(|i| (i as u32, format!("value {i}")))
    );
}
