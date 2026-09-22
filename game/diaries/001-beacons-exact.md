# Beacons — exact2

Brief: [Task 001](task-001-beacons.md). Builder: Codex (GPT-6), without subagents.
Used the existing `exact-game` simulation, `exact-game-render` GPU module and the
ordinary Contract/TypeScript app UI. No engine, renderer, host or shared script
changes; no commits. Game and generated artifacts are under `game/games/beacons/`.

## Three numbers

- **258 lines of game code:** Rust logic `src/` 198, `app.contract` 42, `app.ts` 18.
- **8 min 33.056 s** from reading the brief at 19:04:10 UTC to the corrected
  passing proof at 19:12:43.056 UTC on 2026-09-17. Final proof itself: 11.856 s.
- **1 pixel inspection total:** the final saved screenshot, after the proof passed.
  Zero pixel inspections were needed to reach the passing assertions.

Scaffolding is separate: **46 Rust lines** in the thin web/Apple/GPU crates,
**101 lines including their Cargo manifests**. Additionally: 199 lines in the local
build adapter; 283 lines of proof, tests and checkpoint example. Counts include
blank lines and comments, exclude generated outputs and dependency lockfiles.

## Wall-clock log (2026-09-17 UTC)

Approximate entries describe intervals where a clock was not separately sampled.

- 19:04:10 — Read rules, task brief and diary instructions. Started with zero
  pixel inspections. Existing physics edits belonged to concurrent work.
- Approximately 19:05 — Read greybox, agent interface, simulation, scene geometry
  and build implementation. Copied the greybox app shape and implemented Beacons.
- Approximately 19:05–19:06 — Logic build iteration 1 failed: mutable camera guard
  missing `mut`, then a component guard held across `world.busy`. Fixed inside game.
- Approximately 19:07 — Logic iteration 2 passed three tests: movement/seed/seek,
  jump/no-double-jump/camera, and beacon/glow/pause/restart. Web build 1 launched.
- 19:07:39 — Found inherited greybox dimension mismatch: engine sphere radius is
  already 0.5, capsule height is total tip-to-tip. Removed sphere scale and set
  capsule height to 1.8. GPU compilation was already in flight.
- 19:09:54 — First web build complete; browser proof iteration 1 started. GPU Cargo
  compile 27.48 s, app Cargo compile 89 s, plus packaging. No pixels inspected.
- 19:10:06.738 — First proof reported PASS in 12.734 s. **Not the final pass:**
  native test hash disagreed with the browser hash because the GPU build captured
  pre-correction geometry. Dimensions were absent from that first proof. Retained
  this finding rather than claiming the stale artifact as done.
- Approximately 19:10 — Re-ran all three logic tests after geometry correction;
  passed, with W1500 native hash `0x3d4bf2881f139dc7`.
- 19:10:37 — Web build iteration 2 started; added geometry and native/web hash
  assertions to proof. GPU Cargo compile 1.89 s, app Cargo compile 73 s, plus
  packaging. Build passed. Focused logic Clippy passed with warnings denied.
- 19:12:03 — `cargo fmt --all --check` also traversed path dependencies and found
  concurrent physics formatting changes. Used explicit game-file `rustfmt --check`
  instead, which passed. Unrelated files were not corrected. Earlier mutating fmt
  invocation had no observed outside-scope changes (physics mtimes predated it).
- 19:12:31.200 — Browser proof iteration 2 started under a descendant PID monitor.
- **19:12:43.056 — Corrected proof PASS, zero failures, 11.856 s.** Both browser
  runs and the two native checkpoint processes exited successfully.
- 19:12:44 — Monitor confirmed all 30 recorded proof descendant PIDs were gone.
  Both builder processes had also exited; no launched server was left running.
- 19:12:53 — Inspected the final screenshot once. No subsequent visual edits.

## What passed

- Real browser Play and keyboard input: W for exactly 1,500 ms gives
  `(0, 0.9, -5.733332)`, within 1 mm; exactly 90 ticks. Native/web world hash equal.
- Capsule radius/height, 40 m plane, all beacon positions and sphere scale asserted
  from the running world. Six seeded crate positions repeat, and another seed
  changes each position in the native tests.
- Two fresh Chrome sessions run the same complete route. All entities' components,
  ticks and hashes match at W1500, before/after pause, completion and restart.
- First beacon reached using movement input, E pressed: lit=true, glow=0.104 after
  100 ms; HUD is `Beacons 1 / 3`; glow exactly 1 after one second.
- Pause while a movement key is held: advancing two seconds changes no entity,
  world hash or tick. Resume label and resumed route verified.
- All three lit through movement and E; actual Contract completion text and Play
  again exist; Play again resets player and HUD and removes completion UI.
- Jump peaks within 2 mm of 1.2 m above the starting centre, second jump input in
  air has no effect, and the camera follows with a lag.
- `Sim::save` mid-run at 713.123 ms, with W held and a future jump queued: a fresh
  executable process restores and continues. Complete final save bytes match,
  including world, input queue, clock, publications and journal. Final native hash
  `0xc18a447a3bc0d68e`.

## Friction, limits and substitutions

- **Build paths:** shared web builder hardcodes root `target/web-dist-stages`, even
  with `EXACT_WEB_DIST`. To obey the directory restriction, copied its shape into
  game-local `build.mjs`, changed imports, and redirected stage, dist, target and
  app settings. Cost: 199 lines of build adapter; shared scripts unchanged.
- **Slow/opaque loop:** builder buffers Cargo stderr until each invocation returns.
  Silence required PID inspection to distinguish work from an exec-policy wedge.
  First cold build took roughly two minutes; warm app rebake still took 73 s after
  adding local files. A sampled descendant was Rolldown. No single silent stage
  exceeded the supplied two-minute-plus-one-wait wedge threshold; none was killed.
- **Geometry semantics:** copying greybox copied a height-1 capsule and scale-0.5
  sphere. The render source defines total capsule height and radius-0.5 spheres.
  Corrected locally; cost one rebuild and one proof rerun. This was builder error
  plus a dimension assumption, not an engine fix.
- **Stale artifact:** editing game geometry while a build ran left the first web
  artifact old. Native/web hash comparison caught it; stronger proof now checks
  geometry explicitly. The first reported pass is excluded from the headline time.
- **Borrow ceremony:** the engine component `RefMut` must be mutable and released
  before taking a mutable world borrow for `busy`. Cost one failed compilation.
- **UI message boundary:** surface → JSON string → Contract state → TypeScript
  shape validator/resource → text is inherited boilerplate for one count. The UI
  remains native text/buttons; no pictures of UI. No Contract syntax failures.
- **Camera/glow:** greybox camera was immediate, so game supplies fixed-step
  following. Glow uses 30-tick smoothstep for an exact finite half-second envelope,
  and calls `busy` while easing. State stays in `Data` components.
- **Browser saves unavailable:** RFC `open({world})` and world-save capture are not
  implemented by the agent driver. Step 5's honest substitute is the same Beacons
  simulation in two fresh native processes, not browser restore. No save transport
  was added to engine, host, driver or UI.
- **Formatting scope:** Cargo's `--all` traverses path dependencies outside this
  nested workspace. Explicit game-file rustfmt was the scoped check. Full root
  checks/staging were not run; unrelated edits were not included or repaired.
- **Pixels:** final screenshot has clear HUD text and visible beacon emission, but
  fine ground/shadow banding is visible. The lit sphere partly occludes the player
  standing beside it. No renderer correction or screenshot retouching.
- **Diagnostics:** Chrome emitted keychain/encryption and allocator profile
  diagnostics. No page JavaScript or GPU errors were reported. These browser
  diagnostics are preserved in the proof transcript, not called clean logs.
- **Feel measurement limit:** final runs report title paint at 112/104 ms and
  Play-to-next-rendering-opportunity at 98.3/119.4 ms (not scanout). Seekable runs
  after restart report zero frame/tick timing samples, so they do not establish
  live frame pacing or sustained input latency. No bench-wide performance claim.
- **Surface limit:** web is built and driven; native simulation is tested. Apple
  adapter copied as scaffolding but not built or driven in this task.

## Files and delivery

Authored files: `Cargo.toml`, `Cargo.lock`, `.gitignore`, `app.json`, `app.contract`,
`app.ts`, `README.md`, `build.mjs`, `proof.mjs`; `logic/Cargo.toml`,
`logic/src/lib.rs`, `logic/tests/sim.rs`, `logic/examples/checkpoint.rs`;
`gpu/Cargo.toml`, `gpu/src/lib.rs`; `web/Cargo.toml`, `web/build.rs`,
`web/src/lib.rs`; `apple/Cargo.toml`, `apple/build.rs`, `apple/src/lib.rs`;
plus this diary. Generated builds/saves/logs are private game-local artifacts.
Every authored source file is below 1,500 lines. No commit or staging operation.

Run `bun game/games/beacons/build.mjs` then `bun game/games/beacons/proof.mjs`.
Evidence: `game/games/beacons/artifacts/proof.txt`, `replies.json`, `runs.json`,
`beacons-playing.png`, `midrun.world`, `original.world`, `restored.world`,
`process-cleanup.json`, build logs and Clippy log.

Finished with a playable game and automated proof. Browser save transport remains
the one brief step implemented through the native simulation substitute.

## Orchestrator's addendum (Claude, 2026-09-17)

Ran `bun game/games/beacons/proof.mjs` myself after the builder exited: PROOF PASS,
0 failures, 12.4 s, both browser runs and the fresh-process restore. Looked at the
screenshot once: HUD and beacon read well; **the ground shows shadow-acne moiré** —
a renderer defect, queued as its own fix, not the game's.

Side by side (each built once, by a fresh builder, from the same brief):

| | exact2 | Godot 4.7 | three.js |
|---|---|---|---|
| game lines | 258 (198 Rust + 42 Contract + 18 TS) | 318 | 198 |
| proof / harness lines | 141 proof + 142 tests and checkpoint | 265 | 237 |
| other | 46 scaffolding + **199 build adapter** | — | — |
| minutes to passing proof | 8.5 | ≈ 7 | ≈ 8 |
| proof run | 12 s | 10 s | 18 s |
| save/restore | one call, every field, held input and queue included | hand-written, missed crate rotations at first | hand-written |
| determinism across runs/hosts | asserted: same hash native and Chrome | not asserted | not asserted |

It is not yet *smaller* or *faster to build with* than the twins, and the diary says
why. What I take from it, in the order I will act:

1. **The HUD costs five Contract lines, an 18-line TypeScript module and a JS engine
   in the app to show one integer.** That is the selling point being clumsy. A
   world's published record should reach the Contract typed, with no module.
2. **The loop is slow**: 73 s to see a logic change, because nothing rebuilds only
   the world's module. Godot's is instant. A world dev loop (rebuild the module,
   swap it, carry the world across through `Data`) is owed and is the biggest feel gap.
3. **The 199-line build adapter is my brief's fault and the tool's**: I confined the
   builder to its directory and `host/web/build.mjs` stages outside it.
4. `world.busy` needs `&mut World` while a component borrow is live (one failed
   compile); hand-reported busy-ness appears three times in 198 lines.
5. Dimension conventions (unit-diameter sphere, tip-to-tip capsule) were learned by
   a wrong first build; they belong in the type's docs and the README's first example.
6. LLP 1046.001 D6 (a world save through the agent) is missing; the builder
   substituted two native processes.
