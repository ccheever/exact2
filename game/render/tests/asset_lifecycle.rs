use exact_game::{asset::Model, bin, Game, Input, Mesh, Sim, Transform, World};
use exact_game_render::WorldSurface;
use exact_gpu::{fixture, Frame, Surface};
struct Art;
impl Game for Art {
    const ID: &'static str = "asset-lifecycle";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn((Transform::default(), Mesh::asset("crate.model")));
    }
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
fn fresh() -> WorldSurface<Art, (), true> {
    let mut surface = WorldSurface::default();
    surface.bind(&[], None).unwrap();
    surface
}
#[test]
fn deferred_refusal_reports_once_and_leaves_fresh_world_usable() {
    let mut surface = fresh();
    surface
        .restore(b"invalid save", exact_gpu::Restore::Open)
        .unwrap();
    surface.asset("crate.model", Ok(&bin::to_vec(&Model::default())));
    assert!(surface.take_error().unwrap().0.contains("save"));
    assert!(surface.take_error().is_none());
    assert!(surface.error().is_none());
    surface.bind(&[], None).unwrap();
    assert!(!surface.sim().unwrap().is_loading());
}
#[test]
fn pending_restore_never_escapes_as_a_current_save() {
    let mut source = Sim::<Art>::new(()).unwrap();
    source
        .asset("crate.model", Some(&bin::to_vec(&Model::default())))
        .unwrap();
    source.run(500.);
    let mut surface = fresh();
    surface
        .restore(&source.save().unwrap(), exact_gpu::Restore::Open)
        .unwrap();
    assert!(surface.carry().is_none());
}
#[test]
fn terminal_declaration_requests_no_more_frames() {
    let Ok(gpu) = fixture::device() else { return };
    let mut surface = fresh();
    surface.asset("crate.model", Err(exact_gpu::AssetError::Missing));
    let (_, wants) = fixture::render(
        &gpu,
        &mut surface,
        &Frame {
            width: 16.,
            height: 16.,
            scale: 1.,
            now_ms: 0.,
            seekable: false,
            period_ms: 0.,
            children_generation: 0,
            shader_generation: 0,
        },
    )
    .unwrap();
    assert!(!wants);
}

#[test]
fn mirrored_entity_model_refuses_by_name() {
    let Ok(gpu) = fixture::device() else { return };
    let mut renderer = exact_game_render::Renderer::new(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    renderer
        .prepare_model("mirrored.model", &Model::default())
        .unwrap();
    let mut world = World::new(60, 0);
    world.register_scene();
    let pose = Transform {
        scale: exact_game::Vec3::new(-1., 1., 1.),
        ..Default::default()
    };
    world.spawn((pose, Mesh::asset("mirrored.model")));
    world.propagate();
    let error = exact_game_render::Feed::default()
        .feed(&world, &mut renderer)
        .unwrap_err();
    assert!(error.to_string().contains("mirrored.model"));
}

#[test]
fn primitive_module_refuses_model_by_name_at_bind() {
    let mut surface = WorldSurface::<Art>::default();
    let error = surface.bind(&[], None).unwrap_err();
    assert!(error.0.contains("crate.model") && error.0.contains("game.assets"));
    assert!(surface.bind(&[], None).is_err());
    assert!(surface.sim().is_none());
}

#[test]
fn loaded_content_reprepares_after_device_loss() {
    let Ok(gpu) = fixture::device() else { return };
    let mut surface = fresh();
    surface.device_ready();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    surface.asset("crate.model", Ok(&bin::to_vec(&model)));
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    let hash = surface.sim().unwrap().world().hash();
    surface.device_lost();
    assert!(!surface.sim().unwrap().is_loading());
    assert_eq!(surface.sim().unwrap().world().hash(), hash);
    assert_eq!(
        surface.assets(),
        textures.keys().cloned().collect::<Vec<_>>()
    );
    surface.device_ready();
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    assert!(surface.sim().unwrap().device_assets_ready());
}

#[test]
fn loss_during_loading_reissues_unanswered_names() {
    let mut surface = fresh();
    surface.device_ready();
    assert_eq!(surface.assets(), ["crate.model"]);
    surface.device_lost();
    surface.device_ready();
    assert_eq!(surface.assets(), ["crate.model"]);
}

#[test]
fn attaching_a_device_after_headless_delivery_requests_texture_bytes() {
    let mut surface = fresh();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    surface.asset("crate.model", Ok(&bin::to_vec(&model)));
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    surface.device_ready();
    assert_eq!(
        surface.assets(),
        textures.keys().cloned().collect::<Vec<_>>()
    );
}

#[test]
fn deferred_restore_preserves_the_last_texture_until_upload() {
    let Ok(gpu) = fixture::device() else { return };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    let mut source = Sim::<Art>::new(()).unwrap();
    source
        .asset("crate.model", Some(&bin::to_vec(&model)))
        .unwrap();
    for (name, texture) in &textures {
        source.asset(name, Some(&bin::to_vec(texture))).unwrap();
    }
    let save = source.save().unwrap();
    let mut surface = fresh();
    surface.device_ready();
    surface.restore(&save, exact_gpu::Restore::Open).unwrap();
    surface.asset("crate.model", Ok(&bin::to_vec(&model)));
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    surface.prepare_assets(
        &gpu.device,
        &gpu.queue,
        exact_gpu::wgpu::TextureFormat::Rgba8Unorm,
    );
    assert!(surface.sim().unwrap().device_assets_ready());
}

#[path = "../../games/asset-fixture/logic/src/lib.rs"]
mod asset_fixture;
#[test]
fn replacement_device_draws_identical_pixels() {
    let Ok(gpu) = fixture::device() else { return };
    let mut surface = WorldSurface::<asset_fixture::AssetFixture, (), true>::default();
    surface.device_ready();
    surface.bind(&[], None).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (model, textures) = exact_game_bake::assets(&path).unwrap();
    surface.asset("crate.model", Ok(&bin::to_vec(&model)));
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    let frame = Frame {
        width: 128.,
        height: 128.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let (before, _) = fixture::render(&gpu, &mut surface, &frame).unwrap();
    surface.device_lost();
    let replacement = fixture::device().unwrap();
    surface.device_ready();
    assert_eq!(
        surface.assets(),
        textures.keys().cloned().collect::<Vec<_>>()
    );
    for (name, texture) in &textures {
        surface.asset(name, Ok(&bin::to_vec(texture)));
    }
    let (after, _) = fixture::render(&replacement, &mut surface, &frame).unwrap();
    assert_eq!(before.data, after.data);
}

#[test]
fn textureless_live_model_survives_unrelated_retirement_and_module_device_loss() {
    use exact_game::{Actions, Camera};
    use exact_gpu::{Module, Registry};
    struct Pair;
    impl Game for Pair {
        const ID: &'static str = "retirement-pair";
        const ASSETS: &'static [&'static str] = &["a.model", "b.model"];
        type Args = ();
        fn actions() -> Actions {
            Actions::new().button("retire", &["KeyR"])
        }
        fn setup(w: &mut World, _: &()) {
            w.spawn_named("a", (Transform::at(100., 0., 0.), Mesh::asset("a.model")));
            w.spawn((Transform::default(), Mesh::asset("b.model")));
            w.spawn((Transform::at(0., 0., 5.), Camera::default()));
        }
        fn tick(w: &mut World, i: &Input, _: &()) {
            if i.held("retire") {
                if let Some(e) = w.named("a") {
                    w.despawn(e);
                }
            }
        }
    }
    static REGISTRY: Registry = Registry {
        surfaces: &[("world", 0, || {
            Box::<WorldSurface<Pair, (), true>>::default()
        })],
        shaders: &[],
    };
    let Ok(gpu) = fixture::device() else { return };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../games/asset-fixture/art/crate.gltf");
    let (mut model, _) = exact_game_bake::assets(&path).unwrap();
    model.textures.clear();
    for material in &mut model.materials {
        *material = Default::default();
    }
    let bytes = bin::to_vec(&model);
    let mut module = Module::new(&REGISTRY);
    module.set_gpu(gpu);
    module.set_seekable(true);
    let id = module.create_headless("world").unwrap();
    assert!(module.bind(id, &[], None));
    assert_eq!(module.take_assets(id), ["a.model", "b.model"]);
    for name in ["a.model", "b.model"] {
        assert!(module.asset(id, name, Ok(&bytes)));
    }
    let mut frame = Frame {
        width: 64.,
        height: 64.,
        scale: 1.,
        now_ms: 0.,
        seekable: true,
        period_ms: 0.,
        children_generation: 0,
        shader_generation: 0,
    };
    let (before, _) = module.readback(id, &frame).unwrap();
    assert!(
        before.data.chunks_exact(4).any(|p| p != &before.data[..4]),
        "model must have visible pixels"
    );
    assert!(module.input_json(
        id,
        r#"{"t":"key","code":"KeyR","key":"r","down":true,"repeat":false,"at":1}"#
    ));
    module.agent(id, r#"{"op":"clock","now":17}"#);
    assert!(
        module.take_assets(id).is_empty(),
        "textureless model needs no delivery to prepare"
    );
    assert_eq!(module.take_retired_assets(id), ["a.model"]);
    frame.now_ms = 17.;
    let (retired, _) = module.readback(id, &frame).unwrap();
    assert_eq!(before.data, retired.data, "retire A while B remains");
    module.lose_device();
    module.set_gpu(fixture::device().unwrap());
    assert!(module.take_assets(id).is_empty());
    let (recovered, _) = module.readback(id, &frame).unwrap();
    assert_eq!(before.data, recovered.data, "textureless module recovery");
}
