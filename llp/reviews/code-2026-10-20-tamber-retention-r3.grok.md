# Code review: LLP 1121 stage 12 B1 follow-up, the retention build: stop at retention, reuse types and expansion, round 3 of 3, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `ec35893f05`.
- **Method:** one brief (sha256 `9fd08efb38e785ac75ff8a2c570ba9c1055be0eb684d04653771aaf7aa7f96e0`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a cache in the compiler path; theme2's 11:32 and 12:15 rulings), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (1 NIT).
- **Disposition:** taken (the commit message's "Round 3").

## NIT

**`QUEUE.md:16`** — The entry says a value edit that keeps the retention key (`one = 1` to `one = -1` under `-exact-press-scale="one"`) is answered `live` and that this is pre-existing on main. That is true only for an app that already has synthetic expressions (theme2 at 12:14). For an app with none, main left the state unknown after that failing build, so later proposes were `rebuild`. This commit is the change that answers those later edits `live`; 12:15 assigns the restore to the next commit. The commit message states that correctly (empty candidates, unknown on main, background whole build next, option (4)). The queue entry drops the qualifier.

The same line's "kind check would close both" is theme2's 12:14 wording, and the entry does not say a background worker is already running.

## What checks out

**Step 1.** `RETENTION` and `CANDIDATES` are written once, at `contract/lower/src/theme.rs:348-355`, and the flag returns at `:356-358`. Nothing writes them again. `lower_body` then returns `Err(Vec::new())` (`contract/lower/src/lib.rs:455-456`). A build with no design returns before that write (`theme.rs:243-245`), so `retention()` stays `None`. `params::reached` (`contract/lower/src/params/rewrite.rs:443-445`) only skips its diagnostics; it clones the view and does not change the snapshot. No pass before `declare_theme` reads the flag. Load, types, and analysis do not.

`retention().is_some()` (`contract/cli/src/tamber_serve.rs:430`) is the right "reached" bit, including an empty candidate list. A miss that never fixes retention returns `None`, and the plan is classed `rebuild` (`tamber_serve.rs:1094`). The old empty-candidate failure half is what the commit message leaves to the next commit.

The flag is a thread-local with its previous value restored by `Scope` (`contract/cli/src/retained.rs:78-101`), including a panic after the guard exists, and a nested `retention_build` saves the outer program. `retention()` is read after the guard drops, so a later compile on the thread does not stop. `retention_only` is a `Cell` replace. The serve loop does not catch panics.

**Step 2.** Reuse requires every design and package design to parse, `mapped` equal, `Strings` equal by value, and the merged `File` equal with `design` and `package_designs` cleared (`retained.rs:62-67`, `:122-134`). Those cleared sources hold the design text, path, bindable flag, and package identity. Types and expansion do not read `.design` or `package_designs`. What the loader derives into the program is the motion literals `substitute` writes into views, styles, and keyframes, and those stay in the compared file. A motion token used only as a component argument is read, not written (`contract/lower/src/motion.rs:324-334`), so that edit reuses and retention is recomputed from the new design. `modules::desugar` rewrites the file and does not read the design. `bindable` is also in `sources_key` (`contract/cli/src/tamber_walk.rs:245-251`); lowering reads it from the live file. A sources-key miss drops `kept` before the gate (`tamber_serve.rs:407-412`). Strings are compared even though they are not in that key.

`keep` stores the types actually used. Under `EXACT_TAMBER_RETENTION_VERIFY`, a mismatch sets `stale` and the answer uses the fresh check (`contract/cli/src/lib.rs:990-1002`). `refused_fresh` (`retained.rs:179-181`, `lib.rs:995-996`) marks stale when the fresh check refuses what reuse would have taken, before the `?` return, so that log line is `built stale` and `keep` does not run.

**Q2.** A kind change is classed `rebuild`, and `compile_path` reports `tamber-kind` (`contract/cli/tests/it/tamber_retention_reuse.rs:469-472`). The server still does not compile a write's Contract. The commit message does not say this commit runs the background worker or refuses the press-scale case.

**Round 1 and 2.** Present: the scope guard, parse required for reuse, both the production and verifying paths, a synthetic expression on every `with()` app, package-header repair, repair after a parse failure, strings edited with an alias move, motion-as-argument reuse, a type error before the design caching nothing, `refused_fresh`, and the in-process oracle (`tamber_retention_reuse.rs:391-438`) for alias, set-param, case, icon, pair, and header. That oracle checks a reused retention build's retention and non-empty candidates against an ordinary whole build. The whole build writes those two cells at the same point and does not update them in the fold.

**Tests still don't** compare retention itself for a package-design alias, a motion-as-argument edit, or strings. Those server tests compare propose answers. Strings and a motion literal in a row take the full check; the package design is merged before the same snapshot. The motion-argument probe only asks `ink`.

**Bench.** Not re-run here. 561 ms and 555 ms for an alias move sit where a stopped build should (load plus `declare_theme`, types and the post-theme lower skipped). The value-edit numbers match a retention-key hit. The five verifying alias moves claiming `built` once and `built reused` five times, with no `built stale`, were not repeated.
