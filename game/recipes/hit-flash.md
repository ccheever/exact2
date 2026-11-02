# Recipe: a material flash on hit

Save when the hit happened; derive the flash from it in `Game::present`. The
flash is presentation: it never enters a save or the hash, and a restored world
flashes exactly as the continuous one did.

```rust
use exact_game::*;
#[derive(Default, Component)]
struct Hurt { at: Option<u64> }
const FLASH_SECONDS: f32 = 0.2;
struct Arena;
impl Game for Arena {
    const ID: &'static str = "recipe-hit-flash";
    type Args = ();
    fn actions() -> Actions { Actions::new().button("hit", &["KeyH"]) }
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("crab", (Transform::default(), Mesh::sphere(0.5),
            Material::rgb(0.8, 0.3, 0.2), Hurt::default()));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        if input.pressed("hit") {
            let now = w.tick();
            w.require_mut::<Hurt>("crab").at = Some(now);
        }
    }
    fn present(p: &mut Present<'_>, _: &()) {
        let (tick, span) = (p.tick(), FLASH_SECONDS * p.hz() as f32);
        let mut flashes = Vec::new();
        p.for_each::<Hurt>(|e, hurt| if let Some(at) = hurt.at {
            let left = 1. - (tick - at) as f32 / span;
            if left > 0. { flashes.push((e, left)); }
        });
        for (e, left) in flashes {
            p.insert(e, Tint { color: [1.; 4].into(), emissive: [4. * left; 3] });
        }
    }
}
let mut sim = Sim::<Arena>::new(()).unwrap();
sim.run(20.);
assert!(sim.world().get::<Tint>("crab").is_none());
sim.tap("KeyH");
sim.run(50.);
let flash = sim.world().require::<Tint>("crab").emissive[0];
assert!(flash > 0. && flash < 4.);
let saved = sim.save().unwrap();
let mut restored = Sim::<Arena>::new(()).unwrap();
restored.restore(&saved).unwrap();
assert_eq!(restored.world().require::<Tint>("crab").emissive[0], flash);
for s in [&mut sim, &mut restored] { s.run(50.); }
assert_eq!(restored.world().require::<Tint>("crab").emissive[0],
    sim.world().require::<Tint>("crab").emissive[0], "the restored flash fades as the original does");
sim.run(500.);
assert!(sim.world().get::<Tint>("crab").is_none(), "the flash is over");
```

- Writing `Material` or a handle's `tint` instead changes simulation state: it
  saves, hashes and keeps `clock settle` awake ([pitfalls](../../docs/game-pitfalls.md)).
- `Tint` is for primitives. A model's look changes per material with
  `MaterialOverrides` (a `MaterialOverride` with `emissive` per material index) or
  per node with `NodeMaterials`, written from `present` the same way.
- This fragment is an engine doc-test.
