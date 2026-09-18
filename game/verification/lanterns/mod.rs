include!("game.rs");

fn bake_scene() -> exact_game_scene::bake::Baked {
    exact_game_scene::bake::compile(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../verification/lanterns/scene.json"),
        &scene_types(),
        Lanterns::assets(),
    )
    .unwrap()
}
#[path = "before.rs"]
mod before;
#[path = "scene.rs"]
mod historical_scene;
