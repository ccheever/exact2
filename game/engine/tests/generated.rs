use exact_game::{asset::MeshData, Mesh, Transform, World};

fn triangle() -> MeshData {
    MeshData {
        positions: vec![0., 0., 0., 0., 0., 1., 1., 0., 0.],
        normals: vec![0., 1., 0., 0., 1., 0., 0., 1., 0.],
        uvs: vec![0.; 6],
        colors: vec![1., 0., 0., 1., 0., 1., 0., 1., 0., 0., 1., 1.],
        indices: vec![0, 1, 2],
        bounds: [0., 0., 0., 1., 0., 1.],
        ..Default::default()
    }
}
#[test]
fn generated_mesh_is_shared_immutable_and_saved_by_identity() {
    let mut w = World::new(60, 0);
    let mesh = w.generated("rock.model", triangle()).unwrap();
    for _ in 0..100 {
        w.spawn((mesh.clone(), Transform::default()));
    }
    let original = w.model("rock.model").unwrap() as *const _;
    let revision = w.model_revision();
    w.generated("rock.model", triangle()).unwrap();
    assert_eq!(revision, w.model_revision());
    assert_eq!(original, w.model("rock.model").unwrap() as *const _);
    let saved = w.save();
    assert!(!saved.windows(9).any(|s| s == b"positions"));
    let mut fresh = World::new(60, 0);
    fresh.register::<Mesh>();
    fresh.register::<Transform>();
    assert!(fresh
        .load(&saved)
        .unwrap_err()
        .to_string()
        .contains("rock.model"));
    fresh.generated("rock.model", triangle()).unwrap();
    fresh.load(&saved).unwrap();
    assert_eq!(fresh.hash(), w.hash());
    let mut different = triangle();
    different.positions[0] = 0.25;
    let mut changed = World::new(60, 0);
    changed.register::<Mesh>();
    changed.register::<Transform>();
    changed.generated("rock.model", different.clone()).unwrap();
    assert!(changed
        .load(&saved)
        .unwrap_err()
        .to_string()
        .contains("rock.model"));
    assert!(w
        .generated("rock.model", different)
        .unwrap_err()
        .contains("rock.model"));
}
#[test]
fn generated_mesh_refuses_bad_colors_and_upload_ranges() {
    for bad in [0, 1, 2] {
        let mut mesh = triangle();
        match bad {
            0 => {
                mesh.colors.pop();
            }
            1 => mesh.colors[0] = f32::NAN,
            _ => mesh.indices[0] = 100,
        }
        assert!(World::new(60, 0).generated("bad.model", mesh).is_err());
    }
}

#[test]
fn a_megabyte_of_vertices_does_not_enter_the_save() {
    let mut small = World::new(60, 7);
    let mesh = small.generated("terrain.model", triangle()).unwrap();
    small.spawn((Transform::default(), mesh));
    let mut data = triangle();
    data.positions = data.positions.repeat(40_000);
    data.normals = data.normals.repeat(40_000);
    data.uvs = data.uvs.repeat(40_000);
    data.colors.clear();
    let mut large = World::new(60, 7);
    let mesh = large.generated("terrain.model", data).unwrap();
    large.spawn((Transform::default(), mesh));
    assert!(large.save().len().abs_diff(small.save().len()) < 10);
}

#[test]
fn identity_is_required_even_when_the_saved_world_has_no_geometry() {
    let empty = World::new(60, 0).save();
    let mut w = World::new(60, 0);
    w.generated("terrain.model", triangle()).unwrap();
    assert!(w
        .load(&empty)
        .unwrap_err()
        .to_string()
        .contains("terrain.model"));
}

#[test]
fn setup_geometry_remains_resident_before_its_first_entity() {
    struct Late;
    impl exact_game::Game for Late {
        const ID: &'static str = "late-generated";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.generated("rock.model", triangle()).unwrap();
        }
        fn tick(w: &mut World, _: &exact_game::Input, _: &()) {
            if w.tick() == 1 {
                w.spawn((Transform::default(), Mesh::asset("rock.model")));
            }
        }
    }
    let mut sim = exact_game::Sim::<Late>::new(()).unwrap();
    assert!(sim.take_assets().is_empty());
    sim.run(1000. / 60.);
    assert!(sim.take_assets().is_empty());
    assert!(sim.world().model("rock.model").is_some());
    assert!(sim.save().is_ok());
}

#[test]
fn setup_reconstructs_generated_geometry_and_drops_old_identities() {
    use exact_game::{Args, Game, Input, Sim, Value};
    #[derive(Default, Args)]
    struct Options {
        seed: u32,
    }
    struct Seeded;
    impl Game for Seeded {
        const ID: &'static str = "seeded-generated";
        type Args = Options;
        fn setup(w: &mut World, args: &Options) {
            if args.seed < 2 {
                let mut mesh = triangle();
                mesh.positions[0] = args.seed as f32 * 0.25;
                let mesh = w.generated("rock.model", mesh).unwrap();
                w.spawn((Transform::default(), mesh));
            }
        }
        fn tick(_: &mut World, _: &Input, _: &Options) {}
    }
    let mut sim = Sim::<Seeded>::new(Options::default()).unwrap();
    let original = sim.save().unwrap();
    sim.bind(&[Value::Number(1.)], None).unwrap();
    assert_eq!(sim.world().query::<&Mesh>().iter().count(), 1);
    assert_eq!(
        sim.world().model("rock.model").unwrap().meshes[0].positions[0],
        0.25
    );
    assert!(sim.take_assets().is_empty());
    let changed = sim.save().unwrap();
    assert!(sim
        .restore(&original)
        .unwrap_err()
        .to_string()
        .contains("rock.model"));
    assert_eq!(sim.save().unwrap(), changed, "refused restore is atomic");
    sim.bind(&[Value::Number(0.)], None).unwrap();
    sim.restore(&original).unwrap();
    assert_eq!(sim.save().unwrap(), original);

    sim.bind(&[Value::Number(2.)], None).unwrap();
    assert!(sim.world().model("rock.model").is_none());
    assert!(sim.take_assets().is_empty());
    assert!(sim.presentation_models().next().is_none());
    let fresh = Sim::<Seeded>::new(Options { seed: 2 }).unwrap();
    assert_eq!(sim.world().save(), fresh.world().save());
    sim.restore(&fresh.save().unwrap()).unwrap();
}

/// A generated model's identity is in every save of a world that holds it.
/// Material names (LLP 1139.004 D3) are informational: model record 2 writes
/// no name for an unnamed material, so the identity, and the saves, of
/// existing generated models stay as they were.
#[test]
fn an_unnamed_material_keeps_a_generated_models_identity() {
    let mut w = World::new(60, 0);
    w.generated("rock.model", triangle()).unwrap();
    let model = w.model("rock.model").unwrap();
    assert_eq!(model.materials.len(), 1);
    assert_eq!(exact_game::hash::of(model), 9794033405689772318);
}

/// The material record's name rule is the record's alone: present when not
/// empty, absent meaning unnamed, assigned on every read. The other fields
/// keep the codec's lossless encoding, negative zero included, and the hash
/// tells a named material from an unnamed one.
#[test]
fn the_material_record_round_trips_its_name_and_every_bit() {
    use exact_game::asset::MaterialData;
    use exact_game::{bin, hash};
    let named = MaterialData {
        name: "Hull".into(),
        metallic: -0.0,
        roughness: 0.0,
        ..Default::default()
    };
    let back: MaterialData = bin::from_slice(&bin::to_vec(&named)).unwrap();
    assert_eq!(back.name, "Hull");
    assert_eq!(back.metallic.to_bits(), (-0.0f32).to_bits());
    assert_eq!(back.roughness.to_bits(), 0.0f32.to_bits());
    // A stale value never survives a read of a record without a name.
    let mut dirty = named.clone();
    bin::read_into(&bin::to_vec(&MaterialData::default()), &mut dirty).unwrap();
    assert_eq!(dirty.name, "");
    let unnamed = MaterialData {
        name: String::new(),
        ..named.clone()
    };
    assert_ne!(hash::of(&named), hash::of(&unnamed));
    assert_ne!(
        hash::of(&named),
        hash::of(&MaterialData {
            name: "Hul".into(),
            ..named.clone()
        })
    );
    // Only the name is optional: the derive has no attribute that drops a field by value.
    assert_ne!(
        hash::of(&MaterialData {
            metallic: 1.0,
            roughness: 0.0,
            ..unnamed.clone()
        }),
        hash::of(&MaterialData {
            metallic: 0.0,
            roughness: 1.0,
            ..unnamed.clone()
        })
    );
}

/// A baked `.model` file is versioned (`asset::MODEL_RECORD`); a file of an
/// older bake or another record is refused by name, never read as unnamed.
#[test]
fn a_model_of_another_record_is_refused_by_name() {
    use exact_game::asset::{decode_model, encode_model, Model, MODEL_RECORD};
    let model = Model::default();
    let file = encode_model(&model);
    assert_eq!(&file[..8], b"EXMODEL\x02");
    assert_eq!(MODEL_RECORD, 2);
    decode_model(&file).unwrap();
    let old = exact_game::bin::to_vec(&model);
    let error = decode_model(&old).unwrap_err();
    assert!(
        error.contains("baked before model records") && error.contains("bake art/ again"),
        "{error}"
    );
    let mut later = file.clone();
    later[7] = 3;
    let error = decode_model(&later).unwrap_err();
    assert!(
        error.contains("model record 3") && error.contains("reads record 2"),
        "{error}"
    );
}

/// A complete record (a tuning table's JSON) names every material field, as the
/// derive's records must; only the name may be absent. A short record and
/// trailing input are refused.
#[test]
fn a_complete_material_record_requires_every_field_but_its_name() {
    use exact_game::asset::{MaterialData, Table};
    use exact_game::{bin, json};
    let table = Table::of::<MaterialData>("material.json");
    let error = table.decode("{}").err().unwrap().to_string();
    assert!(
        error.contains("missing field") && error.contains("base_color"),
        "{error}"
    );
    let full = json::to_string(&MaterialData::default()).unwrap();
    assert!(!full.contains("\"name\""), "{full}");
    table.decode(&full).unwrap();
    let named = json::to_string(&MaterialData {
        name: "Hull".into(),
        ..Default::default()
    })
    .unwrap();
    table.decode(&named).unwrap();
    let short = full.replacen("\"metallic\":1.0,", "", 1);
    assert_ne!(short, full);
    let error = table.decode(&short).err().unwrap().to_string();
    assert!(
        error.contains("missing field") && error.contains("metallic"),
        "{error}"
    );
    let mut trailing = bin::to_vec(&MaterialData::default());
    trailing.push(0);
    assert!(bin::from_slice::<MaterialData>(&trailing).is_err());
}
