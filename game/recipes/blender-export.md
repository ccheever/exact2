# Recipe: exporting from Blender

Put the `.glb` (or `.gltf`) under `art/`; the bake makes `<stem>.model`. The bake
refuses what it cannot draw, by name, but three things a Blender export carries
through it change the picture instead.

Before exporting:

- **Backface culling.** Blender's materials are double-sided unless *Backface
  Culling* is ticked (Material Properties → Settings), and the exporter writes
  that as glTF `doubleSided`. A double-sided wall hides what is behind it from
  both sides: tick it on every material whose back faces should not draw.
- **Vertex colours on joined parts.** `COLOR_0` multiplies a material's colour.
  Joining a mesh that has a colour attribute with parts that have none gives those
  parts black, so they draw black. Fill the attribute white on every part before
  joining, or export without vertex colours (the glTF exporter's mesh options)
  when no material uses them.
- **One scene.** The bake takes exactly one scene: limit the export to the
  active scene (*Include → Limit to → Active Scene*).

What the bake accepts: triangles with `POSITION`, `NORMAL`, `TANGENT`,
`TEXCOORD_0`, `COLOR_0`, `JOINTS_0`/`WEIGHTS_0` (skins), textures on `TEXCOORD_0`
as 8-bit PNG or JPEG, skeletal animation, and the extensions
`KHR_materials_emissive_strength` and `KHR_texture_transform`. It refuses any other
extension (Draco or meshopt compression, `KHR_materials_specular`,
`KHR_materials_ior`, transmission, variants), morph targets and their animation,
sparse accessors, points and lines, and a second UV or colour set, each naming the
file. A model's materials keep their glTF index for `MaterialOverrides`.

A game's own test can catch the first two, on every model it ships:

```rust
use exact_game::asset::{MaterialData, MeshData, Model};
/// What an export carries into the bake that changes the picture.
fn export_problems(name: &str, model: &Model) -> Vec<String> {
    let mut found = Vec::new();
    for (i, material) in model.materials.iter().enumerate() {
        if material.double_sided {
            found.push(format!("{name}: material {i} is double-sided; tick Backface Culling"));
        }
    }
    for (i, mesh) in model.meshes.iter().enumerate() {
        if mesh.colors.chunks_exact(4).any(|c| c[0] + c[1] + c[2] == 0.) {
            found.push(format!("{name}: mesh {i} has black COLOR_0 vertices"));
        }
    }
    found
}
// In logic/tests, over the game's baked art:
//     let sim: Sim<MyGame> = exact_game::sim_with_art!(Options::default()).unwrap();
//     for (name, model) in sim.presentation_models() {
//         assert_eq!(export_problems(name, model), Vec::<String>::new());
//     }
let joined = Model {
    materials: vec![MaterialData { double_sided: true, ..MaterialData::default() }],
    meshes: vec![MeshData { colors: vec![1., 1., 1., 1., 0., 0., 0., 1.], ..MeshData::default() }],
    ..Model::default()
};
assert_eq!(export_problems("tank.model", &joined).len(), 2);
let clean = Model { materials: vec![MaterialData::default()], meshes: vec![MeshData::default()], ..Model::default() };
assert!(export_problems("tank.model", &clean).is_empty());
```

- `MeshData.colors` empty means white; only a present, black attribute darkens.
- Units: glTF is metres. An asset whose `extras.metersPerUnit` is set is scaled by it.
- `game/bake/tests/game_art.rs` bakes a double-sided and a black-`COLOR_0` glTF through
  the same in-memory bake and reads back the two fields this check reads. The bake's
  refusals are in [bake](../bake/README.md); this fragment is an engine doc-test.
