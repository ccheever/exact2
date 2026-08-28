# Code review: the plan lane, 2026-08-28 (codex)

- **Family:** codex (OpenAI)
- **Provider/runtime:** `codex exec -s workspace-write`, model `gpt-5.6-sol`, `model_reasoning_effort=xhigh` (verified in the run log), fresh session `01a04a96-9dc3-7051-a146-606a0158c291`, workdir = a disposable export of the staged working tree (223 files; tree digest `67a4a7789d565e92`) with a warm cargo cache; cargo checks permitted offline
- **Method:** one-shot code review from `REVIEW-BRIEF.md` (sha256 `13414825188482e3e1c193e898bb7edc593f158926ea16956deaca826e92e03b`), mutually blind to the same-day grok review (separate export, no shared notes). Scope: `plan/`, `runner/`, `contract/`, `apps/caltrain/`, the kernel's `set_dynamic` change; authority: RULES.md, NOT-DOING.md, LLP 1004/1005/1006.
- **Launched:** 2026-08-28T22:56:19Z
- **Disposition:** see the orchestrator's fold recorded at the end of this file once applied.

---

## Verbatim review

## Overall assessment

The happy path is coherent, but the scoped landing is not ready: hostile plans can hang or stack-overflow instead of failing closed, and rollback/type invariants have gaps. `cargo test --workspace --offline`, `cargo clippy --workspace --all-targets --offline -- -D warnings`, and `cargo build --workspace --offline --target wasm32-unknown-unknown` all passed. No source files were modified.

## Findings

1. **HIGH —** FLAG: `plan/build.rs:642` validates bytecode linearly and only requires the final decoded opcode to be `Return`; it does not validate jump targets or forbid backward edges. `runner/src/vm.rs:270` then follows a jump without an instruction budget. A body encoded as `Jump 0; Return` passes `Plan::validate` and loops forever when used as a slot initializer during boot. Validate targets as instruction boundaries and reject backward/self jumps, since generated expressions only need forward control flow. Not executed because the reproduction intentionally hangs.

2. **HIGH —** FLAG: region structure is not semantically validated at `plan/build.rs:609`. In `runner/src/instance.rs:487`, a true root `when` with an empty arms range obtains `None` from `nth(0)` and calls `realize(..., arm=None)`; `runner/src/instance.rs:152` then realizes the same root region recursively until stack overflow. Enforce exact arm counts (`when`/`match`: 2, `each`: 1), arm ownership, and valid site topology during plan validation.

3. **HIGH —** `contract/lower/src/lib.rs:195` casts timer literals directly to `u32`, admitting `every(0, action)` and fractional values that truncate to zero. At `runner/src/runner.rs:337`, a zero interval leaves `next_ms` unchanged, so `advance` never exits; passing positive infinity to `advance` has the same outcome. The compiler acceptance was verified by streaming a corpus variant with `every(0, tick)` through the CLI, which exited successfully. FLAG: the resulting hang was confirmed by inspection, not executed. Reject non-positive/non-integral intervals in both compiler and plan validation, and reject non-finite clock inputs.

4. **MEDIUM —** FLAG: settlement rollback retains partially updated resource caches. `runner/src/runner.rs:482` moves the committed states into a working vector, `runner/src/runner.rs:581` mutates it after each successful query, and error paths restore that already-mutated vector. If resource A succeeds for a new slot value and later resource B fails, the slot rolls back but `resource("A")` exposes the failed update’s value while VM reads still use the old `resource_values`; a retry can also incorrectly reuse A without querying. Stage resource states separately and publish them only after the entire fixpoint succeeds.

5. **MEDIUM —** FLAG: declared slot, derive, and action-parameter types are not enforced by the runner. Slot initializers are adopted without conformance checks at `runner/src/runner.rs:174`, action arguments are checked only for arity at `runner/src/runner.rs:354`, and `StoreSlot` checks only the writes roster at `runner/src/vm.rs:311`. A hostile plan can declare a numeric slot, initialize or write it with a string, and successfully boot/update if nothing immediately performs a typed operation on it. Check initializer, derive, parameter, and write values with `Value::conforms`.

6. **MEDIUM —** FLAG: count validation permits large allocation amplification. `plan/src/bytes.rs:55` checks only `count <= remaining`, while generated decoding reserves `Vec::with_capacity(count)` for strings and every table at `plan/build.rs:555` and `plan/build.rs:568`. A roughly 16 MiB payload can announce 16,777,216 strings, causing a roughly 384 MiB `Vec<String>` reservation before the first inevitable truncation—enough to trap a wasm loader instead of returning `PlanError`. Generate minimum-width checks per table/string entry before allocation and tighten value element limits.

7. **MEDIUM —** FLAG: the component depth guard does not bound total inlining. `contract/syntax/src/inline.rs:47` limits only path depth, while `contract/syntax/src/inline.rs:75` recursively duplicates each use. A 32-level acyclic component chain where every component uses the next twice expands a small source into approximately 2³² nodes, exhausting memory before the depth check fires. Add a checked global expansion/node budget.

8. **MEDIUM —** FLAG: keyed top-level rows can reorder without reordering kernel roots. `runner/src/instance.rs:595` emits `AttachRoot` only for roots absent from the previous set, while `kernel/src/arena.rs:330` preserves original attachment order. A sole top-level `each` changing `[a,b]` to `[b,a]` reuses keyed views and updates `Tree::roots`, but `Kernel::roots` remains `[a,b]`; no parent exists to receive `SetChildren`. Reject root regions capable of multiple roots in v1, or provide an ordered root update.

9. **MEDIUM —** attribute value types are not checked during compilation: `contract/types/src/lib.rs:1024` merely infers and discards each attribute’s type. This produces plans that fail only while realizing the first frame. CLI probes verified that both `width=true` and `translate=1` compile successfully. `runner/src/bridge.rs:49` rejects the boolean width, while `contract/lower/src/tags.rs:201` exposes `translate` even though the generated Vec2 row at `kernel/build.rs:1193` has no representable `Value` conversion. FLAG: boot failures were traced rather than executed. Validate attributes against generated kernel metadata and remove `translate` until it has an authorable value form.

10. **MEDIUM —** FLAG: the compiler refusal tuple is incomplete. Although the compiler emits an identity at `contract/lower/src/lib.rs:65` and LLP 1006 requires rejection at `llp/1006-contract-compiler-v1.spec.md:130`, `Runner::boot` checks only the kernel schema at `runner/src/runner.rs:137`. Changing only the encoded `compiler_identity` therefore still yields an accepted plan. Make the expected compiler identity part of the runner’s boot contract and compare it before evaluation.

11. **MEDIUM —** FLAG: duplicate-key detection and row reuse use different numeric equivalence. `runner/src/instance.rs:409` detects duplicates through formatted strings, but `runner/src/instance.rs:434` reuses rows through VM equality. `0.0` and `-0.0` have distinct formatted keys at `runner/src/instance.rs:560` but compare equal at `runner/src/vm.rs:364`; a list containing both is admitted, and reordering it can swap row identities. Use one canonical key representation/equality rule for both operations, including an explicit NaN policy.

12. **LOW —** FLAG: finite `f64` values can become non-finite style rows. `kernel/src/style.rs:132` and `kernel/src/style.rs:168` test the source `f64` for finiteness before casting it to `f32`; `f64::MAX`, for example, becomes infinity and `set_dynamic` returns success. A later kernel apply rejects it, but the dynamic setter has already produced an invalid patch. Check the converted `f32` before returning success.

## Suggested tests

- A decoded body containing `Jump 0; Return` is rejected before VM execution.
- Zero-arm and wrongly-owned root regions return typed plan errors without recursion.
- `every(0, ...)`, fractional intervals, and non-finite `advance` targets are rejected.
- When resource A succeeds and resource B fails, all resource caches remain at their committed values.
- Wrongly typed initializers, action arguments, derives, and slot writes are refused.
- Maximum-count short inputs fail before any large table or string reservation.
- A branching component graph hits a typed global expansion limit.
- Reordering a top-level keyed `each` keeps `Runner::roots` and `Kernel::roots` identical.
- `width=true` and currently unrepresentable `translate` values are compiler reject fixtures.
- A mismatched compiler identity is refused at boot.
- Numeric keys `0.0`, `-0.0`, and NaN exercise one consistent duplicate/reuse policy.
- `StyleValue::Number(f64::MAX)` and oversized percentages are refused without changing the patch.

## Summary

1. Make bytecode control-flow validation guarantee termination and valid jump boundaries.
2. Add semantic plan validation for region topology and timer progress.
3. Make settlement transactional across resource caches and enforce declared value types at runtime boundaries.


---

## Disposition (orchestrator, 2026-08-28, same day)

Applied, each with a test: **1** forward-only, instruction-aligned jumps in `check_code` (`control_flow_is_forward_only_and_instruction_aligned`); **2** region arm counts and arm ownership in `Plan::validate_semantics`, plus `RootRegion` at boot (`region_topology_and_timer_progress_are_validated`, `a_region_at_the_plan_root_is_refused`); **3** `lower-timer-interval` (whole, ≥1) and `ZeroInterval` in validation, `NonFiniteClock` on `advance`; **4** settlement works on a copy and publishes only on success (`a_settlement_refusal_publishes_no_partial_resource_state`); **5** conformance of slot initializers, writes, derives, and action arguments (`values_conform_to_declared_types_at_every_boundary`); **6** reservations capped at `bytes::RESERVE` (`a_huge_announced_count_reserves_little_before_it_is_refused`); **8** a root region is refused in v1 (`RootRegion`); **11** one key rule — `-0` is `0`, NaN refused; **12** `f32` finiteness checked after the cast; **9** (part) `translate` removed from the tag table. Spec corrected for **10** (LLP 1006 §6: the runner carries, a host compares). **Circle-back, recorded in LLP 1006 §8:** **7** a total inlining budget; **9** compile-time attribute value typing.
