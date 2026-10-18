# Recipe: a door that answers a message

Contract posts a command with `postMessage("open", "world")`. The next tick reads
it once; publish the state and emit an acknowledgement to the canvas's `message=`.

```rust
use exact_game::*;
#[derive(Default, Component)]
struct Door { open: bool }
struct Doors;
impl Game for Doors {
    const ID: &'static str = "recipe-door";
    type Args = ();
    fn setup(w: &mut World, _: &()) { w.spawn_named("door", Door::default()); }
    fn tick(w: &mut World, input: &Input, _: &()) {
        for message in input.messages() {
            if message == "open" {
                w.require_mut::<Door>("door").open = true;
                w.emit("opened");
            }
        }
        w.publish("open", w.require::<Door>("door").open);
    }
}
let mut sim = Sim::<Doors>::new(()).unwrap();
sim.post("open"); sim.run(20.);
assert!(sim.get::<Door>("door").unwrap().open);
assert_eq!(sim.world().published("open"), Some(Value::Bool(true)));
```

- Post commands, not per-frame state. Messages queue while paused and save until
  their tick. `emit` travels in the other direction.
- [Engine HUD and events](../engine/README.md#hud-and-events) covers delivery;
  this fragment is a doc-test. Register custom components for fresh restores.
