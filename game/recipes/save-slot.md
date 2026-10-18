# Recipe: a save slot

The host stores opaque `Sim::save()` bytes. Restore registers the schema without
running setup. Contract's canvas uses its game save/restore storage seam.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Score { points: u32 }
struct Saved;
impl Game for Saved {
    const ID: &'static str = "recipe-save";
    type Args = ();
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) { w.register_resource::<Score>(); }
    fn setup(w: &mut World, _: &()) { w.insert_resource(Score::default()); }
    fn tick(w: &mut World, _: &Input, _: &()) { w.resource_mut::<Score>().points += 1; }
}
let mut sim = Sim::<Saved>::new(()).unwrap();
sim.run(100.);
let slot = sim.save().unwrap();
let expected = sim.world().hash();
sim.run(100.);
assert_ne!(sim.world().hash(), expected);
sim.restore(&slot).unwrap();
assert_eq!(sim.world().hash(), expected);
assert_eq!(sim.world().resource::<Score>().points, 6);
```

- Never hand-list components in a save. Custom types that can appear after setup
  must register; presentation is rebuilt and never saved.
- This fragment is a doc-test; [starter tests](../new/logic/tests/sim.rs) restore
  a live game. [Integration](../README.md#exact2-integration) covers app storage.
