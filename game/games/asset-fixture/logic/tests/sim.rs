#[path = "../../../../bake/tests/support/mod.rs"]
mod baked;
use asset_fixture_logic::AssetFixture;
use exact_game::Sim;

#[test]
fn proof_endpoint_pin() {
    let assets =
        baked::assets(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../art/crate.gltf"))
            .unwrap();
    let mut sim = Sim::<AssetFixture>::with_assets((), |name| {
        assets.get(name).cloned().ok_or(name.to_owned())
    })
    .unwrap();
    assert!(!sim.world().model("crate.model").unwrap().meshes.is_empty());
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 60);
    sim.assert_pin(include_str!("../../pins.json"));
}
