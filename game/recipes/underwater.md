# Recipe: an underwater tank

`Environment::underwater()` is the water: a green-blue sky that the fog covers
entirely, uniform fog that hides 90% of a surface 32.9 m away, no sun disc, and
exposure 0.9. `Water::at(8.)` is its surface at the top of the tank: seen from
below, with caustics on the sand and shafts in the water. The scene around them
decides the rest: a sun from above, nothing opaque between the sun and the sand,
and a camera that stays inside the glass.

```rust
use exact_game::*;
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
    // The surface at the top of the tank, with its caustics and shafts.
    w.insert_resource(Water::at(GLASS_MAX.y));
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
/// the sub, exactly once the follower has caught up with the offset. The
/// follower eases between points inside, so it never leaves.
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
assert_eq!(Water::at(8.), Water {
    height: 8.,
    color: [0.02, 0.12, 0.15],
    clarity: 1.,
    waves: Waves { amplitude: 0.02, wavelength: 2., speed: 0.6 },
    caustics: Some(Caustics { strength: 0.45, scale: 1.5 }),
    shafts: Some(Shafts { intensity: 0.7, length: 0.6 }),
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
| `Water::at(8.)` | the tank's top; waves 0.02 m high, 2 m long, at 0.6 m/s | a surface that moves without hiding the sky in its window |
| `Caustics` | strength 0.45, cells 1.5 m | a net on the sand that leaves its shadows readable |
| `Shafts` | intensity 0.7, length 0.6 of the frame | streaks that brighten the water towards the sun without washing out the sand |

Measured on the GPU by [the underwater test](../render/tests/underwater.rs), which
renders this scene at 192 × 108 (on an Apple M5 Max under Metal):

| Number | Value | How |
|---|---|---|
| Mean luminance at the start | 0.236, band 0.13–0.33 | Rec. 709 over decoded sRGB, as the look frame measures |
| Exposure × 0.25, × 4 | 0.034 and 0.658, outside the band | the negative control |
| Mean luminance with the sub at the glass | 0.261, the sub wholly in the frame | three seconds in |
| The look frame at 500 ms | 0.284, no tell | the GPU-less proof host's look frame |
| Sand in the bottom eighth | 0.302; 0.127 under an opaque lid | the lid shades the whole floor and its caustics |
| Fog transmittance at 16.45 m, 32.9 m | 0.311, 0.095 (0.316, 0.1 expected) | black faces crossing the probe rays at those distances, lights off |

A frame looking 50° up into
Snell's window measures 0.32 over the whole frame and 0.48 over its top quarter
([the water test](../render/tests/water.rs)), so the band does not reject the window
and the window clears the band by more than 0.1.

The frame at the start: teal water fills the top and the far end, under the
surface's slow ripples; pale green-grey sand, netted with caustics, fades into it by
the third rock, 24 m away, and faint shafts of light stand in the water above it;
the sub, centred, reads yellow-green under the blue-green sun with the net across
its side, and its shadow on the sand beneath has none. At the glass the camera
looks down on the sub's top from about 5 m above it, and the open water past the
tank's front edge fills the bottom of the frame. At a quarter of the exposure the
sand is olive-black; at four times it is near white, the "washed out" frame.
`EXACT_GPU_OUT=<dir>` with the test writes each frame as a PPM.

- Tune the fog by its reach, not its density: `density = ln 10 / reach`. A tank
  shorter than the reach shows its far glass; a sea longer than it fades out.
- An opaque lid over the water shades the whole floor from the sun, caustics and
  all: a tank modelled as a closed box is one. Leave the top open and let `Water`
  draw the surface; a mesh at the same height would fight it for the same pixels.
  The shafts come from the surface the frame can see, so a lid hidden behind it
  does not stop them.
- `Water` is a saved resource: adding it to a game moves its pins, so repin.
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
