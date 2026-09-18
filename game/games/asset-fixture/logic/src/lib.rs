use exact_game::*;
pub struct AssetFixture;
impl Game for AssetFixture {
    const ID: &'static str = "asset-fixture";
    const ASSETS: &'static [&'static str] = &["crate.model"];
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        assert!(w.model("crate.model").is_some());
        w.spawn_named(
            "crate",
            (Transform::at(0.0, 0.6, 0.0), Mesh::asset("crate.model")),
        );
        w.spawn_named(
            "ground",
            (
                Transform::default(),
                Mesh::plane(12., 12.),
                Material::rgb(0.2, 0.25, 0.3),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(2.8, 2.0, 3.8).looking_at(Vec3::new(0., 0.6, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.publish("ready", true);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.publish("tick", w.tick() as u32);
    }
}
