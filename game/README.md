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

On Windows, build from the generated game's directory in PowerShell:

```powershell
bun exact.mjs windows --run
```

For an older external game, run `bun ../exact2/scripts/exact.mjs new . --update`
once to refresh its runner. From the SDK root, the equivalent is
`bun host/windows/build.mjs my-game` with `EXACT_APP_DIR` naming the game.

The resulting `my-game/dist-windows` directory contains the standalone executable,
game DLL and assets. Keep these files together; the executable resolves assets
relative to itself and needs no web server. `--release` selects the release profile,
and `--run` opens the game. A running copy holds its files open; `--out <dir>`
packages into another directory instead. The initial Windows host uses GPU canvas readback and
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
wasm-bindgen, Binaryen and Bun dependencies, and builds the engine rustdoc that
`exact game vocab <name>` reads, so a first lookup skips that cold build (seconds, not
minutes). `setup --check` diagnoses missing or
mismatched tools. See [offline setup](#offline-setup) for populating Cargo's cache.
Cargo installs wasm-bindgen into `$CARGO_HOME/bin` (normally `~/.cargo/bin`);
include that directory in `PATH` for web builds and the dev loop.
Setup provisions source dependencies; it does not compile the optional engine.

```sh
bun game/new.mjs ./my-game
bun game/dev.mjs ./my-game
```

Open the dev server's printed URL. It answers with a building page before compiling.
`bun exact.mjs status` lists each web/prove invocation, its phase and elapsed time.
Start `web` in one terminal, then `bun exact.mjs web --ready --timeout 600` in another
to wait for that invocation's current build. It refuses a failed rebuild even when
the server still shows the previous game. A dead process is reported as `unknown`;
`running` means the process exists, not that the compiler is making progress.

The dev loop serves on 8765, or on the next free port when
another dev loop holds 8765 (it says so); `--port <n>` chooses one and fails at once,
before building, if that port is in use. `--lan` serves a phone on this network. Edit `my-game/logic/src/lib.rs` for gameplay or
`my-game/app.contract` for UI. `bun exact.mjs check` (or `bun game/app/shells.mjs <game> --check`) checks the logic
library for type errors and determinism violations, without linking. The dev loop prints
that result before building each Rust save and keeps the running game when it fails.
Host, linker and target errors still belong to the build.
Gameplay reloads carry the running world in games without tables. In table games,
successful Rust saves rebuild the primary module and restart the world; table-only web
edits preserve it. Shared app-runtime edits reload the page. The server also prints **Open in native**.
Edits inside `Game::setup` take effect on a fresh game; in the starter, pause and
choose **Restart** to apply them.
The web dev loop builds the app host under the size-tuned `web` profile and its
GPU module under `gpu-dev`. After a Rust edit, it builds only the affected GPU module,
then runs wasm-bindgen and swaps it into the page without `wasm-opt`. The first
such edit compiles that profile's dependencies; later edits reuse them.
To explore the existing sample instead, run `bun game/dev.mjs beacons`.
`game/dev.mjs` passes the dev server's flags through (`--port`, `--lan`, and
`--allow-host <name>`, repeatable, to serve the game through a tunnel such as
`tuft host`; without it a request under any name but the printed ones gets 421).

Once the game plays and its `proof.mjs` describes it, establish its proof baseline:

```sh
bun game/prove.mjs ./my-game
```

`prove` takes the first development baseline while you build your game;
`prove --release` checks the shipping matrix before you ship. A first baseline and
`--repin` run Linux and web under `gpu-dev` in continuous, Save and FreshGame modes.
`--release` adds native release and shipping-profile web runs in continuous mode.
With existing pins, ordinary `prove` runs `test-rust` first, then one native
`gpu-dev` continuous leg. It fails naming the tick if the simulation changed and
never writes `pins.json`. The first baseline publishes; subsequent publication,
including release coverage, requires `--repin` (`prove --repin --release` to ship).
Tests are excluded from pin inputs, so editing a test does not invalidate a pin.
Ordinary web builds and deploys keep the shipping profile.
Use `bun my-game/proof.mjs web --screenshot-only` for pixels only; this reports
`UNVERIFIED` because it skips gameplay checks. Its screenshot is
`my-game/artifacts/web/game.png`. The starter includes movement, jumping, two
beacons, pause/restart, touch controls, a ground grid, pads, fog, sun shadows and saved glow,
and the [gameplay kit](#what-every-game-needs) in its tick: a critter that follows the
player and keeps away from lit beacons, the coins it drops, and a two-item shop.

A bare name (`bun game/new.mjs my-game`) creates `game/games/my-game`; a path chooses
the directory. From an empty directory, `bun /path/to/exact2/game/new.mjs .` works too.
A game outside this checkout — `bun scripts/exact.mjs new ../my-game --game` makes
one, as `exact new` makes an app — also gets an app's `exact.mjs` and `AGENTS.md`:
`bun exact.mjs test-rust` (the hostless tests), `web`, `web-build`, `test web`
(each `*.test.contract`; also `macos`, `ios`), `agent`, `mac`, `ios`, `windows`, `contract`,
`prove` and `feedback`, with the exact2 checkout named once (LLP 1086).
Generation resolves the generated workspace offline against the current SDK lock, so
the first locked test and proof need no lock refresh. Generation writes only inside that game. A game is the files its author writes; bakes
generate its hosts, Cargo workspace and lock, ignored, under `.shells/`.

| File | What you own |
|---|---|
| `logic/src/lib.rs` | Scene setup, typed arguments and the tick function |
| `app.contract` | Menus, HUD, layout, accessibility and app actions |
| `proof.mjs`, `logic/tests/*.rs` | Real-host assertions and hostless simulation tests |
| `app.test.contract` | Menus and HUD driven on a host (`bun scripts/agent.mjs <host> --test`) |
| `pins.json` | Verified tick/save baselines, written by `prove.mjs` |
| `app.json` (optional) | Authored keys only: a title, bundle id, `game.audio`, `game.assets`, `game.look`, data and render crates |
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
when Cargo starts elsewhere. That config also names an absolute `build-dir` (the
game's `target/`), where Cargo keeps its intermediate artifacts: a build kept apart
from it sets `CARGO_BUILD_BUILD_DIR` as well as `CARGO_TARGET_DIR`. Regenerate the shells after updating the SDK.

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
crate, or `bun exact.mjs lock` when adding dependencies.
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
        w.prop("player", Mesh::cube(1.0)).at(0., 0., 0.).color("#fff");
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
- Spawn literals with `w.prop(name, mesh).at(x, y, z).color("#4d6669")` or
  `w.sprite(name, texture, [width, height]).at(x, y)`; `.entity()` returns the
  entity for adding components. Tuples remain available. [Literal builders](recipes/literal-builders.md) shows both.
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
[Game recipes](recipes/README.md) answer one task each with a runnable `Sim` fragment.
`bun exact.mjs game vocab <Component|verb>` in a generated game shows rustdoc,
fields and evaluated defaults. [Familiar names](names.md) maps three.js, Unity and Godot.

### What every game needs

Steering, pickups, spawners and a shop are the engine's, as pure functions and
values the tick calls; nothing runs that the tick does not call, and nothing is
saved that the game does not save. The [starter](new/logic/src/lib.rs) uses each,
and its `logic/tests/sim.rs` has a bot that plays five minutes to check the prices
against the income.

| Need | Engine | Recipe |
|---|---|---|
| Seek, arrive, flee, wander | `steer::seek(&pose, target, max_speed, turn_rate, dt)` and its siblings return this tick's `Steer { velocity, facing }` | [Steering](recipes/steering.md) |
| A school | `steer::school(&pose, w.near_xz::<Fish>(e, r).limit(7), separation, alignment, cohesion, …)` | [Schooling](recipes/schooling.md) |
| Pickups that come to the player | `collect::drift(item, player, radius, speed, dt)`, `collect::Collectable` | [Pickup](recipes/pickup.md) |
| Spawning on world time | a `spawn::Spawner` per kind in a resource; `spawner.due(w.tick())` | [Spawner](recipes/spawner.md) |
| A shop | `Actions` on `Digit1`…, rows with prices and lock reasons published to the HUD | [Shop](recipes/shop.md) |

A steer is a velocity and a facing, not a component: the game writes it into the
motion the entity already has. A creature with no physics moves its `Transform`
(`pose.position += s.velocity * dt; pose.rotation = s.facing`). A
[`Character`](engine/src/character.rs) is the mover when the creature should
accelerate, brake, jump and keep to bounds like the player: step it with
`s.velocity / character.speed` as the wish. A physics body applies the change of
velocity as its own force.

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

`Game::render_scale(args)` sets the world's presented raster relative to canvas
device pixels: return a finite value in `(0, 1]`, which stands on every rung of the
quality ladder below. The default, `AUTO_RENDER_SCALE`, leaves the scale to the
rung (full size on `High`). The compositor stretches that raster to the same canvas
box. Camera layout and input coordinates stay in CSS pixels, and ordinary Contract
HUD children keep native resolution. Bind a `#[live]` argument through this method
for an app setting. This is the presented raster's size, and a look frame's; a
fixture's or a readback's size is the one its caller asks for.

`Game::quality(args)` picks the renderer's rung of the quality ladder, read once,
when the renderer is made, and never saved: the world hash is the same on every
rung. The default `Quality::Auto` follows the device's GPU tier
(`exactViewport().gpuTier`, `high` under the agent until `prefer gpu-tier
low|mid|high`). The web chooses it before the GPU device from `GPUAdapterInfo`,
else the screen's class; a native module chooses it from the adapter it requests,
before it makes the device, and its host reports that as the page fact once the
module loads (`high` until then). A rung clamps only what the game left unsaid:

| rung | scene samples | sun shadows | raster scale | water's caustics and shafts |
|---|---|---|---|---|
| `Low` (a phone GPU of the Adreno 600 class) | 1 | none | 0.5 | not drawn |
| `Mid` (a recent phone or tablet) | `Game::SAMPLES` | 1 cascade | 0.75 | as `Water` says |
| `High` (a computer; the agent's) | `Game::SAMPLES` | as `Shadows` says | 1 | as `Water` says |

What the game says stands on every rung: an inserted `Shadows` resource, an
inserted `AmbientOcclusion`, and any `render_scale` the game returns (the default,
`AUTO_RENDER_SCALE`, leaves the scale to the rung). `Low` leaves out a `Water`'s
caustics and shafts at present time (`Renderer::set_water_effects`): the saved
resource keeps them. The rung is read once, from the
canvas's first arguments, when its renderer is made: a player's setting read in
`Game::quality` takes effect when the canvas is made again. A look frame is the
rung's picture too (its samples, clamps and scale).
On a Pixel 5 the starter draws its world at 16 fps under `High` and at the 90 Hz
display's 90 under `Low`; the world's `state` reports its row as `quality`
(`asked`, `tier`, `rung`, `samples`, `scale`). Under the agent `prefer gpu-tier`
remakes each world's renderer at the new rung, on the same device and under the
same world, on every host.

On a browser without WebGPU (or with no adapter) a game's web page shows, in the
box of each world surface (its build names them on the page's root,
`data-exact-worlds`, from the game's GPU declaration), a `role=status` page that names WebGPU and the browsers that have
it, and the game's iOS and macOS installs when `app.json`'s `install` lists any;
the canvas's own HUD is inert under it, the rest of the app keeps working, and a
world request answers `host web refused (gpu): no adapter: …`, which the proof's
web leg reports as its result. The page waits at most a second for the install
data, and goes, with the HUD's inertness, when its surface does. Any other GPU
surface, a game's or an app's (a decorative backdrop, such as Caltrain's sky), shows
no page: its content stays live over the empty canvas. No WebGL renderer
exists (LLP 1128 I6).

After the first GPU build, the compiler and dev loop check surface names and
arguments against the emitted `.shells/surfaces.json`, including hidden branches
and imports. Rust argument edits reach that declaration through the next GPU build,
so while the game's Rust is newer than it, `contract build` reports its findings as
warnings beside every other diagnostic, and `contract types` and `contract rust`
never stop on them; a bake checks them against the declaration it has just written.
The dev compiler retains its last good plan on an error.

### Controls

`Game::actions` declares the controls once, by name; the tick reads them with
`input.held(name)`, `input.pressed(name)`, `input.released(name)` and
`input.stick(name)`. `pressed` is a rising edge since the last tick, even when the
release lands in the same tick; `held` is the state at the tick's end, so a quick
tap is `pressed` and never `held`.

A key is its `KeyboardEvent.code` (`KeyW`, `Space`); a mouse button is `MouseLeft`,
`MouseRight` or `MouseMiddle`; a pad button is its position in the W3C Gamepad API's
standard mapping, so an Xbox pad's A and a Nintendo pad's B are both `GamepadSouth`:
`GamepadSouth`, `GamepadEast`, `GamepadWest`, `GamepadNorth`, `GamepadL1`, `GamepadR1`,
`GamepadL2`, `GamepadR2`, `GamepadSelect`, `GamepadStart`, `GamepadL3`, `GamepadR3`,
`GamepadUp`, `GamepadDown`, `GamepadLeft`, `GamepadRight`, `GamepadHome` (`PAD_BUTTONS`;
another `Gamepad…` name panics at declaration). One action takes keys and pad buttons
together: it is held while any is down and released when the last lifts, so switching
device mid-hold neither presses again nor drops.

| Task | Declare and read |
|---|---|
| A fire button, one shot per press | `.button("fire", &["KeyF", "MouseLeft", "GamepadR2"])` read with `input.pressed("fire")` |
| Fire while held, and on a quick tap | `input.held("fire") \|\| input.pressed("fire")` (Rivals): `held` alone misses a tap whose press and release share a tick |
| Move with keys, a stick or touch | `Stick::wasd().or_arrows().or_pad(Pad::Left).or_touch(Region::Left)`; `.dead_zone(0.15)` (radial, the default) and `.curve(Curve::Power(1.5))` shape the pad's reading (Beacons) |
| Throttle and brake | `Stick::pad(Pad::Triggers)`: forward is `GamepadR2`'s travel less `GamepadL2`'s |
| Move by tilting the device | `Stick::wasd().or_tilt(Tilt { max_degrees: 20.0, dead_zone: 2.0 })`, fed by the app's `startSensorInput(surface="world", sensor="gravity")` (LLP 1133.013 D6): full deflection at `max_degrees` from level, nothing within `dead_zone` degrees, nothing before the first sample or after the input ends (`exact_game::tilt_stick` is the map). Level is the first sample, or what the tick sets with `w.set_tilt_neutral("move", gravity)` from a posted reading, from the next tick (Beacons' `tilt x y z`) |
| Read a sensor every frame | `input.sensor(SensorKind::Gyroscope)`, the latest sample, which holds through ticks that receive none and is gone after the input ends; `input.sensor_samples(kind)`, this tick's own samples, every one the host delivered |
| Sprint after holding a key | `.hold("sprint", &["ShiftLeft", "GamepadL1"], 0.2)`: `pressed` when held 200 ms, `held` until release; a shorter press fires nothing (Beacons) |
| A double tap | `.double("roll", &["Space"], 0.25)`: `pressed` on a second press within 250 ms |
| A short press distinct from a hold | `.tap("dash", &["ShiftLeft"])`: `pressed` and `released` together when a press of at most 200 ms (`Actions::TAP`; `tap_within` for another) lifts, so it fires late by design |
| A chord | `.chord("special", &["ShiftLeft", "KeyE"])`: `pressed` when the last key goes down while the others are held |
| Controls for one mode | `.context("driving", \|a\| a.button("brake", &["KeyB"]))` and `w.input_context("driving", true)`: its actions read only while it is active (Beacons' `light`, within reach of a beacon) |
| Let the player rebind | `w.rebind("jump", &["GamepadNorth"])`, or `w.rebind_text("jump GamepadNorth")` from a settings screen's `postMessage` |
| Keep the player's bindings for every game | `fn bindings(args: &Args) -> String` returning a `#[live]` argument the app binds from its storage: a new game copies it into `Bindings` |

Every window is world time: a trigger fires on the tick that contains its instant,
from the stamps of the events that moved its keys, so `clock +N` and a 60 Hz panel
fire the same ticks. A pending hold, tap or double window is work: `clock settle`
runs to its deadline and `state`'s `changing` names it (`input trigger sprint`). A context or a binding the tick changes applies from the next
tick: leaving the context, or rebinding a held action, releases it then (a journal line
names each), and a key already down as an action enters a context or gets it reads up
until it lifts and goes down again.

A host reports a pad's whole state when a button or an axis changed (in 1/256 steps),
stamped like a key; a press keeps its own stamp, and only a sticks-only change may fold
into a later one in the same tick. Pads go to the canvas that has the focus (on the Mac,
in the key window; on Linux, Windows and Android, with nothing focused, to the first
canvas that takes input; an editor's focus takes them from every canvas). When the focus
or the key window moves them, the canvas they leave hears each pad go, whether the focus
left the canvas or a control in it, and the canvas that takes them hears each connected
pad as the host last read it (the table says when that is). A blur releases everything
the world held, and a disconnect what that pad held; a pad that connects again at that
index is a fresh device. Of several pads, a stick reads the one pushed furthest (of two
pushed as far, the lower index); a button is down while any pad holds it. Per host:

| Host | Reads | Limits |
|---|---|---|
| Web | the Gamepad API, polled each frame while a pad is connected; a pad without the standard mapping is reported (`pad 1 connected unmapped` in the journal) and delivers nothing | as the browser exposes pads |
| macOS, iOS, tvOS | GameController's extended gamepad, on each change, and every pad as it is now when a canvas or a control in it takes or gives up the focus, when the app becomes active, and on the Mac when a window becomes or stops being key | a controller without the extended profile (the Siri Remote) is unmapped; on iPadOS with several windows the pads go to the earliest-made canvas holding the focus in any of them, not to the key window's |
| Android | gamepad keys and joystick axes; each trigger with an axis (LTRIGGER or BRAKE for the left, RTRIGGER or GAS for the right) reports its travel, one without its key | every pad is reported standard; the host reads no pad's state on its return from the background, so a pad reports itself with its next event, and a button held through the background reads up until pressed again |
| Linux | evdev, at each report, stamped with the report's time moved into the host's clock by one origin per read and kept when that report moves the pads to a canvas, so readings buffered in one read keep their spacing and several pads' arrive in the order they happened (ties by index, then as read); in the kernel's gamepad layout (Xbox drivers' letters read as their positions); a pad's state is read when it opens and delivered ahead of the reports buffered since | pads present at launch only; every pad is reported standard |
| Windows | XInput's four slots, every 8 ms while one is connected and every 250 ms otherwise, while the window has focus, and all four at once when it regains focus, so a pad unplugged meanwhile reads gone | `GamepadHome` is never down (XInput does not report the guide button); a reading is stamped at its poll |

What is saved and what is hashed differ. A save holds the input queue and the keys,
pads and contacts held, which the world hash leaves out. The world hash covers the world
the input changed, plus resources that exist only once a game uses them:
`InputContexts`, `Bindings`, and `InputState`: the triggers' gestures in progress, the
latched actions, and the contexts and bindings as last applied. So two worlds whose holds
started at different times differ, as do two whose context or rebind has yet to apply,
and a restore mid-hold continues the hold. A sensor input's samples wait in the queue a save holds;
the latest sample of each sensor and the tilt neutrals are a fourth resource, `SensorState`, which
exists only once a sample arrives or a neutral is set. `InputEvent`'s `player` is 0 from every host and means
nothing yet.

### Movement and appearance

| Task | Existing engine operation |
|---|---|
| Move and jump without physics | `w.character("player").step(input.stick_xz("move"), input.pressed("jump"))` |
| Change speed while playing | `w.require_mut::<character::Character>("player").speed = 8.0` preserves velocity and the rest of the character. |
| Move against colliders | [Physics controller example](physics/tests/living.rs), with explicit `Move`, `Jump` and `Gravity` |
| Follow the player | Attach `Follow`; the scene steps it after the tick. Call `scene::follow(w)` explicitly to choose an earlier order. |
| Find a nearby unlit beacon | `w.nearest_xz_mut::<Beacon>("player", 1.5, \|b\| !b.lit)` returns its entity and mutable row handle together; `beacon.name()` reads its name. |
| Count for the HUD | `w.count::<Beacon>(\|b\| b.lit)` |
| Colour a surface | `Material::hex("#527b65")` (any CSS colour Contract takes: hex, `forestgreen`, `rgb(…)`); `Material::srgb(r, g, b)` for a picker's 0–1 floats. `Material::rgb` takes linear light, the renderer's own space, where 0.5 is not mid-grey; reach for it only for computed light |
| Sky, fog, tint and sprite colours | `Environment.background`, `Fog.color`, `Tint.color` and `Sprite.color` are a `Color`: `Some(Color::hex("#bad6d1"))`; `[r, g, b].into()` keeps linear values |
| Ring, torus, cone and halo | `Mesh::ring(inner, outer)`, `Mesh::torus(radius, tube)`, `Mesh::cone(radius, height)`, `Material::alpha(a)`; [halo recipe](recipes/halo.md). |
| Grid and fog | `Material::grid(Color::hex("#6f8486"), spacing)` and the saved `Environment`/`Fog` resource |
| An underwater scene | `w.insert_resource(Environment::underwater())`, a sun from above and no opaque surface over the floor; [underwater recipe](recipes/underwater.md) |
| A water surface, caustics and light shafts | `w.insert_resource(Water::at(8.))`: the surface at that height, seen from below through Snell's window, caustics on lit surfaces under it and shafts in the water (`caustics: None`, `shafts: None` to leave either out); [underwater recipe](recipes/underwater.md) |
| Fade a glowing mesh | Attach `Glow` to `Material::glow`; call `beacon.glow_to(1.0, 0.5)` once. The renderer samples at frame time. |
| Fade a point light | Attach `Lit(Spring)` and call `lit.to(w.tick_end(), intensity)`. |
| A flashlight | `SpotLight { inner, outer, range, intensity, .. }` along the entity's −Z; intensity in candela, as `PointLight`'s |
| Shadows from a lamp | Add `LightShadows` to a `SpotLight` or `PointLight`; the renderer shadows the nearest few |
| A moon | A second `DirectionalLight` (in entity order) is an unshadowed fill |
| Team colours, mutation looks | A `Material` on a model entity tints every node and adds emission; `NodeMaterials` per named node or `MaterialOverrides` per material (one model in every team's colours), written from `Game::present` ([team colour from a saved `Team`](render/tests/instances.rs): `p.insert(e, MaterialOverrides(vec![MaterialOverride { material: 0, color: Some(team_rgba), ..Default::default() }]))`) |
| Cheaper far trees and crowds | `ModelLod { levels: vec![LodLevel { distance: 30., model: "tree_low.model".into() }], hide: Some(120.) }` on the entity |
| Fade a tree between camera and player | `Opacity(0.3)` on the entity from `Game::present`: a dithered fade, no sorting, never in a save or pin |
| A first-person weapon | Add `ViewModel` to each part; it draws in front of the world and casts no shadow |
| Contact shadows in creases | `w.insert_resource(AmbientOcclusion::default())` turns on SSAO (off by default) |
| Lighting from a photographed sky | `w.insert_resource(EnvironmentMap::new("sky.tex"))` with the equirect in `Game::ASSETS` |
| Texture generated terrain | UVs in the `MeshData`, a material sampling a `.tex` from `art/textures/`, through `w.generated_model` |
| Fire, smoke and sparks | `ParticleLook { texture, atlas, fps, stretch, soft }` beside an `Emitter`; `soft: 0.5` fades smoke into the ground |
| A painted, visible sky | The same `EnvironmentMap` with `visible: true` (and `rotation` to turn it) |
| Mouse look | `input.pointer()`'s `delta` is the device's motion this tick, not a difference of positions. Mark the canvas `data-pointer-lock="true"` (declared in `app.json`'s `data`) and a mouse press captures the mouse on the web, macOS and iPadOS (`GCMouse`) until Escape or blur, so the delta never stops at an edge; the Linux host always sends evdev's relative motion. |
| Turn the drawn camera between ticks | Put `MouseLook { yaw_per_point, pitch_per_point, pitch_limit }` on the camera at the rates the tick turns it by: the drawn camera and its children turn by `Sim::unshown_motion()`, so a turn shows at the next frame whatever the tick and display rates (`engine/tests/look.rs`). Presentation only; insert it in `setup` or register it. |
| Right or middle mouse button | Bind it as a key: `.button("aim", &["MouseRight"])`; `MouseLeft` and `MouseMiddle` too (`MOUSE_BUTTONS`). Touch contacts press none. |
| Start a selection rectangle | `input.pointer().and_then(\|p\| p.press_origin)` is the latest MouseLeft/touch Down point, even if movement and Up reach the same tick. Gate commands on your action's pressed/released edges. The origin survives normal Up and saves; Cancel, Blur or replacement clears it. |

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

`WorldSurface` also publishes the reserved boolean `frame_ready`. It starts false
and becomes true after submission of a complete frame for the current world
generation. Restore and device loss make it false again. A same-world resource
replacement that still draws valid retained imagery remains available. The field
is host state; it does not enter simulation publications, saves, hashes or journal. The
host overwrites a game publication named `frame_ready` in the outgoing record.

An app can add `frame_ready: bool` to its `exactSurface` shape and keep a loading
cover over the mounted canvas while it is false. The app owns its pause policy.
If simulation ticks build terrain or accept bootstrap data, finish those steps
before arming the pause. `PENDING` still permits ordinary simulation ticks.

`w.emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events save in order but stay outside the simulation hash.

Commands go the other way as messages: an action calls
`postMessage("buy carrot", "world")` and the next tick reads every message posted
since, in order, from `input.messages()`. Nothing is coalesced, nothing needs an
acknowledgement, and a delivered message is not part of a later save. Use live
arguments for settings, messages for things that happen once. A paused world holds
a message for its first tick after the pause.

A message carries who posted it (LLP 1133.022 D6). One from a tool's commit, or
a wake's, arrives marked with that origin; the world keeps the least privileged
origin it has heard until a fresh start (and saves it), and marks each string it
`emit`s with it, so the `message=` handler runs as that origin and a tool cannot
start a device through the world. A person's or the app's own post leaves no mark;
`state` shows `origin` only for a marked world. The runner and the engine carry the
mark; a game never sees it.

The driver posts the same message with `post [<surface>] <text>`, short for
`type <surface> message <text>`: `post spawn-gulper` to the page's one world,
`post world "spawn gulper"` to a named surface (a canvas's testId works too). It is
the page's `postMessage` in every respect: saved until a tick reads it, held while
no canvas of the surface is live (64 at most, the 65th refused by name). Past the
world's 1,024 queued events the world drops it, logs that once and counts it in
`state`'s `input.refusedPosts`; the op has already succeeded. Use it to stage or script a scene the game
already listens for, as `Sim::post` does in a hostless test. As from the page, a
`set entity Component.field value` message is a
[development edit](engine/README.md#development-edits), which a pin refuses.

Typed simulation events use `#[derive(Default, Event)]`, `register_event`,
`emit_event` and next-tick `input.events::<T>()`. Their bounded queues are resources
inside saves and hashes. Read delivered records with `state world events [Type]`.
String `emit` is the presentation path to Contract `message=`, outside the hash;
typed events do not deliver a second Contract message. Same-tick effects are function calls.
See the [tick admission contract](engine/README.md#tick-admission).

## Proof pins

The async lane runs `bun game/app/shells.mjs --test`. It gates each game's
hostless current-tick assertions: `Sim::assert_pin` compares only the tick the
simulation has reached, not every checkpoint or any continuation-save digest.
Later checkpoints not asserted by those tests and continuation saves remain the
real-host proof's responsibility. Advancing a hostless test to another pinned tick
adds coverage only when its options and inputs reproduce that proof's state.

`pins.json` owns the game's expected tick hashes and continuation-save digests.
`hosts`, `modes` and `profiles` record their coverage: modes `0`, `1` and
`fresh-game` run under `gpu-dev`; the `release` profile covers continuous mode
on the native host and web when requested. Missing `modes` or `profiles` means
the full matrix that older files used; `hosts` still bounds their host coverage.
A development baseline has its own publication verdict and reports `release not
pinned`. `proof.mjs` names missing host, mode and release coverage separately from
the simulation checks it ran. Deploy and push require a native host, web, all three
development modes and release. Native-only repins remain available but cannot ship.
An absent `profiles` field counts as release-covered under the legacy rule;
missing hosts never gain coverage.
The generated proof uses `pin(tick, snapshot)` and `pinSave(name, path)`;
Rust tests use `sim.assert_pin(include_str!("../../pins.json"))`.

A run containing a development `set` edit refuses both pin matching and repinning,
including after save/restore or journal rollover. `pinSave` inspects the saved bytes
and refuses admitted edits and complete edit commands retained in the input queue,
even after the live queue is cleared. Pending observations are tracked per session.
Production bakes refuse the command.

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

Repinning runs Off, Save and FreshGame and requires matching execution inputs and
host observations. With `--release` it also compares the fast profile with native
release and, when requested, shipping web. A profile disagreement, failed proof or missing requested browser
leaves the pins unchanged. Existing pins may be updated with
`--hosts linux` explicitly; the first baseline always requires Linux and web.
Summaries and every pin update record two digests: `inputs` identifies the exact
execution-input tree, including runtime files under `assets/` and `.baked-assets.json`,
even when Git ignores those generated products;
`sourceInputs` identifies portable authored sources. The latter follows authored-input tracking and omits the bake's
manifest-owned outputs and the manifest itself, and names app and SDK files
relative to their own roots. Rust crate `tests/` and `examples/` directories are
excluded only when their parent contains `Cargo.toml`, inside or outside the SDK.
Art and asset directories with those names remain inputs. Unowned assets remain authored,
whatever their names or extensions. Machine-specific texture encodings and sound PCM can therefore
share source provenance while retaining distinct execution identities. Repinning
still refuses mixed execution trees within one transaction, even when their
`sourceInputs` agree. A Git commit is recorded in `at` when one exists; a game
without commits records `at: inputs:<sourceInputs>` under the same agreement checks.
`CHROME` selects the headless browser.

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
screenshot alone is not a verified simulation. Proof exits are **0 PASS**, **1 FAIL**
and **3 UNVERIFIED**, including build-only, pixel-only and baseline-collection legs.
Each verdict names its reason. `prove` accepts exit 3 from a collecting leg; it writes
pins only after the complete matrix agrees. Callers that treated UNVERIFIED as exit 1,
or build/collection runs as exit 0, must handle 3 explicitly. `diff` retains exit 2
for a refused capture. `prove.mjs` prints its unique
`ARTIFACTS` directory. Its `src/` copy supplies the game inputs and proof program;
edits to the original game do not change a running proof. `legs.json` records each
host/mode/profile result. Resume with `bun game/prove.mjs <game> --continue <run-directory>`;
completed legs are reused and failed or interrupted legs rerun. Continuation refuses
changed game/SDK inputs, changed proof helpers, a changed matrix (including
`--compare-saves`) or damaged receipts. Keep the original comparison flags when resuming.
The SDK stays in its checkout and must remain unchanged throughout the run.
Proof imports and declared `export const proofData = ["relative/file.json"];` data
are checked separately from bake inputs; game-local dependencies are copied into
`src/`. SDK helpers remain live and are checked before and after every leg.
Snapshots share the original game’s `target/` for compilation and validated receipts
under `artifacts/build/`. A target lock covers proof build validation and copying;
each Linux proof then executes private products under its source copy's
`artifacts/build/`, checked against their input and product digests by the driver.
Linux GPU receipt hits restore the surface declaration and generated runtime assets.
The web leg can copy a completed resident producer build from the game's target
cache when its portable execution-input identity, mode and build configuration
match. The cache is populated from the invocation's private stage before publishing
the shared output. Reuse revalidates recorded compiler and build-script environment
dependencies, compiler identity and copied product digests before driving the snapshot. A changed
source, damaged product or different profile is a cache miss and builds normally;
the proof never reads the live world's state. Release builds do not use this cache.
Web GPU modules compile their paranoid mode into `Sim`; a continuous-mode
module cannot serve a Save or FreshGame proof. Those modes build separately
unless a completed receipt for that same mode matches. The seven-leg development
matrix reuses a warm continuous web leg, but that hit does not warm the Save or
FreshGame compilation. GPU-only hot-swap wasm is not reused; it needs a completed
page bake. The cache keeps six keys, evicting by receipt publication mtime.
Every proof leg checks each observed simulation's reported mode; Save and FreshGame
require completed reconstruction samples, including for each advanced world.
Game-local `.ts` proof helpers are relocated like `.js` and `.mjs`; an unsupported
`proofData` declaration is refused by name.
Direct `proof.mjs` runs use `artifacts/<host>/`.

Use `--report` on `prove.mjs` to explain recorded stalls and refusals.
`ios` selects the simulator; `--device` selects a paired phone and requires an
accessible development signing key. Direct proofs accept `ios --device --phone
<device-id>` too. Phone artifacts and build receipts use `ios-device`; the proof's
1280 × 720 logical viewport fits into the phone window for equal input dimensions.

**The look result.** Beside the simulation result a proof reports a look result, on its own line:
`simulation: PASS · look: PASS, 0 tells`, or `look: FAIL, near-black`. At each
session's close the runner draws every world offscreen from its own camera at the
tick the clock holds (a look frame, [render README](render/README.md#look-frames)),
the page as composited where the canvas draws, and a contact sheet over 600 ms at
half resolution, and prints each as a `LOOK` path. A tell fails the look: a
near-black or near-white frame, more than 5% of pixels clipped, a particle draw that
covered nothing (no fragment of it contributed in front of the scene), a shadowed
sun whose maps hold no texel nearer than their clear depth. Tells are judged
on whole frames from the game's camera: the close step's, the one a drawing host
takes at every `clock settle` and pixel `screenshot` (their replies carry its
`look` lines, `lookGaps`, `lookFailed` and the canvases looked at, `lookSeen`), and
each frame of a full-resolution `from camera` film. The contact sheet and the orbit
and plan views are pictures. A tell is counted once a frame that told it, and fails
the look alone: the simulation result and its failures do not change, and a repin
records its pins. A look the game intends is silenced by name in `app.json`,
`"game": {"look": {"silence": ["near-black"]}}`, and still printed and recorded.
The look passes only when every world any reply showed (a settle's or a state's
world list, a tree's canvases) was looked at, at its close, in a judged frame or by
a `look(session)` from the game's camera, and every measurement a tell reads was
taken. A world with no picture (the GPU-less host
without a GPU, or a model-capable module there: `look: no picture on this host`), a
drawing canvas with no picture, a look frame that failed, a world shown and never
looked at, or a gap (`particles: not measured`) leaves it `UNVERIFIED`, named on a
`look: unverified` line. `prove.mjs` passes the look only when every requested
host's every run did; a run with no summary is a gap. The tells see brightness and
coverage, not colour, facing or layout: open a `LOOK` picture once before reporting.
A proof script takes its own with `look(session, view, label)`, `view` being `{}`
(the game's camera, judged), `{top: 30}` or `{yaw: 35, pitch: -20, distance: 40}`;
the look step runs in mode 0 only, beside the simulation the paranoid modes check,
whose journal comparison leaves the `look:` lines out.

The [shared proof runner](proof.mjs) handles builds, sessions, transcripts and
cleanup. Each game's proof owns its assertions. `check` records failures while
independent assertions continue. The terminal shows checks and failure details;
`replies.json` retains complete operation replies. Browser runs provide canvas pixels; the GPU-less
Linux host exercises simulation, input, Contract UI, CPU picks and saves.

Proof receipts bind the completed build's inputs. Editing authored inputs during
a build rejects the proof and removes its receipt. Directory tracking includes
the app root, shared sources and every admitted source subtree, including empty
directories, so creating and removing a source
during compilation also rejects the proof. Changes to entries in a tracked
directory require a rerun, even when the changed entry is excluded from inputs.
Asset directory entry changes also require a rerun, including generated file
creation, pruning and atomic replacement. The baker uses atomic replacement for
changed assets. Rerun after the tree settles; the proof rebuilds before caching
the result. In-place rewrites of manifest-owned generated files can complete in
the same invocation. The app root is tracked whether or not `assets/` exists,
so temporary files and source trees there cannot go unnoticed.

Direct web proofs build into `artifacts/web-dist/`. The web builder replaces
that directory on every build, so an `EXACT_WEB_DIST` override must also live
inside an excluded output tree, such as `artifacts/` or `target/`, or outside
the source trees. Otherwise each build changes a monitored parent directory
and the proof cannot accept its receipt.

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

Use Exact2's same ten operations: `tree`, `screenshot`, `tap`, `type`, `state`,
`layout`, `logs`, `clock`, `prefer` and `perf`. `trace <file>` reads an exported
trace separately from these session operations. The CLI also accepts `check`,
`sample`, `resize`, `close`, `back`, `pick`, `reload`, `fail fetch` and `pass fetch`,
and `post`, a form of `type` (above). An entity is addressed as `world:player`.

The agent drives a pad as it drives keys: `type world key GamepadSouth [for <ms>]`
presses a pad button and `tap world stick left 0.7 -0.2 [for <ms>]` moves a stick, in
the Gamepad API's axes (positive y is down). Each is the agent's pad 0, held across
requests and queued as a host queues a real pad (`delivery: synthesized`); without
`for`, `tap world stick left 0 0` lets the stick go. `state world input` lists the
declared actions and their keys, the keys and pads held, contexts, bindings, the
actions held, and the actions pressed and released at the last tick.

```sh
EXACT_APP_DIR=./my-game EXACT_WEB_DIST=./my-game/artifacts/web-dist bun scripts/agent.mjs web \
  "tap play" "type world key KeyW for 1500" "layout world:player" \
  "state world:* busy" logs "screenshot run.world world save"
```

For a `prove.mjs` web build, set `EXACT_WEB_DIST` to the printed artifact directory's
`dist/`. `state world:*` reads entities a page at a time (512 by default):
`state world:* from 512 limit 2000` reads the next ones, and the reply carries
`total` and `next`; `under world:player` narrows it. `state world:* resources` adds
every resource's value. `logs` reads only new journal lines; `logs --all` reads
all retained lines (the journal holds 4,096), reporting any dropped prefix.
`perf world:world` reads exclusive tick spans with source lines and p50/p95
milliseconds; `perf world:world during "clock +1000"` measures that drive.
In a proof, `session.world('world').snapshot({all:true})`
reads every page at one tick and `resources()` the resources.
`clock settle` advances the owned clock and explains remaining work instead of sleeping.

A world answers a few questions about its picture from a look frame, the tick the
clock has reached drawn offscreen (presentation only; no tick runs and the hash does
not move):

```sh
bun scripts/agent.mjs web "tap play" "clock settle" \
  "screenshot side.png world from 35 -20 40" "screenshot plan.png world from top 60" \
  "state world draws" "state world:beacon-1 visible" "state world look"
```

`screenshot <png> <canvas> from <yaw> <pitch> <distance>` sees the scene from a
camera that many degrees round and down from the centre of its drawn entities, that
many metres off; `from top <height>` is an orthographic plan that many metres tall;
`from camera` is the game's own camera; `over <ms> every <ms>` films any of them.
`state world draws` lists each draw in pass order with its batch, instances submitted
and kept after culling, whether it covered a sample and what it wrote.
`state world:<entity> visible` answers one word and a reason: `drawn`, `frustum`,
`occluded`, `hidden`, `hidden-ancestor <name>`, `opacity 0`, `asset-pending <model>`,
`lod-hidden <d> m`, `outside-cascade <n>`, `custom-material`, `not-drawn` or
`no-particles`, read from what the renderer did (a sprite's texture residency and
its quad's extent about its anchor included). `state world look` gives the frame's
luminance, clipped fraction, tells and gaps; after `clock settle` and every pixel
`screenshot` a canvas that draws adds them to the world's `logs` (`look: …` lines,
never its journal, which a save carries) and to the reply. The GPU-less Linux host
draws look frames on this machine's GPU where it has one. A production build
refuses all of these.

Linux GPU loading and surface/asset refusals, and web GPU/surface refusals, carry
`{refused:{host,subsystem,message}}`. The driver prints the host and subsystem first;
`EXACT_AGENT_DEBUG=1` or `--json` includes its stack. Browser console diagnostics
remain available through `logs --all` after a wasm trap, including the panic hook's
message. This does not make a trapped world usable again; reload it before driving.

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

Clippy holds every game's logic library to rules 1–3 with the SDK's
[`app/determinism/clippy.toml`](app/determinism/clippy.toml): std float transcendentals
(`sin` … `powi`, `hypot`, `mul_add`), `HashMap`, `HashSet`, `RandomState`, `Instant`,
`SystemTime`, `std::thread::spawn` and `rand`'s ambient generators are refused, each
naming its replacement. `sqrt` is correctly rounded and allowed; glam is built with
`libm`, so `Vec3::length`, `Quat::slerp` and the other glam methods are portable. The
lints run in `shells.mjs --test`, before first pins and `--repin`, and in every
production-profile bake (web, Apple, release Linux, deploy); the dev loop also checks the logic library before each Rust rebuild. An `#[allow(clippy::disallowed_types)]` on an item is a
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
accessibility and controls; the world owns the canvas contents. A HUD button with
`action="jump"` holds that `Actions` action while pressed; its `aria-keyshortcuts`
takes its chord (the key with exactly its modifiers) before the focused world hears
it and presses the action once per keydown, never on its repeats, moving no focus,
on every host (the rule `host/linux/src/presenter/shortcuts.rs` states). The bake warns
when `Actions` bind the same key. Show touch controls under
`exactViewport().pointer == "coarse"` ([recipe](recipes/touch-controls.md)). Hosts consume
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
`exact-game.workspace = true`; a game with `art/` also lists `exact-game-bake.workspace =
true` under `[dev-dependencies]` for `sim_with_art!`) and captures its own `Cargo.lock` with `bun exact.mjs
lock` (from exact2, `bun game/app/shells.mjs ./my-game --lock`): it fetches what the
graph needs, resolves with the SDK lock's versions first and prints what changed
(`--update-lock` resolves offline from the cache alone); commit and review that lock.
When exact2 moves under a captured lock, a build refuses it by name ("exact2 moved since
this game's Cargo.lock was captured (added: …; updated: …)") and `lock` refreshes it;
`bun exact.mjs update` says so too, and generating `.shells/` alone (`bun exact.mjs shells`) warns. SDK
crates alone need no lock of the game's own. When the SDK's dependencies change,
`bun game/app/shells.mjs --update-lock` refreshes the SDK lock, and
`bun app/shells.mjs --test` refuses a stale one. The game workspace also carries
the core’s vendored patches and `host-dev` profile; the existing SDK test checks
those against the root workspace. Web builds retain the game’s non-contracting
floating-point flag alongside the core’s path-remapping flags.

Linux proofs use `gpu-dev` with separate completed host and GPU receipts;
a logic-only edit can rebuild the GPU alone. Completed development receipts bind
the whole file. `bun proof.mjs linux --build-only` builds this host/module pair.
The generic `bun exact.mjs linux` verb builds only the host, under `host-dev`.
The registry cache in `game/app/crates.mjs` is built but not enabled. D2 requires
three cold `gpu-dev` runs under load 5 with an eligible registry dependency
chain over a quarter of the total before wiring it into `buildBake` or setup.
See `game/bench/cold-build/README.md` for the measurement commands.
The cache separates channel, profile, target triple, rustc, toolchain overrides
and compiler/profile flags. It keeps two coherent registry generations,
excluding path/git crates and their dependants, and uses Cargo metadata's
resolved build and product directories. A build with any private profile does
not publish a generation. Cargo validates copied fingerprints;
a rebuilt copied unit means the warm failed. Cache files are not game inputs.
Web and release builds do not use it.
Release/production bakes bind the product digest: a signed 64-bit
Mach-O leaves out its signature and normalizes the signature and `__LINKEDIT` size
fields; other files hash whole. Generated profiles disable floating-point contraction; `gpu-dev` disables
debug assertions and overflow checks as release does. These profiles come from `game/Cargo.toml`, not authored per-game host code.

`game.assets: true` selects the model-capable module; `game.audio: true` links the
audio executor, which plays synthesized and sampled sounds. Sampled sounds need only
`game.audio: true`. Optional capabilities are separate artifacts/executors, never
Cargo features on a core crate. The engine's own workspace is outside Exact2's
six core checks.

### Assets, animation and placed UI

Put models, sprite PNGs and WAV or Ogg Vorbis sounds under `art/`;
[the baker](bake/README.md) produces validated `.model`, `.tex` and `.sound` assets,
each texture as RGBA8, BC and ASTC files of which a device fetches one
([texture payloads](bake/README.md#texture-payloads)). Name the authored `x.tex`
everywhere. Declare simulation dependencies in `Game::ASSETS`; setup waits for them.
Put everything else the game shows in `Game::STREAMED`: those are fetched from the
start, after `ASSETS`, but Play does not wait for them; each draws as it lands and
stays resident. Simulation cannot read a streamed asset (`w.model` is None), so
load order never reaches the hash; a save refuses only while a shown one is in
flight. A model a tick animates or reads belongs in `ASSETS`. Streamed names are
models and textures (sounds are not streamed yet), and never also in `ASSETS`.
Models and sprites need the asset-capable module; sounds and untextured emitters do
not. `assets/` is the bake's output, not a test's input: a hostless test builds its world
with `exact_game::sim_with_art!(Options::default())`, which bakes the game's `art/` in
memory (the same bytes) from either manifest layout and any working directory, then hands
them to `Sim::with_assets`; `bun game/new.mjs <game> --assets` starts a game that does.
A generated `logic` member gets `exact-game-bake` as a dev-dependency when the game has
`art/`; an authored `logic/Cargo.toml` declares it. See [the audio executor](audio/README.md) and
[the audio fixture](games/audio-fixture/logic/src/lib.rs) for sampled sounds.

For a data-authored level, derive `Data` for its record and declare `Game::LEVEL`.
JSON levels work in the primitive module and need no `game.assets` setting or art importer.
Setup reads it with `w.level::<T>(name)` after the
asset barrier. `w.generated(name, mesh_data)` registers immutable generated geometry;
saves check its reconstructed identity rather than storing render vertices. See
[the level example](games/asset-fixture/logic/src/lib.rs) and [engine reference](engine/README.md).

Tuning records use `Game::TABLES`, for example
`const TABLES: &'static [asset::Table] = &[asset::Table::of::<Movement>("movement.json")];`.
A record derives `Data` and is `Send + Sync`. The bake validates every named field;
a wrong type or missing field names its path. `w.table::<Movement>("movement.json")?`
borrows an immutable decoded version. `asset::Curve(Vec<(f32, f32)>)` samples with
`at(x)`, interpolating between ordered finite points and clamping at the endpoints.
[Beacons' movement table](games/beacons/movement.json) controls its speed and jump.

The web dev loop validates table saves in-process using that bake's compiled decoder,
then delivers all tables together through `assets/exact-tables.json`. It prints validation
time and subprocess count. No Cargo or rustc runs for a tuning save. A complete valid
transaction activates at the next simulation tick; a rejected transaction keeps the old
values. Paused tables await resume without preventing settlement. Restart and initial setup
admit the most recently delivered live transaction before setup reads it. Replay transactions
strictly after that live transaction's tick keep their recorded ticks across the initial asset
barrier and restart; earlier and equal-tick replay rows are superseded. A timestamped restart
first advances the old world to that time, then admits its resulting tables.
Levels require a new session. Tables never spawn entities.

Active table bytes appear in world resources and contribute to the hash. Every admitted
version remains in saved inputs for the session. `sim.table_inputs()` exports these
clock-stamped bytes; `sim.replay_tables(&inputs)` queues them on a tick-zero simulation
loaded with their original setup version. A restore requires the same active table bytes
and declared type identities (`table:` plus the declaring module path and type name),
and refuses changed tables by name. Development swaps are reported as `tableEdits` in
world state and refuse a proof pin. Delivery while running is admitted at the next tick;
replay preserves those recorded ticks, including transactions still pending at export.

Web dev clients swap tables under the running world, preserving its tick. Native Apple
dev clients apply a whole generation and restart the world on table delivery. For table
games, successful Rust saves rebuild the primary module and restart the web world,
validating the current table files before publishing the new declarations; the validator never follows an unselected side module.
The server retains only the three most recent table generations in memory. A client still
fetching an older generation after three later edits receives a refusal and must reload
the latest generation; those table generations are not retained on disk.

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

From `game/` (an unset `EXACT_UPDATE_TRUST` bakes development trust):

```sh
cargo build --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bun test ./proof*.test.mjs
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
