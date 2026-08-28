# LLP 1000: Exact2 Project Root

**Type:** Explainer
**Status:** Draft
**Role:** Root
**Systems:** All
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-28
**Related:** LLP 1001, LLP 0552, LLP 0491, LLP 0507, LLP 0486, LLP 0492

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
- `motion/` — `exact-motion`, the motion evaluator: shared values, closed-form
  drivers, a virtual clock, gestures, interactive navigation. Ported from exact1
  and trimmed to `rules/NOT-DOING.md`'s shape (RFC 0492).
- `vendor/taffy/` — Taffy 0.9.2 plus two Exact patches (`EXACT-PATCHES.md`).
- `scripts/` — `caps` (the budget check) and `issue` (filesystem issues, `docs/issues.md`).
- `skills/`, `.claude/skills/` — the orchestrate skill and the LLP skills (installed
  from `ccheever/llp@v0.5.1`; receipt in `.llp/skills-receipt.json`).

Not built yet, in the order they are expected: the plan runner (LLP 0485's flat
plan as the producer that emits ops), the web host (the kernel as wasm over the real
DOM, LLP 0517/0483), the Apple host and the generated C ABI, the Linux DRM host,
the Contract compiler, the eight-operation agent API.

## The five checks

`build` · `test` · `lint` · `caps` · `boot`

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
node scripts/caps.mjs
# boot: counts the module graph reachable before first pixel — nothing to count until a host exists
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

Written on 2026-08-28 alongside the first kernel landing (LLP 1001). Draft until
Charlie confirms the map; it moves to `llp/foundation/` when it becomes Active.
