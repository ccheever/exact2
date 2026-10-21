# Recipe: a school of fish

`steer::school` keeps a fish `separation` metres from its neighbours, turns it
with their mean heading and toward their centre. The neighbours are
`near_xz`'s rows, `.limit(n)` of the nearest, in entity order.

```rust
use exact_game::*;
#[derive(Default, Component)]
struct Fish;
struct School;
impl Game for School {
    const ID: &'static str = "recipe-schooling";
    const HZ: u32 = 120;
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        // Twenty fish packed 0.6 m apart, closer than they like, facing anywhere.
        for i in 0..20 {
            let yaw = w.rand(-3.0f32..3.0);
            let at = Transform::at((i % 5) as f32 * 0.6, 0., (i / 5) as f32 * 0.6);
            w.spawn_named(format!("fish-{i}"), (Fish, Transform { rotation: Quat::from_rotation_y(yaw), ..at }));
        }
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        let dt = w.dt();
        // Decide for every fish, then move them all: each decision reads the
        // school as this tick found it, whatever the entity order.
        let steers: Vec<(Entity, steer::Steer)> = w.query::<&Transform>().with::<Fish>().iter()
            .map(|(e, pose)| {
                let mates = w.near_xz::<Fish>(e, 3.).limit(7);
                (e, steer::school(pose, mates, 1., 0.5, 0.3, 1.5, 3., dt))
            })
            .collect();
        for (e, s) in steers {
            let mut pose = w.require_mut::<Transform>(e);
            pose.position += s.velocity * dt;
            pose.rotation = s.facing;
        }
    }
}
let mut sim = Sim::<School>::new(()).unwrap();
sim.run(10_000.);
let at: Vec<Vec3> = sim.world().query::<&Transform>().with::<Fish>().iter()
    .map(|(_, t)| t.position).collect();
let closest = at.iter().enumerate()
    .flat_map(|(i, a)| at[i + 1..].iter().map(move |b| a.distance(*b)))
    .fold(f32::INFINITY, f32::min);
let centre = at.iter().copied().sum::<Vec3>() / 20.;
let spread = at.iter().map(|p| p.distance(centre)).fold(0., f32::max);
// After ten seconds no two fish are closer than 0.8 of the separation, and
// none is farther than 4 m from the school's centre.
assert!(closest > 0.8 && spread < 4., "closest {closest}, spread {spread}");
```

- `separation` is a distance; `alignment` and `cohesion` are weights beside the
  fish's own heading's one. A neighbour inside the separation pushes up to eight,
  so spacing wins. Raise `cohesion` for a tighter ball, `alignment` for a column.
- Steer every fish before moving any. Moving each as it is decided lets the
  first fish's move change the last fish's choice, an order effect, not a bug of
  determinism, but not a school either.
- `near_xz` scans every `Fish` per call, so a school costs its size squared per
  tick: hundreds of fish, not thousands. `.limit(7)` bounds the steering work,
  not the scan.
- Combine with [steering](steering.md): a hungry fish seeks food and a fish near a
  predator flees; a fish that does neither schools. This fragment is a doc-test.
