# Recipe: a pickup and its prompt

Select once, read the handle's name for the HUD, and retarget its glow on a press.

```rust
use exact_game::*;
#[derive(Default, Component)]
struct Pickup { taken: bool }
struct Collect;
impl Game for Collect {
    const ID: &'static str = "recipe-pickup";
    type Args = ();
    fn actions() -> Actions { Actions::new().button("take", &["KeyE"]) }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", Transform::default());
        w.spawn_named("coin", (Transform::at(1., 0., 0.), Pickup::default(),
            Mesh::sphere(0.2), Material::glow([1.; 3]), Glow::default()));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut prompt = "";
        if let Some((_, mut coin)) = w.nearest_xz_mut::<Pickup>("player", 1.5, |p| !p.taken) {
            if input.pressed("take") {
                coin.taken = true;
                coin.glow_to(1., 0.5);
            } else { prompt = coin.name().unwrap_or_default(); }
        }
        w.publish("near", prompt);
        w.publish("taken", w.count::<Pickup>(|p| p.taken));
    }
}
let mut sim = Sim::<Collect>::new(()).unwrap();
sim.run(20.);
assert_eq!(sim.world().published("near").unwrap().as_str(), Some("coin"));
sim.tap("KeyE"); sim.run(20.);
assert!(sim.get::<Pickup>("coin").unwrap().taken);
assert_eq!(sim.world().require::<Glow>("coin").0.target, 1.);
assert_eq!(sim.world().published("near").unwrap().as_str(), Some(""));
```

- Radii are inclusive XZ metres; equal distances choose entity order.
- Keep authored emission constant: `Glow` multiplies it. Copying the tween into
  `Material.emissive` multiplies twice.
- Used in [Beacons](../games/beacons/logic/src/lib.rs); this fragment is a doc-test.
