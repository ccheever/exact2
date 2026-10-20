# Recipe: spawners

A `spawn::Spawner` saves when the next spawn is owed and its serial. Embed one
per kind of thing in a resource, call `due` from the tick, and place each spawn
with world randomness.

```rust
use exact_game::*;
use exact_game::spawn::Spawner;
#[derive(Default, Resource)]
struct Spawners { fish: Spawner, bubbles: Spawner }
struct Tank;
impl Game for Tank {
    const ID: &'static str = "recipe-spawner";
    const HZ: u32 = 120;
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        w.register_resource::<Spawners>();
    }
    fn setup(w: &mut World, _: &()) {
        let hz = w.hz() as u64;
        // A fish every 2 s; bubbles four a second, up to four owed to a late call.
        w.insert_resource(Spawners { fish: Spawner::every(2 * hz), bubbles: Spawner::every(hz / 4).cap(4) });
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let tick = w.tick();
        // Take the serials, then release the resource: spawning needs `&mut World`.
        let mut s = w.resource_mut::<Spawners>();
        let fish: Vec<u32> = std::iter::from_fn(|| s.fish.due(tick)).collect();
        let bubbles: Vec<u32> = std::iter::from_fn(|| s.bubbles.due(tick)).collect();
        drop(s);
        for serial in fish {
            let at = Transform::at(w.rand(-4.0..4.0), 1., w.rand(-4.0..4.0));
            w.spawn_named(format!("fish-{serial}"), (at, Mesh::sphere(0.3)));
        }
        for serial in bubbles {
            w.spawn_named(format!("bubble-{serial}"), (Transform::at(0., 0.1, 0.), Mesh::sphere(0.05)));
        }
    }
}
let mut sim = Sim::<Tank>::new(()).unwrap();
sim.run(5000.);
let saved = sim.save().unwrap();
sim.run(5000.);
// Ticks 0 to 1199 ran: a fish at ticks 240, 480, 720 and 960, a bubble at each
// multiple of 30 up to 1170.
let world = sim.world();
assert_eq!(world.tick(), 1200);
assert_eq!(world.count::<Mesh>(|m| *m == Mesh::sphere(0.3)), 4);
assert_eq!(world.count::<Mesh>(|m| *m == Mesh::sphere(0.05)), 39);
assert!(world.named("fish-3").is_some() && world.named("fish-4").is_none());
let expected = world.hash();
// A fresh game restored from the save spawns the same serials in the same places.
let mut fresh = Sim::<Tank>::new(()).unwrap();
fresh.restore(&saved).unwrap();
fresh.run(5000.);
assert_eq!(fresh.world().hash(), expected);
```

- `due` takes `w.tick()`, the number of the tick running. The first spawn is owed
  at tick `every`, then every `every` ticks, on the grid of multiples of `every`
  from tick 0: by the tick numbered `t`, `t / every`. Set `next` for another first tick.
- Call `due` in a loop. A call that comes late (the game skipped it while a gate
  was shut) owes one spawn per interval that passed, up to `cap` (one unless set);
  the oldest beyond the cap are skipped and the grid is kept.
- Serials count from zero and save with the resource, so a restore never repeats a
  name. `Spawner::default()` has `every` 0 and never spawns.
- Adding a `Spawner` to an existing resource changes its hash: a game repins. A
  game owns its calls; nothing steps a spawner but `due`. This fragment is a doc-test;
  [particles](../games/particles-fixture) shows an engine emitter.
