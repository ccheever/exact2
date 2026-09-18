use asset_fixture_logic::AssetFixture;
use exact_game::{asset::Model, bin, Sim, World};

#[test]
fn proof_endpoint_pin() {
    let mut sim = Sim::<AssetFixture>::new(()).unwrap();
    // Geometry is presentation data, outside the simulation hash. The host proof
    // separately loads and verifies the baked textured crate.
    sim.load_assets(|_| Ok::<_, String>(bin::to_vec(&Model::default())))
        .unwrap();
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 60);
    World::assert_pin(
        include_str!("../../pins.json"),
        "asset-fixture",
        60,
        sim.world().hash(),
    );
}
