use exact_game::*;

/// What reached the world: the HUD's Buy button (`action="buy"`, which
/// declares `aria-keyshortcuts="1"`) and the world's own Digit1 binding.
#[derive(Default, Resource)]
pub struct Shop {
    pub buys: u32,
    pub ones: u32,
}
#[derive(Default, Data)]
struct Hud {
    buys: u32,
}

pub struct ShortcutFixture;
impl Game for ShortcutFixture {
    const ID: &'static str = "shortcut-fixture";
    const HZ: u32 = 60;
    type Args = ();
    fn actions() -> Actions {
        Actions::new().button("buy", &[]).button("one", &["Digit1"])
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Environment {
            background: Some([0.04, 0.08, 0.12]),
            fog: None,
            bloom: None,
            ..Default::default()
        });
        w.insert_resource(Shop::default());
        w.spawn_named(
            "crate",
            (
                Transform::at(0., 0.5, 0.),
                Mesh::cube(1.),
                Material::rgb(0.7, 0.5, 0.2),
            ),
        );
        w.spawn_named(
            "camera",
            (
                Transform::at(0., 2., 5.).looking_at(Vec3::new(0., 0.5, 0.), Vec3::Y),
                Camera::default(),
            ),
        );
        w.publish_record(&Hud::default());
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut shop = w.resource_mut::<Shop>();
        shop.buys += u32::from(input.pressed("buy"));
        shop.ones += u32::from(input.pressed("one"));
        let buys = shop.buys;
        w.publish_record(&Hud { buys });
    }
}
