# Recipe: a press that lands inside one tick

Read a fire button with `pressed`, not `held`. A tap whose down and up both arrive
between two ticks (`sim.tap`, `tap world KeyF`, a fast click, a flick under pointer
lock) is never held at a tick, but it is pressed on the next one.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Shots { by_pressed: u32, by_held: u32, aim: f32 }
struct Gun;
impl Game for Gun {
    const ID: &'static str = "recipe-press";
    type Args = ();
    fn actions() -> Actions { Actions::new().button("fire", &["KeyF", "MouseLeft"]) }
    fn setup(w: &mut World, _: &()) { w.insert_resource(Shots::default()); }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut shots = w.resource_mut::<Shots>();
        if input.pressed("fire") { shots.by_pressed += 1; }
        if input.held("fire") { shots.by_held += 1; }
        shots.aim += input.pointer().map_or(0., |p| p.delta.x);
    }
}
let mut sim = Sim::<Gun>::new(()).unwrap();
sim.tap("KeyF");            // down and up before the next tick
sim.run(20.);
let shots = sim.world().resource::<Shots>();
assert_eq!((shots.by_pressed, shots.by_held), (1, 0));
drop(shots);
sim.hold("KeyF", 100.);     // a press across ticks: pressed once, held on each
assert_eq!(sim.world().resource::<Shots>().by_pressed, 2);
assert!(sim.world().resource::<Shots>().by_held > 1);
// A flick: the left button goes down, moves 30 points and lifts inside one tick.
let mut sim = Sim::<Gun>::new(()).unwrap();
for (phase, dx, buttons) in [(PointerPhase::Down, 0., 1), (PointerPhase::Move, 30., 1), (PointerPhase::Up, 0., 0)] {
    sim.input(InputEvent::Pointer { id: 1, phase, x: 100. + dx, y: 100., dx, dy: 0., buttons, at_ms: 0., player: 0 });
}
sim.run(20.);
let shots = sim.world().resource::<Shots>();
assert_eq!((shots.by_pressed, shots.by_held, shots.aim), (1, 0, 30.));
```

- `pressed` is a rising edge since the last tick; `released` is the falling one.
  Automatic fire is `held`; one shot per click is `pressed`; both is `pressed || held`.
- Mouse buttons bind as keys (`MouseLeft`, `MouseRight`, `MouseMiddle`), so a click
  that moves and lifts inside one tick is `pressed`, its motion in `delta`
  ([mouse look](mouse-look.md)). Inferring a click from position changes misses it.
- This fragment is an engine doc-test; `sim.tap` is the agent's `tap world KeyF`.
