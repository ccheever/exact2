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

The rules are in `rules/RULES.md` and `rules/NOT-DOING.md`; those two files bind
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

- `kernel/` — `exact-kernel`, the layout/wire/export kernel. Specified by LLP 1001.
  Design lineage: RFC 0491 (kernel refresh), LLP 0507 (EXWF wire), LLP 0486/0487
  (layout language), LLP 0297 (threading contract).
- `motion/` — `exact-motion`, the motion evaluator: CSS `transition` semantics
  over `translate`/`scale`/`rotate`/`opacity`, one spring, a seekable clock.
  Decided by LLP 1002, specified by LLP 1003. The kernel depends on it for the
  `transition` row's type; it depends on nothing. Rebuilt from scratch
  2026-08-28 (the ported RFC 0492 crate is gone; 0492 is research).
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
  parity harness (`parity.mjs`), and the dev loop (`node host/web/dev.mjs`:
  edit `app.contract`, the page restarts in ~20 ms). LLP 1007.
- `host/apple/` — `exact-apple`, the Apple host: the runner and kernel as a static
  library with a C ABI (`include/exact.h`), the kernel's own layout with CoreText
  measurement through a registered callback, `exact-motion` as the executor, typed
  batches; `macos/` is the AppKit presenter (SwiftPM, no SwiftUI) with its smoke and
  screenshot. LLP 1008.
- `vendor/taffy/` — Taffy 0.9.2 plus two Exact patches (`EXACT-PATCHES.md`).
- `scripts/` — `caps` and `boot` (two of the five checks), `metrics` (the startup and
  speed numbers in one run, diagnostic), and `issue` (filesystem issues, `docs/issues.md`).
- `skills/`, `.claude/skills/` — the orchestrate skill and the LLP skills (installed
  from `ccheever/llp@v0.5.1`; receipt in `.llp/skills-receipt.json`).

Not built yet, in the order they are expected (decided 2026-08-28: **the web
version works really well first, then Apple, then Linux**): the Linux DRM host, the
eight-operation agent API, iOS on the Apple host's shape. (The macOS host landed
2026-08-29, LLP 1008, with the C ABI its consumer made concrete. The plan
format, runner, and compiler landed together as one lane — LLP 1006 §7; the web
host followed the same day, LLP 1007, and its owed pieces — springs lowered to
frames, the browser-driven motion parity harness, the resident dev driver — the
same day again, LLP 1007 §3, §5, §6.)

## The five checks

`build` · `test` · `lint` · `caps` · `boot`

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
node scripts/caps.mjs
node scripts/boot.mjs   # counts the module graph before first pixel: host glue only, no app JS
```

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
