# Recipe: a third-person orbit camera

Save the orbit's yaw and elevation, turn them from pointer `delta`, and place the
camera on a sphere around the player every tick. Movement turns with the camera, so
W always walks away from it.

```rust
use exact_game::*;
#[derive(Default, Resource)]
struct Orbit { yaw: f32, elevation: f32 }
const DISTANCE: f32 = 6.;
const SENSITIVITY: f32 = 0.004;
struct Third;
impl Game for Third {
    const ID: &'static str = "recipe-orbit";
    type Args = ();
    fn actions() -> Actions { Actions::new().stick("move", Stick::wasd()) }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) { w.register_resource::<Orbit>(); }
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Orbit { yaw: 0., elevation: 0.35 });
        w.spawn_named("player", (Transform::at(0., 0.9, 0.), Mesh::capsule(0.4, 1.8)));
        w.spawn_named("camera", (Transform::default(), Camera::default()));
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let delta = input.pointer().map_or(Vec2::ZERO, |p| p.delta);
        let (yaw, elevation) = {
            let mut orbit = w.resource_mut::<Orbit>();
            orbit.yaw -= delta.x * SENSITIVITY;
            orbit.elevation = (orbit.elevation + delta.y * SENSITIVITY).clamp(-0.3, 1.2);
            (orbit.yaw, orbit.elevation)
        };
        let turn = Quat::from_rotation_y(yaw);
        let step = turn * input.stick_xz("move") * 4. * w.dt();
        let target = {
            let mut player = w.require_mut::<Transform>("player");
            player.position += step;
            player.position + Vec3::Y * 0.6
        };
        let eye = target + turn * Quat::from_rotation_x(-elevation) * Vec3::new(0., 0., DISTANCE);
        *w.require_mut::<Transform>("camera") = Transform::at(eye.x, eye.y, eye.z).looking_at(target, Vec3::Y);
    }
}
let mut sim = Sim::<Third>::new(()).unwrap();
// A quarter turn of pointer motion swings the camera round to the player's side.
let points = std::f32::consts::FRAC_PI_2 / SENSITIVITY;
sim.input(InputEvent::Pointer { id: 1, phase: PointerPhase::Move,
    x: 100., y: 100., dx: points, dy: 0., buttons: 0, at_ms: 0., player: 0 });
sim.run(20.);
let eye = sim.local_position("camera").unwrap();
let before = sim.local_position("player").unwrap();
assert!(((eye - before - Vec3::Y * 0.6).length() - DISTANCE).abs() < 0.01);
assert!(eye.x < -5., "the camera is now beside the player: {eye}");
sim.hold("KeyW", 500.);
let walked = sim.local_position("player").unwrap() - before;
assert!(walked.x > 1.5 && walked.z.abs() < 0.01, "W walks away from the camera: {walked}");
// A restore resumes the same view: the orbit is saved, so W still walks the same way.
let saved = sim.save().unwrap();
let mut restored = Sim::<Third>::new(()).unwrap();
restored.restore(&saved).unwrap();
for s in [&mut sim, &mut restored] { s.hold("KeyW", 200.); }
assert_eq!(restored.local_position("camera"), sim.local_position("camera"));
```

- Keep yaw and elevation in a saved, registered resource: a restore resumes the
  same view. Clamp elevation short of straight up or down, where `looking_at`
  has no single answer.
- For a camera that trails rather than orbits, use [follow camera](follow-camera.md);
  for first person, [mouse look](mouse-look.md). On the web, pointer lock
  (`data-pointer-lock`, as mouse look describes) keeps `delta` coming when the
  pointer would leave the canvas. This fragment is an engine doc-test.
