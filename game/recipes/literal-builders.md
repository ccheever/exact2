# Recipe: spawn from literals

`prop` and `sprite` spawn immediately. Their setters produce the same rows as a
tuple; `.entity()` returns the entity for attaching your own components.

```rust
use exact_game::*;
let mut w = World::new(120, 7);
let pad = w.prop("plinth-1", Mesh::cylinder(0.9, 0.2))
    .at(2., 0.1, 0.).color("#4d6669").entity();
w.sprite("player", "strip.tex", [24., 32.]).at(0., 20.);
assert_eq!(w.local_position(pad), Some(Vec3::new(2., 0.1, 0.)));
assert_eq!(w.require::<Sprite>("player").size, Vec2::new(24., 32.));
```

- `color` takes sRGB hex and converts to the material's linear channels. Do not
  put byte values or sRGB fractions into `Material::rgb`, which is already linear.
- The sprite is in XY; props are in XYZ. Size is world units, not screen pixels.
  Declare its `.tex` in `Game::ASSETS` and enable `game.assets` to render it.
- Tuples remain useful for components whose fields you set directly. A default
  prop is exactly `(Transform::default(), mesh, Material::default())`; a default
  sprite is `(Transform::default(), Sprite::new(texture, size))`.
- This fragment is a doc-test. [Builder tests](../engine/tests/builders.rs) compare
  hashes, save bytes, component JSON, journal lines and full agent state.
