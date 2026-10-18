# Recipe: an underwater tank

`Environment::underwater()` is the water: a green-blue sky that the fog covers
entirely, uniform fog that hides 90% of a surface 32.9 m away, no sun disc, and
exposure 0.9. The scene around it decides the rest: a sun from above, nothing
opaque between the sun and the sand, and a camera that stays inside the glass.

```rust
use exact_game::*;
const GLASS_MIN: Vec3 = Vec3::new(-10., 0., -30.);
const GLASS_MAX: Vec3 = Vec3::new(10., 8., 6.);
fn tank(w: &mut World) {
    w.insert_resource(Environment::underwater());
    w.spawn_named("sun", (
        Transform::at(0., 20., 0.).looking_at(Vec3::new(3., 0., -4.), Vec3::Z),
        DirectionalLight { color: [0.8, 0.95, 1.], illuminance: 6000., shadows: true },
    ));
    w.spawn_named("sand", (Transform::at(0., 0., -12.), Mesh::plane(20., 36.),
        Material::rgb(0.45, 0.38, 0.24)));
    // The surface faces down at the camera and, translucent, casts no shadow.
    w.spawn_named("surface", (
        Transform::at(0., 8., -12.).looking_at(Vec3::new(0., 8., -13.), Vec3::NEG_Y),
        Mesh::plane(20., 36.), Material::rgb(0.6, 0.9, 0.95).alpha(0.25)));
    for (i, z) in [-4., -12., -24.].into_iter().enumerate() {
        w.spawn_named(&format!("rock-{i}"), (Transform::at(-3. + 3. * i as f32, 0.6, z),
            Mesh::sphere(1.2), Material::rgb(0.1, 0.1, 0.09).rough(0.9)));
    }
    w.spawn_named("sub", (Transform::at(0., 2., 0.), Mesh::cuboid(Vec3::new(1.2, 1., 2.5)),
        Material::rgb(0.8, 0.55, 0.1)));
    w.spawn_named("camera", (Transform::default(), Camera::default(),
        Follow::new("sub").offset(0., 1.5, 5.).look_at_offset(0., 0., -6.).lag(0.2)));
}
/// Step the follower now, then hold the camera half a metre inside the glass.
fn keep_inside_glass(w: &World) {
    scene::follow(w);
    let mut camera = w.require_mut::<Transform>("camera");
    camera.position = camera.position.clamp(GLASS_MIN + 0.5, GLASS_MAX - 0.5);
}
struct Tank;
impl Game for Tank {
    const ID: &'static str = "recipe-underwater";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        tank(w);
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        // The sub backs towards the front glass and stops against it.
        let mut sub = w.require_mut::<Transform>("sub");
        sub.position.z = (sub.position.z + 4. * w.dt()).min(GLASS_MAX.z - 1.25);
        drop(sub);
        keep_inside_glass(w);
    }
}

let water = Environment::underwater();
let fog = water.fog.unwrap();
assert_eq!(fog.height_falloff, 0.);
let reach = 10f32.ln() / fog.density; // where 10% of a surface's colour is left
assert!((reach - 32.9).abs() < 0.05);

let mut sim = Sim::<Tank>::new(()).unwrap();
sim.run(3000.);
let (sub, camera) = (sim.local_position("sub").unwrap(), sim.local_position("camera").unwrap());
assert!(sub.z + 5. > GLASS_MAX.z, "the follow offset alone would put the camera outside");
assert!(camera.z <= GLASS_MAX.z - 0.5 && camera.y <= GLASS_MAX.y - 0.5);
```

Measured on the GPU by [the underwater test](../render/tests/underwater.rs), which
renders this scene at 192 × 108 (on an Apple M5 Max under Metal):

| Number | Value | How |
|---|---|---|
| Mean luminance | 0.207, band 0.13–0.33 | Rec. 709 over decoded sRGB, as the look frame measures |
| Exposure × 0.25, × 4 | 0.029 and 0.613, outside the band | the negative control |
| Sand in the bottom eighth | 0.287; 0.131 under an opaque lid | the lid shades the whole floor |
| Fog transmittance at 16.45 m, 32.9 m | 0.328, 0.105 (0.316, 0.1 expected) | black cubes against open water, lights off |

The frame: teal water fills the upper half and the far end; pale green-grey sand
fades into it by the third rock, 24 m away; the sub reads yellow-green under the
blue-green sun and its shadow lies on the sand beneath it. At a quarter of the
exposure the sand is olive-black; at four times it is near white, the "washed out"
frame. `EXACT_GPU_OUT=<dir>` with the test writes each frame as a PPM.

- Tune the fog by its reach, not its density: `density = ln 10 / reach`. A tank
  shorter than the reach shows its far glass; a sea longer than it fades out.
- An opaque lid over the water shades the whole floor from the sun: a tank
  modelled as a closed box is one. Draw the surface as a translucent plane facing
  down (`Material::alpha`), which casts no shadow, or leave the tank open.
- A follow camera's offset carries it through the glass when the followed body
  backs into a wall. Step the follower in the tick and clamp the camera inside
  the glass by more than the near plane (0.1 m by default).
- Brighten with `exposure`, not the sun or `ambient`: those raise lit surfaces and
  not the fog's colour, so they also change how far the water seems to reach.
  Fish that should glow use `Material::glow`; bloom picks up only what emits.
