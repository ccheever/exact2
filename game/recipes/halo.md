# Recipe: a ring and a translucent halo

Primitive dimensions are metres, Y up. A ring lies in XZ; a torus revolves around Y;
a cone's tip is at +height/2. Alpha uses straight blending, depth tests and a
back-to-front sort, with no depth writes or refraction.

```rust
use exact_game::*;
let mut w = World::new(120, 7);
w.spawn_named("pad", (Transform::at(0., 0.1, 0.), Mesh::cylinder(0.9, 0.2),
    Material::rgb(0.3, 0.4, 0.42)));
w.spawn_named("ring", (Transform::at(0., 0.205, 0.), Mesh::ring(0.72, 0.9),
    Material::glow([0.12, 0.55, 0.6])));
w.spawn_named("halo", (Transform::at(0., 1., 0.), Mesh::sphere(0.7),
    Material::glow([0.12, 0.55, 0.6]).alpha(0.12)));
w.spawn((Transform::at(2., 0., 0.), Mesh::torus(0.8, 0.1)));
w.spawn((Transform::at(-2., 0., 0.), Mesh::cone(0.8, 1.5)));
assert_eq!(w.count::<Material>(|m| m.color[3] < 1.), 1);
assert_eq!(*w.require::<Mesh>("ring"), Mesh::ring(0.72, 0.9));
```

- Compatible translucent triangles are instances in the particle quad arena.
  One halo and a hundred take one translucent draw. Texture/blend interleaving
  can split it, just as with particles. Geometry still costs instances and sorting.
- Use emission for a halo. This quad path supplies flat hemisphere/directional
  diffuse light and fog; primitive PBR, local-light shadows and procedural grids
  remain on the opaque path. `Opacity` is a separate presentation coverage fade.
- `Collider::of` needs an authored collision shape for ring, torus and cone.
- Used in [Beacons](../games/beacons/logic/src/lib.rs). This fragment is a doc-test;
  [GPU tests](../render/src/translucent_tests.rs) check pixels, depth, batching,
  culling and restore, with the surface's draw counter.
