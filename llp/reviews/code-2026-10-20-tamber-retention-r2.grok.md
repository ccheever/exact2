# Code review: LLP 1121 stage 12 B1 follow-up, the retention build: stop at retention, reuse types and expansion, round 2 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `1cd4f1fbd8`.
- **Method:** one brief (sha256 `e6108e9f9a7c120b998c1997b4ec7ace2ee767a0795471f7a51d594d74f2c208`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (2 NIT).
- **Disposition:** nits taken in the next amend (the commit message's "Round 2").

**BLOCKING:** none.
**SHOULD-FIX:** none.

**NIT** `contract/cli/tests/it/tamber_retention_reuse.rs:415-417`. The comment says the propose after the failing build "reuses" that build. The assertion is only that two proposes add one log line. The second propose is a retention-key hit and never enters `retention_build`, so it cannot log `built reused`. The count is the right check.

**NIT** `contract/cli/tests/it/tamber_retention_reuse.rs:31-37`, `:333-351`, `:291-292`, `:385-390`. The primed-versus-fresh oracle is the propose JSON for the named probes, not `Retention` or the candidate vector. The motion-argument case probes only `ink`, so a retention word that does not move that answer is unchecked. The package-contract comment says the file gate compared every loaded file; that edit already changes `sources_key`, so `kept` is dropped before `reuse`. The strings test (`:310`) and the motion-row test (`:255`) are the ones that reach the gate. The Q2 comment's "the failure is never hidden" and "the dev loop's whole build … as it does here" describe this kind change plus an in-process `compile_path`. That matches theme2's 11:54 ruling. The commit message does not claim the deferred case is fixed.

## What checks out

**Step 1.** `RETENTION` and `CANDIDATES` are written once, at `contract/lower/src/theme.rs:348-355`, and the flag returns at `:356-358`. Nothing writes them again. `lower_body` (`contract/lower/src/lib.rs:455-456`) then returns `Err(Vec::new())`. `lower_all` lowers once. Paths that never set retention leave it `None`, and `synthetic` does not update the cache (`contract/cli/src/tamber_serve.rs:430`): no design and no params (`theme.rs:243-245`), a font or sound error (`lib.rs:443-444`), a load or type-check error. `retention_build` takes both cells on entry (`contract/cli/src/retained.rs:86-87`), so a failed build cannot return an older pair. `shown_errors` (`theme.rs:874-887`) rewrites messages only.

`params::reached` (`contract/lower/src/params/rewrite.rs:443-445`) is called only from `declare_theme`, before the write, and returns diagnostics. Skipping it does not change the stored values. Both servers set the flag, so both skip it. Nothing before lowering reads the flag.

`retention().is_some()` is the right reached bit (`tamber_serve.rs:430`). This compile always returns `Err` at the stop, including when the candidate list is empty. The commit message says that, says main left that empty-candidate failure unknown, and assigns the restore to the next commit (12:15, option (4)). `QUEUE.md` records the same deferral. Nothing here claims this commit runs that worker or refuses the press-scale case.

**Flag.** `Scope` (`retained.rs:78-84`) restores the previous flag and the outer `PROGRAM` on drop, including a panic. `retention_only(true)` runs before `Scope` exists and does not panic, so the flag is not left set. It is off again before `retention()` is read (`:100-104`) and before `uses` compiles for real. The serve loop is one thread and does not catch a panic. Other lowering cells are not restored; the next `declare_theme` clears its own at `:226-234`, and load does not read them.

**Step 2.** The gate (`retained.rs:114-134`) requires every design and package design to parse, then `mapped`, `Strings` by value, and the merged `File` with `design` and `package_designs` cleared (`:62-67`). That `File` is every loaded unit, packages' Contract included. Motion literals the loader writes into rows stay on it and are compared; a motion alias move therefore checks again. `Use` arguments are not written back, so a motion token used only as an argument still matches, and retention is recomputed from the new design. `modules` desugar does not read the design. Types, expansion, and analysis do not read `file.design` or `package_designs`. A `sources_key` mismatch drops `kept` before the gate (`tamber_serve.rs:408-412`); bindable and code-beside are in that key. Strings are not, and the gate compares them on a miss.

**Condition (b).** With `EXACT_TAMBER_RETENTION_VERIFY` set, `compile_file_warned` (`contract/cli/src/lib.rs:984-998`) still checks fresh and uses the fresh types; `verify` (`retained.rs:169-174`) sets `stale` on a `Debug` mismatch of `Types` or a `PartialEq` mismatch of `Expanded`. `Types` is `BTreeMap`/`BTreeSet`. The length mutex is released before `check_all` returns, and the style-attribute function is the same address in one process, so that `Debug` compare does not false-stale in this shape. Unset, the reused types are what the answer uses. `primed_as_fresh` runs both (`tamber_retention_reuse.rs:45-50`).

**Round 1, present.** Unwind guard (`retained.rs:76-101`). Parse failure never reuses (`:120-126`). Tests run the production branch and the verifying branch. Every `with()` app has a synthetic expression (`:100`). Package-header repair (`:197`), repair after a parse failure (`:214`), strings with an alias move (`:310`), motion-as-argument (`:333`), and a type error before the design caching nothing (`:359`) are in the file. The old QUEUE entry is gone; the two new entries match 11:54 and 12:15.

**Q2 claim.** The kind change (`gap` `8px` to `#444444`, `padding="gap"`) is classed `rebuild`, and `compile_path` reports `tamber-kind` (`:411-414`). That is the 11:54 case. The comment's "dev loop" is this in-process compile.

**Bench.** Not re-run. 561 ms and 555 ms for an alias move sit where a reuse should: load plus `declare_theme` up to the stop, types and the post-theme lower skipped. The value edit, 227 ms and 229 ms, matches main's key hit, which never calls `retention_build`. The claimed test totals were not re-run either.
