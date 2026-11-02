# Recipe: a timer HUD

Save an endpoint tick and publish the remaining seconds. Contract renders the HUD
from `exactSurface("world")`; use `font-variant-numeric="tabular-nums"` on its number.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Timer { ends: u64 }
struct Countdown;
impl Game for Countdown {
    const ID: &'static str = "recipe-timer";
    const HZ: u32 = 120;
    type Args = ();
    fn setup(w: &mut World, _: &()) { w.insert_resource(Timer { ends: 240 }); }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let left = w.resource::<Timer>().ends.saturating_sub(w.tick_end().tick);
        w.publish("seconds", left.div_ceil(w.hz() as u64) as u32);
    }
}
let mut sim = Sim::<Countdown>::new(()).unwrap();
sim.run(1000.);
assert_eq!(sim.world().published("seconds").unwrap().as_number(), Some(1.));
sim.run(1000.);
assert_eq!(sim.world().published("seconds").unwrap().as_number(), Some(0.));
```

- Use the endpoint being authored, not wall time. Counting display frames drifts.
- Register custom resources in `Game::register` if a fresh restore has not run setup.
- The [starter HUD](../new/app.contract) demonstrates publications. This fragment
  is a doc-test; [engine HUD](../engine/README.md#hud-and-events) covers records.
