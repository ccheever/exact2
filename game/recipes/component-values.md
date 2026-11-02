# Recipe: inspect a component's fields and defaults

Ask `bun exact.mjs game vocab Material` before constructing a row. The output comes
from rustdoc; default values are evaluated by Rust. Units belong beside each field.
`World::spawn_named` accepts any component or tuple implementing `Bundle`.

```rust
use exact_game::*;
fn spawn_default<C: Component + Default>(w: &mut World, name: &str) -> Entity {
    w.spawn_named(name, C::default())
}
let mut w = World::new(120, 7);
let material = spawn_default::<Material>(&mut w, "surface");
assert_eq!(w.require::<Material>(material).roughness, 0.5);
assert_eq!(w.require::<Material>(material).color, [1.; 4]);
```

- A component without `Default` needs its constructor. A value is not installed
  until spawned or inserted; a resource uses `insert_resource` instead.
- Simulation components save and hash; `Presentation` components are rebuilt in
  `Game::present`. `vocab` identifies which. A draw list is never saved state.
- A verb's arguments are required unless its rustdoc names a constructor default.
  A row handle is a borrowed view, not a retained object; release it before spawning.
- This fragment is a doc-test. Use a task recipe for behavior and the engine
  reference for World operations; `game vocab` lists every public World verb.
