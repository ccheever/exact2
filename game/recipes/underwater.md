# Recipe: an underwater tank

`Environment::underwater()` is the water: a green-blue sky that the fog covers
entirely, uniform fog that hides 90% of a surface 32.9 m away, no sun disc, and
exposure 0.9. The scene around it decides the rest: a sun from above, nothing
opaque between the sun and the sand, and a camera that stays inside the glass.

```rust
use exact_game::*;
const GLASS_MIN: Vec3 = Vec3::new(-10., 0., -30.);
const GLASS_MAX: Vec3 = Vec3::new(10., 8., 6.);
/// Where the camera rides behind the sub, in metres.
const BOOM: Vec3 = Vec3::new(0., 1.5, 5.);
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
        w.spawn_named(format!("rock-{i}"), (Transform::at(-3. + 3. * i as f32, 0.6, z),
            Mesh::sphere(1.2), Material::rgb(0.1, 0.1, 0.09).rough(0.9)));
    }
    w.spawn_named("sub", (Transform::at(0., 2., 0.), Mesh::cuboid(Vec3::new(1.2, 1., 2.5)),
        Material::rgb(0.8, 0.55, 0.1)));
    w.spawn_named("camera", (Transform::default(), Camera::default(),
        Follow::new("sub").offset(BOOM.x, BOOM.y, BOOM.z).lag(0.2)));
}
/// Near the front glass, shorten the boom's reach back and lift it by the same
/// length: the camera stays half a metre inside the glass and keeps aiming at
/// the sub. The follower eases between points inside, so it never leaves.
fn fit_boom(w: &World) {
    let sub = w.require::<Transform>("sub").position;
    let back = (GLASS_MAX.z - 0.5 - sub.z).clamp(0., BOOM.z);
    let lift = (BOOM.length_squared() - back * back).sqrt().min(GLASS_MAX.y - 0.5 - sub.y);
    w.require_mut::<Follow>("camera").offset = Vec3::new(0., lift, back);
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
        fit_boom(w);
    }
}

// The values, pinned: change them here and in the measured table below.
let water = Environment::underwater();
assert_eq!(water, Environment {
    background: None,
    zenith: [0.3, 0.62, 0.7],
    horizon: [0.03, 0.16, 0.2],
    ground: [0.02, 0.05, 0.05],
    ambient: 0.6,
    sun_disc: 0.,
    exposure: 0.9,
    tone_mapping: ToneMapping::Aces,
    fog: Some(Fog::new(0.07, 0.)),
    bloom: Some(Bloom { threshold: 1., intensity: 0.1, radius: 1.5 }),
});
let reach = 10f32.ln() / 0.07; // where 10% of a surface's colour is left
assert!((reach - 32.9).abs() < 0.05);

let mut sim = Sim::<Tank>::new(()).unwrap();
sim.run(3000.);
let sub = sim.local_position("sub").unwrap();
let camera = *sim.get::<Transform>("camera").unwrap();
assert_eq!(sub.z, GLASS_MAX.z - 1.25, "the sub is against the glass");
assert!(sub.z + BOOM.z > GLASS_MAX.z, "the boom alone would put the camera outside");
assert!((camera.position.z - (GLASS_MAX.z - 0.5)).abs() < 0.01, "on the inner face");
assert!(camera.position.y <= GLASS_MAX.y - 0.5);
// The sub is in the middle of the frame (the vertical half-field is 30°).
let off_axis = (camera.rotation * -Vec3::Z).angle_between(sub - camera.position);
assert!(off_axis < 2f32.to_radians(), "{} degrees off axis", off_axis.to_degrees());
```

The values, which the fragment pins:

| Field | Value | Why |
|---|---|---|
| `zenith`, `horizon`, `ground` | [0.3, 0.62, 0.7], [0.03, 0.16, 0.2], [0.02, 0.05, 0.05] | light through the surface; the water's colour, which fog fades to; little from below |
| `ambient` | 0.6 | the sky's share of the light on surfaces |
| `sun_disc` | 0 | no sun disc is seen through water |
| `exposure`, `tone_mapping` | 0.9, ACES | puts the start frame mid-band |
| `fog` | `Fog::new(0.07, 0.)` | uniform; 10% of a surface left at 32.9 m |
| `bloom` | threshold 1, intensity 0.1, radius 1.5 | a weaker glow than the default 0.16 |
| Sun | 6,000 lux, [0.8, 0.95, 1], from above | sunlight through blue-green water |

Measured on the GPU by [the underwater test](../render/tests/underwater.rs), which
renders this scene at 192 × 108 (on an Apple M5 Max under Metal):

| Number | Value | How |
|---|---|---|
| Mean luminance at the start | 0.230, band 0.13–0.33 | Rec. 709 over decoded sRGB, as the look frame measures |
| Exposure × 0.25, × 4 | 0.033 and 0.644, outside the band | the negative control |
| Mean luminance with the sub at the glass | 0.269, the sub wholly in the frame | three seconds in |
| Sand in the bottom eighth | 0.319; 0.127 under an opaque lid | the lid shades the whole floor |
| Fog transmittance at 16.45 m, 32.9 m | 0.311, 0.095 (0.316, 0.1 expected) | black faces crossing the probe rays at those distances, lights off |

The frame at the start: teal water fills the top and the far end; pale
green-grey sand fades into it by the third rock, 24 m away; the sub, centred,
reads yellow-green under the blue-green sun, with its shadow on the sand beneath.
At the glass the camera looks down on the sub's top from about 5 m above it, and the
open water past the tank's front edge fills the bottom of the frame. At a quarter
of the exposure the sand is olive-black; at four times it is near white, the
"washed out" frame. `EXACT_GPU_OUT=<dir>` with the test writes each frame as a PPM.

- Tune the fog by its reach, not its density: `density = ln 10 / reach`. A tank
  shorter than the reach shows its far glass; a sea longer than it fades out.
- An opaque lid over the water shades the whole floor from the sun: a tank
  modelled as a closed box is one. Draw the surface as a translucent plane facing
  down (`Material::alpha`), which casts no shadow, or leave the tank open.
- A follow camera's offset carries it through the glass when the followed body
  backs into a wall. Clamping the camera's position after the follower has
  aimed loses the sub from the frame: at the glass it sits about 47° below the
  view axis, past the 30° half-field. Change the follower's offset instead, as
  `fit_boom` does, and keep the camera more than the near plane (0.1 m by
  default) inside.
- Brighten with `exposure`, not the sun or `ambient`: those raise lit surfaces and
  not the fog's colour, so they also change how far the water seems to reach.
  Bloom spreads whatever is brighter than 1 in its brightest channel before
  exposure, lit or emissive; a fish that should glow needs `Material::glow`
  bright enough to pass it.
