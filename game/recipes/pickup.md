# Recipe: pickups that drift to the player, and their prompt

Mark pickups `Collectable`; each tick `collect::drift` moves those within reach
toward the player and says which arrived. The game removes those and credits
itself. The HUD prompt names the nearest pickup in sight.

```rust
use exact_game::*;
use exact_game::collect::{drift, Collectable};
#[derive(Default, Resource)]
struct Purse { coins: u32 }
struct Coins;
impl Game for Coins {
    const ID: &'static str = "recipe-pickup";
    const HZ: u32 = 120;
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        w.register_resource::<Purse>();
    }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Purse::default());
        w.spawn_named("player", Transform::at(0., 0.9, 0.));
        for (i, x) in [2., 4.5].into_iter().enumerate() {
            w.spawn_named(format!("coin-{i}"), (Transform::at(x, 0.1, 0.), Collectable,
                Mesh::cylinder(0.2, 0.05), Material::hex("#f2c94c")));
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let dt = w.dt();
        let player = w.require::<Transform>("player").position;
        let mut taken = Vec::new();
        // Within 3 m a coin flies to the player at 6 m/s.
        for (coin, pose) in w.query::<&mut Transform>().with::<Collectable>().iter() {
            let d = drift(pose.position, player, 3., 6., dt);
            pose.position = d.position;
            if d.taken { taken.push(coin); }
        }
        for coin in taken {
            w.despawn(coin);
            w.resource_mut::<Purse>().coins += 1;
        }
        let near = w.nearest_xz::<Collectable>("player", 5.).and_then(|e| w.name(e));
        w.publish("near", near.unwrap_or_default());
        w.publish("coins", w.resource::<Purse>().coins);
    }
}
let mut sim = Sim::<Coins>::new(()).unwrap();
let coins = |sim: &Sim<Coins>| sim.world().published("coins").unwrap().as_number();
// coin-0 is 2.2 m away: it drifts for 44 ticks and is taken on arrival, not before.
sim.run(350.);
assert!(sim.world().named("coin-0").is_some() && coins(&sim) == Some(0.));
sim.run(50.);
assert!(sim.world().named("coin-0").is_none() && coins(&sim) == Some(1.));
// coin-1, 4.5 m out, stays put; the prompt names it.
assert_eq!(sim.global_position("coin-1"), Some(Vec3::new(4.5, 0.1, 0.)));
assert_eq!(sim.world().published("near").unwrap().as_str(), Some("coin-1"));
sim.world().require_mut::<Transform>("player").position.x = 2.;
sim.run(500.);
assert_eq!(coins(&sim), Some(2.));
assert_eq!(sim.world().published("near").unwrap().as_str(), Some(""));
```

- `drift` is 3D: a coin on the floor rises into the player's middle. Its radius is
  inclusive and its step never passes the collector.
- `Collectable` is a marker for queries (`.with::<Collectable>()`, `nearest_xz`).
  Adding it, or any component type, changes the world hash: a game repins.
- A pickup that waits for a press: `w.nearest_xz_mut::<Pickup>("player", 1.5, |p| !p.taken)`
  yields the row and its handle, whose `name()` feeds the prompt and whose
  `glow_to` lights it ([Beacons](../games/beacons/logic/src/lib.rs)). Keep authored
  emission constant: `Glow` multiplies it.
- Collect the taken entities, then despawn: a structural write needs the query's
  lease released. This fragment is a doc-test.
