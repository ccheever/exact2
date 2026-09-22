# Beacons on exact2 — second build

Brief: [task-001-beacons.md](task-001-beacons.md). Builder: Codex (GPT-6), no sub-agents. All authored files and build artifacts stay in `game/games/beacons/`, plus this diary. Engine, renderer, hosts and shared scripts are unchanged. No commits.

## Timeline (UTC)

- 2026-09-18T02:45:10Z — Started reading repository rules and brief (start timestamp reconstructed from the first tool call; first explicit clock reading was 02:45:36Z).
- 2026-09-18T02:46:20Z — Template generation started. The required generator calls `cargo update --workspace`, which would write outside the permitted directory. Ran its actual copy/substitution code with that subprocess replaced by read-only `cargo metadata --locked --offline --no-deps`; Beacons packages already exist in the shared lockfile. The metadata subprocess is silent so far.

## Friction and scope

- Generator assumes permission to update the shared lockfile. Used a Bun module mock for its registration subprocess only; no shared source edits.
- Build outputs normally go to shared `game/target/`; this run sets `CARGO_TARGET_DIR` to the game's own `target/` and `EXACT_WEB_DIST` to its `dist/`. This isolation adds cache setup cost unrelated to game behavior.
- Every Cargo/build invocation uses `DEVELOPER_DIR=/Library/Developer/CommandLineTools` and development update trust.

## Results

Pending implementation and proof. Pixel inspections: 0. No headed measurements attempted.
- 2026-09-18T02:48:00Z — Found my Bun mock recursed through the imported spawn binding; no cargo child ran. Stopped recorded PID 64384. Template was already copied correctly. This is builder error, not engine friction; subsequent Cargo commands are direct and locked. Cloning the warm target via APFS `cp -cR` into the game keeps writes isolated.
- 2026-09-18T02:48:38Z — Native build/test iteration 1 started (approximate invocation time). Passed: W for 1.5 seconds ends at `[0, 0.9, -5.7333384]`, one large seek matches 1,500 small seeks byte for byte, seeded scenery repeats and changes with another seed, hop peaks at 1.2 m, airborne Space cannot double jump, save continuation matches, and the world settles. Simulation assertions take 0.01 s.
- 2026-09-18T02:50:09Z — Browser build/proof iteration 1 started, including two full repeated input scripts, beacon/HUD/glow, pause, cross-process save continuation, victory/restart, keyboard activation, screenshot and cleanup.
- 2026-09-18T02:50:27Z — Browser iteration 1 completed in 17.986 s (web/GPU compile phases 2.30 s and 2.33 s). Three assertions failed: both pause comparisons and restored entity comparison. Inspection of structured replies showed **only the carrier `clock` differed**; entity components and world tick were identical, and the complete saves already matched byte for byte. Corrected the proof to compare `{tick, entities, truncated}` for world snapshots. This was a proof-author error caused by conflating the carrier clock with world time; no game/engine fix.
- 2026-09-18T02:51:00Z — First and only pixel inspection: playing screenshot. Formatted the four game-owned Rust packages. Browser proof iteration 2 follows.
- 2026-09-18T02:51:06Z — Browser build/proof iteration 2 started after the snapshot comparison correction and Rust formatting.
- 2026-09-18T02:51:19.499Z — **Passing proof**, 0 failures, 13.455 s including build, four fresh browser sessions and cleanup. The two reported Cargo web phases were 0.94 s and 0.08 s. `artifacts/process-cleanup.json` reports `remaining: []`.

## Final results and three numbers

- **Game code: 181 lines** — 144 in `logic/src/lib.rs`, 37 in `app.contract`, counted as physical lines including whitespace/comments after Rust formatting.
- **Scaffolding: 55 lines** — three one-line host/GPU Rust shells plus 52 lines in their three Cargo manifests. Separately: proof 111 lines; native tests 54; environment/command launcher 14. The logic manifest and app manifest are configuration, excluded from game-code and host/GPU counts.
- **Brief → passing proof: approximately 6 minutes 9 seconds**, from the estimated first reading at 02:45:10Z to the recorded artifact timestamp 02:51:19.499Z. The strictly timestamped interval from the first explicit clock read (02:45:36Z) is 5 minutes 43.5 seconds; the initial reading adds approximately 26 seconds. This timing is approximate, not a subsecond measurement of the initial read.
- **Pixel inspections: 1.** One playing screenshot inspected after the first run. The passing run captures the same scene; no second visual inspection.
- Build/run iterations: one native test build, two web proof builds. One proof correction round, no game behavior correction. No engine, renderer, shared host or shared script changes.

## What passed

All six brief requirements passed through the real headless browser carrier: exact 90-tick W hold and stated millimeter-tolerance position; repeated complete runs with byte-identical saves; first beacon lit, HUD text and partial/full material emission at 0.1/1 seconds; all entity components and world tick frozen for two seconds while W is held; mid-glow save with held movement and queued jump restored before first render in a fresh process, then byte-identical continuation; and a playing PNG.

Additional proof: light all three through movement/E input, victory text and Play again, reset position/count, real Space activation of the focused Pause button, viewport projection, no page/GPU errors, all recorded descendants exited. Native tests exercise jump height, no double jump, alternate seed, and seek partitioning.

## Current friction, costs and limits

- `state world:*` includes the carrier's `clock`, `epoch`, and `incarnation` beside simulation fields. Whole-reply equality falsely fails across pause/restore. Reading the structured differences cost about 40 seconds; comparing simulation fields solved it. Complete save-byte comparisons remained enabled and passed.
- Chrome on the locked Mac reports Keychain/password-store encryption diagnostics and an allocator warning. They are captured in logs; there were no page exceptions or GPU errors. No headed app or screen-unlocking attempt.
- The shared template and grey box supplied the required APIs directly: `Tween` gives finite half-second glow, `Follow` gives saved camera smoothing, and `exactSurface` supplies the HUD. No data module or custom rendering code was needed. A non-live `round` argument reconstructs the world for Play again while leaving the seed fixed.
- The edit→verified loop actually cost **13.455 seconds** for the formatted game/proof correction through the browser assertions, screenshot and cleanup. Initial web build/proof cost **17.986 seconds**. Native compile was **1.91 seconds** plus 0.01 seconds of tests. These are warm-cache, headless end-to-end timings. The 30 GB existing target tree was copied with APFS copy-on-write into the permitted directory; it is generated cache, not game code. No repository clone was made.
- I did not run the interactive dev server: direct proof builds were already short, the display was locked, and the task ends at passing proof. Consequently hot-swap latency and live edit→human-visible latency are **not measured**. The screenshot proves captured presentation, not display scanout, frame pacing or physical input latency. No native-host or cross-host parity claim is made.
- “Glow” is the renderer's emissive material (asserted numerically and seen as a bright sphere), with no added bloom/postprocess. The player can overlap scenery/beacons; the captured player is standing at the lit sphere, so those silhouettes overlap. This follows the pass-through scenery requirement and did not require renderer changes.
- The all-entity snapshot comparison excludes only carrier metadata; no world component is excluded. The fresh-process continuation also compares the complete simulation save bytes, covering state beyond the visible entities. No required proof step needed a substitute.
- No exec-policy stall occurred during actual Cargo builds or executable launches. The only silent wait was the builder's recursive Bun mock described above.

## Files and scope

Fourteen authored files under `game/games/beacons/`: `README.md`, `app.contract`, `app.json`, `proof.mjs`, `run.sh`, `logic/Cargo.toml`, `logic/src/lib.rs`, `logic/tests/sim.rs`, and `Cargo.toml`/`src/lib.rs` pairs under `gpu/`, `web/`, `apple/`. This diary is the fifteenth authored file. Generated artifacts are confined to the game's `target/`, `dist/` and `artifacts/`. Git status shows only the new game directory and this diary; no tracked files changed. No staging, commits, remote commands or sub-agents. All owned sessions and subprocesses have completed.
