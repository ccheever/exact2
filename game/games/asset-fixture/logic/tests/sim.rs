use asset_fixture_logic::AssetFixture;
use exact_game::{asset::Model, bin, Sim};
#[path = "../../../../paranoid-test.rs"]
mod paranoid;
#[test]
fn every_tick_save_matches_normal_script() {
    paranoid::compare(
        || {
            let mut sim = Sim::<AssetFixture>::new(()).unwrap();
            let model = Model {
                bounds: [-0.5, -0.5, -0.5, 0.5, 0.5, 0.5],
                ..Model::default()
            };
            sim.asset("crate.model", Some(&bin::to_vec(&model)))
                .unwrap();
            sim
        },
        |sim| {
            sim.run(1000.0);
        },
    );
}
