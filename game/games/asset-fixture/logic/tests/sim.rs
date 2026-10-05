#[path = "../../../../bake/tests/support/mod.rs"]
mod baked;
use asset_fixture_logic::AssetFixture;
use exact_game::Sim;

#[test]
fn proof_endpoint_pin() {
    let mut assets =
        baked::assets(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/crate.gltf"))
            .unwrap();
    assets.insert(
        "island.level.json".into(),
        include_bytes!("../../assets/island.level.json").to_vec(),
    );
    let mut sim = Sim::<AssetFixture>::with_assets((), |name| {
        assets.get(name).cloned().ok_or(name.to_owned())
    })
    .unwrap();
    assert!(!sim.world().model("crate.model").unwrap().meshes.is_empty());
    for (name, expected) in [
        ("terrainA", 0.375),
        ("terrainB", 1.4375),
        ("terrainC", 2.25),
        ("slopeContact", 0.6399267),
    ] {
        let exact_game::Value::Number(actual) = sim.world().published(name).unwrap() else {
            panic!("missing {name}")
        };
        assert!((actual - expected).abs() < 0.0001, "{name}: {actual}");
    }
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 60);
    sim.assert_pin(include_str!("../../pins.json"));
}
