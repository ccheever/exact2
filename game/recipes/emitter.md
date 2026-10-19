# Recipe: bubbles, a wake and a splash

One `Emitter` per source. The engine steps every emitter after the tick; the tick
only moves it, starts or stops it, or queues a burst. Its births are saved, its
particles derived, so a restored stream draws where the continuous one does.

```rust
use exact_game::*;
use exact_game::emitter::{Shape, WorldSpace};
struct Tank;
impl Game for Tank {
    const ID: &'static str = "recipe-emitter";
    type Args = ();
    fn actions() -> Actions { Actions::new().button("splash", &["Space"]).button("row", &["KeyD"]) }
    fn setup(w: &mut World, _: &()) {
        // Bubbles: small, translucent, rising against drag.
        w.spawn_named("bubbler", (Transform::default(), Ambient, Emitter {
            shape: Shape::Sphere(0.2), rate: 8., lifetime: 3., speed: 0.3, spread: 0.3,
            gravity: Vec3::new(0., 0.6, 0.), drag: 1.5, size: [0.05, 0.12],
            color: [[0.8, 0.9, 1., 0.6], [0.8, 0.9, 1., 0.]], additive: false,
            bound: [-1., 0., -1., 1., 6., 1.], ..Emitter::default()
        }));
        // A wake: WorldSpace leaves each particle where the boat was at its birth.
        w.spawn_named("boat", (Transform::default(), WorldSpace::default(), Emitter {
            rate: 30., lifetime: 1.5, speed: 0.2, spread: 1.4, gravity: Vec3::ZERO,
            size: [0.15, 0.5], color: [[1.; 4], [1., 1., 1., 0.]], additive: false,
            bound: [-6., -1., -6., 6., 1., 6.], ..Emitter::default()
        }));
        // A splash: no rate, only bursts the tick queues.
        w.spawn_named("splash", (Transform::default(), Emitter { rate: 0., ..Emitter::sparks() }));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        // The boat moves, and leaves a wake, only while rowed.
        let (rowing, dt) = (input.held("row"), w.dt());
        if rowing { w.require_mut::<Transform>("boat").position.x += 2. * dt; }
        if w.require::<Emitter>("boat").running != rowing {
            w.require_mut::<Emitter>("boat").running = rowing;
        }
        if input.pressed("splash") {
            w.require_mut::<Emitter>("splash").state.burst += 12;
        }
    }
}
let mut sim = Sim::<Tank>::new(()).unwrap();
sim.hold("KeyD", 2000.);
let (hz, w) = (sim.world().hz(), sim.world());
let mut highest = f32::MIN;
w.require::<Emitter>("bubbler").particles(hz, 1., |p| highest = highest.max(p.position.y));
assert!(highest > 0.5, "bubbles rise: {highest}");
let boat = w.require::<Transform>("boat").position.x;
let mut furthest_behind = 0f32;
w.require::<Emitter>("boat").particles(hz, 1., |p| furthest_behind = furthest_behind.max(boat - p.position.x));
assert!(furthest_behind > 1., "the wake stays behind: {furthest_behind}");
assert_eq!(w.require::<Emitter>("splash").state.alive, 0);
sim.tap("Space");
sim.run(20.);
assert_eq!(sim.world().require::<Emitter>("splash").state.alive, 12);
// Once the boat stops and the wake and splash have died away, the clock rests: the
// bubbles are Ambient.
assert!(sim.settle());
```

- A running emitter changes every tick, so `clock settle` waits on it: mark a stream
  that never stops `Ambient`, stop one that belongs to motion (`running = false`), and
  a burst settles once its particles die.
- `gravity` is per emitter and may point up; `drag` damps `speed` and `gravity` alike.
- Without `WorldSpace` the whole stream moves with its entity: right for a torch's
  flame, wrong for a trail. A particle's `position` is then local.
- `bound` is the authored box layout and culling use; make it hold the stream.
- `running = false` stops births and lets the living ones finish. The
  [particles fixture](../games/particles-fixture) has more shapes; this fragment is
  an engine doc-test.
