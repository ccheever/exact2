# Recipe: touch controls

Declare each action once in `Actions`, keys and all, and give the HUD a button
whose `action` names it: a press on the button holds the action, and a stick's
button reports how far the finger moved from where it touched, unit-clamped at
60 points.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Steer { x: f32, y: f32, jumps: u32 }
struct Pad;
impl Game for Pad {
    const ID: &'static str = "recipe-touch";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().stick("move", Stick::wasd().or_arrows()).button("jump", &["Space"])
    }
    fn setup(w: &mut World, _: &()) { w.insert_resource(Steer::default()); }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let v = input.stick("move");
        let jumped = input.pressed("jump");
        let mut steer = w.resource_mut::<Steer>();
        (steer.x, steer.y) = (v.x, v.y);
        steer.jumps += u32::from(jumped);
    }
}
let control = |name: &str, phase, x, y| InputEvent::Control {
    name: name.into(), id: 1, phase, x, y, at_ms: 0., player: 0 };
let mut sim = Sim::<Pad>::new(()).unwrap();
sim.input(control("move", PointerPhase::Down, 48., 48.));
sim.input(control("move", PointerPhase::Move, 108., 48.));
sim.run(20.);
assert_eq!(sim.world().resource::<Steer>().x, 1.);
sim.input(control("move", PointerPhase::Up, 108., 48.));
sim.input(control("jump", PointerPhase::Down, 0., 0.));
sim.input(control("jump", PointerPhase::Up, 0., 0.));
sim.run(20.);
assert_eq!(sim.world().resource::<Steer>().x, 0.);
assert_eq!(sim.world().resource::<Steer>().jumps, 1);
```

Show the buttons only on a touch screen, by the primary input's
`exactViewport().pointer`, as the starter does; a mouse and keyboard player
never sees them, and the player's own setting can override it:

```contract
shape Viewport
  pointer: string
component Game
  resource viewport = exactViewport() as shape Viewport
  state touchControls = "auto"
  view
    canvas surface=world() testId="world" width="100%" height="100%"
      if touchControls == "always" || (touchControls == "auto" && viewport.pointer == "coarse")
        button action="move" role="slider" aria-label="Move" testId="move" position="absolute" left=24 bottom=24 width=96 height=96 border-radius=48
        button action="jump" testId="jump" position="absolute" right=24 bottom=24 padding=16
          text "Jump"
```

- The starter puts them in its HUD's bottom group, a strip along the bottom edge in
  portrait (`game/new/app.contract`); its panel's Touch controls button is the
  player's override.
- An iPad with a trackpad or mouse attached answers `fine`, and the buttons hide;
  the pointer changes while the game runs.
- Drive both on a desktop carrier: `bun scripts/agent.mjs web "prefer pointer coarse" tree`
  shows them, `prefer pointer fine` hides them.
- A button's `aria-keyshortcuts` takes its chord before the world hears it; do not
  also bind that key in `Actions`.
