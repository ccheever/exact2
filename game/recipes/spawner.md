# Recipe: a spawner

Save the next spawn tick and serial. Draw randomness from the world.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Spawner { next: u64, serial: u32 }
struct Spawn;
impl Game for Spawn {
    const ID: &'static str = "recipe-spawner";
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) { w.register_resource::<Spawner>(); }
    fn setup(w: &mut World, _: &()) { w.insert_resource(Spawner::default()); }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let mut spawner = w.resource_mut::<Spawner>();
        if w.tick() < spawner.next { return; }
        let name = format!("drop-{}", spawner.serial);
        spawner.serial += 1;
        spawner.next = w.tick() + w.hz() as u64;
        drop(spawner);
        w.spawn_named(name, (Transform::at(w.rand(-2.0..2.0), 1., 0.), Mesh::sphere(0.2)));
    }
}
let mut sim = Sim::<Spawn>::new(()).unwrap();
sim.run(1500.);
assert_eq!(sim.world().count::<Mesh>(|_| true), 2);
let saved = sim.save().unwrap();
sim.run(1000.);
let expected = sim.world().hash();
sim.restore(&saved).unwrap(); sim.run(1000.);
assert_eq!(sim.world().hash(), expected);
```

- Release row/resource guards before spawning. A structural write needs `&mut World`.
- A game owns its calls; no component can register a tick callback. This fragment
  is a doc-test; [particles](../games/particles-fixture) shows an engine emitter.
