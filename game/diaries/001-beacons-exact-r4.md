# Beacons on exact2 — fourth build

Brief: [task-001-beacons.md](task-001-beacons.md). Builder: Codex, no sub-agents. Engine, renderer, hosts and shared scripts stay unchanged.

## Timeline (UTC)

- Approximately 2026-09-18T16:41Z — Started reading rules, brief, diary rules and current README. First explicit timestamp: 16:44:20Z. Initial start is minute-level estimated.
- 2026-09-18T16:44:44Z — Read current generator, character, tween, greybox and driver; historical r3 diary only for scope workaround. Disk has 24 GiB available; shared target is 41 GiB logical.

## Friction as encountered

- Literal `bun game/new.mjs beacons` would write outside the allowed directory: generator synthesizes shared shells (and may update shared lockfile). Existing Beacons shells/lock entries remain from previous runs. Use actual exported createGame with shell synthesis mocked off, retaining its locked/offline metadata check. Match existing shell's `Beacons` type; subsequent resolution must leave shells byte-identical. No shared changes.
- Cargo outputs normally live outside scope. Copy the existing target with APFS copy-on-write into this game's directory and set CARGO_TARGET_DIR there. This is a warm-cache build measurement. All Cargo commands use the requested DEVELOPER_DIR, SDKROOT and development trust.
2026-09-18T16:46:27Z native test iteration 1
2026-09-18T16:47:23Z browser proof iteration 1

- 2026-09-18T16:46:34Z (approximately, test start + reported compile duration) — Native test passed first iteration: 6.62 seconds compilation, 0.01 seconds test execution. Includes arrow/WASD parity, differently partitioned time, seed variation, top speed, braking, saved airborne continuation, approximately 1.2 m jump and no double jump.
- 2026-09-18T16:47:40Z — **Browser proof passed on first iteration**, zero failures, **16.948 seconds** including compilation, three Chrome sessions, screenshot and child cleanup. GPU/web Cargo phases: 3.00 / 2.19 seconds. Timestamp comes from artifact mtime (09:47:40 PDT converted to UTC).
- 2026-09-18T16:47:55Z (approximately) — One pixel inspection after passing: readable HUD/Resume, capsule, crates, unlit sphere and bright lit sphere visible. No visual correction needed.

## Three numbers

- **187 lines of game code**: 148 Rust lines in `logic/src/lib.rs` plus 39 lines of `app.contract`, including whitespace/comments after formatting.
- **Scaffolding: 0 authored lines.** Four existing generated host/GPU crates reused read-only: **82 lines** (16 Rust source/build lines, 66 manifest lines). The web+GPU subset is 45 lines. Game/logic manifests, proof, native test, launcher and README are additional files, excluded from game-code count.
- **Reading brief → passing proof: approximately 6 minutes 40 seconds** (estimated first read 16:41Z; pass 16:47:40Z). Strictly timestamped first explicit clock → pass: **3 minutes 20 seconds**. Start was not captured exactly; do not treat the approximate total as stopwatch precision.
- **Pixel inspections: 1**, after passing; zero before passing.

## What worked and what hurt now

- All six requested proof steps passed; no required step needed a substitute. W for exactly 90 ticks is asserted against `[0, 0.9, -5.3666644]` within 1 mm. Two complete input sequences match simulation snapshots and binary saves. E outside range fails honestly; walking to beacon 1 gives a lit component, typed publication and real `Beacons 1 / 3` live HUD. Emissive material is strictly between zero/full at +100 ms, exactly full at +1000 ms. Pause with W held preserves the complete simulation snapshot across +2 seconds. A mid-glow checkpoint with W held and Space queued continues in a fresh browser to identical simulation and complete save bytes. Screenshot captured the playing scene paused for inspection.
- Additional browser assertions cover proximity prompt, title keyboard focus, accessible names, capsule dimensions, player screen bounds, all-three victory, and Play again resetting the world/HUD.
- Generator/scope mismatch was the main friction: used the current generator's exported function with Bun module mock disabling only shell synthesis. Its real locked/offline metadata check succeeded; lockfile unchanged. No copied/reimplemented engine or shared-script edit. Existing shell type required renaming template `SmallGame` to `Beacons` and matching the manifest. Reading/build-boundary checking cost roughly three minutes before authoring.
- The inherited generator template's two nearby Spring beacons were adapted to the requested three positions and 0.5s Tween. Character defaults already match speed/acceleration/gravity/jump; no private movement integrator was necessary. Seeded `w.rand`, `Follow`, typed HUD publication and restart generation were directly usable.
- Actual **edit → compiled browser → automated verification/screenshot was 16.948 seconds**, with a warm APFS-cloned cache. Direct proof loop used; no dev server, hot-swap, physical input latency, scanout or frame pacing measurement. No native-host rendering parity claim. No exec-policy stall occurred.
- Browser stderr contains macOS keychain/encryption-unavailable messages and a repeated allocator-loading diagnostic. No page exception, console error or reported GPU error occurred; these Chrome diagnostics were retained in the transcript, not silently discarded. They did not stop any proof assertion.
- Glow proof samples the actual rendered material's emission. Screenshot shows the lit sphere white/bright; bloom is not part of this game's shading. Proximity uses XZ ground distance. Crates are scenery; no collisions or physics add-on. No textures or audio.
- `world().snapshot()` already excludes host/carrier fields, so exact comparisons require no game-private state filtering. Full binary-save equality also covers held input/queue/clock. Browser cleanup evidence reports `remaining: []` after all recorded descendants exited.

## Delivery and scope

Game source, manifest, proof, native test, launcher, README, logs, dist and artifacts are under `game/games/beacons/`; this diary is the only other authored path. Existing tracked files are unchanged. Four pre-existing untracked feel reports were present at start and untouched. No staging, commit, stash, clone, remote command, sub-agent or headed measurement. Root five checks were not run: the add-on is outside that workspace and the task restricts writes; targeted native test plus real browser proof ran. No engine, renderer, host or shared-script modifications.
