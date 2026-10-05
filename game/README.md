# game/ — the optional game engine add-on

A game is an Exact2 app with a world inside a `canvas`. Write gameplay in Rust and
menus, HUDs and accessible controls in Contract. The simulation and renderer share
one module, loaded after the app's first pixel. Apps without a world carry none of it.

## Optional by construction

`game/Cargo.toml` owns this workspace. It is absent from exact2's root members,
and no core crate depends on an `exact-game*` crate or Rapier. An app opts in by
linking `exact-game` and `exact-game-render` into its **GPU module**, never its
host or data module. The host loads that artifact after first paint when a canvas
needs it. An app whose other screens draw too can give the world a module of its own
(`gpu.modules` in `app.json`, LLP 1009 D6), loaded only when the world's canvas mounts.
Removing the app's game module removes the engine from its bundle.
The root build, test and boot checks stay independent of the engine workspace.

On Windows, build the generated native shell from the exact2 root in PowerShell:

```powershell
$env:EXACT_APP_DIR = 'C:\path\to\my-game'
bun host/windows/build.mjs my-game
```

The resulting `my-game/dist-windows` directory contains the standalone executable,
game DLL and assets. Keep these files together; the executable resolves assets
relative to itself and needs no web server. `--release` selects the release profile,
and `--run` opens the game. The initial Windows host uses GPU canvas readback and
CPU UI composition, and reports mean and p50/p95 frame costs when its window closes.
It supports store level 0; native update-store delivery is refused. Use
`bun my-game/proof.mjs windows` for the native gameplay and pixel proof.
For a bounded development window measurement, set `EXACT_AGENT_WINDOW_FRAMES`
to 1–10,000 and optionally `EXACT_AGENT_WINDOW_TAP` to one authored test ID to tap
after first pixel. Production bakes ignore these agent variables.

Asset names have no fixed per-surface count limit. Web loading still bounds
concurrent requests and decoded asset bytes; a queued request's timeout begins
when its fetch starts.
Weird Castle was the first external consumer, with a full-screen Beacons demo in its own
engine module; its title sky never loads the engine.

The integration source is Black's `lane/game` at `126daba5` plus its working-tree
snapshot `321055e1` (2026-09-21). The original checkout remains intact.

## Start a game

Run these commands from the Exact2 repository root. With the pinned Bun and rustup
installed, `bun scripts/exact.mjs setup` installs the SDK's Rust toolchains,
wasm-bindgen, Binaryen and Bun dependencies. `setup --check` diagnoses missing or
mismatched tools. See [offline setup](#offline-setup) for populating Cargo's cache.

```sh
bun game/new.mjs ./my-game
bun game/dev.mjs ./my-game
```

Open the dev server's printed URL. It serves on 8765, or on the next free port when
another dev loop holds 8765 (it says so); `--port <n>` chooses one and fails at once,
before building, if that port is in use. `--lan` serves a phone on this network. Edit `my-game/logic/src/lib.rs` for gameplay or
`my-game/app.contract` for UI. Gameplay reloads carry the running world; shared
app-runtime edits reload the page. Once serving, the loop builds the gameplay
module's `gpu-dev` build in the background, so the first gameplay edit is warm. The server also prints **Open in native**.
Edits inside `Game::setup` take effect on a fresh game; in the starter, pause and
choose **Restart** to apply them.
To explore the existing sample instead, run `bun game/dev.mjs beacons`.
`game/dev.mjs` passes the dev server's flags through (`--port`, `--lan`, and
`--allow-host <name>`, repeatable, to serve the game through a tunnel such as
`tuft host`; without it a request under any name but the printed ones gets 421).

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
A game outside this checkout — `bun scripts/exact.mjs new ../my-game --game` makes
one, as `exact new` makes an app — also gets an app's `exact.mjs` and `AGENTS.md`:
`bun exact.mjs test-rust` (the hostless tests), `web`, `web-build`, `test web`
(`app.test.contract`; also `macos`, `ios`), `agent`, `mac`, `ios`, `contract`,
`prove` and `feedback`, with the exact2 checkout named once (LLP 1086).
Generation writes only inside that game. A game is the files its author writes; bakes
generate its hosts, Cargo workspace and lock, ignored, under `.shells/`.

| File | What you own |
|---|---|
| `logic/src/lib.rs` | Scene setup, typed arguments and the tick function |
| `app.contract` | Menus, HUD, layout, accessibility and app actions |
| `proof.mjs`, `logic/tests/*.rs` | Real-host assertions and hostless simulation tests |
| `app.test.contract` | Menus and HUD driven on a host (`bun scripts/agent.mjs <host> --test`) |
| `pins.json` | Verified tick/save baselines, written by `prove.mjs` |
| `app.json` (optional) | Authored keys only: a title, bundle id, `game.audio`, `game.assets`, data and render crates |
| `logic/Cargo.toml`, `Cargo.lock` (optional) | Only when the game adds dependencies ([below](#exact2-integration)) |

The crate is `<Game::ID>-logic` and the bundle id `com.exact.<Game::ID>`; the title
follows the directory name. Run the Rust tests with
`bun game/app/shells.mjs ./my-game --test` — in a fresh clone too, with no bake and
no environment variables. It generates `.shells/`, resolves offline and locked, runs
the determinism lints and then `cargo test` on the game's crates.
The generated workspace carries the SDK's Rust toolchain pin, including for games
outside the SDK. Explicit `cargo +toolchain` and `RUSTUP_TOOLCHAIN` overrides still
apply. Run direct Cargo commands from the game's `.shells/` directory so they also load its deterministic compiler
flags and lint configuration; `--manifest-path` alone does not load that config
when Cargo starts elsewhere. Regenerate the shells after updating the SDK.

### Authored render hooks

Use `game.render` to add game-owned passes through the existing
`exact_game_render::Hooks` API. Keep GPU resources in an authored `render/`
crate; its Cargo package must match the declaration, end in `-render`, and set
`workspace = "../.shells"` under `[package]`. `bun game/new.mjs my-game --render`
scaffolds one:

```json
{"game":{"render":{"crate":"my-game-render","hooks":"Fog","shaders":"shaders::SHADERS"}},
 "gpu":{"shaderRoots":["render/shaders"]}}
```

`hooks` and optional `shaders` are Rust paths exported by that crate: `hooks` its
`Hooks` type, `shaders` its reflected registry symbol (not shader files). Omit
`shaders` for the empty registry. Shader files and preludes use the explicit
`gpu.shaderRoots` and `gpu.shaderPreludes` declarations; nothing is discovered
implicitly. The registry has type `&'static [(&'static str, u64)]`, holding shader
names and reflected interface hashes. The generated GPU shell combines these hooks
with `game.audio` and `game.assets`. Never edit `.shells/` to install hooks. The
older `game.presentation`/`presentation/`/`type` spelling is refused with this
rename (LLP 1046.008 amendment); "presentation" now names only derived appearance
state (`#[derive(Presentation)]`, `Game::present`).
Render games also expose `exact-gpu.workspace = true` and
`exact-gpu-reflect.workspace = true` (the latter as a build dependency). The bake
writes the shader inventory, each shader after its `gpu.shaderPreludes` exactly as
it ships, to the directory `EXACT_GAME_SHADERS` names; `render/build.rs` calls
`exact_gpu_reflect::generate` on it, writes the generated Rust to `OUT_DIR`, and
exports its `SHADERS` table, so the registry matches what hosts register. A prelude
may name the renderer's own WGSL by set (`exact-game-render:frame`,
`exact-game-render:material`, `exact-game-render:material_shadows`, the files of
`FRAME_WGSL`, `MATERIAL_WGSL` and `MATERIAL_SHADOWS_WGSL`) rather than SDK paths.
Generated registry keys are file stems such as `fog`, without `.wgsl`.

The render crate may depend on logic to read component/resource types through
`RenderWorld`. Logic, data, native/web host adapters and build-time metadata must
not depend on it, including indirectly or behind target-specific/build
dependencies. The bake checks Cargo's unfiltered dependency graph. Hooks cannot
mutate the saved world through `RenderWorld`; gameplay state stays in logic.
The render crate receives normal strict Clippy and package tests, while the simulation
determinism lints remain scoped to logic. Run the ordinary bake after declaring the
crate, or `bun game/app/shells.mjs ./my-game --update-lock` when adding dependencies.
Source/manifest edits invalidate GPU builds and proof inputs; declared shaders keep
their existing reflection, delivery and live-reload behavior.

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
  explicit `physics::step(w)`; animation is `animation::step(w)`. Emitters step after the tick unless it
  called `emitter::step(w)` to choose their order; they never step twice a tick.
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
- For immediate device input in a `Sim` test, use `sim.input_now(event)`; it stamps
  the event with the current host clock. `sim.input(event)` preserves an external
  device timestamp. World seconds are not that clock after restoring a save.
  `key_down`, `key_up` and `post` already stamp immediate input correctly.

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
and imports. Rust argument edits reach that declaration through the next GPU build,
so while the game's Rust is newer than it, `contract build` reports its findings as
warnings beside every other diagnostic, and `contract types` and `contract rust`
never stop on them; a bake checks them against the declaration it has just written.
The dev compiler retains its last good plan on an error.

### Movement and appearance

| Task | Existing engine operation |
|---|---|
| Move and jump without physics | `w.character("player").step(input.stick_xz("move"), input.pressed("jump"))` |
| Change speed while playing | `w.require_mut::<character::Character>("player").speed = 8.0` preserves velocity and the rest of the character. |
| Move against colliders | [Physics controller example](physics/tests/living.rs), with explicit `Move`, `Jump` and `Gravity` |
| Follow the player | Attach `Follow`; the scene steps it after the tick. Call `scene::follow(w)` explicitly to choose an earlier order. |
| Put a HUD label over a world point | `w.project(point, input.viewport())` returns CSS pixels through the current camera, including same-tick parent movement. It returns `None` outside the camera's clip volume; use a physics ray separately to hide labels behind cover. |
| Find a nearby unlit beacon | `w.nearest_xz_mut::<Beacon>("player", 1.5, \|b\| !b.lit)` returns its entity and mutable component together. |
| Count for the HUD | `w.count::<Beacon>(\|b\| b.lit)` |
| Grid and fog | `Material::grid(color, spacing)` and the saved `Environment`/`Fog` resource |
| Fade a glowing mesh | Attach `Glow(Tween)` to `Material::glow`; retarget once and let the renderer sample at frame time. |
| Fade a point light | Attach `Lit(Spring)` and call `lit.to(w.tick_end(), intensity)`. |
| A flashlight | `SpotLight { inner, outer, range, intensity, .. }` along the entity's −Z; intensity in candela, as `PointLight`'s |
| Shadows from a lamp | Add `LightShadows` to a `SpotLight` or `PointLight`; the renderer shadows the nearest few |
| A moon | A second `DirectionalLight` (in entity order) is an unshadowed fill |
| Team colours, mutation looks | A `Material` on a model entity tints every node and adds emission; `NodeMaterials` per named node or `MaterialOverrides` per material (one model in every team's colours; metallic and roughness too), written from `Game::present` |
| A glint, flicker or cycling hue | A `MaterialOverride`'s `shimmer` (`Pulse`, `Flicker`, `Hue`): the GPU moves it with the displayed time, so derive it once and return `Derived::Kept` |
| Cheaper far trees and crowds | `ModelLod { levels: vec![LodLevel { distance: 30., model: "tree_low.model".into() }], hide: Some(120.) }` on the entity |
| Fade a tree between camera and player | `Opacity(0.3)` on the entity from `Game::present`: a dithered fade, no sorting, never in a save or pin |
| Walk cycles on streamed models | `animation::ShownClips::clip("run", metres / stride)` on the entity from `Game::present`: any arrived model, drawn only ([engine](engine/README.md#movement-animation-and-sound)) |
| A model per growth stage, a day and night, a switchable look | From `Game::present`: `DrawnMesh::model(name).lod(..)` in place of the simulated `Mesh`, `DrawnLight::Directional(..)` (with an `Offset` to aim it) for a sun, and `DrawnEnvironment` on the camera for the sky; read the look from a `#[live]` argument and switching keeps the world; derive per-entity rows with `p.each::<Plant>(..)` so a present costs the entities that changed ([engine](engine/README.md#settling-and-data-formats)) |
| A first-person weapon | Add `ViewModel` to each part; it draws in front of the world and casts no shadow |
| Contact shadows in creases | `w.insert_resource(AmbientOcclusion::default())` turns on SSAO (off by default) |
| Lighting from a photographed sky | `w.insert_resource(EnvironmentMap::new("sky.tex"))` with the equirect in `Game::ASSETS` |
| Code-made models | `asset::MeshBuilder::flat()` or `smooth()`: boxes, ellipsoids, tubes, lathes and sheets painted per vertex, then `finish()` into `w.generated` or `Model::parts` ([engine](engine/README.md#saves-and-assets)) |
| Texture generated terrain | UVs in the `MeshData`, a material sampling a `.tex` from `art/textures/`, through `w.generated_model` |
| Fire, smoke and sparks | `ParticleLook { texture, atlas, fps, stretch, soft }` beside an `Emitter`; `soft: 0.5` fades smoke into the ground |
| A painted, visible sky | The same `EnvironmentMap` with `visible: true` (and `rotation` to turn it) |
| Mouse look | `input.pointer()`'s `delta` is the device's motion this tick, not a difference of positions. Mark the canvas `data-pointer-lock="true"` (declared in `app.json`'s `data`) and a mouse press captures the mouse on the web, macOS and iPadOS (`GCMouse`) until Escape or blur, so the delta never stops at an edge; the Linux host always sends evdev's relative motion. |
| Turn the drawn camera between ticks | Put `MouseLook { yaw_per_point, pitch_per_point, pitch_limit }` on the camera at the rates the tick turns it by: the drawn camera and its children turn by `Sim::unshown_motion()`, so a turn shows at the next frame whatever the tick and display rates (`engine/tests/look.rs`). Presentation only; insert it in `setup` or register it. |
| Right or middle mouse button | Bind it as a key: `.button("aim", &["MouseRight"])`; `MouseLeft` and `MouseMiddle` too (`MOUSE_BUTTONS`). Touch contacts press none. |
| Start a selection rectangle | `input.pointer().and_then(\|p\| p.press_origin)` is the latest MouseLeft/touch Down point, even if movement and Up reach the same tick. Gate commands on your action's pressed/released edges. The origin survives normal Up and saves; Cancel, Blur or replacement clears it. EXSIM v7 saves are required. |

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
again; no app data module is needed. The whole record may be up to 16 MiB of JSON
(a whole inventory fits); a field change re-encodes only that field. A record over
the limit, or one the shape refuses, leaves the last accepted one standing and is
named, with its size and the limit, in the app's log and the agent's
`state.surfaceRefusals`.
Only the first live canvas owns a given surface's public record.

Give a read-only HUD overlay `pointer-events="none"` on its Contract container.
The transparent parts of a wide positioned column still participate in hit-testing;
an extra status line can make that column cover a nearby button without hiding it.
Put interactive controls in their own container. Garden's watering proof exercises
this by adding the empty-plot hint before tapping the Water button.

`w.emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events save in order but stay outside the simulation hash.
When comparing continuation saves, make the pending input equivalent too.
Opening a native HUD panel can queue `Blur`; doing that after the last clock
step leaves a different save even when the world tick and hash match. Perform
the UI setup before the shared continuation steps so those inputs are consumed,
or deliberately compare the same queued input on both sides. Do not discard
pending input just to make saves agree.

Untargeted `state` exposes each world's `input.pending`: `total` and counts by
event kind (`key`, `pointer`, `control`, `wheel`, `blur`, `message`). These count
events currently retained after coalescing and refusals, not input history.
For example, `total: 3, blur: 3` identifies three focus-loss events waiting for
a tick even if `held` and `forwarded` are empty. Inspection consumes nothing;
the summary survives save/restore and clears when normal simulation consumes
the queue. Counts omit payloads, order and timestamps, so equal counts alone
do not prove equal saves. Compare the complete save bytes for that.

Commands go the other way as messages: an action calls
`postMessage("buy carrot", "world")` and the next tick reads every message posted
since, in order, from `input.messages()`. Nothing is coalesced, nothing needs an
acknowledgement, and a delivered message is not part of a later save. Use live
arguments for settings, messages for things that happen once.

## Proof pins

`pins.json` owns the game's expected tick hashes and continuation-save digests.
The generated proof uses `pin(tick, snapshot)` and `pinSave(name, path)`;
Rust tests use `sim.assert_pin(include_str!("../../pins.json"))`.

Pins cover the simulation only. Presentation state (`#[derive(Presentation)]`,
`Game::present`) is outside hashes and saves, so prove looks separately: assert
presentation values (`sim.world().get::<Offset>(e)`) at an explicit tick, and compare
pixels at an explicit tick and alpha; never by moving a pin. "Settled" means the
simulation settled; a bobbing world rests. Agent state reports simulated poses
(`Transform`, `global`); the displayed pose is `World::drawn`, and presentation rows
appear beside the components they decorate, read-only.

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
Every update records the matching source-input digest; a Git commit is recorded
when one exists. A game without commits can repin under the same agreement checks.
`CHROME` selects the headless browser. Without it, the agent prefers an installed
browser from this checkout's pinned `playwright-core`, then system Chrome (or
Chromium/Edge). Discovery never downloads a browser. An explicit missing `CHROME`
is refused rather than silently replaced.

Paranoid runs check simulation and saves. Save and FreshGame modes rebuild the
world through the normal restore path at the last tick of every advance (every
point a proof observes), at every tick that received input, and every 16th tick
inside an advance, which resets presentation interpolation. A save bug visible only
in a tick none of those cover (state that lives one unobserved, input-free tick
and is gone by the next sample) is no longer caught; run the proof with shorter
`clock` steps to sample more ticks. use ordinary runs for appearance and motion comparisons. The
driver moves a long `clock +N` in one-second steps, each its own operation, so a
seek's length is not bounded by a host's answer window (Chrome's 15 s).

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

### The shipped Mac bundle

Proofs drive development builds. `bun scripts/exact.mjs release <game> --check` drives
what `exact release` ships: the distribution build, stripped and signed inside out
(ad hoc with no timestamp, so it needs no certificate or network; nothing is notarised),
launched from `<target>/dist/<game>-check/` with only its own resources and modules. It
taps `play` when the title has no canvas, then requires the GPU module to load (no
`GPU module …` refusal), the world to tick and hold entities, and the canvas to hold a
picture rather than its background (`shipped.png` beside the bundle). A failure is a
`FAIL <game> <check>: <why>` line. `exact release` drives its Developer ID bundle the same
way before notarising, and the async lane runs it for garden. To drive any bundle by hand:
`bun scripts/agent.mjs macos --app <game> --bundle <Name.app> …`.

### Compare captured world state

In a proof or an agent session, capture inspected state before and after an action:

```js
import {captureWorld, diffWorlds, formatWorldDiff} from '/path/to/exact2/game/proof.mjs';
import {writeFileSync} from 'node:fs';
const before = await captureWorld(session, 'world');
await session.world('world').hold('KeyW', 500);
const after = await captureWorld(session, 'world');
writeFileSync('before.json', JSON.stringify(before));
writeFileSync('after.json', JSON.stringify(after));
console.log(formatWorldDiff(diffWorlds(before, after)));
```

Compare those files later with `bun game/proof.mjs diff before.json after.json`.
Exit codes: 0 for equal inspected values and hashes, 1 for differences, 2 for a
refused capture. The report shows field paths and before/after values, including
added/removed entities, components, resources and array entries. It includes
arguments, input and published values; tick/hash are report metadata. Named
entities match by name even if their slots change; unnamed entities match by
slot ID (inspection does not expose generations, so slot reuse is indistinguishable).
Arrays compare by index. The default report retains 100 differences and counts
omitted ones; `diffWorlds(a, b, {limit: 500})` changes that output limit.

These JSON captures are **not `.world` binary saves** or a complete explanation of
a hash. Opaque executor state stays opaque; inspection can omit or round internal
values. A differing hash with equal inspected values is reported explicitly.
Capture on the agent clock without concurrent drives. Mixed-tick/hash reads and
incomplete entity lists are refused; `captureWorld` reads every page. Capturing and
comparing state does not advance the clock or mutate the world.

## The agent's interface

Use Exact2's same eight operations: `tree`, `screenshot`, `tap`, `type`, `state`,
`layout`, `logs` and `clock`. An entity is addressed as `world:player`.

```sh
EXACT_APP_DIR=./my-game EXACT_WEB_DIST=./my-game/dist bun scripts/agent.mjs web \
  "tap play" "type world key KeyW for 1500" "layout world:player" \
  "state world:* busy" logs "screenshot run.world world save"
```

While `bun game/dev.mjs my-game` runs, its compiler rewrites `dist/`'s plan on every
edit; a drive of that `dist/` then goes to the running loop, which says so.
For a `prove.mjs` web build, set `EXACT_WEB_DIST` to the printed artifact directory's
`dist/`. `state world:*` reads entities a page at a time (512 by default):
`state world:* from 512 limit 2000` reads the next ones, and the reply carries
`total` and `next`; `under world:player` narrows it. `state world:* resources` adds
every resource's value. `state world perf` arms the renderer's sample rings
(`perf_reset` clears them); until then `perf.armed` is false and the rings read zero. In a proof, `session.world('world').snapshot({all:true})`
reads every page at one tick and `resources()` the resources.
`clock settle` advances the owned clock and explains remaining work instead of sleeping.
Inspected `Data` enums are objects keyed by variant: Forest's food item has
`kind: {Food: {}}`. Test it with `Object.hasOwn(item.kind, 'Food')`; variants with
payloads keep those fields under the same key.

In a proof, `session.world('world')` supplies `hold`, `run`, `settle`, `get`,
`layout`, `snapshot` and `save`. A held key is released on the same carrier even
when advancing time fails. `open({fresh:true, world:'run.world'})` opens a new
process and restores when Play creates the surface. Current app bindings win over
saved arguments; `restoredFrom` exposes the saved ones while `restored` is true
(a save leaves out arguments equal to their defaults).

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

For model-directed exploratory play, import `decide` from `game/proof.mjs`.
Pass a small `state` observation, a `goal`, a `choices` record mapping allowed
action names to descriptions, and a `transcript` JSONL path. Action names start
with a lowercase letter and contain lowercase letters, digits, `_` or `-`;
descriptions and the goal must be nonempty strings. Setup errors name every
invalid action before making a request. Jev returns a
`choice`, its confidence (and, from the gateway, probabilities), usage and request
duration. The game script executes that named action through the ordinary session
driver. Only the supplied observation leaves the machine, and no key enters the app.
Unknown choices, missing sources and network errors refuse the step; there is no
substitute policy or retry loop.

The model behind `decide` is chosen in the driver's environment; every source reads
the same request (goal, observation, choices) and must answer one offered action id:

| `EXACT_JEV_SOURCE` | Needs | Asks |
|---|---|---|
| `gateway` | `AI_GATEWAY_API_KEY` | `typesafe-ai/jev` on the Vercel AI Gateway |
| `anthropic` | `ANTHROPIC_API_KEY` | the Messages API, structured output, effort `low` |
| `claude` | the Claude Code CLI, logged in | `claude -p` with no tools, settings or project files |

Unset, a gateway key wins, then an Anthropic key; `claude` is only ever named. The
last two take `EXACT_JEV_MODEL` (default `claude-opus-5-5`). With none of these the
playtest refuses at its first decision and names all of them. Each transcript row
records its `source`. Through the CLI a decision takes 6–30 s (a 12-decision compost
playtest on `claude-haiku-4-5`: about 6 minutes).

A recorded playtest replays without any model: `--replay <recording>` (a
`*decisions.jsonl`, or the artifact directory holding one) feeds its decisions in
order, by action id, to the current build. Pass the flags the recording ran with;
`summary.json` keeps them as `args` from now on, and a changed goal at decision 1
says so. The replay writes to `artifacts/replay-<host>` (`EXACT_PROOF_OUT` moves it):

```sh
bun game/games/garden/proof.mjs web --playtest --compost --replay game/games/garden/artifacts/jev-compost-web
```

Each decision compares what the build shows with what the recording showed
(all but `recent`, which repeats earlier observations). A difference in words
alone (the same numbers in the same order) is noted as `REPLAY wording or shape
only`, so a rewritten sentence without numbers reads as wording too. A changed
number is a value divergence and a HUD row or control that came or went is a
row change; the first of each is reported with the decision that saw it. An entity
handle (`{index, generation}`) that moved slot is a note: one entity more or fewer
spawned earlier shifts every later one. A recorded action the build no longer
offers, a build that wants more or fewer decisions than were recorded, a changed
number among the outcome file's values, or a different final tick fails the proof
and names the first value divergence and row change. Changed words in the outcome, a number
recorded where its label was, or a field only one run recorded are notes. A
different world hash is a note too, listed by entity name and component, since a
scene or art change rewrites entities the play never touched. `replay.json` holds
every decision's differences. Replay checks that HUD and gameplay changes keep a
recorded play's meaning; it cannot say what a model would choose differently now.

Recordings are artifacts, so they are not committed: replay one from the checkout
that made it, or copy its directory alongside.

Forest's `bun game/games/forest/proof.mjs web --playtest` is the first consumer
(`macos` works too). It allows at most 48 decisions, reads only the visible HUD,
and follows the player's compass with short key holds. Its artifacts include
`jev-decisions.jsonl`, `jev-outcome.json` and a screenshot. Exploratory runs report
`UNVERIFIED` even when they finish without errors: model decisions are observations,
not fixed simulation pins. Keep deterministic gameplay and save proofs alongside
them.

## Determinism — the contract (LLP 1046.001 D5)

Same seed, tick-stamped inputs and completed tick must produce the same world hash
on every host. Pixels have a tolerance; simulation state is exact.

1. Use only world time and the world's RNG (`rand`, `chance`, `pick` or `rng`).
2. Use `exact_game::math` for transcendentals, such as sine.
3. Keep tick iteration ordered; no `HashMap` iteration or threads in a tick.
4. Keep semantic state in components and resources, without interior mutation
   through shared references. The journal is telemetry, outside hashes and observation.

Clippy holds every game's logic library to rules 1–3 with the SDK's
[`app/determinism/clippy.toml`](app/determinism/clippy.toml): std float transcendentals
(`sin` … `powi`, `hypot`, `mul_add`), `HashMap`, `HashSet`, `RandomState`, `Instant`,
`SystemTime`, `std::thread::spawn` and `rand`'s ambient generators are refused, each
naming its replacement. `sqrt` is correctly rounded and allowed; glam is built with
`libm`, so `Vec3::length`, `Quat::slerp` and the other glam methods are portable. The
lints run in `shells.mjs --test`, before first pins and `--repin`, and in every
production-profile bake (web, Apple, release Linux, deploy); the gpu-dev edit loop
leaves them to those. An `#[allow(clippy::disallowed_types)]` on an item is a
visible exception — a `HashMap` that is only ever looked up, say. Tests may time
themselves. Setting `RUSTFLAGS` replaces the workspace's `-fp-contract=off`, so a
game build under it is refused until the flag is appended.

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

Own its code and manifest under `data/`, with `package.workspace = "../.shells"`;
like any added dependency, it resolves against the SDK lock or the game's own.
The type implements `DataSource + Default`; the generated hosts supply the existing
`Storage<D>` adapter. Return synchronous resource placeholders before `activate`;
storage work starts after first pixel. This linked composition uses `rust: false`.
Keep saves, scores and settings in app storage (SQLite or files under `app:/data`,
with `exact-data.workspace = true`), not under `secret.keep`, which is for secrets;
`docs/reference.md`, "Rust data sources", has the requests and a best-times example.
Games without `game.data` add no data-source dependency.

[Tennis](games/tennis/README.md) uses one for HTTP: the world publishes a numbered
question, `resource plan = jev(hud.ask)` posts it, and the answer returns as a
`#[live]` string argument that the tick applies only when its id matches.

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
its own `target/`, including intermediate builds, so copies of a game cannot
overwrite each other's units. Cargo's standard `CARGO_BUILD_BUILD_DIR` overrides
that location when explicitly set. Without
`logic/Cargo.toml`, the workspace's generated member builds `logic/src/lib.rs` and
`logic/tests`, `examples` and `benches` where the author wrote them.

The SDK owns one lock, [`app/shells.lock`](app/shells.lock): the lock of the union of
every generated shell's dependencies. A game's workspace starts from it and keeps its
subset; each version must be the SDK lock's, so bakes, deploys and proofs still resolve
`--locked --offline` and no local cache chooses a version. A registry package the SDK
lock lacks but exact2's root `Cargo.lock` pins (a core crate's new dependency) is
admitted at the root lock's version and checksum until the SDK lock is refreshed. A game that adds a
dependency writes `logic/Cargo.toml` (`package.workspace = "../.shells"`, SDK crates as
`exact-game.workspace = true`) and captures its own `Cargo.lock` with
`bun game/app/shells.mjs ./my-game --update-lock`; commit and review that lock. SDK
crates alone need no lock of the game's own. When the SDK's dependencies change,
`bun game/app/shells.mjs --update-lock` refreshes the SDK lock, and
`bun app/shells.mjs --test` refuses a stale one. The game workspace also carries
the core’s vendored patches and `host-dev` profile; the existing SDK test checks
those against the root workspace. Web builds retain the game’s non-contracting
floating-point flag alongside the core’s path-remapping flags.

Linux development uses `gpu-dev` with separate completed host and GPU receipts;
a logic-only edit can rebuild the GPU alone. Release/production bakes bind its exact
digest. Generated profiles disable floating-point contraction; `gpu-dev` disables
debug assertions and overflow checks as release does. These profiles come from `game/Cargo.toml`, not authored per-game host code.

`game.assets: true` selects the model-capable module; `game.audio: true` links the
audio executor, which plays synthesized and sampled sounds. Sampled sounds need only
`game.audio: true`. Optional capabilities are separate artifacts/executors, never
Cargo features on a core crate. The engine's own workspace is outside Exact2's
five core checks.

### Assets, animation and placed UI

Put models, sprite PNGs and WAV or Ogg Vorbis sounds under `art/`;
[the baker](bake/README.md) produces validated `.model`, `.tex` and `.sound` assets,
each texture as RGBA8, BC and ASTC files of which a device fetches one
([texture payloads](bake/README.md#texture-payloads)). Name the authored `x.tex`
everywhere. Declare simulation dependencies in `Game::ASSETS`; setup waits for them.
Put everything else the game shows in `Game::STREAMED`: Play does not wait for
them; each draws as it lands and stays resident. They never compete with the
first frame: one an entity shows is fetched with what setup and the screen wait
for, the rest only once nothing else is in flight, and a device prepares none
before its first drawn frame, then a few models per frame, shown ones first.
`Game::prefetch(name, args)` says which are fetched before anything shows them
(all, by default): a look that never shows a set leaves it unfetched, and a live
switch to one that does fetches the set then, what it shows first. Simulation
cannot read a streamed asset (`w.model` is None), so load order never reaches the hash; a save refuses only while a shown one is in
flight. A model whose animation a tick reads (root motion, markers, sockets it
queries) belongs in `ASSETS`. A streamed model, or one loaded on sight, animates
from `Game::present` instead: `animation::ShownClips` names clips at times derived from
saved causes, and draws once the model lands (below). Streamed names are
models and textures (sounds are not streamed yet), and never also in `ASSETS`.
A generated model (`w.generated`) cannot take a name `ASSETS`, `STREAMED` or the
level declares: registration refuses it, so a hostless test sees the collision.
Models and sprites need the asset-capable module; sounds and untextured emitters do
not. See [the audio executor](audio/README.md) and
[the audio fixture](games/audio-fixture/logic/src/lib.rs) for sampled sounds.

For a data-authored level, derive `Data` for its record and declare `Game::LEVEL`.
JSON levels work in the primitive module and need no `game.assets` setting or art importer.
Setup reads it with `w.level::<T>(name)` after the
asset barrier. `w.generated(name, mesh_data)` registers immutable generated geometry;
saves check its reconstructed identity rather than storing render vertices. See
[the level example](games/asset-fixture/logic/src/lib.rs) and [engine reference](engine/README.md).

Animation order is explicit: `animation::step` → apply root motion → query sockets.
`SocketFollow` attaches to a joint without changing the saved local transform.
See [the skinned example](games/skinned-fixture/logic/src/lib.rs).
Animation nothing simulated reads — a soldier's run, a survivor's idle — can be
presentation: `p.insert(e, animation::ShownClips::clip("walk", walked / stride)
.in_place("root"))` in `Game::present` plays on any model that has arrived, its
props on sockets following the drawn rig, and it never enters a hash, save or pin
([the streamed fox](games/skinned-fixture/logic/tests/streamed.rs)).

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

From `game/` (an unset `EXACT_UPDATE_TRUST` bakes development trust):

```sh
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bun test ./proof.test.mjs ./decisions.test.mjs
bun app/shells.mjs --test
```

The last command checks the SDK lock, then lints and tests every in-tree game's
crates in its own workspace. Generated adapters are products, not empty test harnesses.
Pass an app directory to test only that app: `bun app/shells.mjs /path/to/my-game --test`.
Run the affected game's real-host proof too. Device lifecycle tests are opt-in:
`cargo test -p exact-game-render surface_lifecycle -- --ignored` on a GPU host.
Apple host builds use Xcode's developer directory and no inherited `SDKROOT`.

### Offline setup

The checkout expects sibling `../ibex` and `../snapback-sb4` source repositories.
For an isolated checkout, links in its private parent may point at existing copies.
Run `bun install --frozen-lockfile` at the Exact2 root before Cargo validation.

`bun scripts/exact.mjs setup` fetches the SDK lock's versions into Cargo's cache
(`setup --check` says whether they are all there). A game that adds packages fetches
its own after materializing its adapters:

```sh
bun game/app/shells.mjs ./my-game
cargo fetch --manifest-path ./my-game/.shells/Cargo.toml
```

The generated shell starts from the SDK lock, so the fetch downloads its versions; a
resolution that would take any other version is refused.
