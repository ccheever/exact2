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
| `motion/` (`exact-motion`) | CSS `transition` semantics over `translate`/`scale`/`rotate`/`opacity`, one spring, a seekable clock. The web executes it as CSS; everywhere else this crate does. | LLP 1002 (decision), LLP 1003 (spec) |
| `plan/` (`exact-plan`) | The plan format: tables, bytecode, and the validating decoder, generated from one JSON authority. Depends on nothing. | LLP 1005 |
| `runner/` (`exact-runner`) | The plan runner: VM, keyed instances, kernel ops, events, timers under a seekable clock, the data seam. | LLP 1005 |
| `contract/` | The Contract compiler in Rust: `syntax` → `types` → `analyze` → `lower`, the `contract` driver and CLI, and the corpus. | LLP 1004 (decision), LLP 1006 (spec) |
| `apps/caltrain/` | The v1 app: `app.contract`, its Rust data crate, and its wasm crate; the end-to-end fixture. | — |
| `gpu/` (`exact-gpu`) | The GPU canvas: a `Surface` trait against wgpu, a per-app module loaded on demand (a dylib on macOS, a second wasm on the web) after the first pixel; the same Rust renders on Metal and on the browser's WebGPU. `apps/caltrain/gpu` is the line map. | LLP 1009 |
| `host/apple/` (`exact-apple`) | The Apple host: runner + kernel as a static library with a C ABI, the kernel's layout with CoreText measurement through a callback, `exact-motion` as the executor, typed batches; `macos/` is the AppKit presenter (SwiftPM). `node host/apple/build.mjs --run`. | LLP 1008 |
| `host/web/` (`exact-web`) | The web host: runner + kernel in wasm over the real DOM, CSS computed once from the kernel's rows, springs lowered to frames the browser plays, a no-`unsafe` ABI, ~150 lines of glue, a headless-Chrome smoke, the motion parity harness, and the dev loop (`node host/web/dev.mjs`, edit → present ~20 ms). | LLP 1007 |
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
