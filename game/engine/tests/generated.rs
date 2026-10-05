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

#[test]
fn a_parts_model_keeps_each_part_material_in_its_identity() {
    use exact_game::asset::{MaterialData, Model, Node};
    let mut lifted = triangle();
    for y in lifted.positions.iter_mut().skip(1).step_by(3) {
        *y = 2.;
    }
    lifted.bounds = [0., 2., 0., 1., 2., 1.];
    let gloss = MaterialData::surface(0., 0.2);
    let gold = MaterialData {
        base_color: [1., 0.8, 0.3, 1.],
        emissive: [0.5, 0.4, 0.],
        ..MaterialData::surface(1., 0.3)
    };
    let model = || Model::parts([(triangle(), gloss.clone()), (lifted.clone(), gold.clone())]);
    let mut w = World::new(60, 0);
    let mesh = w.generated_model("fruit.model", model()).unwrap();
    w.spawn((Transform::default(), mesh));
    let m = w.model("fruit.model").unwrap();
    assert_eq!(m.bounds, [0., 0., 0., 1., 2., 1.]);
    let parts: Vec<_> = m
        .nodes
        .iter()
        .map(|n| &m.materials[m.meshes[n.mesh.unwrap() as usize].material as usize])
        .map(|p| (p.metallic, p.roughness, p.emissive))
        .collect();
    assert_eq!(parts, [(0., 0.2, [0.; 3]), (1., 0.3, [0.5, 0.4, 0.])]);
    let saved = w.save();
    let world = || {
        let mut w = World::new(60, 0);
        w.register::<Mesh>();
        w.register::<Transform>();
        w
    };
    let mut matte = world();
    let mut rough = model();
    rough.materials[0].roughness = 1.;
    matte.generated_model("fruit.model", rough).unwrap();
    let refused = matte.load(&saved).unwrap_err().to_string();
    assert!(refused.contains("fruit.model"), "{refused}");
    let mut same = world();
    same.generated_model("fruit.model", model()).unwrap();
    same.load(&saved).unwrap();

    // `generated` is the one-part, matte, non-metal case, with the same identity.
    w.generated("rock.model", triangle()).unwrap();
    let single = Model {
        bounds: triangle().bounds,
        meshes: vec![triangle()],
        materials: vec![MaterialData::surface(0., 1.)],
        nodes: vec![Node {
            mesh: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    w.generated_model("rock.model", single).unwrap();
}

// Golden/storybook generated `meadow.model` while the art pass streamed one:
// setup refused only once the streamed bytes landed (a crash on macOS), which
// a hostless test never delivers. The name is refused at registration.
thread_local! {
    static REFUSED: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn register_all(w: &mut World) {
    for name in ["meadow.model", "tree.model", "own.model"] {
        if let Err(error) = w.generated(name, triangle()) {
            REFUSED.with(|r| r.borrow_mut().push(error));
        }
    }
}
#[test]
fn generated_names_refuse_every_declared_name_at_registration() {
    use exact_game::{Game, Input, Sim};
    struct Declares;
    impl Game for Declares {
        const ID: &'static str = "generated-collides";
        const ASSETS: &'static [&'static str] = &["tree.model"];
        const STREAMED: &'static [&'static str] = &["meadow.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            register_all(w);
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    REFUSED.with(|r| r.borrow_mut().clear());
    let mut sim = Sim::<Declares>::new(()).unwrap();
    // Setup waits for the declared model; nothing streamed is ever delivered.
    let model = exact_game::bin::to_vec(&exact_game::asset::Model::default());
    sim.asset("tree.model", Some(&model)).unwrap();
    assert!(!sim.is_loading());
    let refused = REFUSED.with(|r| r.take());
    assert_eq!(refused.len(), 2, "{refused:?}");
    for (error, name, declaration) in [
        (&refused[0], "`meadow.model`", "Game::STREAMED"),
        (&refused[1], "`tree.model`", "Game::ASSETS"),
    ] {
        assert!(
            error.contains(name) && error.contains(declaration),
            "{error}"
        );
    }
    assert!(
        sim.world().model("own.model").is_some(),
        "an undeclared name registers"
    );
    // A world without a game declares nothing.
    register_all(&mut World::new(60, 0));
    assert!(REFUSED.with(|r| r.take()).is_empty());
}
