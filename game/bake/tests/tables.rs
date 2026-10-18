use exact_game::*;
#[derive(Default, Data)]
struct Movement {
    speed: f32,
}
struct Tuning;
impl Game for Tuning {
    const ID: &'static str = "bake-tables";
    const TABLES: &'static [asset::Table] = &[asset::Table::of::<Movement>("movement.json")];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[test]
fn bake_validates_before_replacing_the_complete_asset() {
    let directory = std::env::temp_dir().join(format!("exact-tuning-bake-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(directory.clone());
    let source = directory.join("movement.json");
    let asset = directory.join("assets").join(asset::TABLE_ASSET);
    std::fs::write(&source, r#"{"speed":4}"#).unwrap();
    exact_game_bake::bake_game_tables::<Tuning>(&directory).unwrap();
    let good = std::fs::read(&asset).unwrap();
    for text in [r#"{"speed":"fast"}"#, "{}"] {
        std::fs::write(&source, text).unwrap();
        let error = exact_game_bake::bake_game_tables::<Tuning>(&directory).unwrap_err();
        assert!(
            error.contains("movement.json") && error.contains("speed"),
            "{error}"
        );
        assert_eq!(std::fs::read(&asset).unwrap(), good);
    }
}
