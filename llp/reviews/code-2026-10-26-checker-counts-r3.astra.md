# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), round 3 of 3 (security pass), 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `7e3d0f7eb5`.
- **Method:** one brief (sha256 `b2a37ff0a45734ddb58408ab617becaeb54566b930a6993b076691b1d619a5a2`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [tamber/src/export.rs:1273](tamber/src/export.rs:1273): dial fallback can panic on role spelling.** `place` is keyed by exported names, while candidates carry authored names. Repro: color dial `c`, with `canvas = c`, `ink = c`, and `keep = c`. Export, then change `Canvas` to white and `ink` to red in both schemes. The conflicting values trigger fallback, which indexes `place["canvas"]`; only `"Canvas"` exists. Carry the candidate’s index or use one consistent spelling.

- **BLOCKER — [contract/cli/src/tamber_params.rs:183](contract/cli/src/tamber_params.rs:183): the principal read paths bypass the cache.** `describe` and both forms of `resolve` call `design_with → fold_with → check_design`. Repeating a small resolve therefore repeats the entire influence sweep on an unchanged design. Caching `design()` and the subsequent components fold does not protect this path, so the condition supporting the queued overlapping-dial cost remains unmet.

- **SHOULD — [contract/lower/src/params.rs:578](contract/lower/src/params.rs:578): a client token can disable range batching.** `exact.probe.dial0` is a valid authored token name. Adding it to a design with a dial makes the synthetic batch contain a duplicate; the failed batch then folds every value separately at line 651. With 1,500 literal overrides, that means 1,501 full checks on each subsequent write. Allocate collision-free probe names.

- **SHOULD, remaining path — [contract/lower/src/params.rs:626](contract/lower/src/params.rs:626): ordinary token-valued ranges remain quadratic.** Give N ranged params overrides naming one constant theme token, such as `shared: number = 0.5`. Every override enters `alone`, causing N whole-design checks without a budget. The conservative classification preserves correctness but leaves the original cost on valid pushes. Independence needs transitive dependency information, or equivalent probes need reuse.

- **SHOULD — [contract/lower/src/params.rs:237](contract/lower/src/params.rs:237): `shown()` still rebuilds its index per value.** `components()` calls it for every default and override. With N defaults referencing one shared param, each call contains `param.` and rebuilds the N-entry map and sorted lengths: quadratic work for a linear-sized answer. Construct and reuse the formatter’s index across the listing.

- **SHOULD — [tamber/src/export_dials.rs:236](tamber/src/export_dials.rs:236): exhaustion during fallback does not stop the dial pass.** If an early `settle_alone()` spends the budget, remaining callbacks still run, later dial groups still execute full checks, and the combined check at line 284 still runs. The guard in `export.rs:1307` comes afterward. Propagate exhaustion immediately from each callback and stop before further checks.

I found no stale-answer defect in the cache’s full key, owned clones, or poisoned-lock fallback.

Cargo tests and plan-byte comparisons could not run because the read-only sandbox blocked target-directory creation. No files changed.
