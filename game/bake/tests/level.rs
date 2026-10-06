use exact_game::{asset::Level, Data, Game, Input, Vec3, World};
#[derive(Default, Data)]
struct Island {
    seed: u64,
    lanterns: Vec<Vec3>,
    heights: Vec<f32>,
    sign: String,
}
struct LevelGame;
impl Game for LevelGame {
    const ID: &'static str = "typed-level-bake";
    const LEVELS: &'static [Level] = &[Level::of::<Island>("island.level.json")];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn level_bake_validates_the_authored_file_before_delivery() {
    let dir = std::env::temp_dir().join(format!("exact-level-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let level = Level::of::<Island>("island.level.json");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    let source = dir.join("assets").join(level.name);
    std::fs::write(
        &source,
        r#"{"seed":7,"lanterns":[[1,"bad",3]],"sign":"hello"}"#,
    )
    .unwrap();
    let error = exact_game_bake::bake_game_levels::<LevelGame>(&dir).unwrap_err();
    assert!(error.contains("lanterns.0"), "{error}");
    std::fs::write(&source, r#"{"heights":[1,"bad"]}"#).unwrap();
    let error = exact_game_bake::bake_game_levels::<LevelGame>(&dir).unwrap_err();
    assert!(error.contains("heights.1"), "{error}");
    std::fs::write(
        &source,
        r#"{"seed":7,"lanterns":[[1,2,3]],"heights":[0.25,0.5],"sign":"hello"}"#,
    )
    .unwrap();
    exact_game_bake::bake_game_levels::<LevelGame>(&dir).unwrap();
    // The authored file is what is delivered: the bake checks it, writes nothing.
    assert_eq!(std::fs::read_dir(dir.join("assets")).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
