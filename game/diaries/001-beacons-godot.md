# Beacons — Godot diary

Brief: [Task 001](task-001-beacons.md). Rules: [diary rules](README.md).
Builder: Codex (GPT-6), no editor, no subagents. Godot 4.7.2.stable.official.ed1daf0bf, local macOS binary.

- 2026-09-17 15:48:26 UTC — Read brief/rules; started. Scope conflicts: diary requested outside sole writable project subtree. Asked for an exception; keeping diary here pending reply.
- 2026-09-17 15:50:58 UTC — Authored project and scene in text; world, native UI, movement and binary persistence in GDScript. Examined local Godot CLI help. No engine changes.

Implementation: Node3D meshes, analytic ground clamp and gravity in native fixed physics callbacks (no rigid-body simulation needed for pass-through scenery); InputMap, real Control/Label/Button UI, one directional sun, seeded RandomNumberGenerator, tick-driven material emission. Camera state and in-flight glow are included in binary Variant saves.

Friction and limits so far:
- Programmatic UI needs anchors, offsets, containers, focus and accessibility names explicitly; these are part of game code, not excluded scaffolding.
- Button Space activation conflicts with jump; consume Space in gameplay input so a focused Pause button cannot also activate.
- Saving just player position would omit velocity, camera lag, glow progress and pending commands. All authoritative state is captured; physical held keys are external input, released at the save boundary and reapplied by the continuation script.
- Compatibility renderer uses emissive material glow, not bloom; plain shading and one sun retained.
- Engine binary is unchanged. No packages installed or remote commands used.

- 2026-09-17T15:52:40.304231+00:00 — Import iteration 1: default user-data creation denied outside subtree; res:// is not accepted by CLI log-file. Redirecting both to absolute project-local paths.

- 2026-09-17T15:53:42.358615+00:00 — Headless proof iteration 1: 23/24 checks passed in 0.37 s. W ended at z=-5.366664 (analytic expectation -5.3666666667), glow=0.104 at 0.1 s, jump height=1.159875 m. Exact crate-transform equality after restore/restart failed: assigning Euler rotation to an existing basis preserves decomposed scale/roundoff. Reset now constructs the complete Transform3D from seeded values. Absolute custom user-data names are appended under Application Support, not honored as absolute; using a relative path back to the permitted project subtree. Import also attempted editor-settings writes outside the subtree, which sandbox denied; runtime requires no editor settings. Fix round 1.

- 2026-09-17T15:53:42.437161+00:00 — Run run-a: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:53:42.926059+00:00 — run-a: FAIL in 0.488 s; 25 Godot assertions.

- 2026-09-17T15:53:42.926271+00:00 — Run run-b: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:53:43.520554+00:00 — run-b: FAIL in 0.594 s; 25 Godot assertions.

- 2026-09-17T15:53:43.522740+00:00 — Run restored: mode=restore, headless=True, --fixed-fps 60.

- 2026-09-17T15:53:44.030773+00:00 — restored: FAIL in 0.508 s; 1 Godot assertions.

- 2026-09-17T15:53:44.032442+00:00 — same_script: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:53:44.033013+00:00 — fresh_process_continuation: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:53:44.033281+00:00 — Run screenshot: mode=screenshot, headless=False, --fixed-fps 60.

- 2026-09-17T15:53:44.094809+00:00 — screenshot: FAIL in 0.061 s; 0 Godot assertions.

- 2026-09-17T15:53:44.096816+00:00 — Aggregate proof FAIL: 1.659 s.

- 2026-09-17T15:54:38.212562+00:00 — User-data fix round 2: XDG_DATA_HOME is ignored by this macOS Godot build; runtime still attempts Application Support. Both complete-script snapshots and all three continuation snapshots are byte-identical. First real-window launch produced no stdout; checking return status and local crash report before attributing failure.

- 2026-09-17T15:55:21.605885+00:00 — Native screenshot diagnostic iteration 2: Metal/mobile also exits -6 with empty output. Local macOS crash report for the first launch shows SIGABRT in HIServices _RegisterApplication → NSApplication init, before game startup; failure is at application registration, not evidence of a rendering or scene bug. No sandbox workaround attempted. User-data redirects abandoned after the bounded attempts; report their stderr separately from successful gameplay assertions. Review found Control.position was being used as an anchor-relative offset; corrected to explicit offsets and added headless viewport-containment assertions. No pixels inspected.

- 2026-09-17T15:55:21.709535+00:00 — Run run-a: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:55:22.362915+00:00 — run-a: PASS in 0.653 s; exit=0; 28 Godot assertions.

- 2026-09-17T15:55:22.363182+00:00 — Run run-b: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:55:23.279850+00:00 — run-b: PASS in 0.916 s; exit=0; 28 Godot assertions.

- 2026-09-17T15:55:23.280029+00:00 — Run restored: mode=restore, headless=True, --fixed-fps 60.

- 2026-09-17T15:55:23.907738+00:00 — restored: PASS in 0.627 s; exit=0; 1 Godot assertions.

- 2026-09-17T15:55:23.908050+00:00 — same_script: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:55:23.908256+00:00 — fresh_process_continuation: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:55:23.908314+00:00 — Run screenshot: mode=screenshot, headless=False, --fixed-fps 60.

- 2026-09-17T15:55:23.975842+00:00 — screenshot: FAIL in 0.067 s; exit=-6; 0 Godot assertions.

- 2026-09-17T15:55:23.977571+00:00 — Aggregate proof FAIL: 2.269 s.

## Outcome and requested numbers

- Final validation: **2026-09-17 15:55:23.977571 UTC**. Proof steps 1–5 pass; step 6 cannot pass in this execution environment. Full passing-proof time is **not available**. Elapsed from reading the brief to validated steps 1–5 and the established screenshot blocker: **6 min 58 s**. Final aggregate run: **2.269 s**; it correctly exits 1 because the screenshot is missing.
- **318 lines of authored game source/config/scene**: main.gd 291, main.tscn 7, project.godot 20 (physical lines, including whitespace/comments). Executable gameplay GDScript alone: **291 lines**. Separately, proof.gd 190 and run_proof.py 75 = **265 automation lines**. No engine code counted or changed.
- **0 pixel inspections**. No image was produced. This is not a claim of visual polish.

Iterations: one headless import; one initial standalone full proof; first aggregate (two full runs, one fresh restore, one failed window); one headless user-data-path diagnostic; one alternate Metal window launch; final aggregate (two full runs, one fresh restore, one failed window). The timestamped entries above record each substantive run batch. Three window attempts total, then stopped. Gameplay correction rounds: one for seeded-transform reset, one for UI anchor offsets; no unbounded fix loop.

Final assertions: two complete runs each pass 28 assertions. W position is (0, 0.9, -5.366664), within 0.003 mm of the stated expectation. First beacon glow is 0.104 at 0.1 s and 1 at 1 s; HUD reads Beacons 1 / 3. All serialized world state freezes through 120 paused ticks. Fresh-process continuation is byte-for-byte equal to both originals, including camera basis/position, player velocity, scenery and glow. Full-script final states also match. Additional checks cover jumping, no double jump, all three beacons, replay and UI containment.

Limits and closest honest substitutes:
- Screenshot: **not passed**. Native Compatibility and Metal both terminate with SIGABRT before Godot prints its header. The macOS report identifies HIServices _RegisterApplication and NSApplication initialization; this establishes the launch stage of failure, not its deeper OS cause. See artifacts/window-crash-evidence.json and window-launch.txt. No fake headless render or hand-drawn substitute. Closest available evidence is the scene/material state plus headless UI rectangle assertions; it is not visual verification.
- User-data path: Godot attempts Application Support despite local save paths. Absolute custom names are joined under that directory; parent traversals are sanitized; XDG_DATA_HOME has no effect in this macOS build. The sandbox denies those writes. The initial editor import likewise could not save global editor settings. Gameplay runs still exit 0 and all actual save/log evidence is project-local. No HOME override, engine patch or permission workaround used.
- Accessibility: real Label/Button nodes, accessible button names and keyboard focus/activation asserted. No VoiceOver or operating-system accessibility tree inspection; not an end-to-end assistive-technology claim.
- Rendering: emission property is asserted at exact tick deadlines; actual framebuffer brightness/bloom is not. Camera smoothing state is tested by freeze and continuation comparisons, not by human feel.
- Determinism: demonstrated across fresh processes on this machine and engine build. Cross-platform/architecture equivalence and real-time frame pacing/input latency were not measured.

Files authored: project.godot, main.tscn, main.gd, proof.gd, run_proof.py, README.md, .gitignore, this diary. Godot also generated .godot/ metadata and script UID sidecars; proof evidence lives under artifacts/. All authored work stayed inside game/twins/godot/beacons/. The requested diary exception was never confirmed, so this copy remains inside the permitted subtree.

- 2026-09-17T15:58:47.788514+00:00 — Run run-a: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:58:51.756375+00:00 — run-a: PASS in 3.948 s; exit=0; 28 Godot assertions.

- 2026-09-17T15:58:51.761860+00:00 — Run run-b: mode=full, headless=True, --fixed-fps 60.

- 2026-09-17T15:58:52.152244+00:00 — run-b: PASS in 0.386 s; exit=0; 28 Godot assertions.

- 2026-09-17T15:58:52.152447+00:00 — Run restored: mode=restore, headless=True, --fixed-fps 60.

- 2026-09-17T15:58:52.530897+00:00 — restored: PASS in 0.378 s; exit=0; 1 Godot assertions.

- 2026-09-17T15:58:52.531310+00:00 — same_script: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:58:52.531600+00:00 — fresh_process_continuation: PASS, byte-for-byte full serialized state.

- 2026-09-17T15:58:52.531699+00:00 — Run screenshot: mode=screenshot, headless=False, --fixed-fps 60.

- 2026-09-17T15:58:57.868492+00:00 — screenshot: PASS in 5.337 s; exit=0; 24 Godot assertions.

- 2026-09-17T15:58:57.870838+00:00 — Aggregate proof PASS: 10.096 s.

## Orchestrator's addendum (Claude, 2026-09-17 15:59 UTC)

The builder ran inside a process sandbox that forbids registering a macOS application,
which is the whole cause of the screenshot failure above — not Godot's. Run outside the
sandbox, unchanged, `python3 run_proof.py` passes **all six steps in 10.1 s** (two full
headless runs of 28 assertions, the fresh-process restore, and the windowed screenshot,
`artifacts/playing.png`). For scoring, the sandbox friction (user-data path denials, the
window abort) is excluded; everything else in this diary stands. Wall time to a proof
that would have passed: **≈ 7 minutes**. (Diary moved here from `twins/godot/beacons/`.)
