# Exact

A cross-platform application runtime. The Rust kernel computes layout, each platform
renders natively, and Contract is the authoring model.

Surfaces: web, macOS, iOS, Linux.

Start here:

- **`rules/RULES.md`** — how work happens here. One page. Read it before your first PR.
- **`rules/NOT-DOING.md`** — what v1 deliberately excludes, and why.
- **`llp/1000-exact2-root.explainer.md`** — the map: what exists, what is next, how the
  design corpus is laid out.

The predecessor repo is research, not authority: cite it for how something worked,
never to block. Its design documents are imported under `llp/research/`.

## What exists

| Crate | What it is | Spec |
|---|---|---|
| `kernel/` (`exact-kernel`) | Typed columnar arena, EXWF wire frames, validate-then-apply transactions, Taffy layout with changed-geometry receipts, EXNODE columnar export, injected text measurement. Builds for `wasm32-unknown-unknown`. | `llp/1001-kernel-v1.spec.md` |
| `motion/` (`exact-motion`) | Shared values, closed-form drivers, a virtual clock, gesture recognizers, interactive navigation. | RFC 0492 (research) |
| `vendor/taffy/` | Taffy 0.9.2 plus two Exact patches. | `vendor/taffy/EXACT-PATCHES.md` |

Not built yet: the plan runner, the web host, the Apple and Linux hosts, the Contract
compiler, the agent API.

## The five checks

```sh
cargo build --workspace                                                 # build
cargo test --workspace                                                  # test
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check   # lint
node scripts/caps.mjs                                                   # caps
# boot — counts the module graph reachable before first pixel; nothing to count until a host exists
```

`kernel/tables/schema.json` is the one declaration authority for node types, props,
style rows, enums, and opcodes; `kernel/build.rs` generates the Rust from it at build
time. Edit the table, never the generated code.
