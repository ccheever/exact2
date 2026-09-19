use asset_fixture_logic::AssetFixture;
use exact_game::{asset::Model, bin, Sim};

#[test]
fn proof_endpoint_pin() {
    // Geometry is presentation data, outside the simulation hash. The host proof
    // separately loads and verifies the baked textured crate.
    let mut sim =
        Sim::<AssetFixture>::with_assets((), |_| Ok::<_, String>(bin::to_vec(&Model::default())))
            .unwrap();
    sim.run(1000.);
    assert_eq!(sim.world().tick(), 60);
    sim.assert_pin(include_str!("../../pins.json"));
}
