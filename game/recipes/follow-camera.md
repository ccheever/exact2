# Recipe: follow a player

Attach a `Follow` to a camera. The engine places it after setup and steps it after
each tick; an explicit `scene::follow(w)` chooses an earlier order.

```rust
use exact_game::*;
struct Chase;
impl Game for Chase {
    const ID: &'static str = "recipe-follow";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        let player = w.spawn_named("player", Transform::default());
        w.spawn_named("camera", (Transform::default(), Camera::default(),
            Follow::new(player).offset(0., 9., 13.).lag(0.15)));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("player").position.x += w.dt();
    }
}
let mut sim = Sim::<Chase>::new(()).unwrap();
assert_eq!(sim.local_position("camera").unwrap(), Vec3::new(0., 9., 13.));
sim.run(1000.);
let camera = sim.local_position("camera").unwrap();
assert!(camera.x > 0.7 && camera.x < 1.);
```

- `lag` is a time constant in seconds, not a speed. Zero snaps.
- Missing targets leave the camera alone. Restore preserves initialized poses.
- Used in [the starter](../new/logic/src/lib.rs). This fragment is an engine doc-test.
