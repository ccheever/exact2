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
```
