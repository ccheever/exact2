# Code review: the design checker's bound (rt1's PRIORITY finding), round 3 of 3, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `3f56f7a7bd`.
- **Method:** one brief (sha256 `060b0dc4e5a3566075446db2af58390cf3a1556fc10bc5028ad9bbb7bca68026`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Reviewed at 3f56f7a7bd, on exact47's main; with the security pass.

The hardening still has seven blockers and two parity issues. These findings are source-traced; runtime verification was blocked by the read-only sandbox.

1. **BLOCKER — Kind inference still multiplies expression depth by reference depth.** [check.rs:904](tamber/src/check.rs:904)  
   `measures()` recursively enters functions and then calls `named_kind()` → `kind_of()`, but only token entries increment `walk.depth`. A forward chain of 500 tokens, each wrapping the next in 32 `calc()` calls, permits roughly 16,000 nested function walks **before** reaching the guarded fold. The new regression test uses only 40 tokens. Include inference’s function traversal in the combined depth limit and exercise the larger case on a 2 MiB stack.

2. **BLOCKER — DTCG composite aliases have the same multiplication problem.** [dtcg/kinds.rs:91](tamber/src/import/dtcg/kinds.rs:91)  
   `raw()` limits alias count, while `inner()`/`leaf()` recursively traverse composites without counting their nesting. A 28,410-byte document containing 256 dimension tokens, each with 32 nested arrays around an alias to the next, traverses 8,192 composite levels before type refusal. Every string passes the new source bound, individual JSON nesting passes deserialization, and the work budget does not stop this descent. Bound combined traversal depth before resolving composites.

3. **BLOCKER — Importer role analysis still has unrestricted recursive walks.** [import/mod.rs:722](tamber/src/import/mod.rs:722), [shadcn.rs:873](tamber/src/import/shadcn.rs:873)  
   `chain()` and `varies()` have cycle detection but no depth bound. Declare a long alias chain in dependency order, ending with `Canvas` naming its last token: checker memoization lets it pass with shallow calls, but an import’s role analysis traverses the entire chain recursively. These walks also repeatedly scan names and visited vectors. Use indexed, iterative traversal; the checker’s recursion guard does not protect them.

4. **BLOCKER — Wire refusals can still echo an oversized name verbatim.** [tamber_ops.rs:176](contract/cli/src/tamber_ops.rs:176)  
   `param_names()` runs before `edit::valid`. A `set-param` with a 900 KB unknown name reaches [params.rs:615](contract/lower/src/params.rs:615), which includes the entire name in a `LowerError`; the server serializes it directly into `error.message`. Neither `over_bytes` nor `Diagnostic::new` runs. Validate bounds before parameter resolution and bound these refusal messages too.

5. **BLOCKER — Profile substitution remains cubic before preflight.** [bind.rs:75](contract/lower/src/theme/bind.rs:75)  
   Each recognized profile token restarts `position()` from the beginning; each previously processed token searches the growing `out` vector. With N profile tokens or aliases this performs Θ(N³) name comparisons, despite every value being short. This is separate from the three waived algorithms. Index processed names and resolve aliases without repeated whole-design scans.

6. **BLOCKER — Icon checking remains quadratic after linear alias resolution.** [icon.rs:191](tamber/src/icon.rs:191)  
   Every icon scans all theme tokens and all preceding icons for duplicates: Θ(I×T + I²). A design containing only distinct, short literal icons still triggers it. The new linear alias resolver does not remove this path, and it is outside the three queued exceptions. Reuse indexed name membership for duplicate checking.

7. **BLOCKER — Astryx’s `local` values bypass source bounds.** [astryx.rs:150](tamber/src/import/astryx.rs:150)  
   `source_bound()` visits only `light` and `dark`. A document with empty sides and a 5,000-byte string under `local` reaches [astryx.rs:265](tamber/src/import/astryx.rs:265), becomes an unmapped report containing the full string, and returns `Ok`: the outer `bounded()` checks only operations. Include `local` in the source check before mapping or reporting.

8. **SHOULD — Oversized kinds get different words on wire and disk.** [edit.rs:371](tamber/src/edit.rs:371)  
   `AddToken.kind` goes through `ident()`, producing “**a name** is at most 4096 bytes.” Disk preflight produces “**a kind** is at most 4096 bytes.” A 4,097-byte identifier reproduces the discrepancy. Use the same kind-bound helper.

9. **SHOULD — Whitespace changes which values are refused.** [edit.rs:285](tamber/src/edit.rs:285)  
   Wire validation measures the untrimmed value; [syntax.rs:298](tamber/src/syntax.rs:298) trims disk values first. Sending 4,096 spaces followed by `#fff` is refused as 4,100 bytes, while the equivalent design line builds as `#fff`. Apply the bound at the same normalization stage.

The QUEUE entry accurately describes the three deferred algorithms and its extrapolations are consistent with the listed measurements; I could not reproduce those timings. The adjacent claim that checking is linear needs qualification given findings 5–6.

I found no decrement leak in the new `Deeper` guard and no additional state/disk divergence in the commit path. `git diff --check` passed. Cargo tests could not start because creating the target directory was denied, so output compatibility and stack behavior remain unverified at runtime. No files were edited.
