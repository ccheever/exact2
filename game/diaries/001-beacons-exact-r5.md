# Beacons — exact2, fifth build

Brief: [task-001-beacons.md](task-001-beacons.md). Builder: Codex (GPT-6), no subagents. Engine sources are unchanged.

- Initial reads: repository rules, task/diary brief, current README, starter and greybox. **Timing mistake:** I did not capture wall time before the first reads. The first verifiable clock receipt is 2026-09-18 22:42:55 PDT / 2026-09-19 05:42:55 UTC, after reading had begun. An earlier draft used an estimated 05:38 start; removed because it was not measured. Brief-to-pass timing is therefore a lower bound, not an exact duration.
- 05:44 UTC (approximate), scope finding: literal `bun game/new.mjs beacons` writes shared `game/.shells/`, scans/normalizes other games and may write `game/Cargo.lock`. Those writes are explicitly forbidden. Used its current template with the same name substitutions, plus a nested Cargo workspace, instead of invoking the mutating starter. Generated shells, lock and target now live inside this game. This is build isolation scaffolding, not an engine fork. No shared scripts edited.

## Iterations and friction

- 2026-09-19T05:46:24.605489+00:00, iteration 1: first local-workspace logic test build started, cold target. Current starter Character/nearest/tick_end/restart APIs fit the game. Replaced starter Spring with finite Tween for the brief’s half-second glow.
- 2026-09-19T05:47:01.920252+00:00, iteration 1 result: logic test passed; Cargo reported 45.51 s cold compilation and 0.01 s test execution. Position was exactly Vec3(0, 0.9, -5.3666644). Seed variation, chunked seek, hop apex, no double jump and restore tested.
- 05:46:42 UTC, iteration 2: headless web proof launched, cold game-local target. A preceding shell redirection failed because artifacts/ did not exist yet; created the local directory and reran. This was my wrapper mistake, before any build. The proof uses the existing shared runner and driver unchanged.
- Scope observation: git status shows pre-existing game/engine/src/animation/tests.rs and four feel-result files outside my scope. I have not edited or staged those. No staging is performed because the task forbids writes outside the game/diary, including the shared index. Root five checks do not build this separate game workspace; validation stays on the authored game.
- 2026-09-19T05:48:25.852900+00:00, GPU web module cold compile completed in 47.70 s; the host/compiler stage is still running. Removed unused physics/audio profile entries from the game-local workspace (they produced warnings despite no game dependency), and formatted the two Rust files. No behavior change; this will require a warm final-artifact verification if the initial proof passes.
- 2026-09-19 05:48:58 UTC, FIRST PASS: all six brief steps passed on headless Chrome, plus victory/restart. Proof runner reported 136.533 s including cold builds (GPU 47.70 s, host/compiler 58.63 s). Independent expected W position matched within 1 mm; glow was 0.074074 at 0.1 s (first press takes effect at the next tick) and exactly 1 at 1 s. Two complete scripts matched entity snapshots and hash; fresh processes produced byte-identical continuation saves.
- Cleanup limitation: runner printed `SKIP descendant process audit: ps stalled; carrier close still awaited every recorded host process.` Recorded carriers all exited, remaining=[]; no claim of a successful descendant audit. No process was killed by name or by an unowned PID. Chrome logged keychain/encryption diagnostics, but no page/engine exceptions.
- 2026-09-19T05:50:14.113032+00:00, iteration 3: warm proof launched against final formatted source and cleaned local profiles. This is final-artifact validation, not a gameplay fix round. First screenshot inspection (one total): 3D capsule, lit sphere, field, crates and readable UI visible. The player overlaps the first beacon in this proof screenshot because the route stops at its centre; scenery is intentionally non-colliding.

## Results and three requested numbers

- **185 lines of game code:** 137 lines under logic/src/ plus 48 lines of app.contract, counting physical lines including blanks/comments. One Rust source file and one Contract source.
- **Scaffolding separately:** 13 Rust lines across seven synthesized GPU/host/build entry files. Six Cargo manifests (the four synthesized shells, logic manifest and local workspace) total 151 lines. app.json is generated metadata; no hand-authored renderer or host. The nested workspace/lock are additional scope-isolation cost.
- **Wall time from brief to first passing proof: at least 6 min 03 s.** This is the measured interval from the first clock receipt at 05:42:55 to 05:48:58 UTC; initial reading time was not captured and cannot honestly be supplied. Cold proof itself: 136.533 s.
- **Pixel inspections: 1** after the first passing proof, zero required to debug it.
- Automated proof: 104 lines; host-free test: 50 lines. No game/Contract correction was needed after a failing assertion; there were no failing gameplay assertions.

## Findings and limits

- Character, named nearest query, Tween, Follow, live pause, restart edge and typed publication expressed the game directly. Input/clock/save helpers in the driver were sufficient; no bespoke simulation adapter. UI is 48 lines, with focus/accessibility assertions through real tree reads.
- The task-scope conflict in the starter cost a local workspace manifest, lockfile and cold compilation. The shared scripts did work with this isolated workspace; their source was not changed.
- Glow is asserted from the actual Material emissive value, plus one screenshot; no pixel-luminance assertion claims bloom/photometry. Pause compares every entity and tick while velocity/jump are active. Save continuation checks both complete entity snapshots and complete saved bytes.
- No impossible brief steps or substitutes: all six pass on web. Linux/iOS and paranoid cross-host pin generation were not needed for this task and are not claimed. pins.json stays the starter’s empty file; the script asserts a numeric movement oracle and independent same-run/cross-process equality.
- No headed measurements, feel claims, textures, sound, engine changes, commits, staging, stashes, clones, remote commands or subagents. No watcher was launched: the observed edit-to-verification loop is the proof command below, not a claimed ~20 ms dev-reload measurement.

Evidence retained inside the game: artifacts/beacons.png, iteration-1-web.log, iteration-2-web.log, proof.txt, replies.json, summary.json, checkpoint.world, original.world, restored.world and process-cleanup.json. Generated .shells/ and target/ remain game-local.

## Final receipt — 2026-09-19T05:51:41.400632+00:00

Final formatted artifact: **PROOF PASS beacons web, zero failures, 37.651 s**.
Warm GPU rebuild 2.96 s; host build 0.24 s. The rest includes packaging/optimization,
three fresh Chrome sessions, all assertions and cleanup. This is the actual measured
edit → automated screenshot/verification loop; no separate watcher latency was measured.
Clippy for beacons-logic/all-targets with -D warnings passed in 3.37 s; rustfmt check
passed. The earlier simulation test passed without source behavior changes afterward.
All source files are under 1,500 lines. The shared staging-based caps command was
not run because the index is outside the authorized directory.

Both web runs awaited and closed their recorded carriers (remaining=[]). Their
optional ps descendant audit timed out; that limitation remains, rather than being
reported as a passed audit. All command sessions launched by this builder completed.

During the run, QUEUE.md and game/bench/README.md also acquired concurrent changes;
I did not touch them. Authored writes are only beacons/ and this diary.

The game and all six required proof steps are complete. No engine fix was required.
The notable costs were scope isolation/cold builds and incomplete process audit
observability. The missing initial timestamp is my measurement error.
