# LLP 1000: Exact2 Project Root

**Type:** Explainer
**Status:** Active (ratified by Charlie Cheever 2026-08-28)
**Role:** Root
**Systems:** All
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-28
**Related:** LLP 1001, LLP 1002, LLP 1003, LLP 1004, LLP 1005, LLP 1006, LLP 1007, LLP 0552, LLP 0491, LLP 0507, LLP 0486

## Summary

Exact2 is the second repository of Exact: a cross-platform application runtime in
which one Rust kernel computes layout, each platform renders natively, and Contract
is the authoring model. It exists because the first repository (`~/projects/exact`,
"exact1") became unshippable under the weight of its own apparatus — 8.2M words of
design documents, 675 checks, 450 scripts, a 956 ms JS boot — and because the
design that repository converged on (the Refresh program, LLP 0478–0566) is worth
building without the compatibility debt it accumulated getting there.

The rules are in `rules/RULES.md` and `rules/DEFERRED.md`; those two files bind
and this one does not. Read them first.

## How the corpus is laid out

- **`llp/research/`** — documents imported verbatim from exact1. They are the
  design record: how a thing was decided and why. They are **research, never
  authority** (`rules/RULES.md` §Agents): cite them for how something worked,
  never to block. Their numbers are exact1's numbers (below 1000) so a citation
  like `LLP 0491 §3 WS-A` still resolves by grep. Their `Status:` lines describe
  exact1, not this repository.
- **`llp/1000+`** — documents written for exact2. Numbering starts at 1000 so an
  exact2 number can never collide with an imported one; the boundary is the
  authority boundary. A spec here exists only if it has an implementer and a date
  (`rules/RULES.md` §Scope).
- **`llp/foundation/`** — the orientation kernel (≤10 links). **`llp/current/`** —
  the declared working set (≤15 links). Both are symlink overlays; archiving is
  removing a link.

## System map

- `kernel/` — `exact-kernel`, the layout/wire/export kernel. Specified by LLP 1001;
  images (a replaced element across kernel and hosts) by LLP 1011;
  video and its keyboard-resizing consumer by LLP 1042.
  Design lineage: RFC 0491 (kernel refresh), LLP 0507 (EXWF wire), LLP 0486/0487
  (layout language), LLP 0297 (threading contract).
- `motion/` — `exact-motion`, the motion evaluator: CSS `transition` semantics
  over `translate`/`scale`/`rotate`/`opacity`, one spring, a seekable clock.
  Decided by LLP 1002, specified by LLP 1003. The kernel depends on it for the
  `transition` row's type; it depends only on `exact-num`. Rebuilt from scratch
  2026-08-28 (the ported RFC 0492 crate is gone; 0492 is research).
- `num/` — `exact-num`, number text as std reads it without std's tables
  (a correctly rounded decimal-to-float parse); a leaf under motion, text flow,
  the kernel, the runner and the web host. LLP 1047 §6.
- `svg-filter/` — `exact-svg-filter`, a resolved SVG filter as a chain of
  primitives and its flat wire form (LLP 1055.000 D14); a leaf under the kernel,
  which re-exports it as `svg::filter`, and under `exact-svg-raster`, which
  therefore does not compile the kernel (LLP 1036.000 §12).
- `plan/` — `exact-plan`, the plan format: tables, bytecode, and the validating
  decoder, generated from `plan/tables/format.json`. Depends on nothing. LLP 1005.
- `runner/` — `exact-runner`, the plan runner: the VM, keyed instances, kernel
  ops, events, timers under a seekable clock, the data seam. LLP 1005.
- `contract/` — the Contract compiler: `syntax` → `types` → `analyze` → `lower`
  and the `contract` driver/CLI, plus `corpus/`. Decided by LLP 1004, specified
  by LLP 1006.
- `apps/caltrain/` — the v1 app: `app.contract`, its Rust data crate
  (`data/`), and its wasm crate (`web/`), the end-to-end fixture for everything.
- `host/web/` — `exact-web`, the web host: the runner and kernel in wasm over
  the real DOM, CSS computed once from the kernel's rows, springs lowered to
  frames, a no-`unsafe` ABI, the glue, the headless-Chrome smoke, the motion
  parity harness (`parity.mjs`), and the dev loop (`bun host/web/dev.mjs`:
  edit `app.contract`, the page rebuilds and reloads; `--wasm` is the resident
  loop that restarts the plan in place in ~20 ms). LLP 1007.
- `host/web-js/` — the web's default build (LLP 1071): the plan compiled to one
  ES module over a ~20 KB runtime that drives the DOM directly. `build.mjs`
  makes it; a game, delivery's oracle and the parity smokes keep `--wasm`.
- `host/render/` — the render host (LLP 1048): pages pre-rendered per request by
  a Rust renderer, then adopted in place by the runtime rather than hydrated.
- `host/apple/` — `exact-apple`, the Apple host: the runner and kernel as a static
  library with a C ABI (`include/exact.h`), the kernel's own layout with CoreText
  measurement through a registered callback, `exact-motion` as the executor, typed
  batches; `macos/` is the AppKit presenter (SwiftPM, no SwiftUI) and `ios/` the
  UIKit one — the same package shape, run on a simulator — with `swift/` what the
  two share: the bridge, CoreText, the agent's clock, the GPU module's ABI. LLP
  1008 (§9 for iOS).
- `host/linux/` — `exact-linux`, the Linux host and the first that paints: the
  runner and kernel natively, cosmic-text measuring and painting from one
  paragraph cache, `exact-motion` as the executor, and the kernel tree drawn
  by one walk over a backend — vello on the GPU, tiny-skia on the CPU where
  there is none — the kernel is the display list, no batch and no mirror —
  onto DRM/KMS dumb buffers with evdev input, or into a buffer with no display
  at all (the agent API over stdio, a screenshot, the smoke — on fleet Linux
  and on macOS). Pure Rust; no system library is linked. `apps/caltrain/linux`
  is the app's executable. LLP 1015.
- `host/apple-update/`, `host/linux-update/` — optional delivery adapters
  above their hosts (LLP 1030 D4). App bake selects the adapter entry for
  `L=A`, the core entry for `L=0`; neither core host depends on `exact-update`.
- `gpu/` — `exact-gpu`, the GPU canvas (LLP 1009): the `Surface` trait an app's
  GPU crate implements against wgpu, the module that runs surfaces on a device,
  and its ABI — a C ABI for the `dylib` the macOS presenter `dlopen`s, wasm-bindgen
  exports for the wasm the page fetches — both loaded on demand after the first
  pixel. `apps/caltrain/gpu` is the app's module: the aurora, the glass and the
  deck. The line map left it for Canvas 2D (LLP 1056), drawn by the data crate.
- `vendor/taffy/` — Taffy 0.9.2 plus six Exact patches (`EXACT-PATCHES.md`).
- `scripts/` — `caps` and `boot` (two of the five checks), `metrics` (the startup and
  speed numbers in one run, diagnostic), `issue` (filesystem issues, `docs/issues.md`),
  and `exact.mjs` (`exact new`, `setup`, `contract`, `run`, `install`).
- `docs/` — the guides for people and agents building apps: Contract for agents,
  for humans, its grammar, the pitfalls, and the developer reference (LLP 1086).
- `skills/`, `.claude/skills/` — the orchestrate skill and the LLP skills (installed
  from `ccheever/llp@v0.5.1`; receipt in `.llp/skills-receipt.json`).

The graph is layered on purpose, and that is the whole modularity story: each
crate depends on strictly less than the one above it — `plan` and `motion` on
nothing, `kernel` on `motion` and Taffy, `runner` on `kernel` and `plan`, a host
on those — and anything optional is a separate artifact loaded on demand or
another executor (the GPU module after first pixel, LLP 1009 D2; the compiler,
never in the wasm; motion on the web, the browser's), never a feature flag on a
core crate. There is no build matrix. An embedder links the crate it wants and
the linker drops the rest: the web wasm carries no Taffy because nothing on the
web calls layout. (Stated 2026-08-29.)

All four surfaces run the app: iOS landed 2026-08-29 on the Apple host's shape
(LLP 1008 §9 — the UIKit presenter over the same archive, built for the
simulator, the same smoke green). (The decided order — 2026-08-28:
**the web version works really well first, then Apple, then Linux** — is done:
the Linux host landed 2026-08-29, LLP 1015, a painter over DRM/KMS whose
headless form runs the same smoke as the other two hosts. (The macOS host landed 2026-08-29, LLP 1008, with
the C ABI its consumer made concrete. The plan format, runner, and compiler
landed together as one lane — LLP 1006 §7; the web host followed the same day,
LLP 1007, and its owed pieces — springs lowered to frames, the browser-driven
motion parity harness, the resident dev driver — the same day again, LLP 1007
§3, §5, §6. The eight-operation agent API landed 2026-08-29, LLP 1012: the read
operations once in the runner, one export on each ABI, one driver with a carrier
per host, and the smoke as a script of its operations.)

## The five checks

`build` · `test` · `lint` · `caps` · `boot`

```sh
cargo build --all-targets --keep-going
cargo test --lib --bins --tests --no-fail-fast
cargo clippy --all-targets --keep-going -- -D warnings; cargo fmt --all -- --check
bun scripts/caps.mjs
bun scripts/boot.mjs   # counts the module graph before first pixel: host glue only, no app JS
```

Cargo's three cover the root `default-members` (deterministic, in-process
crates); the async lane runs them with `--workspace`, the UIKit XCTests for commits
under `host/apple`, then `metrics.mjs --long`.

## Durable constraints (from the rules, restated for orientation)

- **The web is the standard.** A semantic that could follow CSS follows CSS —
  defaults, property names, value vocabularies, behavior. Deviations are declared
  with a reason (LLP 1001 §1). `AGENTS.md` carries the same rule.
- Delete; don't deprecate. No compat shims before 1.0. Names change.
- Generated files are built, never committed (`kernel/build.rs` → `OUT_DIR`).
- 1,500 lines per source file; vendored code exempt.
- The boot path executes and compiles nothing. No app JS before first pixel.
- Never `git stash`; kill only PIDs you recorded.
- Agents remove apparatus freely and add none without a human saying so.

## Ratification note

Written on 2026-08-28 alongside the first kernel landing (LLP 1001); ratified by
Charlie the same day after the motion rebuild (LLP 1002/1003) and moved to
`llp/foundation/`. Where this map and a spec disagree, the spec wins and this
document is stale.

## Optional game engine

The independent [`game/`](../game/README.md) workspace supplies Rust gameplay,
rendering, physics, audio, and Beacons/Lanterns examples. Apps explicitly link
its adapter into their on-demand GPU artifact; core workspace apps have no
engine dependency. See [LLP 1046.003](1046.003-game-engine-as-built.explainer.md)
for the implementation and September 21 integration provenance.
