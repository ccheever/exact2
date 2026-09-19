use exact_game::*;
struct Loading;
impl Game for Loading {
    const ID: &'static str = "asset-loading";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        assert!(w.model("crate.model").is_some());
        w.spawn_named("crate", (Transform::default(), Mesh::asset("crate.model")));
        w.publish("setup", true);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.publish("ticks", w.tick() as u32);
    }
}
#[test]
fn declared_models_gate_setup_and_ticks_and_survive_restore() {
    let bytes = bin::to_vec(&asset::Model::default());
    let mut a = Sim::<Loading>::new(()).unwrap();
    let mut b = Sim::<Loading>::new(()).unwrap();
    assert_eq!(a.take_assets(), ["crate.model"]);
    assert!(a.take_assets().is_empty());
    assert_eq!(a.world().len(), 0);
    a.run(5000.);
    assert_eq!(a.world().tick(), 0);
    assert!(a.agent(r#"{"op":"state"}"#).contains("crate.model"));
    a.asset("crate.model", Some(&bytes)).unwrap();
    b.asset("crate.model", Some(&bytes)).unwrap();
    assert_eq!(a.world().len(), 1);
    a.run(1000.);
    b.run(1000.);
    assert_eq!(a.world().tick(), 60);
    assert_eq!(a.world().hash(), b.world().hash());
    let saved = a.save().unwrap();
    a.restore(&saved).unwrap();
    assert!(a.world().model("crate.model").is_some());
    assert_eq!(a.world().hash(), b.world().hash());
}
#[test]
fn skinned_bounds_follow_delivery_pose_and_restore_without_changing_saved_state() {
    struct Scene;
    impl Game for Scene {
        const ID: &'static str = "skinned-bounds";
        const ASSETS: &'static [&'static str] = &["rig.model"];
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            w.register::<animation::Pose>();
            w.spawn_named("rig", (Transform::default(), Mesh::asset("rig.model")));
            w.spawn((Transform::at(0., 0., 40.), Camera::default()));
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let model = |reach| asset::Model {
        bounds: [-1., -1., -1., 1., 1., 1.],
        nodes: vec![asset::Node::default()],
        skins: vec![asset::Skin {
            joints: vec![0],
            inverse_binds: Mat4::IDENTITY.to_cols_array().to_vec(),
            ..Default::default()
        }],
        clips: vec![asset::Clip {
            tracks: vec![asset::Track {
                times: vec![0., 1.],
                values: vec![0., 0., 0., reach, 0., 0.],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut sim = Sim::<Scene>::new(()).unwrap();
    sim.asset("rig.model", Some(&bin::to_vec(&model(8.))))
        .unwrap();
    sim.viewport(800., 600.);
    let entity = sim.world().named("rig").unwrap();
    let center = Vec2::new(400., 300.);
    let before = sim.save().unwrap();
    let layout = sim.layout(entity).unwrap().screen;
    for _ in 0..10 {
        assert_eq!(sim.layout(entity).unwrap().screen, layout);
        let hit = sim.pick(center).unwrap();
        assert_eq!(hit.entity, entity);
        assert!((hit.distance - 31.).abs() < 1e-5, "{hit:?}");
    }
    assert_eq!(sim.save().unwrap(), before);
    sim.restore(&before).unwrap();
    assert_eq!(sim.layout(entity).unwrap().screen, layout);
    sim.asset("rig.model", Some(&bin::to_vec(&model(12.))))
        .unwrap();
    assert!((sim.pick(center).unwrap().distance - 27.).abs() < 1e-5);
    assert_ne!(sim.layout(entity).unwrap().screen, layout);
    let bind = animation::bind_pose(sim.world().model("rig.model").unwrap());
    let mut pose = animation::Pose::default();
    pose.previous = bind.clone();
    pose.local = bind;
    pose.bounds = [-2., -2., -2., 2., 2., 2.];
    sim.world_mut().insert(entity, pose);
    assert!((sim.pick(center).unwrap().distance - 38.).abs() < 1e-5);
    let posed = sim.save().unwrap();
    sim.restore(&posed).unwrap();
    assert!((sim.pick(center).unwrap().distance - 38.).abs() < 1e-5);
    sim.world_mut().remove::<animation::Pose>(entity);
    assert!((sim.pick(center).unwrap().distance - 27.).abs() < 1e-5);
}
#[test]
fn missing_declared_model_refuses_by_name_and_never_ticks() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    assert!(sim
        .asset("crate.model", None)
        .unwrap_err()
        .contains("crate.model"));
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 0);
}

struct Cosmetic;
impl Game for Cosmetic {
    const ID: &'static str = "cosmetic";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("late", (Transform::default(), Mesh::asset("late.model")));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.publish("sees_model", w.model("late.model").is_some());
    }
}
#[test]
fn undeclared_arrival_cannot_change_simulation_reads_or_layout() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    let before = sim.agent(r#"{"op":"layout","entity":"late"}"#);
    assert!(before.contains("\"bounds\":null"), "{before}");
    sim.take_assets();
    let model = asset::Model {
        bounds: [-2., -2., -2., 2., 2., 2.],
        ..Default::default()
    };
    sim.asset("late.model", Some(&bin::to_vec(&model))).unwrap();
    assert!(sim.world().model("late.model").is_none());
    assert_eq!(sim.agent(r#"{"op":"layout","entity":"late"}"#), before);
}
#[test]
fn failures_are_named_in_state_and_refused_clock() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    let _ = sim.asset("crate.model", None);
    let state = sim.agent(r#"{"op":"state"}"#);
    assert!(state.contains("\"state\":\"Failed\""), "{state}");
    let clock = sim.agent(r#"{"op":"clock","now":1000}"#);
    assert!(
        clock.contains("crate.model") && clock.contains("missing file"),
        "{clock}"
    );
}
#[test]
fn loading_save_refuses_and_clock_does_not_establish_an_epoch() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    let error = sim.save().unwrap_err().to_string();
    assert!(
        error.contains("crate.model") && error.contains("Pending"),
        "{error}"
    );
    sim.advance(5000., Clock::Seekable);
    sim.asset("crate.model", Some(&bin::to_vec(&asset::Model::default())))
        .unwrap();
    assert_eq!(sim.advance(9000., Clock::Seekable), 0);
    assert_eq!(sim.advance(9500., Clock::Seekable), 30);
}

struct InvalidDeclaration;
impl Game for InvalidDeclaration {
    const ID: &'static str = "invalid-asset-declaration";
    const ASSETS: &'static [&'static str] = &["bad/./name.model"];
    type Args = ();
    fn setup(_: &mut World, _: &()) {
        panic!("must refuse before setup")
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn invalid_declaration_refuses_at_bind_with_the_name() {
    let error = Sim::<InvalidDeclaration>::new(()).err().unwrap();
    assert!(error.contains("bad/./name.model"), "{error}");
}
struct BoundedCosmetic;
impl Game for BoundedCosmetic {
    const ID: &'static str = "bounded-cosmetic";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named(
            "late",
            (
                Transform::default(),
                Mesh::asset("late.model").bounds([-1., -2., -3., 1., 2., 3.]),
            ),
        );
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn authored_cosmetic_bounds_survive_arrival_and_save() {
    let mut sim = Sim::<BoundedCosmetic>::new(()).unwrap();
    let layout = sim.agent(r#"{"op":"layout","entity":"late"}"#);
    let bytes = bin::to_vec(&asset::Model {
        bounds: [-9., -9., -9., 9., 9., 9.],
        ..Default::default()
    });
    sim.asset("late.model", Some(&bytes)).unwrap();
    assert_eq!(layout, sim.agent(r#"{"op":"layout","entity":"late"}"#));
    sim.restore(&sim.save().unwrap()).unwrap();
    assert_eq!(layout, sim.agent(r#"{"op":"layout","entity":"late"}"#));
}

#[test]
fn unused_and_excessive_texture_lists_refuse() {
    let model = asset::Model {
        textures: vec!["unused.tex".into()],
        ..Default::default()
    };
    assert!(model.validate().unwrap_err().contains("unused"));
    let model = asset::Model {
        textures: (0..65).map(|i| format!("{i}.tex")).collect(),
        ..Default::default()
    };
    assert!(model.validate().unwrap_err().contains("64"));
    let model = asset::Model {
        textures: vec!["orphan.tex".into()],
        materials: vec![asset::MaterialData {
            base_color_texture: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(model
        .validate()
        .unwrap_err()
        .contains("unused texture `orphan.tex`"));
}

struct TextureDeclaration;
impl Game for TextureDeclaration {
    const ID: &'static str = "texture-declaration";
    const ASSETS: &'static [&'static str] = &["wrong.tex"];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn texture_declaration_is_allowed_but_a_texture_is_not_a_mesh() {
    let mut declared = Sim::<TextureDeclaration>::new(()).unwrap();
    assert!(declared.is_loading());
    assert_eq!(declared.take_assets(), ["wrong.tex"]);
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    *sim.world()
        .get_mut::<Mesh>(sim.world().resolve("late").unwrap())
        .unwrap() = Mesh::asset("wrong.tex");
    assert!(sim.take_assets().is_empty());
    assert!(sim.agent(r#"{"op":"state"}"#).contains("Failed"));
}

#[test]
fn deferred_textures_replace_retire_and_move_through_restore_in_name_order() {
    struct Textures;
    impl Game for Textures {
        const ID: &'static str = "texture-queue";
        type Args = ();
        fn setup(w: &mut World, _: &()) {
            for name in ["005.tex", "200.tex"] {
                w.spawn((Transform::default(), Sprite::new(name, Vec2::ONE)));
            }
        }
        fn tick(_: &mut World, _: &Input, _: &()) {}
    }
    let texture = |value| asset::TextureData {
        width: 1,
        height: 1,
        mips: vec![vec![value; 4]],
        ..Default::default()
    };
    let mut sim = Sim::<Textures>::new(()).unwrap();
    sim.defer_assets(true);
    for i in (0..256).rev() {
        sim.deliver_asset(
            &format!("{i:03}.tex"),
            Ok(asset::Content::Texture(texture(i as u8))),
        )
        .unwrap();
    }
    assert!(sim
        .deliver_asset("overflow.tex", Ok(asset::Content::Texture(texture(0))))
        .is_err());
    let replacement = texture(99);
    let pixels = replacement.mips[0].as_ptr();
    sim.deliver_asset("005.tex", Ok(asset::Content::Texture(replacement)))
        .unwrap();
    let saved = sim.save().unwrap();
    assert!(sim.restore(b"invalid").is_err());
    sim.restore(&saved).unwrap();
    assert!(sim.take_assets().is_empty());
    let delivered: Vec<_> = sim.take_textures().into_iter().collect();
    assert_eq!(
        delivered
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["005.tex", "200.tex"]
    );
    assert_eq!(delivered[0].1.mips[0], [99; 4]);
    assert_eq!(
        delivered[0].1.mips[0].as_ptr(),
        pixels,
        "restore and upload move mip ownership"
    );
    assert_eq!(delivered[1].1.mips[0], [200; 4]);
    assert!(sim.take_textures().is_empty());
    sim.defer_assets(false);
    sim.deliver_asset("005.tex", Ok(asset::Content::Texture(texture(3))))
        .unwrap();
    assert!(
        sim.take_textures().is_empty(),
        "headless delivery retains no upload payload"
    );
}

#[test]
fn model_delivery_checks_carrier_size_before_decode() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    let error = sim
        .asset("crate.model", Some(&vec![0; 64 * 1024 * 1024 + 1]))
        .unwrap_err();
    assert!(
        error.contains("crate.model") && error.contains("64 MiB"),
        "{error}"
    );
}

#[test]
fn headless_loader_drains_dependencies_and_reports_failures_without_panicking() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    let mut names = Vec::new();
    sim.load_assets(|name| {
        names.push(name.to_owned());
        Ok::<_, String>(bin::to_vec(&asset::Model::default()))
    })
    .unwrap();
    assert_eq!(names, ["crate.model"]);
    assert_eq!(sim.world().len(), 1);
    assert!(sim.save().is_ok());
    let mut failed = Sim::<Loading>::new(()).unwrap();
    let error = failed
        .load_assets(|_| Err::<Vec<u8>, _>("unreadable"))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("crate.model") && error.contains("unreadable"),
        "{error}"
    );
    let error = failed.save().unwrap_err().to_string();
    assert!(
        error.contains("crate.model") && error.contains("Failed"),
        "{error}"
    );
}

#[test]
fn cosmetic_names_retire_and_respawn_requests_again() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    assert_eq!(sim.take_assets(), ["late.model"]);
    sim.asset("late.model", Some(&bin::to_vec(&asset::Model::default())))
        .unwrap();
    let entity = sim.world().resolve("late").unwrap();
    sim.world_mut().despawn(entity);
    assert!(sim.take_assets().is_empty());
    assert!(!sim.agent(r#"{"op":"state"}"#).contains("late.model"));
    sim.world_mut()
        .spawn((Transform::default(), Mesh::asset("late.model")));
    assert_eq!(sim.take_assets(), ["late.model"]);
}
#[test]
fn asset_requests_are_bounded_and_refusal_names_the_excess() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    for i in 0..300 {
        sim.world_mut()
            .spawn((Transform::default(), Mesh::asset(format!("{i:03}.model"))));
    }
    assert!(sim.take_assets().len() <= 256);
    let state = sim.agent(r#"{"op":"state"}"#);
    assert!(state.contains("256") && state.contains("Failed"), "{state}");
}

#[test]
fn a_publication_only_tick_names_the_changing_key() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    sim.asset("crate.model", Some(&bin::to_vec(&asset::Model::default())))
        .unwrap();
    sim.run(1000.);
    let clock = sim.agent(r#"{"op":"clock"}"#);
    assert!(
        clock.contains("\"quiescent\":false") && clock.contains("published.ticks"),
        "{clock}"
    );
}

#[test]
fn declared_delivery_state_retires_but_simulation_data_is_stable() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    sim.asset("crate.model", Some(&bin::to_vec(&asset::Model::default())))
        .unwrap();
    let entity = sim.world().resolve("crate").unwrap();
    sim.world_mut().despawn(entity);
    sim.take_assets();
    assert!(sim.world().model("crate.model").is_some());
    let state = sim.agent(r#"{"op":"state"}"#);
    assert!(state.contains("\"assets\":[]"), "{state}");
    let saved = sim.save().unwrap();
    sim.restore(&saved).unwrap();
    sim.world_mut()
        .spawn((Transform::default(), Mesh::asset("crate.model")));
    assert_eq!(sim.take_assets(), ["crate.model"]);
}

#[test]
fn saving_a_pending_cosmetic_refuses_but_failed_cosmetics_do_not_gate() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    sim.asset_failed("late.model", "missing cosmetic");
    sim.world_mut()
        .spawn((Transform::default(), Mesh::asset("save.model")));
    assert!(sim.take_assets().contains(&"save.model".to_owned()));
    assert!(sim.save().unwrap_err().to_string().contains("save.model"));
    sim.asset_failed("save.model", "missing file");
    assert!(sim.save().is_ok(), "failed cosmetics do not block saving");
}

#[test]
fn save_readiness_tracks_current_meshes_without_request_drain() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    let e = sim.world().named("late").unwrap();
    assert!(sim.save().unwrap_err().to_string().contains("late.model"));
    sim.take_assets();
    sim.world_mut().despawn(e);
    assert!(sim.save().is_ok());
}

#[test]
fn failed_cosmetic_dependencies_do_not_gate_a_save() {
    let mut sim = Sim::<Cosmetic>::new(()).unwrap();
    let mut model: asset::Model =
        bin::from_slice(include_bytes!("../../bake/tests/fixtures/crate.model")).unwrap();
    model.textures.push("still-pending.tex".into());
    model.materials[0].normal_texture = Some(1);
    sim.asset("late.model", Some(&bin::to_vec(&model))).unwrap();
    sim.asset_failed(&model.textures[0], "cosmetic missing");
    assert!(
        sim.save().is_ok(),
        "a failed cosmetic cannot block on its other textures"
    );
}

#[test]
fn loading_refusals_name_the_state_that_contains_pending_assets() {
    let mut sim = Sim::<Loading>::new(()).unwrap();
    let clock = sim.agent(r#"{"op":"clock"}"#);
    let save = sim.save().unwrap_err().to_string();
    for error in [clock, save] {
        assert!(
            error.contains("state.world.loading") && error.contains("state.world.assets"),
            "{error}"
        );
        assert!(!error.contains("state world:*"), "{error}");
    }
    let state = sim.agent(r#"{"op":"state"}"#);
    assert!(
        state.contains("crate.model") && state.contains("Pending"),
        "{state}"
    );
}
