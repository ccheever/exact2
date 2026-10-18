# Recipe: mouse look

Save yaw and pitch, turn the camera in the tick, and attach `MouseLook` with the
same rates so unshown motion reaches the next displayed frame.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Aim { yaw: f32, pitch: f32 }
struct Look;
impl Game for Look {
    const ID: &'static str = "recipe-look";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Aim::default());
        w.spawn_named("camera", (Transform::default(), Camera::default(), MouseLook {
            yaw_per_point: -0.0025, pitch_per_point: -0.0025, pitch_limit: 1.4 }));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let delta = input.pointer().map_or(Vec2::ZERO, |p| p.delta);
        let mut aim = w.resource_mut::<Aim>();
        aim.yaw -= delta.x * 0.0025;
        aim.pitch = (aim.pitch - delta.y * 0.0025).clamp(-1.4, 1.4);
        w.require_mut::<Transform>("camera").rotation =
            Quat::from_rotation_y(aim.yaw) * Quat::from_rotation_x(aim.pitch);
    }
}
let mut sim = Sim::<Look>::new(()).unwrap();
sim.input(InputEvent::Pointer { id: 1, phase: PointerPhase::Move,
    x: 100., y: 100., dx: 40., dy: 0., buttons: 0, at_ms: 0. });
sim.run(20.);
assert!((sim.world().resource::<Aim>().yaw + 0.1).abs() < 0.00001);
```

- Use `delta`, not differences of absolute positions. Declare `data-pointer-lock`
  in app.json's `data`, and set `data-pointer-lock="true"` on the canvas.
- [The look tests](../engine/tests/look.rs) measure tick/display rates and latency;
  this fragment is also a doc-test. Register `Aim` for fresh restores.
