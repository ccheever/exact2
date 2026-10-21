# Coming from three.js, Unity or Godot

| three.js | Unity | Godot | exact2 |
|---|---|---|---|
| `GridHelper` | — | `GridMap` | `Material::grid(color, spacing)` |
| `Fog` | `RenderSettings.fog` | `Environment.fog` | `Fog::linear(near, far)` in `Environment.fog` |
| `shadow.normalBias` | `Light.shadowNormalBias` | `shadow_normal_bias` | `ShadowFilter::PcfSoft { normal_bias, .. }` in `Shadows.filter` |
| `MeshStandardMaterial` | `Standard` shader | `StandardMaterial3D` | `Material` |
| `CSS2DObject` | world-space UI | `Label3D` | `Placed::child` (a Contract child) |
| `requestAnimationFrame` | `Update()` | `_physics_process` | `Game::tick` (fixed simulation ticks) |
| — | `ScriptableObject` | `.tres` | A table; authoring support awaits LLP 1127.001 D3 |
| — | prefab | scene instance | A `Bundle` row; tuples work today, authored bundle derive awaits LLP 1127.001 D4 |
| — | Input System action | `InputMap` | `Actions` |
| `renderer.info` | Frame Debugger | — | `state world draws` (draw diagnostics depend on LLP 1127.000 D4) |
| Yuka `SeekBehavior`, `ArriveBehavior`, `FleeBehavior`, `WanderBehavior` | `Vector3.RotateTowards` | `Vector3.move_toward` | `steer::seek`, `steer::arrive`, `steer::flee`, `steer::wander`: this tick's `steer::Steer { velocity, facing }` |
| Yuka `SeparationBehavior`, `AlignmentBehavior`, `CohesionBehavior` | boids | boids | `steer::school(&pose, w.near_xz::<Fish>(e, radius).limit(7), separation, alignment, cohesion, …)` |
| Yuka `vehicle.neighbors` | `Physics.OverlapSphere` | `Area3D.get_overlapping_bodies` | `w.near_xz::<C>(e, radius)`; `.limit(n)` keeps the `n` nearest, in entity order |
| — | `Vector3.MoveTowards` on a pickup | `move_toward` on a pickup | `collect::drift(item, collector, radius, speed, dt)`; `collect::Collectable` marks pickups |
| `setInterval` | `InvokeRepeating` | `Timer` | `spawn::Spawner::every(ticks)` in a resource; `while let Some(serial) = spawner.due(w.tick())` |
| — | — | — | A shop: `Actions` bound to `Digit1`…`Digit9`, rows published to the HUD ([shop recipe](recipes/shop.md)) |

LÖVE's `love.update` corresponds to `Game::tick`. Its `love.draw` correspondence
will be added when the 2D `Game::draw` hook ships; that hook is not present today.

These are starting points, not equivalence promises: a tick is ordered code,
`Material::grid` paints a surface, and Contract supplies accessible in-world UI.
Use [recipes](recipes/README.md) for runnable tasks and `bun exact.mjs game vocab
<name>` for the current fields, defaults and arguments. A rename keeps the old
spelling as a diagnostic naming its replacement; defaults never change silently.

The shipped API cells compile as an engine doc-test:

```rust
use exact_game::*;
let mut w = World::new(120, 7);
w.spawn((Transform::default(), Mesh::plane(20., 20.), Material::grid([0.2; 3], 1.)));
w.insert_resource(Environment { fog: Some(Fog::linear(5., 40.)), ..Default::default() });
w.insert_resource(Shadows { filter: ShadowFilter::PcfSoft {
    normal_bias: 0.02, depth_bias: 0.0001 }, ..Default::default() });
w.spawn((Transform::default(), Placed::child("label")));
let _actions = Actions::new().button("interact", &["KeyE"]);
fn bundle(value: impl Bundle) -> impl Bundle { value }
w.spawn(bundle((Transform::default(), Mesh::sphere(0.5), Material::default())));
assert_eq!(w.count::<Mesh>(|_| true), 2);
let fish = w.spawn((Transform::default(), Mesh::sphere(0.2)));
let pose = *w.require::<Transform>(fish);
let _seek: steer::Steer = steer::seek(&pose, Vec3::new(0., 0., -4.), 2., 3., w.dt());
let _school = steer::school(&pose, w.near_xz::<Mesh>(fish, 3.).limit(7),
    1., 0.5, 0.3, 2., 3., w.dt());
let _drift: collect::Drift = collect::drift(Vec3::X, Vec3::ZERO, 2., 4., w.dt());
w.spawn((Transform::default(), collect::Collectable));
let mut drops = spawn::Spawner::every(120);
assert_eq!(drops.due(120), Some(0));
```
