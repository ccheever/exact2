# game/ — the game engine add-on

- To see the game: `bun game/dev.mjs beacons` (commands run from the repository root).
- To change it: edit `game/games/beacons/logic/src/lib.rs` or `game/games/beacons/app.contract`; the dev page reloads.
- To verify gameplay: `bun game/games/beacons/proof.mjs linux`.
- To verify pixels: `bun game/games/beacons/proof.mjs web`.
- To drive the simulator: `env -u SDKROOT DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer EXACT_UPDATE_TRUST=development EXACT_IDENTITY=- bun game/games/beacons/proof.mjs ios`.
- To check save/setup determinism: `bun game/games/beacons/proof.mjs linux --paranoid`.
- When pins move intentionally: `bun game/prove.mjs beacons --repin` (all modes, Linux and web).
- Without a web carrier: `bun game/prove.mjs beacons --repin --hosts linux` (records Linux only).
- When something stalls: `EXACT_APP_DIR=game/games/beacons EXACT_WEB_DIST=game/games/beacons/dist bun scripts/agent.mjs web "tap play" "clock settle" "state world:* busy" state logs`.
- To see a box or blocker: `EXACT_APP_DIR=game/games/beacons EXACT_WEB_DIST=game/games/beacons/dist bun scripts/agent.mjs web "tap play" "layout world:player"`.
- To find unused diagnostic facilities after a proof: `bun game/prove.mjs beacons --report`.


An agent-native game engine on exact2, as an **add-on**: its own Cargo workspace,
absent from the root `members`, so the five checks never compile it and an app
without a world carries none of it (LLP 1041 §5). The record of why is LLP 1041 and
its sub-documents; this file is the map and the rules. The code is the authority.

## The one idea

**A world is a guest in a `canvas`**, as a web page is a guest in an `iframe`
(LLP 1020). A game is an ordinary exact2 app — Contract for every screen, a manifest,
a bake, delivery — plus one module holding the simulation *and* its renderer in one
memory, loaded after first pixel like any GPU module. Game UI is the app engine:
canvas children are the HUD, a placement is a sign in the world.

## Layout

| | |
|---|---|
| `engine/` | `exact-game` — the simulation: world, data, ticks, input, scene, the agent's reads. **No GPU, no host.** |
| `bake/` | `exact-game-bake` — build-time glTF, PNG/JPEG decode, geometry preparation and mip generation. Never linked into a running game. |
| `app/` | `exact-game-app` — the shared Rust-only bake for game UIs without data sources. |
| `derive/` | `exact-game-derive` — `#[derive(Data)]`, `#[derive(Component)]`, `#[derive(Args)]`. No `syn`. |
| `render/` | `exact-game-render` — the wgpu renderer and `WorldSurface`, the `exact_gpu::Surface` a canvas binds. |
| `physics/`, `audio/` | Rapier integration and optional synthesis/playback executors |
| `games/` | consumers: `greybox` (LLP 1041.000 S0), `beacons` |
| `bench/`, `twins/` | the same scenes here, in Godot 4 and in three.js; numbers, never checks |
| `diaries/` | what building with it was like, scored against the twins |

## The programming model

A game implements `setup` and `tick`; components and resources are ordinary `Data`.
This small example is compiled by `cargo test --doc -p exact-game`:

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
        w.get_mut::<Transform>("player").unwrap().position.x += w.dt();
    }
}
let mut sim = Sim::<Example>::new(()).unwrap();
sim.run(1000.0);
assert!(sim.local_position("player").unwrap().x > 0.0);
```

See [Beacons](games/beacons/logic/src/lib.rs), [Greybox](games/greybox/logic/src/lib.rs)
and the [new-game template](new/logic/src/lib.rs) for complete movement, HUD and input examples.

`Character` composes `Move`, `Jump` and `Gravity`. Its velocity and configuration
are saved and hashed as a component on the player. `step` reports `displacement`, `grounded`,
`jumped` and `landed`; bounds stop outward velocity, braking and landing arrive
exactly. It uses no physics dependency; collider worlds can use Rapier's controller.
`near` and `near_xz` return `(entity, pose)` in entity order, with an inclusive radius
and global positions, including parent chains (XZ ignores height). A missing origin yields no rows.

- **A tick is a function that calls functions.** No scheduler, no plugins, no
  system parameters. Physics is `physics::step(world)`, written where it runs.
- **All state is in the `World`, and all of it is `Data`**: one derive gives the
  save game, the hash, the agent's JSON, the level file, and what a dev reload
  carries. A `Game` has no fields.
- **Arguments are a struct; field order is canvas order.** `#[derive(Args)]`
  supports bool, u32/u64, i32/i64, f32/f64, and String (`()` for none).
  Unmarked fields construct; `#[live]` fields are read each tick.
  `#[restart] pub restart: bool` marks a restart edge: either transition uses the
  same setup reconstruction path. In Contract, `state again = false`,
  `action restart writes again` / `again = not again`, then `world(7, paused, again)`.
  The explicit boolean replaces a generation counter; `state.world.restarted` counts
  reconstruction in this session, outside the save/hash. No ninth operation.
  Integer bounds are checked before casting; 64-bit fields accept safe f64 integers. A timed
  `bind(values, Some(at_ms))` validates first, seeks under the old arguments, then
  swaps. `Game::validate` runs before construction, binding, seeking for a bind, or restore; a refusal changes nothing. Hosts construct with `Sim::from_values`. Saves encode argument fields by name: reordering is safe, additions default, removals are ignored. Saves carry the game's `ID`, world time and dynamic input; the first restored host clock
  establishes a new epoch.
- **Time is an input.** Under the seekable clock, `tick = floor(clock_ms × hz / 1000)`; a step is `1/hz`
  exactly; there is no `delta`. Rendering interpolates between the last two ticks,
  so motion is smooth at any refresh rate and the simulation never knows.
- **Names are first class.** `world.get::<Transform>("fox")` accepts a name or an entity handle; `world:fox` addresses it in the agent. Consuming `query()` yields guarded items (mutable bindings use `mut`); `.iter()` yields `(Entity, item)` with plain references. `.one()` returns an item and refuses multiple matches in all builds.
- **Iteration is in entity order, always** — storage scans presence bitmasks in
  ascending index order, so a world loaded from a save replays exactly as the one
  that wrote it.

## Proof pins

Each game's `pins.json` is the sole hash authority. Its proof calls `pin(tick, state)`
and `pinSave("continuation", path)`; Rust tests use `World::assert_pin(include_str!("../../pins.json"), game, tick, hash)`.
The tiny Data parser skips metadata; it is smaller than a generated `pins.rs` build step.
`bun game/prove.mjs beacons --repin` runs continuous, Save and FreshGame on Linux
and web, refuses incomplete or disagreeing tick/save observations, then rewrites only
`pins.json` and prints old → new. Other proof assertions still run. `hosts` records
only exercised hosts. A missing configured Chrome executable (ENOENT) records
Linux only; a web proof failure still refuses. `CHROME` selects the browser,
which runs headless. `at` is HEAD, not a claim that the working tree was clean.
New games start with empty pins until that command succeeds. Screenshots, receipts,
transcripts and saves belong under ignored `artifacts/`; only README-cited evidence
is retained. No ninth operation. `--report` names unused facilities tied to observed
refusals/stalls; it does not infer that an unused operation would help a passing run.

## Determinism — the contract (LLP 1041.001 D5)

Same seed, same tick-stamped inputs, same completed tick ⇒ the same `world.hash()`,
on every host, bit for bit. The seekable clock fixes the input-to-tick mapping.
What that costs, and the only rules a game author must remember:

1. No clock but the world's. No randomness but `world.rng()`.
2. Transcendentals come from `exact_game::math` (libm), never `f32::sin`.
3. No `HashMap` iteration in a tick, no threads in a tick.
4. State lives in components and resources, nowhere else.

The journal is telemetry: a record outside the world hash and observation, so a
read that logs (such as a malformed agent request) must not change the world's course.
Semantic `Data` has no interior mutability: derives introduce none; a manual
implementation that changes semantic state through a shared reference is outside
this contract, and quiescence and the hash cache are undefined for it.

Pixels are held to a band; simulation state is held exactly. The two game proofs
and saved physics pile demonstrate this on arm64 macOS, x86-64 Linux and Chrome
wasm; unexercised engine APIs do not inherit a measured parity claim.

## Publications and events

`w.publish_record(&Hud { beacons: count })` publishes a `#[derive(Default, Data)]`
record through its existing field traversal. Nested records, lists, options and scalars
keep their field names and JSON types; Contract validates them against `shape Hud`
at the app boundary. Typed numeric vectors publish arrays; enum variants are not Contract values.
`World::publish("beacons", count)` remains the scalar operation. Contract reads
it with `resource hud = exactSurface("world") as shape Hud`; absent fields default
and extra keys are ignored. It needs no app data module. `Sim::take_published`
drains changed state; a rebuilt or restored simulation publishes again.
The first live canvas owns its surface name: other instances cannot publish or clear its record and produce one diagnostic naming the surface.
`World::emit("won")` separately queues a string for the canvas's `message=` handler.
Undelivered events are saved in order but excluded from the simulation hash.
An empty queue adds no world save bytes.

## The agent's interface

No ninth operation (LLP 1041.001). `tree`, `state`, `layout`, `logs` and `clock`
reach the world through one export on the module; an entity is a target
(`world:fox`); `clock` is the only thing that moves the world; the journal is how an
agent hears. Capture the complete simulation with `s.world('world').save('run.world')`
(CLI: `screenshot run.world world save`). `open({world: 'run.world'})` or
`--world run.world` holds the bytes until Play creates the first carrying surface,
then restores before its first render. Web, macOS, Linux and the iOS Simulator use the
same forms. Linux loads the same module with no device: simulation reads, input,
clock, publications, saves and CPU point picks work; canvas pixels report unavailable.
The native bake binds the GPU product digest to the app and cohort before loading.
`EXACT_GPU_MODULE` (Linux) and `EXACT_GPU_DYLIB` (Apple) select a path only in a development-trust bake; the product must still match its baked digest.
Its screenshots paint the Contract UI with flat canvas rectangles. Both carriers refuse input files and captures above 256 MiB before
reading/encoding the carrier. A refused restore is reported once by the creating
operation and remains in that canvas's `state.world.restoreError` and journal;
other operations continue on the fresh world. The capture replies with the byte count, world hash and tick; state
reports `restored: true` until the next tick or setup-argument rebuild. Current app bindings win over saved
arguments; `state.world.restoredFrom` shows the saved arguments while `restored` is true. Register types first spawned mid-game with `world.register::<Projectile>()` in `setup` so a fresh world can restore them. The iOS simulator carrier is driven by the same proof scripts. Explicit `size` uses a logical viewport fitted into the simulator window, so save bytes and projection checks use the same points as web/Linux; omit `size` to use the phone viewport. UIKit canvas input is labelled `recognized`, since UIKit exposes no synthetic touch constructor.

`state world:*` reads every entity's components in one reply (512 maximum,
then `truncated: true`); `state world:* under world:player` narrows to a subtree.
`s.type('world', {key: 'KeyW', for: 1500})` presses, advances the agent clock,
then releases on the same carrier even if the clock fails. CLI: `type world key KeyW for 1500`. The reply or error transcript retains partial steps.

`game/proof.mjs` supplies `proof(import.meta, async ({open, check, equal}) => { … })`.
`open()` builds this game's app only when its inputs change, opens a fresh session,
and records every operation. `check(label, condition)` reports failures without
stopping independent assertions; `equal(a, b)` compares JSON values. Every session
is closed and recorded children are checked before exit. Artifacts are in the game's
`artifacts/` directory; the first CLI argument selects web, macOS, iOS, or Linux.

## Working here

Commands run from the repository root unless stated otherwise:

```sh
bun game/new.mjs my-game
bun game/dev.mjs beacons
bun game/games/beacons/proof.mjs linux --paranoid
bun game/games/beacons/proof.mjs web
bun game/bench/size.mjs
```

From `game/`, use `cargo test --workspace --no-fail-fast`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`cargo fmt --all -- --check`. Local Cargo validation uses
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`,
`SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk`,
`EXACT_UPDATE_TRUST=development`, and `EXACT_IDENTITY=-`.
Apple host builds use Xcode's developer directory and no `SDKROOT`.
The [proof runner](proof.mjs) owns host setup and cleanup; game proofs own assertions.
Paranoid proofs run Off, Save and FreshGame; only web needs a final build-only
step to restore the ordinary artifact. Native keeps one executable.

The generated shell follows `app.json`: `game.assets: true` selects the model-capable
module; `game.audio: true` links the audio executor. Authored keys override generated
defaults; see [shell generation](app/shells.mjs). No core-crate Cargo feature selects
optional capabilities. Production module sizes and dated evidence live only in
[bench/README.md](bench/README.md#d4--module-size-2026-09-18).

## Assets and animation

Put models and sprite PNGs under `art/`; [the baker](bake/README.md) writes validated
`.model` and `.tex` assets. Declare simulation dependencies in `Game::ASSETS`.
Setup waits for that closure; cosmetic arrival cannot change gameplay reads or bounds.
`Mesh::asset(name).bounds(...)` supplies authored bounds for an undeclared cosmetic.
The primitive module refuses named asset meshes and sprites.

Animation execution stays in [animation.rs](engine/src/animation.rs), linked by games
that explicitly call it and by the model presentation executor for inspection/carry.
The smaller [pose data core](engine/src/asset/pose.rs) contains saved local poses and
rig geometry, without playback. `World` owns no animation-specific runtime field.
Registration follows setup: spawning/registering a controller registers its produced
`Pose`; its derived cache is created on first execution and excluded from saves. A game that spawns a type later registers it in setup so a
fresh process can load it. Unregistered saved types refuse by name.

`let motion = animation::step(w)` returns owned markers/root motion. Apply movement
with `Transform::translate_local`, then read `animation::socket` for a tick-boundary
joint. `SocketFollow::new("fox", "head").offset(t)` declares a displayed attachment;
the renderer recomposes the interpolated local chain. Ordinary transforms use
shortest-path normalized linear quaternion interpolation; skin locals use spherical
interpolation. See the [skinned fixture](games/skinned-fixture/logic/src/lib.rs) and
[its continuation tests](games/skinned-fixture/logic/tests/sim.rs).

`Animation.sampled` is the saved reverse-playback initialization flag; signed zero
has no sentinel meaning. [Pins](games/skinned-fixture/pins.json) are the authority.

## Particles, sprites and placed children

Untextured `Emitter` presentation remains available to primitive games. Emission
intent is saved; positions are derived. Sprites require the model-capable artifact;
its sprite pipelines and instance storage are absent from the primitive web path.
See the [particle game](games/particles-fixture/logic/src/lib.rs),
[sprite game](games/sprites-fixture/logic/src/lib.rs), and their proofs.

A `Placed` component associates a Contract child index with a world plane. The
browser composites CSS homographies; Apple captures child textures and can depth-test
them against the world. Hidden placement is explicit. Kernel layout, accessibility
and ordinary controls remain the app host's responsibility. See the
[placement fixture](games/placement-fixture/logic/src/lib.rs), its
[Contract](games/placement-fixture/app.contract), and [proof](games/placement-fixture/proof.mjs).

## Further reading

- [Engine API](engine/README.md), [renderer contract](render/README.md),
  [physics](physics/README.md), [audio](audio/README.md).
- [Design of record](../llp/1041.003-game-engine-as-built.explainer.md) and
  [agent contract](../llp/1041.001-agent-interface-to-a-game.rfc.md).
- [Bench receipts](bench/README.md) and [ergonomics diary](diaries/002-ergonomics.md).

## I1 simulator proof receipt — 2026-09-18, unfinished

The iOS proof carrier now passes the explicit 1280×720 logical viewport through
to the simulator. UIKit fits it into the phone window; screenshots are 3840×2160
at simulator scale 3. Ordinary launches keep their native viewport. Greybox's HUD
key forwarding now runs for ordinary controls as well as textareas. The simulator
carrier reports `recognized` input delivery because UIKit cannot synthesize platform
touches. This is asserted explicitly. Browser HTTP delivery gates, browser residency
reload probes and physical Metal removal are unavailable on this host; shared
readiness, rendering, save and pin assertions remain in the scripts.

Simulator: **iPhone 17 Pro, iOS 26.5**,
`0CF430AF-CBA5-4F12-88CA-D05DEF06AFFF`. Latest completed I1 receipts below
include builds and build-lock contention, and are not runtime benchmarks. Linux
time sums the three `--paranoid` subprocess wall times (Off, Save, FreshGame).
Runs occurred while D4 was editing the shared tree, so this is not a verification
of one frozen final revision. Logs are `/tmp/i1-<game>-<host>-retry.log` except
Asset web (`/tmp/i1-asset-fixture-web-gate.log`) and the initial iOS Asset, Skinned,
Particles and Placement runs (`/tmp/i1-<game>-ios.log`, Placement uses `placement`).

| Game | Linux paranoid, seconds | Web, seconds | iOS, seconds | macOS, seconds |
|---|---:|---:|---:|---:|
| greybox | PASS, 61.193 | PASS, 24.479 | PASS, 123.920 | — |
| beacons | PASS, 175.768 | PASS, 22.093 | FAIL: focus, 131.156 | FAIL: focus, 174.704 |
| asset-fixture | PASS, 424.072 | PASS, 21.703 | FAIL: GPU limit, 54.138 | — |
| skinned-fixture | FAIL: pins, 64.204 | FAIL, 23.594 | FAIL: GPU limit, 48.504 | — |
| particles-fixture | PASS, 22.721 | PASS, 6.882 | PASS, 88.382 | — |
| sprites-fixture | PASS, 71.871 | PASS, 8.889 | PASS, 106.574 | — |
| placement-fixture | PASS, 25.153 | PASS, 7.733 | PASS, 48.289 | — |

`bun game/prove.mjs greybox --hosts web,linux,ios --compare-saves` also passed:
web 49.442 s, Linux 165.301 s, iOS 191.207 s, with byte-identical continuation
saves. The summary is `games/greybox/artifacts/prove/summary.json`.
`bun scripts/smoke.mjs host-ios` passed in 8.5 s (two embedded sessions, native
navigation, bad-plan refusal and independent destruction). Apple builds used
Xcode's developer directory with SDKROOT unset.

Remaining failures are not skipped: Beacons' victory autofocus fails on both Apple
hosts in shared `ExactKit/Accessibility.swift`; model pipelines need seven vertex
storage buffers, but the shared GPU adapter fallback requests four on this simulator.
Skinned's Linux/web observations now agree with each other but differ from its
checked-in pins: tick 60 `0x4a4c7b164b0790f1`, tick 120 `0xc8337115819ca23c`,
continuation `7dfaf8bd0bf191b8293bf08046d20f5a6e1bd8e138482e784b4976fc6d603f0b`.
No I1 pin was changed.

### Touch binding and measurements still owed

`InputEvent::Control { name, id, phase, x, y, at_ms }` now feeds the existing named
actions. A held contact composes with keyboard input; a stick uses a local origin
and the existing 60-point radius. Up, cancel and blur release it. Three regressions
cover keyboard/contact composition, local stick save/restore and queued-contact
continuation in a fresh simulation. The engine change is 72 net production lines
plus 116 test lines. This is only the engine half: no Contract block, host binding,
Beacons touch controls or template default has been delivered. Connecting ordinary
controls requires shared declaration/input-carrier files outside I1's enumerated
scope; approval for that extension is pending. Existing Beacons instructions still
describe keys.

The Beacons iOS receipt (`games/beacons/artifacts/i1-ios/perf-ios.json`) reports
33 draws, 14 instances and 2,523 triangles. All four timing rings have **zero
samples**, so no frame-time claim follows. After-ready upload, pipeline and
model/skin-buffer-reallocation counts are zero. Actual frame and placement/HUD
measurements remain uncollected; Skinned cannot reach its capture because of the
GPU-limit failure. Reading a live iOS run also requires a change to the executable
adapter, beyond `ExactKit/IOS/**`. Simulator CPU/presentation measurements can
compare this host on this Mac, but cannot provide real-device GPU timing, thermal
behavior, battery cost or phone frame-rate guarantees.

### Validation limits and all checked-in pins

The initial game workspace run passed 530 tests with 15 ignored. All three new
control tests passed. A later full run failed nine targets during concurrent edits;
the third attempt stopped at two render-test E0308 errors. No fourth full attempt
was made. All-target clippy passed; fmt still reports concurrent files outside the
I1 edits. Root workspace build passed earlier; a later build waited on the shared
Cargo lock and was cancelled. Harness tests passed (35 tests, 194 assertions),
boot passed, and scoped caps passed with the staged I1 changes then unstaged.
These are partial receipts, not a clean final-suite claim.

| Game | Tick pins | Continuation SHA-256 |
|---|---|---|
| greybox | 0: `0x9a871d8582d905e7`; 90: `0x71f43e51a13cc49f` | `d3e4364bdf399ae565efbde077bc3cf4bb829b3b855d4a067d06776e8bccae65` |
| beacons | 907: `0x0b132d378ffd3b21` | `eb13fd528bb8f6f071663f07404860d175525b1a3a1970f7854505f841f4ee21` |
| asset-fixture | 60: `0xb1365b0eb9a7c59d` | `6e49ed23ac776fa6f0f6df7198d8f3a7d310c9f93d00634dbba36f4f8a46c07a` |
| skinned-fixture | 60: `0x4d44c305a176e577`; 120: `0xd147cd9b2a8c62f7` | `0d50ebe6388c4db317aae21fb3a1f30269f9e67300c3a0ec50e82eec0b7e5a79` |
| particles-fixture | 300: `0x8dc0cac2d2645d93` | `6f850600b9b7a87f6e12d2c42adc587c8ef5dc9d48ecea1b2af51ecfbd34a7e6` |
| sprites-fixture | 300: `0x2e3d805eb6c89e55` | `ca774847c5db059a2dd7991927ce9c38578c8d0ed45ef34e788b99bcfe4c5b41` |
| placement-fixture | 330: `0x626f1c12836bea76` | `27f28608444a701e73c10a96d20bcb801c840b2f148bf30a1ba26592c6c51b8f` |
