# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), round 1 of 3, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `f705da0591`.
- **Method:** one brief (sha256 `1df957552a166848609ed095bcaca8256f9ca5b1be224d807529c8cc06ef8446`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

No BLOCKER. On `f0eeade..f705da0` (the four checker commits `git log origin/main..HEAD` lists), conditions (1) and (2) match `named()`, the old `laid()`, and a full refold on every input I can construct. A failed charge is not returned as an `Import`, and the next outermost import on the thread gets a fresh budget. Three SHOULD findings and two NITs are below. Tests were not run. `origin/main` is now `81d9a9e`, so a two-dot tree diff also contains main-side files these commits do not touch; this review is the checker range.

## SHOULD

**1. An import that spends the budget exactly is refused.** `tamber/src/budget.rs:62-65` and `tamber/src/budget.rs:79-81`, applied at `tamber/src/export.rs:657-658`.

`spend` returns `Ok` when the subtraction lands on 0. `spent` is true whenever `LEFT == 0` inside a `Work`, then spends one more unit to mint the words. `reimport_against` turns that into `Err` after `reimport_within` has already returned `Ok(Import)`.

A re-import whose charges sum to exactly `REIMPORT` (`3 << 23`) used to succeed: on the merge-base, `reimport_against` already began that `Work`, and a last successful `spend` left `LEFT` at 0 with no `spent()` check. It now refuses in the budget’s words. The charges that can be that last successful spend are the export trees (`export.rs:392`), a when-search candidate (`export.rs:1185`), or a blame round (`export_batch.rs:65`). Nothing after `take` spends (`dials`, `marks`, `counted` do not), so a blame round that lands on 0 is enough.

This is the ordinary-reimport refusal I can show. It is one exact total, and that total always fails. Whole-branch edits do not enter the when-search (`narrow` stays empty, `export.rs:1060-1078`), so the calibrated 500×64-context re-import (about 76% of `REIMPORT`, one blame charge of the source on top) still fits. A partial edit whose candidate charges pass the budget is the refusal theme2 asked for.

**2. Overlapping dial closures are still quadratic on the wire, with no budget.** `tamber/src/check.rs:1069-1137`, reached from `check` and again from `tamber/src/export.rs:673`.

For each dial the walk is exact: readers, then tokens that name them, transitively (`named_by`). Each probe drops those memo entries, refolds them, and compares. Summed over dials, a chain where token `k` reads dial `k` and names token `k-1` refolds a suffix per dial: about `n²/2` folds, times up to 64 tuples (`MAX_TUPLES`) and 5 colour probes (7 numeric samples). `check` never calls `spend`. A 1 MiB design can hold on the order of 10⁴ such pairs, and the axis block that produces 64 tuples is a few lines, so one write or one re-import of that shape is hours of folding. The disjoint case the new test covers (each dial read by one token, no naming chain) is linear. The closure is what theme2 required for exactness; the overlap is the cost that remains.

**3. After a charge fails, judging continues, and the plan is discarded only at the wrapper.** `tamber/src/export_batch.rs:65-66` and `tamber/src/export_batch.rs:99-103`; then `tamber/src/export.rs:1306`, `tamber/src/export.rs:1326`, and `tamber/src/export.rs:1343-1344`. The discard is `tamber/src/export.rs:657-658`.

`take` returns when `spend(at.text.len())` fails, so that round does not call `check`. On the blame loop the text stays the base design. On a halve, the first half can commit `at.text` and `at.folding` (`export_batch.rs:74-76`) before the second half’s spend fails. `settle` then writes that partial `Judged` into the local plan, `export_dials::dials` still runs one check per changed dial, and the second `judged` still runs (its `take` returns immediately once `LEFT` is 0). The `Ok(Import)` from `export.rs:1377` is then dropped, so the caller applies nothing.

A failed `spend` sets `LEFT` to 0, so while this `Work` is alive `spent()` stays `Some`. `Drop` (`budget.rs:50-53`) only decrements `DEPTH`. The next `Work::begin` at depth 0 (`budget.rs:40-42`) refills `LEFT`. No caller wraps `reimport_against` in an outer `Work`, so a later import on the same thread is not left spent. `dials` staying unbounded is the QUEUE entry at `QUEUE.md:95`; running it after the budget is already spent is the part that exceeds that entry.

## NIT

**4. The when-search comment attributes refusal to both stop conditions.** `tamber/src/export.rs:1178-1188`, fallback at `tamber/src/export.rs:1225`. `SELECTORS` is 4096 (`tamber/src/selectors.rs:7`).

The selector cap breaks without spending, then `found.unwrap_or_else(|| modes.clone())` keeps the widest `when`, and the re-import can succeed. That widen is the pre-existing selector rule (`selectors.rs:4-6`). The budget arm does set `LEFT` to 0, so the wrapper refuses and the widened plan is not returned.

**5. The chain-past-budget test does not check that nothing was applied.** `tamber/src/export_batch.rs:520-533`.

It asserts that `spent()` contains “expands past” and that the call finishes within 60 seconds. For that blame chain, `take` does not write `at.text` on a failed round. A halve that crosses the budget can write the first half, and nothing covers that shape. The wrapper is what keeps those edits out of the returned `Import`.

## (1) Params index

`Index::named` (`params.rs:138-150`) is the same answer as `named` (`params.rs:156-167`): the first full-name hit, otherwise the first short-name hit (`or_insert` follows list order, as `find` does), then ambiguous unless the number of distinct identities with that component name is 1. The identity set is the same one `called` sorts and dedups, stored in a `BTreeSet` and counted by `component`. `None` and any count other than 1 are both `Ambiguous`, matching `called(owner).len() != 1`.

`laid` (`params.rs:201-228`) records the first `params` line whose `Index::named` succeeds, keyed by that param’s token. The old `find` kept the first line where `named` succeeded and `q.token == p.token`. Same line, same token. The `HashSet` of the design’s own token and icon names excludes the same joined names the old `any()` equality excluded. Duplicate token strings on two params still share one first line, as they did before. `many_params_lay_in_moments` checks the timing and two resulting values; it does not compare against `named`. I found no input where the index or the map diverges. `contract/lower` still calls `named` per line; that path is outside this wire check.

## (2) Influence set

The set is `used[i]`, filled only when a probe’s `value` is `Some` and differs from the baseline (`check.rs:1132-1135`). That comparison is unchanged from the full refold. `every_token` (`influence_tests.rs:14-18`) seeds every token, so the reference walk’s reach is every token and the memo drop matches the old `memo.clear()`. Fold order can differ; `value` recurses and memoizes, so a dependency is folded with the probe dial before its caller reads it.

- **Memo.** Non-reach entries stay at the baseline. A memo hit returns without folding (`check.rs:960-961`). Reach entries are removed before each probe and copied back from the baseline clone afterward (`check.rs:1139-1147`); `self.memo = baseline` runs once more at `check.rs:1149`. Errors are truncated to the pre-influence length (`check.rs:1150`). The dial’s own value is written back at `check.rs:1139` before the next dial.
- **Refused at declaration.** Influence runs only when `errors` is empty (`check.rs:226-228`), which is the same guard as before. A diagnostic refusal never reaches the new walk. An icon is refused with no diagnostic (`check.rs:651-652`) and is never inserted into `names`, so a fold cannot `value` it; the `refused[i]` skip at `check.rs:1127` matches the old loop.
- **Cycles.** `moving` stops the name walk. `value` returns `None` on a cycle without inserting a memo (`check.rs:965-981`). `is_some_and` does not mark `None` against `None`. A probe cannot create a cycle: names are in the source, not in the dial value.
- **Role spellings.** Both spellings map to one token index before any ref is recorded (`check.rs:1430-1437`). The graph is on indexes.
- **Tuples.** `reads` is per token, so a read on one tuple puts the token in the closure and every tuple is refolded and compared. The old refold did the same comparison.
- **Branches and channels.** `light-dark` folds both sides (`fold.rs:310-314`). A powerless channel can skip a later name (`color_fn.rs:126-127`). The source colour is resolved before those components (`color_fn.rs:499`), so the dial that would make the colour chromatic is already a recorded read and the token is in that dial’s closure. Probing the skipped dial leaves the source at its baseline, so both algorithms leave the token unmarked.

With `errors` empty, every non-icon token was folded in the main loop, so the baseline memo covers them. Icons are not reachable through `named`. A probe therefore does not memoize a token outside the closure and then reuse that entry on the next probe. `refs` / `reads` are snapshotted into `named_by` / `readers` before any probe (`check.rs:1065-1068`), so probe-only reads do not change later dials’ closures.

The corpus test and the 300 randomized designs compare token name plus `dials` only, on small shapes (one axis, no cycle, no role, no refusal). They would not catch a set divergence outside that. I do not have an input that produces one.

## (5) and the move

`QUEUE.md:95-96` replaces the PRIORITY with the per-dial checks and an incremental blame re-check. `tamber/src/export/refs.rs` is the old tree readers: `source_tokens` still spends 1 per level and copies before `flatten`. `export.rs` re-exports `ByContext` and calls `tokens_by_context`.
