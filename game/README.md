# game/ — the optional game engine add-on

A game is an Exact2 app with a world inside a `canvas`. Write gameplay in Rust and
menus, HUDs and accessible controls in Contract. The simulation and renderer share
one module, loaded after the app's first pixel. Apps without a world carry none of it.

## Optional by construction

`game/Cargo.toml` owns this workspace. It is absent from exact2's root members,
and no core crate depends on an `exact-game*` crate or Rapier. An app opts in by
linking `exact-game` and `exact-game-render` into its **GPU module**, never its
host or data module. The host loads that artifact after first paint when a canvas
needs it. Removing the app's game module removes the engine from its bundle.
The root build, test and boot checks stay independent of the engine workspace.
Weird Castle is the external consumer, with a full-screen Beacons demo.

The integration source is Black's `lane/game` at `126daba5` plus its working-tree
snapshot `321055e1` (2026-09-21). The original checkout remains intact.

## Start a game

Run these commands from the Exact2 repository root. A checkout needs Bun's dependencies
installed with `bun install --frozen-lockfile` and a populated Cargo cache (see
[offline setup](#offline-setup)).

```sh
bun game/new.mjs ./my-game
bun game/dev.mjs ./my-game
```

Open the dev server's printed URL. Edit `my-game/logic/src/lib.rs` for gameplay or
`my-game/app.contract` for UI. Gameplay reloads carry the running world; shared
app-runtime edits reload the page. The server also prints **Open in native**.
Edits inside `Game::setup` take effect on a fresh game; in the starter, pause and
choose **Restart** to apply them.
To explore the existing sample instead, run `bun game/dev.mjs beacons`.

In another terminal, establish the new game's proof baseline:

```sh
bun game/prove.mjs ./my-game
```

The first baseline requires Linux and web agreement in all three simulation modes,
plus a Linux release run. Later invocations verify one GPU-less Linux run by default.
Use `bun my-game/proof.mjs web --screenshot-only` for pixels only; this reports
`UNVERIFIED` because it skips gameplay checks. Its screenshot is
`my-game/artifacts/web/game.png`. The starter includes movement, jumping, two
beacons, pause/restart, touch controls, a ground grid, pads, fog, sun shadows and saved glow.

A bare name (`bun game/new.mjs my-game`) creates `game/games/my-game`; a path chooses
the directory. From an empty directory, `bun /path/to/exact2/game/new.mjs .` works too.
Generation writes only inside that game. The first bake creates its ignored hosts
and workspace under `.shells/`.

| File | What you own |
|---|---|
| `logic/src/lib.rs` | Scene setup, typed arguments and the tick function |
| `logic/Cargo.toml` | Rust package identity, dependencies and their paths |
| `app.contract` | Menus, HUD, layout, accessibility and app actions |
| `proof.mjs`, `logic/tests/sim.rs` | Real-host assertions and hostless simulation tests |
| `app.json` | App identity, game entry point and authored overrides |
| `Cargo.lock`, `pins.json` | Captured dependencies and verified tick/save baselines |

## The programming model

A game implements `setup` and `tick`. Components and resources hold all saved state;
a `Game` itself has no fields. This example is a runnable engine doc-test:

```rust
use exact_game::*;
struct Example;
impl Game for Example {
    const ID: &'static str = "example";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.spawn_named("player", (Transform::default(), Mesh::cube(1.0)));
    }
    fn tick(w: &mut World, _: &Input, _: &()) {
        w.require_mut::<Transform>("player").position.x += w.dt();
    }
}
let mut sim = Sim::<Example>::new(()).unwrap();
sim.run(1000.0);
assert!(sim.local_position("player").unwrap().x > 0.0);
```

- A tick calls ordinary functions in the order you write them. Physics is an
  explicit `physics::step(w)`; animation is `animation::step(w)`.
- `#[derive(Component)]` declares per-entity data; `#[derive(Resource)]` declares
  singleton data. Their `Data` representation supplies saves, hashes and agent JSON.
- Names and entity handles address the same world: `w.require_mut::<Transform>("fox")`
  names missing targets or components. `w.get::<Transform>("fox")` returns an option.
- `query().iter()` yields `(Entity, item)` with borrowed references. Consuming
  `query()` yields guarded items. `.one()` returns the sole item and refuses ambiguity.
  Iteration always follows entity order.
- Borrows are per row. A query leases the rows it matches (after `.with`/`.without`);
  `get`, `require` and the position helpers borrow one entity's row. Reading or writing
  another entity's component inside a loop just works. One row takes many shared
  borrows or one exclusive borrow; a conflict panics naming the component, the entity,
  both borrows and both source lines. See [borrowing](engine/README.md#borrowing).
- The world owns fixed ticks, time and randomness. `w.dt()` is one fixed step;
  `w.tick_end()` names the endpoint being authored. Rendering interpolates between
  completed ticks and never changes the simulation.

Each enemy reads the player's Transform inside the loop that moves the enemies'
Transforms. The query holds only the enemies' rows, so the player's row stays free:

```rust
use exact_game::*;
#[derive(Default, Component)]
struct Enemy {
    speed: f32,
}
let mut w = World::new(60, 7);
w.spawn_named("player", Transform::at(0.0, 0.0, 0.0));
w.spawn((Transform::at(10.0, 0.0, 0.0), Enemy { speed: 4.0 }));
for (_, (pose, enemy)) in w.query::<(&mut Transform, &Enemy)>().iter() {
    let player = w.require::<Transform>("player").position;
    pose.position += (player - pose.position).normalize() * enemy.speed * w.dt();
}
assert!(w.query::<&Transform>().with::<Enemy>().one().unwrap().position.x < 10.0);
```

For a complete small game, start with [the template](new/logic/src/lib.rs) or
[Beacons](games/beacons/logic/src/lib.rs). The [engine reference](engine/README.md)
covers borrowing, registration, observation, saving and loading.

### Arguments, pause and restart

`#[derive(Args)]` supplies a typed argument struct. The canvas binds its field names:
`world(seed=7, paused=paused, restart=again)`. Omitted fields use Rust defaults;
`world()` uses every default. Positional calls follow declaration order; a call
uses either names or positions. The supported scalar types are bool, u32/u64,
i32/i64, f32/f64 and String; 64-bit integers must fit the portable safe-f64 range.

Ordinary fields reconstruct setup when changed. `#[live]` fields reach each tick;
`Game::paused` reads the pause field. A `#[restart]` boolean reconstructs on either
edge: the template toggles Contract's `again` state to restart. `Game::validate`
refuses invalid arguments before changing the world or clock.
Canvas arguments keep the app's current bindings when a world is restored. Use live
arguments for app-owned settings; keep choices that should travel with a game save
in components or resources.

After the first GPU build, the compiler and dev loop check surface names and
arguments against the emitted `.shells/surfaces.json`, including hidden branches
and imports. Rust argument edits reach that declaration through the next GPU build.
The dev compiler retains its last good plan on an error.

### Movement and appearance

| Task | Existing engine operation |
|---|---|
| Move and jump without physics | `w.character("player").step(input.stick_xz("move"), input.pressed("jump"))` |
| Change speed while playing | `w.require_mut::<character::Character>("player").speed = 8.0` preserves velocity and the rest of the character. |
| Move against colliders | [Physics controller example](physics/tests/living.rs), with explicit `Move`, `Jump` and `Gravity` |
| Follow the player | Attach `Follow`; the scene steps it after the tick. Call `scene::follow(w)` explicitly to choose an earlier order. |
| Find a nearby unlit beacon | `w.nearest_xz_mut::<Beacon>("player", 1.5, \|b\| !b.lit)` returns its entity and mutable component together. |
| Count for the HUD | `w.count::<Beacon>(\|b\| b.lit)` |
| Grid and fog | `Material::grid(color, spacing)` and the saved `Environment`/`Fog` resource |
| Fade a glowing mesh | Attach `Glow(Tween)` to `Material::glow`; retarget once and let the renderer sample at frame time. |
| Fade a point light | Attach `Lit(Spring)` and call `lit.to(w.tick_end(), intensity)`. |

`Character` saves velocity and configuration and reports displacement, grounded,
jumped and landed. `near`/`near_xz` use current global poses, inclusive radii and
entity-order ties. The starter uses Character's movement defaults and sets its ground
height and bounds explicitly.

`Glow` multiplies authored emissive output. Keep the material's authored colour
constant; copying the same animated intensity into it applies the effect twice.
See the [renderer reference](render/README.md#effects) for appearance conventions.

## Publications and events

Publish HUD values by name with ordinary Rust scalars:

```rust
use exact_game::*;
let w = World::new(120, 7);
w.publish("lit", 1);
assert_eq!(w.published("lit").unwrap().as_number(), Some(1.0));
```

Contract reads that record through the surface's name:

```contract
shape Hud
  lit: number

component Screen
  resource hud = exactSurface("world") as shape Hud
  view
    canvas surface=world() width="100%" height="100%"
      text `Lit ${hud.lit}`
```

For a structured HUD, `w.publish_record(&hud)` publishes the fields of a `Data` record
together. Nested records, lists, options and scalars retain their names and JSON types.
Contract validates kinds against the shape: missing fields default, extra fields
are ignored. Enum variants are not Contract values. Rust field names are not
checked against the Contract shape at bake time. A rebuild or restore publishes
again; no app data module is needed.
Only the first live canvas owns a given surface's public record.

`w.emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events save in order but stay outside the simulation hash.

## Proof pins

`pins.json` owns the game's expected tick hashes and continuation-save digests.
The generated proof uses `pin(tick, snapshot)` and `pinSave(name, path)`;
Rust tests use `sim.assert_pin(include_str!("../../pins.json"))`.

```sh
bun game/prove.mjs ./my-game
bun game/prove.mjs ./my-game --hosts linux,web --compare-saves
bun game/prove.mjs ./my-game --hosts linux,ios --device --phone <device-id> --compare-saves
bun my-game/proof.mjs linux --paranoid
bun game/prove.mjs ./my-game --repin --reason "describe the intended state change"
```

Repinning runs Off, Save and FreshGame, requires matching source inputs and host
observations, and includes a Linux release check. It refuses failures or a missing
requested browser, leaving the pins unchanged. Existing pins may be updated with
`--hosts linux` explicitly; the first baseline always requires Linux and web.
Existing-pin updates require Git provenance. `CHROME` selects the headless browser.

Paranoid runs check simulation and saves. Each tick uses the normal restore path,
which resets presentation interpolation; use ordinary runs for appearance and
motion comparisons.

`PASS` means the complete saved baseline was checked. Partial/build-only runs and
baseline collection report `UNVERIFIED`; failed assertions report `FAIL`. A
screenshot alone is not a verified simulation. `prove.mjs` prints its unique
`ARTIFACTS` directory; direct `proof.mjs` runs use `artifacts/<host>/`.
Use `--report` on `prove.mjs` to explain recorded stalls and refusals.
`ios` selects the simulator; `--device` selects a paired phone and requires an
accessible development signing key. Direct proofs accept `ios --device --phone
<device-id>` too. Phone artifacts and build receipts use `ios-device`; the proof's
1280 × 720 logical viewport fits into the phone window for equal input dimensions.

The [shared proof runner](proof.mjs) handles builds, sessions, transcripts and
cleanup. Each game's proof owns its assertions. `check` records failures while
independent assertions continue. The terminal shows checks and failure details;
`replies.json` retains complete operation replies. Browser runs provide canvas pixels; the GPU-less
Linux host exercises simulation, input, Contract UI, CPU picks and saves.

## The agent's interface

Use Exact2's same eight operations: `tree`, `screenshot`, `tap`, `type`, `state`,
`layout`, `logs` and `clock`. An entity is addressed as `world:player`.

```sh
EXACT_APP_DIR=./my-game EXACT_WEB_DIST=./my-game/dist bun scripts/agent.mjs web \
  "tap play" "type world key KeyW for 1500" "layout world:player" \
  "state world:* busy" logs "screenshot run.world world save"
```

For a `prove.mjs` web build, set `EXACT_WEB_DIST` to the printed artifact directory's
`dist/`. `state world:*` reads up to 512 entities; `under world:player` narrows it.
`clock settle` advances the owned clock and explains remaining work instead of sleeping.

In a proof, `session.world('world')` supplies `hold`, `run`, `settle`, `get`,
`layout`, `snapshot` and `save`. A held key is released on the same carrier even
when advancing time fails. `open({fresh:true, world:'run.world'})` opens a new
process and restores when Play creates the surface. Current app bindings win over
saved arguments; `restoredFrom` exposes the saved ones while `restored` is true.

Restore validates first and installs the decoded world without running setup.
Register types that first appear mid-game in `Game::register`; registration receives
only setup/restart arguments and must have no gameplay side effects. A refused
restore stays visible in `state.world.restoreError` and logs while the fresh world
remains usable. Captures and restore inputs are limited to 256 MiB.

The same driver reaches web, macOS, Linux and iOS. iOS agent input is
labelled `recognized`; it is not physical-touch evidence. Pointer-completed HUD
buttons return focus to the input canvas, while keyboard activation retains button
focus and consumes its activation keys. Native hover/focus-visible parity remains
[open work](../QUEUE.md).

## Determinism — the contract (LLP 1046.001 D5)

Same seed, tick-stamped inputs and completed tick must produce the same world hash
on every host. Pixels have a tolerance; simulation state is exact.

1. Use only world time and the world's RNG (`rand`, `chance`, `pick` or `rng`).
2. Use `exact_game::math` for transcendentals, such as sine.
3. Keep tick iteration ordered; no `HashMap` iteration or threads in a tick.
4. Keep semantic state in components and resources, without interior mutation
   through shared references. The journal is telemetry, outside hashes and observation.

Seekable advances observe the final tick pair. Ambient entities and explicitly
ambient resources/derived publications stay outside rest observation, but retain
their save rules. Settle does not pause time or force physics bodies asleep.
See the [engine reference](engine/README.md) for those declarations and their limits.

Parity evidence applies to exercised operations and hosts. [Bench receipts](bench/README.md)
and [authoring diaries](diaries/001-scores.md) record measurements and remaining targets.

## Exact2 integration

The first bake derives host adapters and `.shells/app.json` from the authored
manifest. It does not rewrite an existing `app.json`. Contract owns layout, UI,
accessibility and controls; the world owns the canvas contents. Hosts consume
declaration data without a build dependency on the engine or gameplay. The GPU
build writes that declaration directly, without linking the host bake.

For app operations such as Save/Continue, declare a linked Rust data source in
`app.json` alongside the game's entry point:

```json
"game": {
  "crate": "my-game-logic", "type": "MyGame",
  "data": { "crate": "my-game-data", "type": "AppData" }
}
```

Own its code and manifest under `data/`, with `package.workspace = "../.shells"`.
The type implements `DataSource + Default`; the generated hosts supply the existing
`Storage<D>` adapter. Return synchronous resource placeholders before `activate`;
storage work starts after first pixel. This linked composition uses `rust: false`.
Games without `game.data` add no data-source dependency.

`Request::capture_surface("world")` returns complete carried bytes through
`Outcome::Surface(SurfaceOutcome::Captured(bytes))`. Chain an ordinary atomic file
write to save them. Continue reads that file, sends
`Request::restore_surface("world", bytes)`, and completes on `SurfaceOutcome::Restored`.
Declare `surface.read world` / `surface.write world` plus the needed filesystem grants.
The host requires exactly one live named surface, settles its pending assets, and
refuses retired requests. Surface payloads are limited to 16 MiB; the existing
storage protocol separately bounds the serialized file request. Pause before saving
or restoring when the surrounding UI should hold the game still. Set the canvas
`inert` while the request is pending to release held controls and prevent new input.

A game owns one generated Cargo workspace and keeps final products and receipts in
its own `target/`. Intermediate builds reuse the SDK's `game/target/` across apps;
Cargo's standard `CARGO_BUILD_BUILD_DIR` overrides that location.
Keep the source `Cargo.lock` in version control: bakes and deploys resolve offline and locked.
After intentionally changing dependencies, run
`bun game/app/shells.mjs ./my-game --update-lock` and review that lock.
An authored `logic/Cargo.toml` uses `package.workspace = "../.shells"`, concrete
package fields and path dependencies. The first default is scaffolded only when this
file is missing; after that the author owns every byte and the bake never rewrites it,
including after a directory move. Edit dependency paths directly, then update the lock.

Linux development uses `gpu-dev` with separate completed host and GPU receipts;
a logic-only edit can rebuild the GPU alone. Release/production bakes bind its exact
digest. Generated profiles disable floating-point contraction; `gpu-dev` disables
debug assertions and overflow checks as release does. These profiles come from `game/Cargo.toml`, not authored per-game host code.

`game.assets: true` selects the model-capable module; `game.audio: true` links the
audio executor. Optional capabilities are separate artifacts/executors, never
Cargo features on a core crate. The engine's own workspace is outside Exact2's
five core checks.

### Assets, animation and placed UI

Put models and sprite PNGs under `art/`; [the baker](bake/README.md) produces
validated `.model` and `.tex` assets. Declare simulation dependencies in
`Game::ASSETS`; setup waits for them. Models and sprites need the asset-capable
module. Untextured emitters remain available to primitive games.

For a data-authored level, derive `Data` for its record and declare `Game::LEVEL`.
JSON levels work in the primitive module and need no `game.assets` setting or art importer.
Setup reads it with `w.level::<T>(name)` after the
asset barrier. `w.generated(name, mesh_data)` registers immutable generated geometry;
saves check its reconstructed identity rather than storing render vertices. See
[the level example](games/asset-fixture/logic/src/lib.rs) and [engine reference](engine/README.md).

Animation order is explicit: `animation::step` → apply root motion → query sockets.
`SocketFollow` attaches to a joint without changing the saved local transform.
See [the skinned example](games/skinned-fixture/logic/src/lib.rs).

`Placed::child("sign")` attaches the direct Contract child with `testId="sign"` to
a world plane. The name survives reordering; duplicate names or owners refuse.
Layout, accessibility and ordinary controls stay with the app host. See
[the placement game](games/placement-fixture/logic/src/lib.rs) and
[its UI](games/placement-fixture/app.contract).

## Working here

| Area | Reference |
|---|---|
| Simulation and saved data | [engine](engine/README.md) |
| Rendering and effects | [render](render/README.md) |
| Collision and sound | [physics](physics/README.md), [audio](audio/README.md) |
| App adapters | [shell generation](app/shells.mjs) |
| Measurements and history | [bench](bench/README.md), [diaries](diaries/README.md) |
| Architecture | [as built](../llp/1046.003-game-engine-as-built.explainer.md), [agent contract](../llp/1046.001-agent-interface-to-a-game.rfc.md) |

From `game/`, with `EXACT_UPDATE_TRUST=development` set:

```sh
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bun test ./proof.test.mjs
bun app/shells.mjs --test
```

The last command tests the app-owned workspaces, including logic crates outside
the engine workspace. Generated adapters are products, not empty test harnesses.
Pass an app directory to test only that app: `bun app/shells.mjs /path/to/my-game --test`.
Run the affected game's real-host proof too. Device lifecycle tests are opt-in:
`cargo test -p exact-game-render surface_lifecycle -- --ignored` on a GPU host.
Apple host builds use Xcode's developer directory and no inherited `SDKROOT`.

### Offline setup

The checkout expects sibling `../ibex` and `../snapback-sb4` source repositories.
For an isolated checkout, links in its private parent may point at existing copies.
Run `bun install --frozen-lockfile` at the Exact2 root before Cargo validation.

If the Cargo cache is empty, first generate the game, then materialize its adapters
and fetch its captured dependencies from the Exact2 root:

```sh
bun game/app/shells.mjs ./my-game
cargo fetch --locked --manifest-path ./my-game/.shells/Cargo.toml
```

The template's captured lock is the first-bake seed. A generated shell lock without
a source lock is removed and refused; it cannot choose dependency versions.
