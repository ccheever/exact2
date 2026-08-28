# Code review: the plan lane, 2026-08-28 (grok)

- **Family:** grok (xAI)
- **Provider/runtime:** `grok -p` (headless) with file tools, `--always-approve`, `--disable-web-search`, model `grok-4.6`, `--reasoning-effort xhigh`, cwd = a second disposable export of the same staged tree (tree digest `67a4a7789d565e92`)
- **Method:** one-shot code review from `REVIEW-BRIEF.md` (sha256 `13414825188482e3e1c193e898bb7edc593f158926ea16956deaca826e92e03b`), mutually blind to the same-day codex review. Same scope and authority. The first stdout line is the model's own narration (preserved verbatim per convention).
- **Launched:** 2026-08-28T22:56:19Z
- **Disposition:** see the orchestrator's fold recorded at the end of this file once applied.

---

## Verbatim review

I'll start by reading the review brief and the bound rules so the review stays in the named lane.The brief’s format is the deliverable, so I’ll load the binding rules and specs next, then read the named lane files in full.I’ll start the offline cargo checks in parallel and pull line counts plus the binding specs so the file-by-file read stays complete.Tests passed. Next I’ll run clippy and the wasm build, then read the runner, plan, and compiler files in full.Checks passed. I’ll keep reading the plan, compiler, and kernel files in full and verify the timer, key, and settlement issues.The decoder and VM paths look like they have a few fail-closed holes; next I’ll read lowering, types, and the kernel style seam to confirm.## Overall assessment

The plan/runner/compiler lane is a coherent, table-driven stack: `format.json` is the declaration authority, loading is a validation pass for indexes/ranges/opcodes, compiled Caltrain and the Now-screen corpus behave as specified, and settlement rollback of slot writes is tested. `cargo test --workspace --offline`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `cargo build --workspace --offline --target wasm32-unknown-unknown` all passed. The serious holes are fail-closed on hostile (and one authored) inputs: `check_code` does not bound control flow, a zero timer hangs `advance`, and a region with no arms treats “no arm” as the plan root and recurses until stack overflow.

## Findings

1. **HIGH —** `plan/build.rs:668` / `runner/src/vm.rs:270` — `check_code` accepts any `u32` jump operand (it only checks framing, index operands, enums, finite `f64`, and that the last opcode is `Return`). `jump` only rejects `target > code.len()`. A plan whose slot initializer is `Jump 0 … Return` therefore validates, then `Runner::boot` never returns. The same hole admits a tight `Some`/`List` loop that grows values until OOM. Compiled Contract cannot emit backward jumps, but decode/boot is specified to refuse hostile plans with a typed reason, not hang. FLAG (read of `check_code` + `eval`; not executed).

2. **HIGH —** `runner/src/instance.rs:450` / `runner/src/instance.rs:488` / `runner/src/instance.rs:121` — Region contents are realized as `realize(u, None, arm)`. If `arms` is empty, `iter().next()` / `nth(i)` is `None`, and `sites(plan, None, None)` is the plan’s root sites. On boot, an `each`/`when`/`match` with zero arms therefore recreates the whole tree inside itself until stack overflow — a panic on the decode/boot path. The compiler always emits 1–2 arms; the decoder does not require that. FLAG (not executed).

3. **HIGH —** `runner/src/runner.rs:338` / `contract/lower/src/lib.rs:208` / `plan/tables/format.json:51` — `advance` fires a timer and does `next_ms += interval`. `interval_ms` is an unconstrained `u32`; lowering does `*ms as u32`. `every(0, tick)` and `every(0.9, tick)` both compile to interval 0, after which the first due fire loops forever (`next_ms` never moves). That is reachable from Contract, not only from a crafted plan. FLAG (not executed; the seekable-clock test uses interval 1000).

4. **MEDIUM —** `plan/src/value.rs:15` / `plan/src/value.rs:117` / `runner/src/instance.rs:438` / `runner/src/instance.rs:560` — `Value` uses derived `PartialEq` (IEEE `==`) but canonical encode writes raw `f64` bits, and `each` duplicate detection uses `format!("n:{n}")` while reuse uses `vm::equal`. Concrete failures: `-0.0` and `0.0` compare equal and encode differently; two `-0.0`/`0.0` keys are not duplicates (`"n:-0"` vs `"n:0"`) but reuse as one row; a `NaN` key is never `==` itself, so every update destroys and recreates the row. `conforms` already refuses non-finite numbers at the data seam; VM arithmetic and `each` keys do not. FLAG.

5. **MEDIUM —** `runner/src/bridge.rs:50` / `kernel/src/style.rs:134` / `kernel/src/txn.rs:218` / `runner/src/runner.rs:415` — A style binding whose value is `Inf`/`NaN` (`width = 1/0`, etc.) is a typed `StyleValueError` / `NonFiniteStyle`, which `update` turns into `Poisoned`. LLP 1005 §6 says compiled plans do not reach that path. They do, because `StoreSlot` never `conforms` and division is in the language. Boot of a style bound to `1/0` fails the first `apply` instead. FLAG.

6. **MEDIUM —** `runner/src/runner.rs:383` / `runner/src/runner.rs:394` — Settlement failure rolls back slots and commands; an instance/kernel failure after a successful settle does not. Scenario: an action writes a list with a duplicate `each` key and also `setScheme("dark")`; `settle` commits, `update` returns `DuplicateKey`, the runner is poisoned, and `take_commands()` still yields the command while the kernel is unchanged. FLAG.

7. **MEDIUM —** `contract/lower/src/tags.rs:185` — Attribute `flex` sets only `flex_grow`. CSS `flex: 1` is `1 1 0%`. Caltrain authors `scroll … flex=1` (`apps/caltrain/app.contract:61`); shrink stays 1 and basis stays `auto`, so the scroll pane does not fill like a browser `flex: 1`.

8. **MEDIUM —** `contract/analyze/src/lib.rs:257` / `contract/types/src/lib.rs:1015` — Handler arity is checked only for `Ref::Action`, and analysis walks the pre-inline view. A child `press: action` used as `press=press` (under-applied) or `press=press(a,b)` (over-applied past a bare `action` prop) is not rejected; the inlined call fails later with `RunnerError::Arity`. Over-arity of a *named* root action is a type error on the inlined tree; under-arity is valid currying and is never required to be fully applied at a handler. FLAG.

9. **MEDIUM —** `contract/lower/src/lib.rs:377` / `kernel/src/node.rs:8` — Lowering always attaches children. `input`/`image` cannot hold children; `text` may hold only `text`. `input value=q` with a nested `text` compiles and then `Kernel::apply` refuses (`LeafCannotHoldChildren` / `InlineRunNotText`) on the first frame. FLAG.

10. **LOW —** `contract/lower/src/lib.rs:88` / `contract/analyze/src/lib.rs:307` — `lower` ignores `Analysis` (`_analysis`). `derive_order` is unused (settlement is a fixpoint). `analyze-derive-cycle` is unreachable after `type-derive-cycle` and has no reject fixture.

11. **LOW —** `contract/cli/src/lib.rs:88` — `bake` ends in `b.finish().expect(...)`. A bake that somehow fails validation panics instead of returning `RunnerError`/`PlanError`.

12. **LOW —** `runner/src/instance.rs:108` / `runner/src/instance.rs:516` — `Update::destroyed` is filled (and region `record_destroyed` only records region roots, not descendants) but the runner never reads it. Dead field.

13. **LOW —** `contract/lower/src/tags.rs:108` — `leak` `Box::leak`s semantic tag strings (`main`, `header`, …) on every lookup. Harmless at v1 size; still a process-lifetime leak on the compile path.

14. **LOW —** `apps/caltrain/data/src/lib.rs:239` — `sort_by(...partial_cmp(...).unwrap())` panics if a location argument is NaN. The compiled app never does that; a hostile resource argument can.

## Suggested tests

- Reject (or trap with a bound) a plan whose slot `init` is `Jump 0; Return`; boot must return a typed error, not hang.
- Reject `regions.arms.len == 0` (and `when`/`match` with `want` past `arms.len`); boot must not overflow the stack.
- `every(0, tick)` / `every(0.9, tick)` must not compile, or `advance` must refuse interval 0.
- `each` keys `0` and `-0.0` in one list; `each` key `0/0` (NaN) across two updates must keep identity or refuse the key.
- Style binding `width = 1/0` after an action: typed trap/rollback, not `Poisoned`.
- Action that `setScheme(...)` and then writes a duplicate-keyed list: commands must not be observable if the runner poisons.
- Contract `true and { side-effect-free expr }` / `false and (1/0)`: short-circuit; corpus currently never runs `and`/`or`.
- Nested `each` inside `when` inside `each`: `LoadItem` depths 0 vs 1 vs 2 against a press handler.
- Child component `press: action` used as `press=press` (no args) against a one-parameter root action: compile-time id, not `Arity` at dispatch.
- `input` / `image` / `text` with a non-legal child: `lower-…` rejection.
- `flex=1` vs CSS `flex: 1` on a flex child filling a definite height (or drop the alias).
- Round-trip `Value::Number(-0.0)`: `PartialEq` vs `to_bytes()` bit identity.

## Summary

1. Bound VM control flow in `check_code` (instruction-aligned jump targets; no unbounded loops on boot).
2. Do not treat a missing region arm as the plan root; validate arm counts per `RegionKind`.
3. Refuse `interval_ms == 0` (and stop silently truncating `every`’s f64 to `u32`).


---

## Disposition (orchestrator, 2026-08-28, same day)

Applied, each with a test: **1** forward-only, instruction-aligned jumps; **2** region arm counts, arm ownership, and `RootRegion`; **3** `lower-timer-interval` and `ZeroInterval`; **4** one key rule (`-0` is `0`, NaN refused) used by both duplicate detection and reuse; **5** conformance at every value boundary, so `width = 1/0` is a typed, rolled-back refusal; **6** a poisoned runner clears queued commands (`keys_follow_one_rule_and_a_poisoned_runner_leaks_no_commands`); **7** `flex=n` lowers to CSS `flex: n` (grow n, shrink 1, basis 0%); **9** `lower-leaf-children`; **10** the dead `Analysis.derive_order` and cycle pass removed (types owns `type-derive-cycle`); **11** `bake` returns an error; **12** the dead `destroyed` field removed; **14** NaN-safe sorts in the data crate. **Circle-back, recorded in LLP 1006 §8:** **8** handler arity through a bare `action` prop. **Not taken:** **13** the `Box::leak` of seven short tag names (bounded, once per name per process; noted).
