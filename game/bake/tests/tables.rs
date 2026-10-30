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

#[derive(Default, Data)]
struct Named {
    name: String,
}
struct Ordered;
impl Game for Ordered {
    const ID: &'static str = "bake-tables-ordered";
    // Declared out of byte order: the carrier is a map by name.
    const TABLES: &'static [asset::Table] = &[
        asset::Table::of::<Named>("weapons.json"),
        asset::Table::of::<Named>("labels.json"),
    ];
    type Args = ();
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
/// The bytes the dev loop's table watcher writes too (game/app/tables.mjs `carrier`,
/// host/web/tests/tables.test.mjs), so neither rewrites the other's unchanged carrier.
const ORDERED_CARRIER: &str =
    r#"{"labels.json":"{\t\"name\": \"a\\\"b\"}\n","weapons.json":"{\"name\":\"épée\"}\r\n"}"#;
#[test]
fn the_carrier_is_a_map_by_name_in_the_bytes_the_dev_loop_writes() {
    let directory = std::env::temp_dir().join(format!("exact-tuning-order-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(directory.clone());
    std::fs::write(directory.join("weapons.json"), "{\"name\":\"épée\"}\r\n").unwrap();
    std::fs::write(directory.join("labels.json"), "{\t\"name\": \"a\\\"b\"}\n").unwrap();
    exact_game_bake::bake_game_tables::<Ordered>(&directory).unwrap();
    let asset = directory.join("assets").join(asset::TABLE_ASSET);
    assert_eq!(std::fs::read_to_string(asset).unwrap(), ORDERED_CARRIER);
}
