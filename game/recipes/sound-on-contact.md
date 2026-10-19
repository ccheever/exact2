# Recipe: a sound on contact

Play on the landing edge, then step audio explicitly after gameplay.

```rust
use exact_game::*;
use exact_game::{audio::{self, Synth}, character::Character};
struct Landing;
impl Game for Landing {
    const ID: &'static str = "recipe-contact";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.sounds([("land", Synth::sine(220.).seconds(0.1))]);
        w.spawn_named("player", (Transform::at(0., 1., 0.), Character::new()));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let contact = w.character("player").step(Vec3::ZERO, false);
        if contact.landed { w.play("land").start(); }
        audio::step(w);
    }
}
let mut sim = Sim::<Landing>::new(()).unwrap();
sim.run(1500.);
let hits = sim.world().journal().iter().filter(|e| e.line.contains("sfx land")).count();
assert_eq!(hits, 1);
sim.run(500.);
assert_eq!(sim.world().journal().iter().filter(|e| e.line.contains("sfx land")).count(), hits);
```

- `grounded` stays true while resting; `landed` fires once. For physical collisions,
  use physics contact events after `physics::step` instead.
- A play builder does nothing until `.start()`. Definitions and voices save; audio
  execution is separate. See [audio](../audio/README.md) and its fixture.
