# Recipe: seek, flee and wander

A creature that eats, runs from a predator and meanders otherwise, and the
predator that hunts it: `exact_game::steer` returns this tick's `Steer {
velocity, facing }`, and the game writes it into the motion the entity has.

```rust
use exact_game::*;
/// The fish's turning rate for `wander`, kept between ticks.
#[derive(Default, Component)]
struct Fish { turn: f32 }
struct Tank;
impl Game for Tank {
    const ID: &'static str = "recipe-steering";
    const HZ: u32 = 120;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("fish", (Transform::default(), Fish::default()));
        w.spawn_named("food", Transform::at(0., 0., 6.));
        w.spawn_named("shark", Transform::at(30., 0., 0.));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let dt = w.dt();
        let food = w.global_position("food");
        let fish_at = w.require::<Transform>("fish").position;
        // A predator is seek and flee in the game's own tick: it hunts the fish
        // and keeps out of the light at the origin.
        let shark = {
            let mut pose = w.require_mut::<Transform>("shark");
            let s = steer::flee(&pose, Vec3::ZERO, 2., 3., 2., dt)
                .unwrap_or_else(|| steer::seek(&pose, fish_at, 2.5, 2., dt));
            pose.position += s.velocity * dt;
            pose.rotation = s.facing;
            pose.position
        };
        // The fish: run from the shark first, else eat, else meander.
        {
            let mut pose = w.require_mut::<Transform>("fish");
            let mut fish = w.require_mut::<Fish>("fish");
            let s = steer::flee(&pose, shark, 4., 3., 4., dt).unwrap_or_else(|| match food {
                Some(food) => steer::arrive(&pose, food, 1., 2., 3., dt),
                None => steer::wander(&pose, &mut w.rng(), &mut fish.turn, 1., 2., dt),
            });
            pose.position += s.velocity * dt;
            pose.rotation = s.facing;
        }
        if w.global_position("fish").zip(food).is_some_and(|(a, b)| a.distance(b) < 0.25) {
            let food = w.named("food").unwrap();
            w.despawn(food);
            w.publish("ate", w.tick() as u32);
        }
    }
}
let mut sim = Sim::<Tank>::new(()).unwrap();
// The food is 6 m behind the fish: it turns half a circle at 3 rad/s (1.05 s),
// swims at 2 m/s and slows over the last metre. It eats at tick 477, 3.98 s.
sim.run(5000.);
assert_eq!(sim.world().published("ate").unwrap().as_number(), Some(477.));
// With no food it wanders on the world's random stream, level, at 1 m/s.
sim.run(2000.);
let at = sim.global_position("fish").unwrap();
assert_eq!(at.y, 0.);
// The shark comes for it, and the fish keeps out of its reach.
sim.world().require_mut::<Transform>("shark").position = at + Vec3::new(6., 0., 0.);
sim.run(5000.);
let gap = sim.global_position("fish").unwrap().distance(sim.global_position("shark").unwrap());
assert!(gap > 3.9, "the fish holds the edge of its 4 m radius: {gap}");
```

- The numbers are the author's: speeds in metres per second, turn rates in
  radians per second. A mover slows while it faces away from where it wants to
  go, so it turns before it goes instead of circling its target.
- `flee` is `None` outside its radius; `unwrap_or_else` falls back to the next
  behaviour. Priorities in the order written are the whole arbitration.
- `wander` keeps its turning rate in the game's component (`Fish.turn`) and draws
  from `w.rng()`, so a restored save wanders the same way.
- Steering is 3D. A creature on the ground steers toward targets at its own
  height, `Vec3::new(target.x, pose.position.y, target.z)`, or it pitches.
- Any motion takes a steer. A kinematic mover writes it as above. A
  [`Character`](../engine/src/character.rs) steps with `s.velocity / character.speed`
  as its wish and takes `s.facing` as its rotation. A physics body applies the
  change of velocity as its own force. Nothing here is saved or stepped: a steer is
  a value for this tick.
- Schooling is [its own recipe](schooling.md). This fragment is an engine doc-test.
