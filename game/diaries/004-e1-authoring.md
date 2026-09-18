# E1 — the measured Lanterns authoring burdens

The I1/I2 results do not establish an authoring speedup. E1 addresses their fixture
maintenance, recurring route stalls and first-screen discovery. It changes no
phase vocabulary, gameplay, movement/physics constants or existing game pins.

`bun game/proof.mjs lanterns --repin` stages candidates, runs the normal Rust suite,
then the capture/I3/timing scripts in Save and FreshGame, checks exact checkpoint
bytes across all three modes, and runs all three Linux host proofs. Only then does
it atomically replace `pins.json`, printing three old → new hashes. It refuses an
incomplete, extra, oversized or divergent candidate set. Other games explicitly
refuse this producer; this slice is the measured Lanterns consumer.

The historical source, scene, old world bytes, I3 checkpoints/605 continuation rows
and 612-entry migration inventory live under `verification/lanterns`. The old-byte,
compiled-edit, renderer-carry, clock/input and live-terminal assertions remain.
Each fixture's protection is documented there. Renderer carry tests use the same
frozen game. The inventory records an engine migration and is no longer an author
maintenance task. Current-game route/rule tests still run on the actual game.

New APIs:

- `Sim::move_to(&mut self, entity: &str, target: Vec3, tolerance: f32, speed: f32)
  -> Result<(), String>` and `Sim::route_diagnostic(&self, entity: &str,
  from: Vec3, to: Vec3) -> Result<String, String>`.
- JS `session.world(name).moveTo(entity, [x,z], {tolerance:0.18, speed:4.5})`.
- `layout` with `to` now also returns `route`; JSON `from`/`toward` supports point
  probes. The route object contains `blocker` (name, position, bounds) and
  `nearestClearSide` (side, position), with null for absent evidence.

Movement is at most 160 bursts / 1,760 ticks. The diagnostic uses the existing mesh
BVH, one nearest-blocker scan and four side rays, sharing one million visits over
at most 262,144 slots. Oversized worlds, exhausted budgets and overflowing segments
refuse. Rebuild is O(N log N). Helpers probe 0.65m above the starting Transform for
Lanterns' 1.3m controller; side rays use a 0.4m margin. These are geometry diagnostics,
not a path planner or swept-body clearance promise.

The worst-shape regression uses 200k interleaved/churn entities, checks nearest
blocker order, removes blockers as negative controls, and crosses the slot limit.
Repin tests reject empty/incomplete, extra, oversized and mismatched evidence.
Both the real Sim route and Linux JS driver stall report `sign-board`, position
`[3,1.55,10]`, bounds approximately `[1.75,1.1,9.93]`–`[4.25,2,10.07]`, and the clear
`minX` approach near `[1.35,1.3,10.47]`. Existing capture and terminal assertions pass.

## Task A scratch acceptance

Starting at E1 `85b9a78`, manually added the specified platform at `[-8,.6,7]`, size
`[3,1.2,3]`, and lantern-13 at `[-8,1.2,7]`. The original twelve were preserved.
The ordinary-input capture route proves twelve no longer wins, remote act cannot
light the new lantern, jumping reaches its top, thirteen wins, and save/replay
retains the lit lantern. The existing terminal recapture assertion was retained.

**One `--repin` invocation; six files, +102/−11 lines** (baseline: seven files,
+160). Five authored/test/UI files plus `pins.json`; zero manual fixture repairs.
The run passed **35 Rust tests, 3 ignored diagnostics, and 35 host assertions**.
All three host proofs passed and their final states agreed. Proof wall seconds
including builds: Off **58.054**, Save **12.821**, FreshGame **12.745**. These are
verification times, not an authoring-speed trial or a comparison of simulation cost.

Scratch-only pins at ticks 0/60/180 changed to `78978838421240a2`,
`afb5edfe0cdda004`, `4133ce85bd99c9a4`; shipped twelve-lantern pins are unchanged.
The diff, command output and summary are retained under
`~/lanes/gamenext/scratch/E1/`. The scratch worktree and its generated outputs were
removed; it used the shared Cargo target and had no private target directory.

## Integration and verification

Merged trunk `985c600e62293e8f390cf60012ede820e74b91ed`. The capture-test conflict
retains both C1's live-terminal/fractional-clock coverage and E1's stall coverage.
The initial full build met a stale ignored asset-fixture directory without its
ownership manifest; it was preserved in scratch and the normal bake regenerated
it. No fixture assertion was disabled or relaxed for this environment.

The first repin implementation unnecessarily ran transient cache tests in Save
mode. Their failures left pins unchanged. The final runner retains those assertions
in normal mode and runs the actual scripted evidence in all three modes.

Final game-workspace Rust run: **658 passed, 2 failed, 26 ignored**. The two
failures require a graphics adapter: `skinned_normal_is_inverse_transpose_under_scaled_rotated_joints`
and `first_presented_fox_matches_current_pose_in_fox_rectangle`. Their assertions
remain intact. Game workspace Clippy passes with `-D warnings`; both workspaces'
formatting, caps and boot pass (two reachable JS modules, one Wasm reference).
Bun: **59 passed, 2 failed** — generated-game web proof cannot launch Chrome,
and feel lacks prebuilt 60/120 Hz artifacts. The focused proof-driver suite passes
**31/0**, including repin refusal and bounded-stall negative controls.

Root build/test/Clippy were attempted and stop in TypeScript app bakes because the
lean Hermes executor is unavailable. This is not a green root suite. GPU pixels,
Apple execution and real browser behavior are unverified on this Linux box.

The final Rust sweep also caught and fixed missing-pose handling in the added
layout response. Three strict engine/surface wire goldens gained only the new
`route` field; every prior field and whole-response equality assertion remains.
No saved position, game hash, pose pin or historical fixture byte changed.

The final-code Linux sweep passes **18/18 runs, 522 assertions, zero failures**.
Every final hash/tick/publication/journal comparison agrees across modes. All
recorded host children exited. Per-mode wall seconds include builds:

| Game | Assertions Off / Save / FreshGame | Seconds Off / Save / FreshGame |
|---|---|---|
| lanterns | 11 / 12 / 12 | 49.000 / 11.626 / 11.759 |
| beacons | 57 / 58 / 58 | 42.229 / 2.671 / 2.731 |
| greybox | 68 / 69 / 69 | 21.967 / 2.197 / 2.209 |
| asset-fixture | 19 / 20 / 20 | 21.617 / 1.489 / 1.512 |
| skinned-fixture | 12 / 13 / 13 | 22.766 / 1.193 / 1.158 |
| cubes | 3 / 4 / 4 | 20.682 / 2.716 / 2.730 |

Command receipts, full logs and negative controls remain in `scratch/E1`; the
initial failures and both post-merge proof sweeps are retained separately.
