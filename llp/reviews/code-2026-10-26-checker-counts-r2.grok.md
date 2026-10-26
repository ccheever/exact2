# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), round 2 of 3, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `a490f50748`.
- **Method:** one brief (sha256 `4e226e3784859119552f5faf27922070103dc9e686c81db0937792dc3a51d6fb`), blind to the other reviewer; 3 rounds (security and write-check work), the last with a security pass, then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

## BLOCKER

**A shared range probe drops other lines' `when`s, so an indirect read is checked against the wrong design** — `contract/lower/src/params.rs:508`

`written_ranges` still probes a value on its own when the value text contains a param token, a full name, or a `params` entry name (`:542`). Every other ranged line is folded with its siblings: one probe per value index, and that probe writes every sibling's k-th value and clears every sibling's `when`s (`:508`). The old probe changed only the line under test.

A value that does not spell a param can still follow one. A theme token in between is enough: `App.p1 = accent.fill`, `accent.fill` reads `App.p0`, and `App.p0` has a dark `when` outside `p1`'s range. Both lines are ranged and neither value spells a param, so they share the k = 0 probe. That probe pins `p0` to its default in every tuple. `folded` then walks `p1`'s tuples (`:370`) and never sees the dark value. The build (`theme.rs:311`) and the design server (`tamber_ops.rs:253`) both accept the design. The same probe can also report a range error for a value that is inside its range once the other line's real `when`s are left in place.

Direct references are exact, and so is the fallback when `check_design` fails (`:571`). The 1,500-param test only sets literals, so it does not catch this. Output order is the lines' order: the final sort by line and column matches the old entry-then-value order.

## SHOULD

**A shared dial is still one full check per token, and those checks are not charged** — `tamber/src/export_batch.rs:235`

Dial candidates are not one batch. Each `Colour { dial: Some(_) }` is its own one-member set (`:235`), and `take` charges only rounds after the first (`:67`). A re-import that turns 1,000 tokens on one colour dial from black to white therefore runs 1,000 uncharged `check`s, then `dials()` groups them into one `SetDial`. The budget no longer refuses it, which is what `a_shared_dials_change_reimports_within_the_budget` asserts. It does not bound the work, and the test has no time limit.

Each of those checks still refolds that dial's readers. For this shape the closure is every token, so the cost is about checks × readers × colour probes. At the test's 1,000 tokens that is large but finite. At a wire-sized design of the same shape it is hours, and nothing in the budget stops it. `reimport_within` does return the budget's refusal before `export_dials::dials` once `OVER` is set (`export.rs:1326`). These trials never set it.

## Checked, exact

**Resolver.** `Resolver::resolve` is the old `resolve`: full name, else the first short name, ambiguous unless one component has that name. The ambiguous sentence is still `called()`'s list. `by_token` and `Resolver::lines` keep the first hit, same as `find` and `laid`. `param_names`, `entry_errors`, `unknown`, theme's overridden set, and anatomy's library lines all use that.

**Levels.** `Designs::set` caches the first resolving line's index. Nothing replaces `app` after the cache is filled, so it stays the line `find` used to return.

**Components.** Groups stay first-seen identity, members in `all`'s order. Folded tokens, icon aliases, and param values keep the first match (`rev` then `insert`). `reads` still walks the base value only, as the old `find` did. `short` is still "this component name belongs to one identity": `sharing` counts the same distinct identities the old `any` walk did.

**Budget.** `OVER` is set only when `checked_sub` fails, and cleared when the outermost `Work::begin` runs. A spend that lands on 0 stays inside the budget; `spent()` stays empty. The flag does not stick into the next import. A one-round re-import is not charged, so an ordinary edit is not newly refused. After `OVER` is set, `take` and `settle` return, `reimport_within` returns the budget's words before the dial pass, and `reimport_against` drops the `Import`. A halve that has already committed its first half is in that discarded `Import`. Halving only runs for two or more members, so it does not spin on an empty half.

**Dial order.** Reach is sorted by token index before the refold (`check.rs:1086`), which is declaration order. The 512-chain read at both ends folds the near end first and stays inside the depth bound. Visits are the dial's index in one `moving` vec. `every_token` exists only in test builds; with it on, the reach is every token, sorted, with those memo entries removed first. That is the old full refold. Overlapping closures are still about n²/2 folds. That matches the reference; it is the open queue item (dials grow only on disk), not a wrong influence set.

**`params::shown`.** Still one `replace` per param per string (`params.rs:229`). On a write, `param_values` calls it once per diagnostic, which is small. `components` calls it for every param's default, set value, and options (`tamber_params.rs:543`), so that listing is quadratic. At 1,500 params it is lost in the folds. At the 40,000-param `laid` size it would show up on the components endpoint. It is not on the re-import path, and the per-operation `resolve` scan is gone.

Tests were not run.
