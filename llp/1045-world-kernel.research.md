# LLP 1045: The world kernel — built, verified on four hosts, and parked

**Type:** Research (record)
**Status:** Parked — not landed on `main`, by agreement of the author and a fresh maximum-effort judge
**Systems:** `world/` (`exact-world`, `exact-world-derive`), `game/world-adapter`, `game/world-motion`, `game/games/tally`, GPU module ABI (`gpu/`), web / Apple / Linux hosts
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-20
**Related:** LLP 1041 family (the agent-native game engine add-on), LLP 1014 (GPU surfaces), LLP 1012 (the eight agent operations), LLP 1026 / 1030 (modules by digest)
**Number:** tentative — this record lives on the parked branch; renumber if `main` takes 1045 first.

## Summary

Charlie asked, 2026-09-20, for a small, polished world kernel to enter core only if its author and a fresh
maximum-effort session both agreed it belonged there, with everything else kept as separately installable game modules,
and startup "really, really, really fast". The kernel was built and judged eight times. Both parties agree the kernel is
good work. Both parties also agree it should NOT land on `main` now: nothing on `main` needs it, and its first consumer
depends on roughly 7–8 thousand lines of core seams that exist only on the unlanded engine line. This record says what
exists, what was measured, what is owed, and how to resume.

## What exists (branch `core/world-final`)

- **`world/` — `exact-world`.** A passive, deterministic state kernel plus a small tick driver: typed entities,
  components and resources over paged storage (a flat presence bitset and lazy 64-value chunks; handwritten `unsafe`
  confined to `world/src/storage/`), one state grammar (`Data`) with a bounded binary codec, a streaming hash and bounded
  JSON inspection, `Parent` ownership with ordered reads (`children`, `parent`), a structural journal with subscriber
  cursors, a saved world journal kept apart from an unsaved session journal, fixed ticks under a caller-owned clock
  (`Sim`, `clock_ms`), input at tick boundaries, atomic save / restore / `carry`, and `Paranoid{Off, Save, FreshGame}` —
  save and reload on every tick as the functional proof of continuation. `Game::setup` and `Game::tick` return
  `Result<(), DataError>`: on wasm32 a panic aborts the module and every world in it, so authored code propagates
  refusals with `?`, and a failed tick leaves an inspectable, unpoisoned world that a restore recovers. Dependencies: its
  derive, `libm`, `ryu`. 7,436 of a 7,500-line ceiling (Rust + README + manifests). No ordinary app links it.
- **`game/world-adapter`.** `WorldSurface<G>`: a device-free `exact_gpu::Surface` over the kernel's public API. Its
  checkpoint is exactly `Sim::save()`.
- **`game/games/tally`.** A 15-entity card game whose UI is ordinary Contract over values the world publishes. It is the
  nonspatial consumer the admission test asked for.
- **Hosts.** A surface module is one of two kinds, decided by its ABI: a DEVICE module (exports `gpu_load`) behaves
  exactly as before this work; an OWNERSHIP-ONLY module (no `gpu_load`) is created headless, driven per frame by
  `gpu_advance` through the defaulted `Surface::advance`, and never meets a device. `host/web/gpu-glue.js` differs from
  its pre-change version by 55 added lines and one changed line, all guarded by `gpu.gpu_load`. Apple loads such a module
  with no Metal device. Linux already loaded modules headless.
- **`game/world-motion`.** Saved `Spring` and `Tween` over `exact-motion`, moved OUT of the kernel in round five.
  `motion/` is byte-identical to the branch point. The module has known numerical defects (below) and is not part of
  what would land.

## What was measured

| | |
|---|---|
| Empty world | 1 allocation / 24 bytes |
| 100-entity construction | 21 allocations / 41,880 bytes |
| First publishing tick | 2 / 568 |
| 1,000 sparse-edit ticks at 200,000 entities; 1,000 input-heavy ticks; 1,000 ownership reads | 0 / 0 each |
| 10 KiB restore / carry | 80 / 46,760 · 84 / 46,766 |
| Query scan, dense / sparse | ≈ 1.3 / 3.0 ns per row (the engine it was cut from: 1.2 / 2.8) |
| Run scan, dense / sparse | 0.70 / 2.9 ns per row (engine: 1.03 / 57.8) |
| World activation (create, bind, first tick) | 0.16 ms on Linux, 0.07 ms on macOS |
| Web, cold: first contentful paint → first tick | 122 ms → 130 ms (module preloaded to overlap `app.wasm`) |
| macOS, from process entry | plan decoded 201 ms, first frame 230, module loaded 280, first tick 280 |
| Device-free Tally module (wasm, `-Oz` + `wasm-opt`) | 366 KB raw / 153 KB gzip; kernel floor for a minimal game ≈ 222 KB / 87 KB |

These numbers did not move across the last ten lanes. Tally's tick hashes and continuation bytes are IDENTICAL on web
(headless Chromium on Linux x86-64 and real Chrome on macOS arm64), Linux, macOS and the iOS simulator, in all three
paranoid modes. Miri (x86-64 and i686, Stacked and Tree Borrows) passes the storage, journal and decoder tests, including
the zero-sized-row test a reviewer believed was unsound.

Loading untrusted saves is bounded and the bound is stated in bytes: forged inputs are loaded under a counting allocator
and peak requested bytes must stay within the caller's budget plus 8 KiB. One rule prices an allocation on both the save
and the load side (`max(portable units, native size)`), so a world that saves will load. The derive refuses at compile
time a type whose native size exceeds four times its saved size plus 64 bytes; cache-line and SIMD alignment pass.

## Two things that were learned the hard way

**The review-hardening is not cruft, and a rewrite would not remove it.** An experiment branch (`core/world-lean`)
replaced the admission accounting with a single resident-byte budget. It removed 59 production lines and 2.5 KB of wasm
(1.1%), made a 4.9 MB restore 6.5% slower, and lost a real guarantee. The kernel is near-minimal for its requirements;
the lever is the requirement (memory-bounded loading of untrusted saves), which was kept. The experiment's measured-peak
tests then found a genuine hole in the mainline (1.65 GB allocated under a 256 MiB budget), fixed in nine lines.

**Builder lanes cannot see core regressions.** The fleet builders have no WebGPU Chrome, no Apple SDK and no lean Hermes,
so no lane ever built a core app's web bundle or ran the root workspace checks. The first Mac run found that Caltrain's
GPU module did not compile; a read of one lane's diff found it had deleted the `/__gpu` smoke beacon; three independent
reviews found the first host design had changed readiness, shader ordering and recovery for every app with a canvas. The
design was replaced (device modules untouched, ownership-only modules additive) rather than patched.

## Why it is parked

The kernel, adapter and Tally sit on core seams from the unlanded engine line: a `Surface` that owns state, the
`exactSurface` Contract source and the runner's interception of it, plan v5 named surface arguments, `button action=`
inside a canvas, agent routing into surfaces, save/restore carriers, the Linux headless module loader, the per-app web
surface module. `main` has none of them. The minimal closure is about 7–8 thousand added lines including tests, touches
five files at the 1,500-line cap, collides with `main` on kernel schema IDs 75–78, and rebakes every plan. Landing it
would be about a week of work for one card game. The eighth judge, asked directly: *"I would park this branch with its
evidence and stop broad review rounds… The kernel is coherent, useful work. The likely 'junk' would be prematurely
carrying its surrounding apparatus onto `main`."* The author agrees.

## How to resume

Resume when the engine line's own landing, or another real application, needs surfaces that own state. Then:

1. Route B: a fresh patch series of only the minimal seam closure onto the `main` of that day, then the kernel, adapter
   and Tally, landed as one stack within a couple of days so no seam sits without a consumer. Put Tally at `apps/tally`
   with hand-written shells (as `apps/weatherlight` does); that drops the manifest/shell-generator seam and most of the
   proof apparatus. Do not carry `game/world-motion`, game-specific shell/proof tooling, 3D-only `gpu/` additions or
   lane diaries.
2. `rules/NOT-DOING.md` on `main` has no game or engine admission; one must be written by Charlie with a named take, and
   the proof apparatus needs his explicit approval ("agents add no apparatus").
3. The engine re-homes onto this kernel and deletes its duplicate state implementation BEFORE the engine lands.

## What is owed

- The judge's three open conditions: the 100 ms cold-interactive target (the world costs under 0.1 ms; the time is host
  boot — on macOS, 200 ms pass before the plan is decoded and ≈ 50 ms of module verification runs after the first frame
  and could start at process entry) or an owner-approved trade; a failed-tick recovery proof on Apple; ordinary-device
  shader / recovery / hot-swap evidence on real hardware for the final build.
- Deferred kernel items, recorded with file and line in `QUEUE.md`: the canonical comparison on restore allocates a
  second output buffer outside the load budget; spawn scans retired slots; first insertion at a high slot grows the
  directory by the prefix; `sample`/`settle` membership tests are not charged; the input-edge algorithm (67 ms worst
  admitted tick); derive diagnostics lack field spans; `publish_batch` is a patch with no `unpublish`.
- `game/world-motion`: a finite saved spring can sample to −∞; slow damped springs report no deadline; a tween deadline
  can saturate. The core `motion/` crate, as `main` has it, has an overdamped slow-root cancellation and an undamped
  transient-crossing settlement; neither was touched here.
- Not caused by this work, seen while verifying: three focus checks in `smoke web`, the macOS held-pointer smoke abort
  and five iOS focus/editor checks fail at the branch point too; the engine line's Lanterns imports a generated,
  untracked `.scene/scene.contract`, which fails `contract --test fmt` in any fresh checkout; the engine line's Linux
  display loop never synchronises surfaces (they run only under the agent/headless carrier).
