# LLP 1041: An agent-native game engine on exact2 — the analysis, in outline

**Type:** Research
**Status:** Draft (a record of a conversation with Charlie, 2026-09-17; names no implementer, and is deliberately **not** linked into `llp/current/`, which is at its cap of 15. **Charlie ruled §7's questions the same day:** probably a lane, the take still unnamed (§8 recommends one); a separate directory in this repository; 3D first, 2D later; the Godot twin is worthwhile; not Castle's, for now)
**Systems:** GPU module (`gpu/`, the `Surface` trait), Runner (the clock, the data seam), Agent API (the eight operations), Delivery, the hosts' input paths; a proposed add-on workspace outside the core
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-17
**Related:** `rules/NOT-DOING.md` §Runtime (the GPU door), §Agent API (eight; a ninth replaces one; record/replay refused), §Motion (no gesture arena); `CLAUDE.md` (optional capability is a separate artifact, never a cargo feature on a core crate); LLP 1009 (the canvas, `Surface`, wgpu on every host), LLP 1014 (canvas children; D5 placements), LLP 1012 (the agent API; §2 the clock), LLP 1027.000 (explicit time and randomness), LLP 1027.002 (worker placement), LLP 1029.000 (the replaceable module), LLP 1030 (delivery), LLP 1035.003 (contact phases). Sub-documents: LLP 1041.000 (the test game), LLP 1041.001 (the agent interface), LLP 1041.002 (what the two engines can share)

## Summary

Charlie's question: app code is Rust, WebGPU is a first-class subsystem —
what else would an agent-native game engine want, and could exact2 be as
good at it as Godot? The answer this outline records: **yes for the games
an agent can author end to end, on the platforms exact2 already has; no for
Godot's whole surface, and most of that surface is an editor nobody here
wants.** exact2 already owns the parts of "agent-native" that cannot be
retrofitted — a seekable clock, one agent protocol on every host, a headless
host, a parity oracle. What it lacks is ordinary engine plumbing, most of
which exists as Rust crates. Charlie's direction the same day: the engine is
an **add-on**, not something built into the core by default (§5).

Nothing here is measured. Every size, pace and frame-rate claim is an
estimate and is marked so.

## 1. Findings — what already exists that Godot lacks

- **F1. A seekable clock the agent holds** (LLP 1012 §2). `Frame::now_ms`
  is already that clock, and in agent mode the GPU module "never
  reschedules itself; `clock` asks it for a frame". A fixed-timestep
  simulation under it is a pure function of (seed, inputs, clock). Godot's
  `_process(delta)` is wall time and its physics is not cross-platform
  deterministic.
- **F2. One agent protocol, the same on every host, plus a headless Linux
  host** (LLP 1012, LLP 1015). Godot has `--headless` and text scenes, but no
  stable way to observe a running game; agents screenshot it. One gap: the
  Linux host "does not load" the GPU module yet (LLP 1015 Related, §7), so
  a canvas there is its box and its children, and a world on Linux waits on
  that owed piece.
- **F3. The parity-oracle habit** (LLP 1002, LLP 1009 D1): one
  representation, two executors, held together by fixtures. "Renders the
  same everywhere" is a check here, not a hope.
- **F4. A ~20 ms restart that carries state** (LLP 1007 §6), shaders
  validated at build (LLP 1009 D5), signed delivery (LLP 1030), app logic as
  a replaceable wasm or native module (LLP 1029.000).
- **F5. Real text, layout and accessibility, inside the canvas** (LLP 1014):
  a canvas holds Contract children, and D5 gives each child a homography and
  a depth, hit-testing included. Game UI — every engine's weakest part — is
  the app engine, already built.

## 2. Findings — what is missing: engine plumbing

- **F6. A home for the simulation.** `Surface` is `bind(inputs)` and
  `render(frame)`; values flow runner → surface only. A game inverts the
  app's shape: the simulation is the app and the UI is the overlay.
  The open design question is where the world lives and how it reaches the
  renderer at 60 Hz without crossing plan commit → side-output → batch.
  LLP 1041.002 F3 proposes removing the question rather than answering it:
  simulation and renderer in one module, one memory.
- **F7. Continuous input.** Held keys, stick vectors, pointer lock,
  multi-touch, sampled per tick. Handlers today are discrete events.
- **F8. Audio.** None exists anywhere in the tree.
- **F9. An asset pipeline.** Textures (KTX2/Basis), glTF, audio; baked,
  digest-addressed. Delivery's asset row is the start.
- **F10. A rendering layer over wgpu.** 2D batching is a few thousand
  lines. 3D is §4.
- **F11. Simulation libraries**, assembled, not written: an ECS
  (`bevy_ecs` standalone or smaller), `rapier` (has a cross-platform
  deterministic mode), `glam`, an animation runtime (`ozz-animation-rs`
  class).

## 3. Findings — what is missing: the agent-native part

The test of "agent-native": an agent closes the loop without a human's eyes.

- **F12. The world answers `tree`, `state` and `layout`** — named entities,
  typed components, world and screen positions. Pixels are the fallback.
  (LLP 1041.001.)
- **F13. Determinism end to end**: fixed timestep, seeded randomness as an
  explicit input (LLP 1027.000's rule, extended), input as a clock-stamped
  script. The smoke script then *is* the replay — reproducible bugs, golden
  traces, bisecting, thousands of headless ticks per second for balance and
  fuzzing — with no ninth operation and without the record/replay
  `NOT-DOING.md` refuses.
- **F14. A world hash**: one number that says four hosts simulated the same
  game. Simulation state is held bit-exact; pixels are held to a band.
- **F15. Save states are save games**: world serialization the game needs
  anyway; restoring one is session setup, not a drive.
- **F16. Observations an agent can use**: a filmstrip (the driver tiles
  `clock` + `screenshot`), debug views (ids, depth, collision, top-down),
  audio as journal lines because an agent cannot listen, performance as
  numbers.
- **F17. Content an agent can make**: procedural meshes, SDFs, vector art,
  shaders as art, synthesized audio, scenes as diffable text checked at
  build. Agents are bad at Blender and good at code.
- **F18. GPU on the fleet.** The Hetzner builders have no video group and no
  sudo; headless readback there needs software Vulkan (lavapipe) or stays
  on the Macs.

## 4. Findings — feasibility against Godot

- **F19. Not chased:** the editor (most of Godot); console exports; dynamic
  global illumination (research-grade; Godot is replacing its own); ten
  years of low-end Android and broken-driver fixes.
- **F20. Skeletal animation is small** (corrected in conversation — first
  called out of reach, wrongly). Skinning is a vertex shader over a joint
  buffer; glTF defines skins and channels; sampling and blending is
  hundreds of lines; a tree (blend spaces, additive layers, masks, a state
  machine with crossfades, root motion) is a few thousand; two-bone and
  FABRIK IK are small. Godot's AnimationTree is big because of its editor.
  Under the seekable clock a pose at time *t* is a pure function — a bone
  position is an assertable number. The real costs: retargeting, and content
  (procedural, mocap libraries, text-to-motion — not keyframing).
- **F21. A comparable renderer is feasible; "years" was a human-team
  prior** (also corrected). PBR, clustered lights, cascaded shadows, IBL,
  SSAO/SSR, bloom, TAA, fog, decals, GPU particles, instancing are
  published; Filament's PBR document is nearly a spec; Bevy is a readable
  permissive implementation on the same wgpu. Estimate: tens of thousands
  of lines, weeks to months at this repo's pace.
- **F22. The renderer's real problem is verification, and it has an
  answer in this repo's own shape:** a slow reference path tracer as ground
  truth, the realtime result held to it within a band; white-furnace energy
  tests as numbers; Khronos's glTF sample assets and viewer for materials.
  The path tracer doubles as the lightmap baker.
- **F23. What stays expensive:** the device long tail (shrunk by
  `NOT-DOING.md` excluding Android and Windows — which also shrinks the
  audience); performance at scale (culling, LOD, streaming, GPU-driven
  draws — measurable, so iterable); build-time shader enumeration
  (LLP 1009 D5 — a constraint on the material system, and the cure for
  pipeline-compile stutter); the default look, which needs a human eye.
- **F24. On the web the bar is lower.** As far as known, Godot's web export
  is its WebGL2 Compatibility renderer only; a WebGPU-first renderer with
  compute passes it early. Unverified against Godot's current release.
- **F25. The larger gap to "as good as Godot" is hosts, not rendering:**
  no Windows, no Android, no consoles.
- **F26. Embedding Bevy's renderer** is the shortcut: it brings ECS
  coupling, a wgpu version pin, nondeterminism and tens of megabytes of
  wasm. Read it; do not depend on it.

## 5. The shape: an add-on (Charlie, 2026-09-17)

"This game engine should be implemented as a sort of extension or add-on to
exact2, not something we build into the core by default." That is the rule
`CLAUDE.md` already states for optional capability, and the GPU module is
the precedent. Read concretely:

- The engine is **library crates in their own Cargo workspace**, consuming
  exact2 by path the way weird-castle does. No core crate depends on it; an
  app without it carries nothing; exact2's five checks never compile
  `rapier` or a renderer (wgpu's cold compile is already ~35 s, LLP 1009 §3).
- A game is an ordinary exact2 app: Contract for every screen, a manifest, a
  bake, delivery — plus **one game module**, loaded after first pixel as
  the GPU module is, so the title screen is up at app speed while the engine
  streams in.
- The core gains **seams only**, each small, each with a plausible consumer
  that is not a game, or it is refused. LLP 1041.002 lists them: three new
  (its §3 — raw input to a canvas, a surface posting a message, an agent
  export on the module), and three the corpus has already decided and not
  built (its F13–F15), for which a game would be the consumer.
- **Where it lives (ruled 2026-09-17, §7 Q2): a separate directory in this
  repository**, with its own `[workspace]` and absent from the root
  `members` list — the library crates, Lanterns, and the Godot twin
  together. The root workspace lists its members explicitly, so nothing
  else has to change for the five checks to stay blind to it.

## 6. Where the rules stand

`NOT-DOING.md` binds and none of this is on the doing-list. The GPU door is
open in a declared shape (LLP 1009, LLP 1014); a game engine walks further
through it. The rules' own pattern is the next step if there is one: **no
engine spec — one real small game as the consumer** (LLP 1041.000), with
Charlie naming what comes off. "A spec needs an implementer and a date":
these four documents are a research record and two unassigned RFCs, written
because he asked for them.

## 7. The questions, as Charlie ruled them (2026-09-17)

1. **Is this a lane, and what is the take?** Probably a lane. The take is
   unnamed — he asked what should come off; §8 is the recommendation, and
   until he names one nothing moves in `NOT-DOING.md` and nothing is built.
2. **Where it lives: "separate directory here."** A directory in this
   repository with its own `[workspace]`, outside the root `members` list,
   so `cargo build --workspace` and `cargo test --workspace` never see it.
   `caps` still scans it: the 1,500-line rule holds there too.
3. **"3D first, 2D later."** LLP 1041.000 stands as written.
4. **The Godot twin is worthwhile.** Apparatus, approved by name: the same
   game in Godot 4, so "as well as Godot" is numbers. LLP 1041.000 §6a.
5. **Not Castle's, at least for now** — a separate idea. Nothing here is
   shaped for it.

## 8. The take — a recommendation, his to accept or replace

The lane is months of work, so the take should be the one thing on the
doing-list of comparable size.

**Recommended: Messages' decorative parity comes off outright, not
deferred.** Tapback and Reply artwork, glass and material matching,
animation-timing and motion matching against native iMessage, Translate,
the emoji and sticker picker, contact-details fidelity, date-heading and
balloon raster matching — most of `QUEUE.md` from "Messages iPhone parity"
down. Why this one:

- It is the largest open-ended sink on the list and has no finish line; its
  own record shows the three-round stop firing repeatedly (timestamps, the
  inbox title, thread deletion, adaptive images, the glass button).
- Its oracle is native iMessage's pixels, which is not the oracle the rules
  name. What it taught the platform is already harvested as LLP 1035 and
  its sub-documents, the windowed list, and the Snapback4 consumer.
- It has already been the deferred half of a trade (2026-09-14, behind list
  memory). Deferred work is still owed; this makes the trade real.
- What **stays**: Messages as the Snapback4 and list-memory consumer, its
  functional navigation, editing and ownership fixes, and everything in
  LLP 1035 that is a rule rather than a pixel.

**A matched second trade, smaller:** the Linux host's *presentation* lane
(the KMS surface, evdev and libinput, VNC polish) comes off, and what that
host owes becomes loading the module with `canvas` (LLP 1015 §7) — which
LLP 1041.001 D7 needs. Linux stays the headless fleet host; it stops trying
to be a display.

**Considered and not recommended as the take:** LLP 1024 (native modules)
and LLP 1013 (view transitions) — each has already been spent as a take and
neither is being worked; removing them frees nothing. List memory, the
Router (assigned, in flight), the LLP 1035 rules, delivery's owed
correctness items and the continuous release loop are what the product is;
none should pay for this.

**The working set** is 15 of 15. When the lane starts, one link goes in —
LLP 1041.000, the consumer — and one comes out; LLP 1027.002 landed
2026-09-14 and is the natural one.

## Confidence

High on F1–F8 and F12–F15: read from the tree and the specs on 2026-09-17.
Medium on F20–F23: standard technique, sized by analogy, nothing built.
Low on F24 (Godot's web renderer, from memory) and on every pace estimate.
No number in this document was measured.

## Development experience follow-up (2026-09-18)

[LLP 1041.006](1041.006-game-development-experience.rfc.md) proposes bounded bug captures, dependable state-preserving reload, diagnostics and declarative scene authoring after the Lanterns comparison. It is a requested Draft RFC, not an implementation or scope-policy amendment.
