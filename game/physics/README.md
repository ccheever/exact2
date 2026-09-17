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

## Measured status — P1b, 2026-09-17

**Not acceptance-complete. Stopped after three optimization rounds.** The retained
version passes the correctness checks, but misses 8 ms p95 at 2,000 boxes and the
large-pile sleep requirement. No commit. Changes are confined to this directory.
Final staging was blocked by the sandbox's read-only shared Git index; earlier
source versions are staged, while final documentation and assertion edits remain
in the working tree.

The retained changes use one held query to gather bodies; move the Physics resource
instead of cloning every manifold; reuse derived node, constraint, axis-list and
pair buffers under `#[data(skip)]`; separate static broadphase bounds; use coherent
insertion sorting with an O(n log n) fallback; reject box pairs by bounding sphere
and SAT distance before clipping; keep clipping polygons and prepared rows inline;
and construct each separated box's vertices once instead of inside every edge pair.
The solver precomputes angular impulse responses and tangent inverses per substep.
It uses **eight substeps, one biased solve and one relax pass each**, with clamped
accumulated impulses, 30 Hz capped at one quarter of the substep rate, damping ratio
10 and a 3 m/s push-out cap. Contact and pair order remain fixed.

The capsule ray was correct. At (-3, .63, .13), the cap-center offset is (.03, .13),
so `(t-3)^2 + .03^2 + .13^2 = .4^2` gives `t = 2.622905847 m`.
Rapier/Parry returns 2.6247108 m. The test now uses the closed form for capsule rays,
while retaining the strict 0.1 mm tolerance and independent normal checks.

Sphere contact anchors must stay at ±normal × radius; rotating a material point
on the sphere was creating a false separation and permitting slip. The repaired
fixture asserts full contact-point velocity below 0.02 m/s and acceleration within
0.05 m/s² of `(5/7) g sin(theta)`. Measured tangential slip at 20°/40° is
0 / 0.000000183 m/s, acceleration 2.3965855 / 4.504100 m/s² (expected 2.3965843 /
4.504105). Rolling resistance was not added.

### Validation

All 20 original tests pass; a new mid-bounce resume test makes **21 debug tests**
on Linux. Mac release passes 20 tests (the debug-only parented-body panic test is
excluded there); the 20 original debug tests also passed on Mac in round 2. The
final local debug invocation waited on another Cargo build's lock, so it was
interrupted and verification used release locally and debug on the builder.
The tightened scene assertions pass on both machines. Clippy with
`--all-targets -- -D warnings` passes on Mac release and Linux debug; formatting
and the repository caps check pass.

| Preserved property | Result |
|---|---|
| Single-box rest height | 0.499635 m, **0.365 mm** error; assertion tightened to 0.4 mm |
| Ten-box stack | Stands; 2.074 mm horizontal drift; sleeps at tick 40 |
| 125-box dropped block | Sleeps at tick 91; 17.719 mm maximum oracle position difference |
| Restitution 0.5 sphere | 0.488607 m rebound from a 2 m drop |
| Pile save/resume | Tick-90 save; exact continuation through tick 600 |
| Mid-bounce save/resume | Tick-45 save while rising; every tick through 240 matches |
| Mac arm64 tick-600 hash | `0x07d77113bfe4413b` |
| Linux x86-64 tick-600 hash | `0x07d77113bfe4413b` |

### Throughput and reference

Milliseconds per active tick, p50 / p95. Same unit boxes, density, friction, bin and
100-box pour every 12 ticks for both engines. Rapier 0.35.3 uses its unmodified
single-threaded defaults. All throughput runs allow 2,400 ticks and measure the
active window after the last pour; naturally sleeping runs finish after 180 asleep
samples. Thus Rapier's active window is shorter. These are shared machines, not
isolated CPU measurements. The final Mac run excludes overlap with this task's
compilation; an earlier contaminated run is not used below.

| Machine | Boxes | Before p50 / p95 | After p50 / p95 | Rapier p50 / p95 | Rapier sleep tick |
|---|---:|---:|---:|---:|---:|
| mac | 1,000 | 46.425 / 76.470 | 25.114 / 32.386 | 2.671 / 3.301 | 243 |
| mac | 2,000 | 68.344 / 83.503 | 35.309 / 57.776 | 7.670 / 9.153 | 406 |
| mac | 5,000 | 94.691 / 138.248 | 46.523 / 73.961 | 18.822 / 22.152 | 905 |
| linux | 1,000 | 53.497 / 54.268 | 29.820 / 30.315 | 3.010 / 3.559 | 243 |
| linux | 2,000 | 77.090 / 79.978 | 41.260 / 42.958 | 7.044 / 7.319 | 406 |
| linux | 5,000 | 101.604 / 147.677 | 56.815 / 89.755 | 26.993 / 29.085 | 905 |

**None of the Exact poured piles sleeps before the 2,400-tick limit**, before or
after. Time-to-sleep is unavailable, and **≤0.2 ms asleep at 2,000 is unverified**;
no bodies were forced asleep to manufacture a measurement. Raw `sleep_tick=0`
and zero asleep percentiles mean no samples, not zero-cost sleep. Rapier's reported
asleep calls round to 0.000 ms. Its whole-pile sleep times after the last pour are
2.25 / 2.97 / 5.28 s for 1,000 / 2,000 / 5,000 boxes.

The original bin is only 24 m tall. The unchanged 5,000-box fixture pours above
that height; unstable boxes can spill over its finite ground and fall indefinitely.
It is a throughput stress case, not proof of a contained settled 5,000-box pile.

### Where the tick goes

Mean milliseconds on sampled active ticks at **2,000 boxes**. All clocks and the
counting allocator live in the example. `step_observed` emits phase boundaries but
owns no clock, profile resource or saved measurements. Every 30th active tick is
profiled; these ticks are excluded from throughput percentiles. Phase means include
observer overhead and scheduling, so they are not a decomposition of the p50.
Warm includes force integration, inertia/effective-mass preparation and warm start;
islands includes wake propagation, island construction and constraint preparation;
sleep includes final separation/touching refresh. Write-back includes touch events.

| Phase | Mac before | Mac after | Linux before | Linux after |
|---|---:|---:|---:|---:|
| gather | 0.5218 | 0.1852 | 0.4961 | 0.2175 |
| broadphase | 1.2291 | 1.0709 | 2.0163 | 1.9654 |
| narrowphase | 29.1817 | 7.6566 | 38.4630 | 10.3005 |
| matching | 2.1346 | 1.8364 | 1.7616 | 1.3917 |
| islands | 1.6631 | 1.7351 | 1.5489 | 1.3823 |
| warm | 4.1577 | 10.0457 | 5.6029 | 12.9638 |
| solve | 23.0661 | 6.7369 | 20.5278 | 5.7704 |
| integrate | 0.1459 | 0.2081 | 0.0713 | 0.1399 |
| relax | 5.6681 | 6.8082 | 5.2303 | 5.8452 |
| restitution | 0.0459 | 0.0591 | 0.0191 | 0.0180 |
| sleep | 1.3883 | 1.2102 | 0.9994 | 0.8692 |
| writeback | 1.1097 | 1.2988 | 0.9636 | 0.7100 |
| other | 0.0095 | 0.2150 | 0.0072 | 0.0064 |

Per substep, each cell is **warm / solve / integrate / relax**, milliseconds.

| Substep | Mac before | Mac after | Linux before | Linux after |
|---:|---|---|---|---|
| 0 | 1.0093 / 5.6838 / 0.0342 / 1.3962 | 1.3020 / 0.7608 / 0.0248 / 0.7998 | 1.4087 / 5.1811 / 0.0179 / 1.3072 | 1.6229 / 0.7330 / 0.0176 / 0.7348 |
| 1 | 1.0289 / 5.7553 / 0.0417 / 1.4424 | 1.2019 / 0.9421 / 0.0298 / 0.8351 | 1.3984 / 5.1274 / 0.0179 / 1.3071 | 1.6222 / 0.7267 / 0.0178 / 0.7344 |
| 2 | 1.0924 / 5.7076 / 0.0346 / 1.3999 | 1.1687 / 0.7890 / 0.0245 / 0.7545 | 1.3978 / 5.1111 / 0.0177 / 1.3074 | 1.6204 / 0.7215 / 0.0173 / 0.7315 |
| 3 | 1.0271 / 5.9193 / 0.0354 / 1.4295 | 1.2020 / 0.8277 / 0.0268 / 0.9622 | 1.3980 / 5.1081 / 0.0178 / 1.3085 | 1.6208 / 0.7194 / 0.0174 / 0.7302 |
| 4 | — | 1.4648 / 0.8989 / 0.0251 / 0.9264 | — | 1.6197 / 0.7182 / 0.0175 / 0.7289 |
| 5 | — | 1.2564 / 0.8104 / 0.0239 / 0.8692 | — | 1.6194 / 0.7176 / 0.0175 / 0.7286 |
| 6 | — | 1.2030 / 0.8680 / 0.0271 / 0.8514 | — | 1.6195 / 0.7173 / 0.0175 / 0.7295 |
| 7 | — | 1.2469 / 0.8401 / 0.0262 / 0.8096 | — | 1.6189 / 0.7167 / 0.0175 / 0.7272 |

Mean **candidates / narrowphase tests / touching pairs / contact points**, sampled
before solving. Contact points include speculative contacts. Counts and allocation
averages match between architectures; the solver change alters the later trajectories.

| Boxes | Before counts | After counts | Allocations/tick before → after |
|---:|---|---|---:|
| 1,000 | 9576 / 9576 / 1820 / 12992 | 8896 / 8896 / 2116 / 11902 | 55,775 → 5,013 |
| 2,000 | 15136 / 15136 / 3234 / 18250 | 14328 / 14328 / 3217 / 16573 | 85,022 → 6,802 |
| 5,000 | 19928 / 19928 / 4981 / 27090 | 17711 / 17711 / 4533 / 23537 | 116,327 → 9,751 |

The remaining costs are repeated effective-mass/tensor/anchor preparation across
eight substeps, sequential normal and friction rows in both solve passes, and
exact closest-point searches over 144 edge pairs for nearby separated boxes.
Manifold point Vecs, result Vecs and island arrays still allocate. Cached separating
axes/persistent clipping and complete manifold-buffer reuse were not implemented.
The broadphase is not the largest remaining cost. No engine API change is required:
held queries already provide bulk reads; writes currently insert each component once.

Sleep is still a solver-quality failure as well as a threshold issue. Static ground
does not join dynamic islands, and unchanged poses do not repeatedly wake them.
An island requires every body's linear and angular speed to remain below 0.05 for
30 consecutive ticks. At the end of the 2,000-box run, median linear speed is
0.10883 m/s, maximum 2.70933 m/s and maximum angular speed 2.86256 rad/s. Thus the
calm minimum repeatedly returns to zero; simply forcing sleep would hide motion.

### Three rounds, then stop

| Round | 2,000-box Mac p50 / p95 | Correctness/result |
|---|---:|---|
| 1: geometry/scratch, one solve at 4 substeps | 36.196 / 58.108 | 125-box pile drift 51 mm; never sleeps; rejected solver setting |
| 2: cached impulse responses, one solve at 8 substeps | 34.336 / 45.908 | All original 20 tests pass; retained |
| 3: prepare masses once/tick, paired normal blocks, narrower speculative margin, pose-drift sleep | 19.701 / 26.720 | Oracle position difference 60.331 mm; sleep tick 368 vs 97; all round-3 production changes reverted |

Rounds 1/2 used 900-tick diagnostic windows; round 3 used 2,400 ticks. The full
retained-version measurements are the throughput table above. Neither the 8 ms
p95 budget, 2× Rapier bar nor large-pile sleep acceptance was reached. Optimization
stopped at the specified limit rather than retaining the faster regression.

### Reproduce and inspect

From `game/`, with `EXACT_UPDATE_TRUST=development`:

```sh
cargo run -p exact-game-physics --release --example pile -- 1000 2000 5000
cargo run -p exact-game-physics --release --example pile -- --rapier 1000 2000 5000
cargo run -p exact-game-physics --release --example pile -- --verify
cargo test -p exact-game-physics --no-fail-fast -- --nocapture
cargo clippy -p exact-game-physics --all-targets -- -D warnings
cargo fmt -p exact-game-physics -- --check
```

`PILE_TICKS` overrides the tick limit for shorter diagnostics. `--verify` prints the
125-box tick-600 hash and asserts exact mid-bounce continuation. Raw phase tables
for all three counts, including every substep, are retained here:

- [Mac before](measurements/p1b-before-mac.txt), [Mac after](measurements/p1b-after-mac.txt), [Mac Rapier](measurements/p1b-rapier-mac.txt).
- [Linux before](measurements/p1b-before-linux.txt), [Linux after](measurements/p1b-after-linux.txt), [Linux Rapier](measurements/p1b-rapier-linux.txt).
- [Round diagnostics and rejected regression](measurements/p1b-rounds.txt), [test and Clippy output](measurements/p1b-validation.txt).
