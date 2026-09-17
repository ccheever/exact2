# exact-game-physics

Call `register(world)` in setup, `move_character` after character input, and
`step(world)` once per fixed tick. Physics reads `world.dt()` and keeps settle busy
until all dynamic bodies sleep. No solver survives outside the world's Data.

`Body.previous` records the last post-step pose and velocities. `Physics` holds
ordered manifolds, local anchors, feature IDs, normal/tangent impulses, touching
state, this tick's events, and previous collider poses/materials. Register before
loading a world. A saved contact cache is required for exact continuation.

The primitive shapes are spheres, total-height Y capsules and oriented boxes.
Shapes have positive dimensions; spheres/capsules require uniform positive scale.
Bodies are roots. Collider-only children use their current composed world pose;
sheared geometry is refused. An explicit mass is kilograms; zero uses density
1000 kg/m³. Contacts use geometric-mean friction and maximum restitution.

`events(world)` returns a read guard dereferencing to `[Touch]`, matching the
engine's runtime borrow model. Keep it only as long as needed; holding the guard
while calling `step` is a borrow conflict. `Announce` opts a collider into journal
begin/end lines. Other events remain readable in the Physics resource.

The character owns its capsule response and finite push budget. Its kinematic
collider is a sensor to the rigid solver, preventing an infinite-mass solver push
from overriding the 80 kg default budget. `desired_velocity.x/z` control movement;
the game writes `Character.velocity.y` for gravity and jumps. Grounding clears a
downward velocity. Platforms carry the character using saved relative poses.

Coulomb friction alone cannot hold a free sphere stationary on a slope. The
sphere fixture checks no-slip rolling at 20° and 40°; the box fixture checks rest
at 20° and sliding at 40°. Rolling resistance would be an additional material
property, not a change to Coulomb friction.

Run from `game/` with `EXACT_UPDATE_TRUST=development`:

```
cargo test -p exact-game-physics --no-fail-fast -- --nocapture
cargo clippy -p exact-game-physics --all-targets -- -D warnings
cargo run -p exact-game-physics --release --example pile
```

Rapier is a dev dependency only. The oracle fixture explicitly selects per-point
Coulomb friction, matching contact softness, and disables contact recycling; its
PGS iterations are increased for a more converged reference. Curved GJK queries
are approximate, so strict geometric tolerances use independent dense sampling
(in f64 in tests only) and analytic/bisected reference distances.

P2: heightfields and meshes need stable triangle/edge identities and seam welding;
internal edges must not create ghost normals or duplicate friction. Any BVH that
changes contact order must sort its output. Joints add saved impulses and island
edges, including wake-on-removal. Rotating/box sweeps need a documented time of
impact policy; this crate only sweeps translating spheres/capsules. Speculative
rigid contacts do not provide full rotational CCD.

## Measured status — 2026-09-17

**Not acceptance-complete.** Stopped physics corrections after three rounds per
`rules/RULES.md`. No engine changes and no commit made by this task.

The 20 debug tests finish with 18 passing on arm64 macOS and x86-64 Linux. Build,
Clippy (`--all-targets -- -D warnings`), formatting, and the repository caps check
pass. All Rust files are at most 506 lines; 3,409 Rust lines total (2,246 production,
1,163 tests/example). Staging the final versions was blocked by the sandbox's
read-only Git worktree index; older versions were staged by concurrent work.

| Scene | Measured result | Sleep tick (ours / Rapier) |
|---|---|---|
| Box dropped 2 m | center y 0.499627 m; 0.373 mm penetration | 68 / 72 |
| 10-box stack, 20 s | 2.904 mm maximum horizontal displacement | 42 / 42 |
| 5×5×5 dropped 2 m | 7.808 mm maximum horizontal displacement | 92 / 97 |
| Restitution 0.5 sphere | rebound 0.485361 m from a 2 m drop | — |

Maximum rest-pose disagreement over those three box scenes: 7.536 mm and 0.3996°.
The reference uses four substeps and eight internal PGS iterations; ours uses four
substeps and four biased sweeps. All listed character scenarios pass. Box slope
fixtures hold at 20° and slide at 40°.

Save at tick 90, restore into a fresh Sim, and compare every 50 ticks through 600:
identical. Two fresh runs agree at every tick. Final 125-box tick-600 world hashes:

- Mac arm64: `0x0bc86e72da574d77`
- Linux x86-64: `0x0bc86e72da574d77`

Release `pile`, milliseconds per tick while active after all boxes were poured:

| Machine | Boxes | p50 | p95 | Natural-sleep p50 / p95 |
|---|---:|---:|---:|---|
| Mac arm64 | 1,000 | 40.511 | 49.340 | unavailable |
| Mac arm64 | 2,000 | 58.652 | 102.932 | unavailable |
| Linux x86-64 | 1,000 | 51.044 | 51.796 | unavailable |
| Linux x86-64 | 2,000 | 73.691 | 76.703 | unavailable |

Every poured-bin run remained awake at the 2,400-tick limit. No bodies were forced
to sleep to manufacture an asleep timing. These shared-machine timings miss the
8 ms active target; the 0.2 ms asleep target is unverified.

Remaining failures:

- The ray fixture compares a capsule ray against an approximate Rapier query at
  0.1 mm tolerance. At origin (-3, 0.63, 0.13), radius 0.4, total height 2, along
  +X: analytic distance 2.62290585 m, ours 2.6229064 m, Rapier 2.6247108 m. That
  fixture needs an independent analytic reference, as the sweep fixture has.
- The no-slip sphere fixture fails at 20°: contact-point velocity has a tangential
  residual about 0.0419 m/s, above its 0.02 m/s limit. The original request that a
  free sphere remain stationary also needs rolling resistance or a revised test.
- Large poured piles neither sleep nor meet throughput. Four biased sweeps made
  the small block stable; the active path still rebuilds manifolds and allocates
  clipping polygons, and diagonal separated boxes enumerate edge pairs. Profile
  those costs before adding terrain. Poured-pile rest-pose oracle comparison and
  full slope/bounce scene oracle comparisons remain unverified.
