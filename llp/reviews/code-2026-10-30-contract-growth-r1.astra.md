# Code review: the Contract compiler's quadratic growth, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a36341aa37`.
- **Method:** one brief (sha256 `69c78a856aad24a3aaa26131d7d15446a041fdccfd3545d4f2a322551fe2715a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **P1 — Growing frames introduce a new quadratic regression.** [contract/types/src/frame.rs:43](contract/types/src/frame.rs:43) discards the index after every append. The next lookup rebuilds it over every preceding declaration. Repro: `state first = 0`, followed by N distinct `state sN = first` declarations and a simple text view. The old scan finds `first` immediately.

   Using the saved baseline/final binaries through `contract lean /dev/stdin`, final CPU time was **0.026 s → 0.417 s → 1.723 s** at 1,000/4,000/8,000 states; baseline was **0.003 s → 0.008 s → 0.015 s**. Output matched. Update an initialized index incrementally, preserving first-wins duplicates.

2. **P1 — Tint memoization retains quadratic work for authored buttons.** [contract/lower/src/symbols/tint.rs:124](contract/lower/src/symbols/tint.rs:124) caches the vector, but lines 132–135 scan that vector on every invocation. In a mapped build, N instances sharing a button’s span contribute N entries. With `appearance="none"`, both scans traverse all N entries for every button: Θ(N²).

   Repro: use the growth test’s generated app, adding `appearance="none"` to its button, and compile through the CLI. Cache the aggregate native/drawn/unknown result; keep the attribute fallback specific to each node.

3. **P2 — The growth guard cannot detect a regression in the warning pass.** [contract/cli/tests/growth.rs:26](contract/cli/tests/growth.rs:26) calls `contract::compile`, which disables warnings. Consequently, restoring the original whole-map tint scan has **no effect** on this test. This fails approval condition (4).

   Exercise the warning-enabled, mapped build path used by the CLI, including authored buttons and nonliteral initializers. An unmapped warning build also merges instance rows, so merely switching to `compile_warned` does not cover finding 2. Verify the guard rejects individual scan regressions.

The following observations are non-blocking:

- I found no semantic mismatch. First-wins duplicates, inner-scope shadowing, state-over-mutation precedence, and lifted action names remain intact. Frame mutations preserve index positions or invalidate the index; copy-on-write isolates shared frames. Successful assignments are the sole action-body writes to slot types, including recursive branches.
- The tint span key is semantically sound: cached rows depend only on the immutable map and span; attributes remain per-node. `(span, 0, String::new())` is the correct range start.
- The retyping oracle is meaningful, but [scope_tests.rs:142](contract/types/src/scope_tests.rs:142) counts `option<Unknown>` from initial `none` values as “settled.” That sanity assertion does not establish that writes changed types. Also, its `cfg(test)` hook does not run inside dependent integration-test crates.
- Alternating minimum timings reduces noise but does not ensure both sizes experience equal load. No timing flake occurred in my run; loaded-run qualification should use the corrected workload.

Formatting and `git diff --check` passed. Saved current executables passed the six scope tests and growth test; the saved tip log reports the full Contract suite green. Existing comparison captures and 80 additional piped cases showed no new output mismatch. Fresh Cargo verification was blocked by sandbox-denied target creation. Caps reports the pre-existing oversized Swift test file plus sandbox-denied wasm checks.

**CHANGES**
