# Beacons on exact2 — third build

Brief: [task-001-beacons.md](task-001-beacons.md). Builder: Codex (GPT-6), no sub-agents. Only the game directory and this diary are authored; engine, renderer, hosts and shared scripts are unchanged.

## Timeline (UTC)

- 2026-09-18T05:48:25Z — Began reading rules and brief (approximate initial tool time; first explicit clock reading 05:48:55Z).
- Approximately 2026-09-18T05:50Z — Template generation completed (minute-level estimate; not explicitly clocked). Used the actual `game/new.mjs` createGame implementation, with only `gameShells` mocked to a no-op through Bun's module mock. The generator's locked/offline Cargo registration check passed and reported the shared lockfile unchanged. Existing generated Beacons shells reference `Beacons`; the template exports `SmallGame`, so the game will use `Beacons` to match. A literal generator CLI run would rewrite a shared shell outside permitted scope.

## Friction and scope

- Shell synthesis is an implicit write outside the game directory, even at app resolution. Existing matching shells can be reused read-only; no engine changes. The directory restriction costs an isolated copy-on-write warm Cargo target, stored inside the game. All Cargo commands inherit `DEVELOPER_DIR=/Library/Developer/CommandLineTools` and `EXACT_UPDATE_TRUST=development`.
- Historical r2 diary was read for scope workaround and snapshot-carrier metadata warning; current README, source and greybox guide implementation.

## Build/run entries
2026-09-18T05:51:54Z native test iteration 1
2026-09-18T05:53:12Z browser proof iteration 1
2026-09-18T05:53:27Z native test iteration 2; corrected test to compare positions, not storage guards
- 2026-09-18T05:53:27Z — **Browser proof passed**, first browser iteration, zero failures, 14.070 seconds including builds and three fresh browser processes. Cargo GPU/web phases were 2.29 / 2.02 seconds. Passing artifact mtime converted from local PDT to UTC.
- 2026-09-18T05:53:35Z — Confirmed native iteration 2 passed: two tests, 0.00 seconds simulation time, 0.76 seconds compilation. First native iteration failed to compile a test: storage `Ref<Transform>` has no Debug/PartialEq. Comparing the contained positions fixed the assertion; game code did not change. This was a test-author error at the guard/value API boundary.
- Approximately 2026-09-18T05:53:45Z — One pixel inspection, after passing proof: screenshot shows capsule, crates, unlit and lit spheres, legible HUD and Resume. No visual correction. Screenshot is the playing scene held paused for capture.

## Three numbers and scope of proof

- **181 lines of game code:** 143 Rust logic lines in `logic/src/lib.rs`, 38 Contract lines; physical lines including comments and whitespace after rustfmt.
- **Scaffolding:** zero authored host/GPU lines. Existing generated shells contain **59 lines**: 3 entrypoint lines, 6 build-script lines, 50 Cargo-manifest lines. They were reused unchanged. Game manifest and logic manifest are additional configuration, excluded from both counts; proof is 105 lines, native tests 64, launcher 15.
- **Brief to passing proof: approximately 5 minutes 2 seconds**, from estimated first reading 05:48:25Z to the recorded 05:53:27Z pass. Strictly timestamped first explicit clock read → pass is 4 minutes 32 seconds. Initial reading timestamp is approximate.
- **Pixel inspections: 1**, after the passing proof. No headed measurement.

All six requested proof steps passed. The script states the W position `[0, 0.9, -5.3666644]` and checks within 1 mm after exactly 90 ticks. Two repeated full input sequences produce identical entity state and complete save bytes. E outside range lights nothing; walking to (8, 0), then E, gives one lit beacon and `Beacons 1 / 3` in a polite UI live region. Rendered material emission is strictly partial at +100 ms and complete at +1000 ms. A paused world with W held retains every entity component and its tick across two seconds. A checkpoint taken mid-glow, with W held and a jump queued, restores into a fresh browser and continues to identical complete save bytes. The screenshot is saved for review.

Additional assertions cover victory after walking to all three beacons, Play again resetting position/count, accessible names and initial keyboard focus, no browser/GPU errors, and complete child-process cleanup. Native tests cover alternate seeds, seek partitioning, braking to zero, approximately 1.2 m jump height, no double jump, arrow input and saved continuation.

## What hurt now; measurement limits

- The required `bun game/new.mjs beacons` CLI conflicts with the strict write boundary because it synthesizes shells outside the game. Called its exported generator with a no-op module mock for shell synthesis; its real locked/offline registration check passed without a lockfile write. Naming the new logic type `Beacons` matched the pre-existing shell. This scope workaround cost roughly a minute of reading/tooling. No shared source was edited.
- Shared target artifacts normally land outside scope. APFS `cp -cR` copied the existing 37 GB target tree into the permitted game directory; the copy ran alongside authoring. This is a warm-cache measurement, not a cold build. All new compile/dist/proof outputs stayed under the game directory.
- Native test author initially compared storage guards rather than values; compilation caught it. One correction, under two minutes elapsed while the browser proof was independently being written/run. Engine unchanged.
- `state world:*` exposes carrier clock/epoch/incarnation alongside simulation data. Based on the current API and historical warning, the proof compares tick plus all entities and requires no truncation; carrier metadata is excluded. Complete save-byte equality additionally checks the whole simulation. No required step needed an approximation or substitute.
- The observed **edit → compiled browser → verified screenshot/proof loop was 14.070 seconds**. This run used direct proof builds, not the interactive dev server; display was locked and the headless proof already completed in seconds. Live hot-swap latency, physical input latency, scanout and frame pacing were not measured. No native-host parity claim.
- Glow is the renderer's emissive material, visibly brighter in the single inspected screenshot; bloom was not added. Scenery is plain shaded geometry. UI is real Contract text/buttons; the font and tone mapping are inherited from the engine.
- No exec-policy stall occurred. Native executables and all three headless browsers exited. `artifacts/process-cleanup.json` has `remaining: []`. No remote commands, clones, sub-agents, staging, commits or stash.

## Files

Authored `logic/src/lib.rs`, `logic/tests/sim.rs`, `logic/Cargo.toml`, `app.contract`, `app.json`, `proof.mjs`, `run.sh`, `README.md`, and this diary. Build cache, dist, proof transcript, replies, saves, screenshot and cleanup evidence are generated inside the game. Final git status shows only the game directory and this diary as untracked additions; no existing tracked paths changed. Repository-wide checks were not run because the add-on has its own workspace and the task restricts writes; targeted game tests and the actual browser proof ran.
