# Task 001 — scored

Two judges (sol `gpt-5.6-sol` and grok, both at xhigh, blind to each other, the same
frozen checkout at 96fe3d2, told to be adversarial toward the home engine) read the
brief, the three diaries and the three implementations, and scored the rubric in
[README.md](README.md). They agreed on every number.

| | Godot 4.7 | three.js r186 | exact2 |
|---|---:|---:|---:|
| Small | 4 | 5 | **2** |
| Direct | 4 | 5 | **3** |
| Provable | 5 | 5 | 4 |
| Repeatable | 4 | 4 | **5** |
| Loop | 5 | 5 | **2** |
| Feel | unverified | unverified | unverified |
| UI | 4 | 5 | **3** |
| **of 30** | **26** | **29** | **19** |

Lines, as sol counted them (physical lines, no diaries/READMEs/generated files):
Godot 583 (318 game + 265 proof), three.js 362 (198 + 164), exact2 697 (258 game + 283
proof and tests + 156 host crates and manifests) — 896 with the build adapter.

## What decided it (both judges, same three)

1. **Loop.** A logic edit costs 63–73 s here (measured again by the orchestrator with
   the repo's own dev server: 63.4 s), because the bake watches the whole app
   directory and re-links the app wasm under fat LTO; the world's module alone builds
   in 3 s. Godot and three.js reload a script. *Wanted: rebuild only the world's
   module, swap it into the live canvas, carry the world across through `Data`;
   under 2 s.*
2. **The HUD bus.** One integer travels publish → JSON string → `message=` → a state
   → a TypeScript validator → a resource → text, and drags a JS engine into the app.
   *Wanted: a world's published record read in the Contract as a typed value, no
   module.* (Being built as `exactSurface("<name>")`, a third runner-owned source.)
3. **Size and the last proof step.** Four crates and two build scripts per game that
   a game should not author; save/restore not reachable through the agent
   (LLP 1041.001 D6), so step 5 ran in two native processes instead of two browsers.

Also named: `world.busy` by hand three times and a borrow that had to be dropped
first; `Mesh`'s docs were wrong about the sphere's radius and the capsule's height
(one wasted build); shadow acne on the ground in the screenshot, where three.js's
default-ish `normalBias` is clean; no accessible names asserted; `act` bound to
Enter as well as E is a focus footgun Godot's version avoided.

What the judges said to keep: the save (every field, held input and the queue, no
code), the hash agreeing between native and Chrome, the clock in the script's hands,
and `state world:<entity>` — "the one place exact2 is already ahead".

## What happens next

Each of those is a work item in the lane, in this order: typed publications (E1),
clean shadows by default (R3), save/restore through the agent plus the engine's
paper cuts (D6), the world dev loop (E2), generated host shells (E3). Then Task 001
is built **again, from scratch, by a fresh builder**, and scored again by the same
two judges. The bar is to beat 29.
